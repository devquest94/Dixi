//! Personal dictionary: user-defined vocabulary and transcript substitutions.
//!
//! Two kinds of entry live in the dictionary:
//!
//! * [`DictionaryEntryKind::Term`] — a word or phrase handed to the ASR engine
//!   as vocabulary so it is more likely to be transcribed correctly. The
//!   transcript itself is never rewritten.
//! * [`DictionaryEntryKind::Substitution`] — a find/replace rule applied to the
//!   transcript after transcription. Useful for names, acronyms and product
//!   names the engine keeps getting wrong.
//!
//! Entries live in SQLite rather than in the settings file: the dictionary
//! grows by hundreds of rows, carries mutable counters, and is edited
//! entry-by-entry. The manager keeps an in-memory snapshot because the
//! transcription hot path needs the rules on every single utterance and must
//! not touch the disk.
//!
//! # Scopes
//!
//! Every entry carries a `scope` string. The reserved value [`GLOBAL_SCOPE`]
//! means "applies everywhere"; any other value is a user-defined bucket that
//! a caller can select. Dixi has no profile subsystem yet, so the scope is
//! stored and filtered but not yet surfaced in the UI.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::RwLock;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use super::personal_store::PersonalStore;
use super::text_engine::{apply_rules, TextRule};
use crate::portable::app_data_dir;

/// Scope that applies to every transcription.
pub const GLOBAL_SCOPE: &str = "global";

/// Maximum length of a dictionary term, in characters.
pub const MAX_TERM_LENGTH: usize = 200;

/// Maximum length of a substitution replacement, in characters.
pub const MAX_REPLACEMENT_LENGTH: usize = 500;

/// What an entry does when it matches.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DictionaryEntryKind {
    /// Vocabulary hint only: fed to the ASR engine, never rewrites text.
    Term,
    /// Rewrites the transcript after transcription.
    Substitution,
}

/// A stored dictionary entry.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct DictionaryEntry {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    pub kind: DictionaryEntryKind,
    pub enabled: bool,
    pub scope: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub hit_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Higher priority wins when two rules could match the same text.
    pub priority: i32,
    /// Optional language tag: the rule only applies to transcripts in that
    /// language. Empty means "every language".
    pub language: Option<String>,
}

/// The writable fields of a dictionary entry.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct DictionaryEntryInput {
    pub term: String,
    pub replacement: String,
    pub kind: DictionaryEntryKind,
    /// Defaults to enabled when absent.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Defaults to [`GLOBAL_SCOPE`] when absent.
    #[serde(default)]
    pub scope: Option<String>,
    /// Defaults to case-insensitive matching when absent.
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    /// Defaults to whole-word matching when absent.
    #[serde(default)]
    pub whole_word: Option<bool>,
    /// Defaults to zero when absent.
    #[serde(default)]
    pub priority: Option<i32>,
    /// Defaults to "every language" when absent.
    #[serde(default)]
    pub language: Option<String>,
}

/// Result of running the substitution engine over a transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubstitutionOutcome {
    /// The rewritten text.
    pub text: String,
    /// Ids of the entries that fired, in the order they first fired.
    pub applied: Vec<i64>,
}

/// Owns the dictionary database and the in-memory snapshot of its entries.
pub struct DictionaryManager {
    store: PersonalStore,
    entries: RwLock<Vec<DictionaryEntry>>,
}

impl DictionaryManager {
    /// Opens (or creates) the personal database and loads the snapshot.
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let store = PersonalStore::new(app_handle)?;
        Self::with_store(store)
    }

    /// Opens (or creates) the personal database at an explicit path.
    ///
    /// The path-based constructor is the seam that keeps the manager
    /// independent of the Tauri application handle, and it is what lets the
    /// tests work on an isolated database.
    pub fn with_db_path(db_path: PathBuf) -> Result<Self, String> {
        Self::with_store(PersonalStore::with_db_path(db_path)?)
    }

    /// Uses an already opened personal database.
    pub fn with_store(store: PersonalStore) -> Result<Self, String> {
        let manager = Self {
            store,
            entries: RwLock::new(Vec::new()),
        };
        manager.reload_snapshot()?;
        Ok(manager)
    }

    fn connection(&self) -> Result<Connection, String> {
        self.store.connection()
    }

    /// Re-reads every entry from disk into the snapshot.
    fn reload_snapshot(&self) -> Result<(), String> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, term, replacement, kind, enabled, scope, case_sensitive, whole_word, hit_count, created_at, updated_at, priority, language
                 FROM dictionary_entries
                 ORDER BY priority DESC, LENGTH(term) DESC, term COLLATE NOCASE ASC",
            )
            .map_err(|e| format!("Failed to prepare dictionary query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let kind_text: String = row.get("kind")?;
                Ok(DictionaryEntry {
                    id: row.get("id")?,
                    term: row.get("term")?,
                    replacement: row.get("replacement")?,
                    kind: parse_kind(&kind_text),
                    enabled: row.get::<_, i64>("enabled")? != 0,
                    scope: row.get("scope")?,
                    case_sensitive: row.get::<_, i64>("case_sensitive")? != 0,
                    whole_word: row.get::<_, i64>("whole_word")? != 0,
                    hit_count: row.get("hit_count")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                    priority: row.get("priority")?,
                    language: row.get("language")?,
                })
            })
            .map_err(|e| format!("Failed to read dictionary entries: {e}"))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row.map_err(|e| format!("Failed to read dictionary entry: {e}"))?);
        }

        *self
            .entries
            .write()
            .map_err(|_| "Dictionary lock poisoned".to_string())? = entries;
        Ok(())
    }

    /// Every entry, longest term first (the order the substitution engine
    /// wants).
    pub fn list(&self) -> Vec<DictionaryEntry> {
        self.entries
            .read()
            .map(|entries| entries.clone())
            .unwrap_or_default()
    }

    /// Entries that apply to the given scope (plus the global ones).
    pub fn list_for_scope(&self, scope: &str) -> Vec<DictionaryEntry> {
        self.entries
            .read()
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.scope == GLOBAL_SCOPE || e.scope == scope)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Adds an entry and returns the stored version.
    pub fn add(&self, input: &DictionaryEntryInput) -> Result<DictionaryEntry, String> {
        let normalized = normalize_input(input)?;
        let now = chrono::Utc::now().timestamp_millis();

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO dictionary_entries
                (term, replacement, kind, enabled, scope, case_sensitive, whole_word, hit_count, created_at, updated_at, priority, language)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8, ?9, ?10)",
            params![
                normalized.term,
                normalized.replacement,
                kind_text(normalized.kind),
                normalized.enabled,
                normalized.scope,
                normalized.case_sensitive,
                normalized.whole_word,
                now,
                normalized.priority,
                normalized.language,
            ],
        )
        .map_err(|e| {
            if is_unique_violation(&e) {
                format!(
                    "A rule for '{}' with the same replacement already exists in this scope",
                    normalized.term
                )
            } else {
                format!("Failed to add dictionary entry: {e}")
            }
        })?;

        let id = conn.last_insert_rowid();
        self.reload_snapshot()?;

        self.entries
            .read()
            .ok()
            .and_then(|entries| entries.iter().find(|e| e.id == id).cloned())
            .ok_or_else(|| "Dictionary entry was added but could not be read back".to_string())
    }

    /// Updates the writable fields of an entry.
    pub fn update(&self, id: i64, input: &DictionaryEntryInput) -> Result<(), String> {
        let normalized = normalize_input(input)?;
        let now = chrono::Utc::now().timestamp_millis();

        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE dictionary_entries
                    SET term = ?1, replacement = ?2, kind = ?3, enabled = ?4, scope = ?5,
                        case_sensitive = ?6, whole_word = ?7, updated_at = ?8, priority = ?9, language = ?10
                  WHERE id = ?11",
                params![
                    normalized.term,
                    normalized.replacement,
                    kind_text(normalized.kind),
                    normalized.enabled,
                    normalized.scope,
                    normalized.case_sensitive,
                    normalized.whole_word,
                    now,
                    normalized.priority,
                    normalized.language,
                    id,
                ],
            )
            .map_err(|e| {
                if is_unique_violation(&e) {
                    format!(
                        "A rule for '{}' with the same replacement already exists in this scope",
                        normalized.term
                    )
                } else {
                    format!("Failed to update dictionary entry: {e}")
                }
            })?;

        if changed == 0 {
            return Err(format!("Dictionary entry {id} not found"));
        }
        self.reload_snapshot()
    }

    /// Deletes an entry.
    pub fn delete(&self, id: i64) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute("DELETE FROM dictionary_entries WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete dictionary entry: {e}"))?;

        if changed == 0 {
            return Err(format!("Dictionary entry {id} not found"));
        }
        self.reload_snapshot()
    }

    /// Enables or disables an entry.
    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE dictionary_entries SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![enabled as i64, chrono::Utc::now().timestamp_millis(), id],
            )
            .map_err(|e| format!("Failed to update dictionary entry: {e}"))?;

        if changed == 0 {
            return Err(format!("Dictionary entry {id} not found"));
        }
        self.reload_snapshot()
    }

    /// Adds several entries, skipping the ones that already exist.
    /// Returns the number of entries that were actually added.
    pub fn import(&self, inputs: &[DictionaryEntryInput]) -> Result<usize, String> {
        let mut normalized = Vec::with_capacity(inputs.len());
        for input in inputs {
            normalized.push(normalize_input(input)?);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.connection()?;
        let mut added = 0usize;

        for entry in &normalized {
            let inserted = conn.execute(
                "INSERT INTO dictionary_entries
                    (term, replacement, kind, enabled, scope, case_sensitive, whole_word, hit_count, created_at, updated_at, priority, language)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8, ?9, ?10)",
                params![
                    entry.term,
                    entry.replacement,
                    kind_text(entry.kind),
                    entry.enabled,
                    entry.scope,
                    entry.case_sensitive,
                    entry.whole_word,
                    now,
                    entry.priority,
                    entry.language,
                ],
            );
            match inserted {
                Ok(_) => added += 1,
                // A duplicate is not an import failure: the term is already
                // known, which is exactly what the user wanted.
                Err(e) if is_unique_violation(&e) => continue,
                Err(e) => return Err(format!("Failed to import dictionary entry: {e}")),
            }
        }

        self.reload_snapshot()?;
        Ok(added)
    }

    /// Writes the dictionary to a JSON file.
    ///
    /// The file is written by the backend on purpose: the frontend's
    /// filesystem plugin is scoped to the application data directory, and
    /// widening that scope to every place a user might save a file would hand
    /// the webview more reach than the feature needs.
    ///
    /// An existing file is only replaced when `overwrite` is set, so the
    /// caller has to ask the user first. Returns how many entries were
    /// written.
    pub fn export_to_file(&self, path: &std::path::Path, overwrite: bool) -> Result<usize, String> {
        validate_export_path(path, overwrite)?;

        // A backup carries every entry, including the scoped ones, so that
        // restoring it on another machine reproduces the same dictionary.
        let payload = DictionaryFile {
            entries: self
                .list()
                .iter()
                .map(|entry| DictionaryEntryInput {
                    term: entry.term.clone(),
                    replacement: entry.replacement.clone(),
                    kind: entry.kind,
                    enabled: Some(entry.enabled),
                    scope: Some(entry.scope.clone()),
                    case_sensitive: Some(entry.case_sensitive),
                    whole_word: Some(entry.whole_word),
                    priority: Some(entry.priority),
                    language: entry.language.clone(),
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("Failed to serialize the dictionary: {e}"))?;

        std::fs::write(path, json).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => format!(
                "'{}' cannot be written: the file is read-only or the folder is not writable.",
                path.display()
            ),
            _ => format!("Failed to write '{}': {}", path.display(), e),
        })?;

        Ok(payload.entries.len())
    }

    /// Reads a dictionary from a JSON file and adds its entries.
    ///
    /// Entries that already exist are skipped, so importing the same file
    /// twice changes nothing. Returns how many entries were actually added.
    pub fn import_from_file(&self, path: &std::path::Path) -> Result<usize, String> {
        validate_import_path(path)?;

        let text = std::fs::read_to_string(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => format!(
                "'{}' cannot be read: the file is locked or the user does not have permission.",
                path.display()
            ),
            _ => format!("Failed to read '{}': {}", path.display(), e),
        })?;

        let payload: DictionaryFile = serde_json::from_str(&text)
            .map_err(|e| format!("'{}' is not a valid dictionary file: {e}", path.display()))?;

        self.import(&payload.entries)
    }

    /// The vocabulary to hand to the ASR engine: enabled terms (and the
    /// search term of enabled substitutions), without duplicates.
    pub fn vocabulary(&self, scope: &str) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut vocabulary = Vec::new();
        for entry in self.list_for_scope(scope) {
            if !entry.enabled {
                continue;
            }
            let candidate = match entry.kind {
                DictionaryEntryKind::Term => entry.term.clone(),
                DictionaryEntryKind::Substitution => entry.term.clone(),
            };
            let candidate = candidate.trim().to_string();
            if candidate.is_empty() {
                continue;
            }
            let key = candidate.to_lowercase();
            if seen.insert(key) {
                vocabulary.push(candidate);
            }
        }
        vocabulary
    }

    /// The enabled substitution rules that apply to the given scope.
    pub fn substitution_rules(&self, scope: &str) -> Vec<DictionaryEntry> {
        self.list_for_scope(scope)
            .into_iter()
            .filter(|e| e.enabled && e.kind == DictionaryEntryKind::Substitution)
            .filter(|e| !e.replacement.trim().is_empty() && !e.term.trim().is_empty())
            .collect()
    }

    /// Records that the given entries fired during a transcription.
    pub fn record_hits(&self, ids: &[i64]) {        if ids.is_empty() {
            return;
        }
        let Ok(conn) = self.connection() else {
            return;
        };
        for id in ids {
            if let Err(e) = conn.execute(
                "UPDATE dictionary_entries SET hit_count = hit_count + 1 WHERE id = ?1",
                params![id],
            ) {
                log::warn!("Failed to record dictionary hit for entry {id}: {e}");
            }
        }
        // The counters are not part of the rules the engine uses, so a failed
        // snapshot refresh here is harmless.
        if let Err(e) = self.reload_snapshot() {
            log::warn!("Failed to refresh dictionary snapshot after recording hits: {e}");
        }
    }
}

/// Applies the enabled substitution rules to `text`.
///
/// The engine makes a single pass over the original text and, at every
/// position, takes the rule that matches best there. "Best" means: among the
/// rules that match, the longest term wins; rules of equal length are
/// ordered by descending priority. A shorter rule can therefore never
/// rewrite a fragment produced by a longer one, and a replacement can never
/// be rewritten again by a later rule: no cascading surprises.
///
/// Rules tagged with a language only apply to transcripts in that language;
/// untagged rules apply to every language.
///
/// Only the matched characters are replaced: a substring rule inside a longer
/// word leaves the rest of that word in place ("dix" inside "dixi" becomes
/// "Dixii"). Use a whole-word rule to replace the complete word instead.
///
/// Casing of the matched text is preserved: an all-caps match yields an
/// all-caps replacement, a title-case match yields a title-case replacement,
/// and anything else yields the replacement verbatim.
pub fn apply_substitutions(
    text: &str,
    rules: &[DictionaryEntry],
    language: Option<&str>,
) -> SubstitutionOutcome {
    if text.is_empty() || rules.is_empty() {
        return SubstitutionOutcome {
            text: text.to_string(),
            applied: Vec::new(),
        };
    }

    // The matching itself is the shared engine's, so the dictionary, the
    // snippets and the format rules of a profile cannot drift apart.
    let applicable: Vec<TextRule> = rules
        .iter()
        .filter(|rule| rule_applies_to_language(rule, language))
        .map(|rule| TextRule {
            id: rule.id,
            needle: rule.term.clone(),
            replacement: rule.replacement.clone(),
            whole_word: rule.whole_word,
            case_sensitive: rule.case_sensitive,
            priority: rule.priority,
        })
        .collect();

    let outcome = apply_rules(text, &applicable);
    SubstitutionOutcome {
        text: outcome.text,
        applied: outcome.applied,
    }
}

/// Whether a rule applies to a transcript in the given language.
///
/// An untagged rule applies everywhere. A tagged rule applies when the
/// transcript language matches it, either exactly or as the base language of
/// a regional variant (`es-AR` matches a rule tagged `es`).
fn rule_applies_to_language(rule: &DictionaryEntry, language: Option<&str>) -> bool {
    let Some(tag) = rule.language.as_deref() else {
        return true;
    };
    let Some(language) = language else {
        // The language of the transcript is unknown: a tagged rule cannot be
        // ruled out, so it still applies.
        return true;
    };
    let tag = tag.to_lowercase();
    let language = language.to_lowercase();
    language == tag || language.starts_with(&format!("{tag}-"))
}

/// The on-disk shape of an exported dictionary.
#[derive(Serialize, Deserialize)]
pub struct DictionaryFile {
    pub entries: Vec<DictionaryEntryInput>,
}

/// Checks that a destination is usable for an export.
///
/// The path comes from the native save dialog, so it is one the user chose.
/// It still has to be absolute: a relative one would resolve against the
/// working directory of the process, which is not what the user sees in the
/// dialog.
///
/// An existing file is only replaced when the caller says so, which turns an
/// accidental overwrite into a deliberate one.
fn validate_export_path(path: &std::path::Path, overwrite: bool) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("No destination file was chosen".to_string());
    }
    if !path.is_absolute() {
        return Err(format!(
            "The destination must be an absolute path: '{}'",
            path.display()
        ));
    }
    if path.is_dir() {
        return Err(format!("'{}' is a folder, not a file", path.display()));
    }
    if path.exists() && !overwrite {
        return Err(format!(
            "'{}' already exists. Choose another name, or confirm that it can be replaced.",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.is_dir() {
            return Err(format!("The folder '{}' does not exist", parent.display()));
        }
    }
    Ok(())
}

/// Checks that a source is usable for an import.
fn validate_import_path(path: &std::path::Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("No file was chosen".to_string());
    }
    if !path.is_absolute() {
        return Err(format!(
            "The file must be an absolute path: '{}'",
            path.display()
        ));
    }
    if !path.exists() {
        return Err(format!("'{}' does not exist", path.display()));
    }
    if path.is_dir() {
        return Err(format!(
            "'{}' is a folder, not a dictionary file",
            path.display()
        ));
    }
    Ok(())
}

/// A validated, normalized entry ready to be written.
struct NormalizedEntry {
    term: String,
    replacement: String,
    kind: DictionaryEntryKind,
    enabled: bool,
    scope: String,
    case_sensitive: bool,
    whole_word: bool,
    priority: i32,
    language: Option<String>,
}

/// Validates and normalizes user input.
fn normalize_input(input: &DictionaryEntryInput) -> Result<NormalizedEntry, String> {
    let term = input.term.trim().to_string();
    if term.is_empty() {
        return Err("Term cannot be empty".to_string());
    }
    if term.chars().count() > MAX_TERM_LENGTH {
        return Err(format!(
            "Term cannot be longer than {MAX_TERM_LENGTH} characters"
        ));
    }

    let replacement = input.replacement.trim().to_string();
    if replacement.chars().count() > MAX_REPLACEMENT_LENGTH {
        return Err(format!(
            "Replacement cannot be longer than {MAX_REPLACEMENT_LENGTH} characters"
        ));
    }
    if input.kind == DictionaryEntryKind::Substitution && replacement.is_empty() {
        return Err("A substitution needs a replacement".to_string());
    }

    let scope = input
        .scope
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(GLOBAL_SCOPE)
        .to_string();

    Ok(NormalizedEntry {
        term,
        replacement,
        kind: input.kind,
        enabled: input.enabled.unwrap_or(true),
        scope,
        case_sensitive: input.case_sensitive.unwrap_or(false),
        whole_word: input.whole_word.unwrap_or(true),
        priority: input.priority.unwrap_or(0),
        language: input
            .language
            .as_ref()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_lowercase()),
    })
}

/// Textual form of a kind, as stored in SQLite.
fn kind_text(kind: DictionaryEntryKind) -> &'static str {
    match kind {
        DictionaryEntryKind::Term => "term",
        DictionaryEntryKind::Substitution => "substitution",
    }
}

/// Parses a stored kind, defaulting to `Term` for unknown values so a future
/// or corrupted row never breaks loading the whole dictionary.
fn parse_kind(text: &str) -> DictionaryEntryKind {
    match text {
        "substitution" => DictionaryEntryKind::Substitution,
        _ => DictionaryEntryKind::Term,
    }
}

/// Whether a rusqlite error is a UNIQUE constraint violation.
///
/// The message is inspected instead of the raw SQLite error code so the
/// module does not depend on the `rusqlite::ffi` re-export.
fn is_unique_violation(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message)) => {
            message.contains("UNIQUE constraint failed")
        }
        _ => false,
    }
}

/// Looks up an entry by id.
#[cfg(test)]
pub fn find_entry(entries: &[DictionaryEntry], id: i64) -> Option<&DictionaryEntry> {
    entries.iter().find(|e| e.id == id)
}

#[cfg(test)]
mod e2e_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, term: &str, replacement: &str) -> DictionaryEntry {
        DictionaryEntry {
            id,
            term: term.to_string(),
            replacement: replacement.to_string(),
            kind: DictionaryEntryKind::Substitution,
            enabled: true,
            scope: GLOBAL_SCOPE.to_string(),
            case_sensitive: false,
            whole_word: true,
            hit_count: 0,
            created_at: 0,
            updated_at: 0,
            priority: 0,
            language: None,
        }
    }

    fn term_entry(id: i64, term: &str) -> DictionaryEntry {
        DictionaryEntry {
            kind: DictionaryEntryKind::Term,
            replacement: String::new(),
            ..rule(id, term, "")
        }
    }

    // --- substitution engine -------------------------------------------------

    #[test]
    fn replaces_a_whole_word() {
        let rules = vec![rule(1, "perro", "Dixi")];
        let outcome = apply_substitutions("el perro ladra", &rules, None);
        assert_eq!(outcome.text, "el Dixi ladra");
        assert_eq!(outcome.applied, vec![1]);
    }

    #[test]
    fn does_not_replace_inside_a_word() {
        let rules = vec![rule(1, "perro", "Dixi")];
        let outcome = apply_substitutions("el perrote", &rules, None);
        assert_eq!(outcome.text, "el perrote");
        assert!(outcome.applied.is_empty());
    }

    #[test]
    fn matches_at_word_boundaries_with_punctuation() {
        let rules = vec![rule(1, "Dixi", "Dixi")];
        let outcome = apply_substitutions("¿Dixi? ¡Dixi! (Dixi)", &rules, None);
        assert_eq!(outcome.text, "¿Dixi? ¡Dixi! (Dixi)");
        assert_eq!(outcome.applied, vec![1]);
    }

    #[test]
    fn substring_rule_replaces_inside_a_word() {
        let mut r = rule(1, "js", "JavaScript");
        r.whole_word = false;
        let outcome = apply_substitutions("un js y otro jsx", &[r], None);
        assert_eq!(outcome.text, "un JavaScript y otro JavaScriptx");
    }

    #[test]
    fn a_substring_rule_does_not_swallow_the_rest_of_the_word() {
        // The engine replaces exactly the matched characters and keeps the
        // rest of the word, so "dix" inside "dixi" leaves the trailing "i".
        // A whole-word rule is the one that avoids this.
        let mut r = rule(1, "dix", "Dixi");
        r.whole_word = false;
        let outcome = apply_substitutions("dixi", &[r], None);
        assert_eq!(outcome.text, "Dixii");
    }

    #[test]
    fn case_insensitive_by_default_and_keeps_matched_style() {
        let rules = vec![rule(1, "dixi", "Dixi")];
        assert_eq!(
            apply_substitutions("dixi", &rules, None).text,
            "Dixi",
            "lowercase match keeps the replacement verbatim"
        );
        assert_eq!(
            apply_substitutions("DIXI", &rules, None).text,
            "DIXI",
            "all-caps match produces an all-caps replacement"
        );
        assert_eq!(
            apply_substitutions("Dixi", &rules, None).text,
            "Dixi",
            "title-case match produces a title-case replacement"
        );
    }

    #[test]
    fn case_sensitive_rule_does_not_match_other_case() {
        let mut r = rule(1, "Dixi", "Dixi");
        r.case_sensitive = true;
        let outcome = apply_substitutions("dixi y Dixi", &[r], None);
        assert_eq!(outcome.text, "dixi y Dixi");
        assert_eq!(outcome.applied, vec![1]);
    }

    #[test]
    fn longest_rule_wins() {
        let rules = vec![rule(1, "new", "nuevo"), rule(2, "new york", "Nueva York")];
        let outcome = apply_substitutions("voy a new york", &rules, None);
        assert_eq!(outcome.text, "voy a Nueva York");
        assert_eq!(outcome.applied, vec![2]);
    }

    #[test]
    fn a_replacement_is_not_rewritten_again() {
        // "dixi" -> "dixi" would loop forever if the engine cascaded.
        let rules = vec![rule(1, "dixi", "dixi"), rule(2, "x", "y")];
        let outcome = apply_substitutions("dixi", &rules, None);
        assert_eq!(outcome.text, "dixi");
        assert_eq!(outcome.applied, vec![1]);
    }

    #[test]
    fn empty_rules_leave_the_text_untouched() {
        let outcome = apply_substitutions("sin cambios", &[], None);
        assert_eq!(outcome.text, "sin cambios");
        assert!(outcome.applied.is_empty());
    }

    #[test]
    fn handles_multibyte_text() {
        let rules = vec![rule(1, "ñandú", "ñandú，养")];
        let outcome = apply_substitutions("el ñandú corre", &rules, None);
        assert_eq!(outcome.text, "el ñandú，养 corre");
    }

    #[test]
    fn applies_every_occurrence_once() {
        let rules = vec![rule(1, "ia", "IA")];
        let outcome = apply_substitutions("ia, la, ia", &rules, None);
        assert_eq!(outcome.text, "IA, la, IA");
        assert_eq!(outcome.applied, vec![1]);
    }

    #[test]
    fn higher_priority_wins_between_rules_of_equal_length() {
        let mut preferred = rule(1, "equipo", "equipo propio");
        preferred.priority = 10;
        let fallback = rule(2, "equipo", "equipo genérico");
        let outcome = apply_substitutions("el equipo", &[fallback, preferred], None);
        assert_eq!(outcome.text, "el equipo propio");
        assert_eq!(outcome.applied, vec![1]);
    }

    #[test]
    fn a_longer_term_still_beats_a_higher_priority_shorter_one() {
        let mut short_high = rule(1, "dix", "corto");
        short_high.priority = 100;
        let long_low = rule(2, "dixi", "Dixi");
        let outcome = apply_substitutions("dixi", &[short_high, long_low], None);
        assert_eq!(outcome.text, "Dixi");
        assert_eq!(outcome.applied, vec![2]);
    }

    #[test]
    fn a_language_tagged_rule_only_applies_to_that_language() {
        let mut tagged = rule(1, "servidor", "server");
        tagged.language = Some("en".to_string());
        let rules = vec![tagged];

        assert_eq!(
            apply_substitutions("el servidor", &rules, Some("en")).text,
            "el server"
        );
        assert_eq!(
            apply_substitutions("el servidor", &rules, Some("es")).text,
            "el servidor",
            "an English rule must not touch a Spanish transcript"
        );
    }

    #[test]
    fn a_language_tag_matches_the_base_of_a_regional_variant() {
        let mut tagged = rule(1, "car", "auto");
        tagged.language = Some("es".to_string());
        let rules = vec![tagged];
        assert_eq!(
            apply_substitutions("mi car", &rules, Some("es-AR")).text,
            "mi auto"
        );
    }

    #[test]
    fn an_untagged_rule_applies_to_every_language() {
        let rules = vec![rule(1, "dixi", "Dixi")];
        assert_eq!(apply_substitutions("dixi", &rules, Some("en")).text, "Dixi");
        assert_eq!(apply_substitutions("dixi", &rules, Some("es")).text, "Dixi");
        assert_eq!(apply_substitutions("dixi", &rules, None).text, "Dixi");
    }

    // --- validation ----------------------------------------------------------

    #[test]
    fn rejects_an_empty_term() {
        let input = DictionaryEntryInput {
            term: "   ".to_string(),
            replacement: "x".to_string(),
            kind: DictionaryEntryKind::Term,
            enabled: None,
            scope: None,
            case_sensitive: None,
            whole_word: None,
            priority: None,
            language: None,
        };
        assert!(normalize_input(&input).is_err());
    }

    #[test]
    fn rejects_a_substitution_without_replacement() {
        let input = DictionaryEntryInput {
            term: "a".to_string(),
            replacement: "  ".to_string(),
            kind: DictionaryEntryKind::Substitution,
            enabled: None,
            scope: None,
            case_sensitive: None,
            whole_word: None,
            priority: None,
            language: None,
        };
        assert!(normalize_input(&input).is_err());
    }

    #[test]
    fn applies_defaults() {
        let input = DictionaryEntryInput {
            term: "  Dixi  ".to_string(),
            replacement: String::new(),
            kind: DictionaryEntryKind::Term,
            enabled: None,
            scope: None,
            case_sensitive: None,
            whole_word: None,
            priority: None,
            language: None,
        };
        let normalized = normalize_input(&input).expect("valid input");
        assert_eq!(normalized.term, "Dixi");
        assert!(normalized.enabled);
        assert_eq!(normalized.scope, GLOBAL_SCOPE);
        assert!(!normalized.case_sensitive);
        assert!(normalized.whole_word);
    }

    #[test]
    fn unknown_kind_text_falls_back_to_term() {
        assert_eq!(parse_kind("substitution"), DictionaryEntryKind::Substitution);
        assert_eq!(parse_kind("term"), DictionaryEntryKind::Term);
        assert_eq!(parse_kind("future-kind"), DictionaryEntryKind::Term);
    }

    #[test]
    fn finds_an_entry_by_id() {
        let entries = vec![term_entry(1, "a"), term_entry(2, "b")];
        assert_eq!(find_entry(&entries, 2).map(|e| e.term.as_str()), Some("b"));
        assert!(find_entry(&entries, 9).is_none());
    }
}
