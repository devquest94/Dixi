//! Snippets: reusable text fragments that expand when their trigger is
//! dictated.
//!
//! A snippet is a phrase the user says and a piece of text that replaces it.
//! The difference with a dictionary substitution is the *template*: a snippet
//! can carry variables, so the same trigger can produce a different text
//! depending on what the caller knows. That is the capability Wispr Flow
//! does not have (see `docs/dixi-reverse-engineering/05-SNIPPETS.md`), and
//! it is the reason this module exists.
//!
//! The matching is not written here. Snippets, dictionary substitutions and
//! the format rules of a writing profile all go through
//! [`super::text_engine`], so a trigger behaves the same way everywhere:
//! the longest trigger wins, ties are broken by priority and then by the
//! order of the rules, the match is a whole word, and the text is rewritten
//! in a single pass. A snippet whose expansion contains its own trigger
//! therefore cannot loop: the replacement is never examined again.
//!
//! # Scopes
//!
//! A snippet can be bound to a set of applications (`app_names`, empty means
//! "everywhere") and to a language (`language`, empty means "every
//! language"). Both are compared the way the rest of the application
//! compares them: application names case-insensitively by process name, and
//! a language tag matches the base of a regional variant (`es` matches
//! `es-AR`).
//!
//! # Variables
//!
//! The expansion is a template with `{name}` placeholders. `{fecha}` and
//! `{hora}` are resolved locally, with the current date and time. Every
//! other placeholder is resolved from the values the caller supplies:
//!
//! * a required placeholder without a value stops the expansion and reports
//!   the variable, so the caller can ask the user for it;
//! * an optional placeholder without a value becomes an empty string.
//!
//! A placeholder that is not a valid identifier (`{"key": 1}`) is literal
//! text, so an expansion may carry code.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use super::personal_store::PersonalStore;
use super::text_engine::{apply_rules, TextRule};

/// Maximum length of a trigger phrase, in characters.
pub const MAX_TRIGGER_LENGTH: usize = 60;

/// Maximum length of an expansion, in characters.
pub const MAX_EXPANSION_LENGTH: usize = 4000;

/// The placeholder resolved with today's date.
pub const DATE_VARIABLE: &str = "fecha";

/// The placeholder resolved with the current time.
pub const TIME_VARIABLE: &str = "hora";

/// Makes the identifiers of two snippets added in the same millisecond
/// different.
static SNIPPET_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A placeholder inside an expansion.
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct SnippetVariable {
    /// The name used in the template: `{name}`.
    pub name: String,
    /// Whether the expansion must stop until the value is known.
    #[serde(default)]
    pub required: bool,
    /// The question to ask when the value is missing.
    #[serde(default)]
    pub prompt: String,
}

/// A stored snippet.
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct Snippet {
    /// The identifier of the snippet.
    pub id: String,
    /// The name the user gave to the snippet.
    pub name: String,
    /// The phrase that triggers the expansion.
    pub trigger_phrase: String,
    /// The text written in place of the trigger, as a template.
    pub expansion: String,
    /// The variables the template may use.
    pub variables: Vec<SnippetVariable>,
    /// The folder the snippet belongs to, when the user groups them.
    pub category: Option<String>,
    /// The applications the snippet applies to. Empty means "everywhere".
    pub app_names: Vec<String>,
    /// The language the snippet applies to. Empty means "every language".
    pub language: Option<String>,
    /// Whether the snippet is starred.
    pub is_favorite: bool,
    /// Whether the snippet is used at all.
    pub enabled: bool,
    /// How many times the snippet was expanded.
    pub usage_count: i64,
    /// When the snippet was created, in milliseconds since the epoch.
    pub created_at: i64,
    /// When the snippet was changed, in milliseconds since the epoch.
    pub updated_at: i64,
}

/// The outcome of expanding a snippet.
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Expansion {
    /// The expansion, fully resolved.
    Text(String),
    /// A required variable has no value: the caller must ask the user.
    NeedsInput {
        /// The snippet that could not be expanded.
        snippet_id: String,
        /// The variable whose value is missing.
        variable: SnippetVariable,
    },
}

/// The writable fields of a snippet.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SnippetInput {
    /// The name the user gave to the snippet.
    pub name: String,
    /// The phrase that triggers the expansion.
    pub trigger_phrase: String,
    /// The text written in place of the trigger, as a template.
    pub expansion: String,
    /// The variables the template may use.
    #[serde(default)]
    pub variables: Vec<SnippetVariable>,
    /// The folder the snippet belongs to.
    #[serde(default)]
    pub category: Option<String>,
    /// The applications the snippet applies to. Empty means "everywhere".
    #[serde(default)]
    pub app_names: Vec<String>,
    /// The language the snippet applies to.
    #[serde(default)]
    pub language: Option<String>,
    /// Whether the snippet is starred.
    #[serde(default)]
    pub is_favorite: bool,
    /// Whether the snippet is used at all.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// The on-disk shape of an exported snippet file.
#[derive(Serialize, Deserialize)]
pub struct SnippetFile {
    /// The snippets carried by the file.
    pub snippets: Vec<SnippetInput>,
}

/// Owns the snippets table and the in-memory snapshot of its rows.
pub struct SnippetManager {
    store: PersonalStore,
    snippets: RwLock<Vec<Snippet>>,
}

impl SnippetManager {
    /// Opens (or creates) the personal database and loads the snapshot.
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        Self::with_store(PersonalStore::new(app)?)
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
            snippets: RwLock::new(Vec::new()),
        };
        manager.reload_snapshot()?;
        Ok(manager)
    }

    fn connection(&self) -> Result<Connection, String> {
        self.store.connection()
    }

    /// Re-reads every snippet from disk into the snapshot.
    fn reload_snapshot(&self) -> Result<(), String> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, trigger_phrase, expansion, variables, category, app_names, language,
                        is_favorite, enabled, usage_count, created_at, updated_at
                 FROM snippets
                 ORDER BY is_favorite DESC, name COLLATE NOCASE ASC, created_at ASC, id ASC",
            )
            .map_err(|e| format!("Failed to prepare the snippets query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Snippet {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    trigger_phrase: row.get("trigger_phrase")?,
                    expansion: row.get("expansion")?,
                    variables: parse_variables(&row.get::<_, String>("variables")?),
                    category: row.get("category")?,
                    app_names: parse_list(&row.get::<_, String>("app_names")?),
                    language: row.get("language")?,
                    is_favorite: row.get::<_, i64>("is_favorite")? != 0,
                    enabled: row.get::<_, i64>("enabled")? != 0,
                    usage_count: row.get("usage_count")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })
            .map_err(|e| format!("Failed to read the snippets: {e}"))?;

        let mut snippets = Vec::new();
        for row in rows {
            snippets.push(row.map_err(|e| format!("Failed to read a snippet: {e}"))?);
        }

        *self
            .snippets
            .write()
            .map_err(|_| "Snippet lock poisoned".to_string())? = snippets;
        Ok(())
    }

    /// Every snippet, favourites first.
    pub fn list(&self) -> Vec<Snippet> {
        self.snippets
            .read()
            .map(|snippets| snippets.clone())
            .unwrap_or_default()
    }

    /// Adds a snippet and returns the stored version.
    pub fn add(&self, input: SnippetInput) -> Result<Snippet, String> {
        let normalized = normalize_input(&input)?;
        let now = chrono::Utc::now().timestamp_millis();
        let id = new_id(now);

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO snippets
                (id, name, trigger_phrase, expansion, variables, category, app_names, language,
                 is_favorite, enabled, usage_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?11)",
            params![
                id,
                normalized.name,
                normalized.trigger_phrase,
                normalized.expansion,
                serialize_variables(&normalized.variables),
                normalized.category,
                serialize_list(&normalized.app_names),
                normalized.language,
                normalized.is_favorite as i64,
                normalized.enabled as i64,
                now,
            ],
        )
        .map_err(|e| format!("Failed to add the snippet: {e}"))?;

        self.reload_snapshot()?;
        self.get(&id)
            .ok_or_else(|| "The snippet was added but could not be read back".to_string())
    }

    /// Updates the writable fields of a snippet.
    pub fn update(&self, id: &str, input: SnippetInput) -> Result<(), String> {
        let normalized = normalize_input(&input)?;
        let now = chrono::Utc::now().timestamp_millis();

        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE snippets
                    SET name = ?1, trigger_phrase = ?2, expansion = ?3, variables = ?4, category = ?5,
                        app_names = ?6, language = ?7, is_favorite = ?8, enabled = ?9, updated_at = ?10
                  WHERE id = ?11",
                params![
                    normalized.name,
                    normalized.trigger_phrase,
                    normalized.expansion,
                    serialize_variables(&normalized.variables),
                    normalized.category,
                    serialize_list(&normalized.app_names),
                    normalized.language,
                    normalized.is_favorite as i64,
                    normalized.enabled as i64,
                    now,
                    id,
                ],
            )
            .map_err(|e| format!("Failed to update the snippet: {e}"))?;

        if changed == 0 {
            return Err(format!("Snippet '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Deletes a snippet.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute("DELETE FROM snippets WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete the snippet: {e}"))?;

        if changed == 0 {
            return Err(format!("Snippet '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Stars or unstars a snippet.
    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE snippets SET is_favorite = ?1, updated_at = ?2 WHERE id = ?3",
                params![favorite as i64, chrono::Utc::now().timestamp_millis(), id],
            )
            .map_err(|e| format!("Failed to update the snippet: {e}"))?;

        if changed == 0 {
            return Err(format!("Snippet '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Enables or disables a snippet.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE snippets SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![enabled as i64, chrono::Utc::now().timestamp_millis(), id],
            )
            .map_err(|e| format!("Failed to update the snippet: {e}"))?;

        if changed == 0 {
            return Err(format!("Snippet '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Records that a snippet was expanded.
    pub fn record_use(&self, id: &str) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE snippets SET usage_count = usage_count + 1 WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to record the use of the snippet: {e}"))?;

        if changed == 0 {
            return Err(format!("Snippet '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// The rules that apply to an application and a language.
    ///
    /// The rules come back in resolution order: the longest trigger first,
    /// then the higher priority, then the alphabetically first trigger. The
    /// identifier of a rule is its position in that list, because the shared
    /// engine resolves a tie between two rules of the same length with the
    /// priority stored at that position.
    pub fn rules_for(&self, app_name: Option<&str>, language: Option<&str>) -> Vec<TextRule> {
        self.applicable(app_name, language)
            .into_iter()
            .map(|(rule, _)| rule)
            .collect()
    }

    /// The rules that compete for the same trigger.
    ///
    /// Two snippets may share a trigger, and then the order of the rules is
    /// what decides which one expands. The list comes back in the same
    /// resolution order the engine uses, so the winner is the first entry.
    pub fn competing_rules(&self, trigger: &str) -> Vec<TextRule> {
        let needle = trigger.trim().to_lowercase();
        self.applicable(None, None)
            .into_iter()
            .filter(|(rule, _)| rule.needle.to_lowercase() == needle)
            .map(|(rule, _)| rule)
            .collect()
    }

    /// The trigger phrases of the snippets that apply, without duplicates.
    pub fn trigger_vocabulary(
        &self,
        app_name: Option<&str>,
        language: Option<&str>,
    ) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut vocabulary = Vec::new();
        for snippet in self.list() {
            if !snippet.enabled {
                continue;
            }
            if !applies_to_app(&snippet, app_name) {
                continue;
            }
            if !applies_to_language(&snippet, language) {
                continue;
            }
            let trigger = snippet.trigger_phrase.trim().to_string();
            if trigger.is_empty() {
                continue;
            }
            if seen.insert(trigger.to_lowercase()) {
                vocabulary.push(trigger);
            }
        }
        vocabulary
    }

    /// Expands a snippet, asking for the variables it cannot resolve.
    pub fn expand(&self, snippet: &Snippet, values: &HashMap<String, String>) -> Expansion {
        let mut out = String::new();
        for segment in parse_template(&snippet.expansion) {
            match segment {
                Segment::Literal(text) => out.push_str(&text),
                Segment::Variable(name) => {
                    if name == DATE_VARIABLE {
                        out.push_str(&local_date());
                        continue;
                    }
                    if name == TIME_VARIABLE {
                        out.push_str(&local_time());
                        continue;
                    }
                    if let Some(value) = values.get(&name) {
                        out.push_str(value);
                        continue;
                    }
                    let declared = snippet.variables.iter().find(|v| v.name == name);
                    match declared {
                        Some(variable) if variable.required => {
                            return Expansion::NeedsInput {
                                snippet_id: snippet.id.clone(),
                                variable: variable.clone(),
                            };
                        }
                        // An optional variable without a value becomes an
                        // empty string, and a variable that is not declared
                        // at all cannot be asked for either.
                        _ => {}
                    }
                }
            }
        }
        Expansion::Text(out)
    }

    /// Finds the snippet whose trigger appears in the text, and expands it.
    ///
    /// The first snippet that fires is the one that is expanded. When it
    /// needs a value that is not available, the result is
    /// [`Expansion::NeedsInput`] instead of a text, so the caller can ask
    /// the user before anything is pasted.
    pub fn expand_in_text(
        &self,
        text: &str,
        values: &HashMap<String, String>,
        app_name: Option<&str>,
        language: Option<&str>,
    ) -> Option<Expansion> {
        let applicable = self.applicable(app_name, language);
        if applicable.is_empty() {
            return None;
        }

        let rules: Vec<TextRule> = applicable.iter().map(|(rule, _)| rule.clone()).collect();
        let outcome = apply_rules(text, &rules);
        let first = outcome.applied.first()?;
        let (_, snippet) = applicable.iter().find(|(rule, _)| rule.id == *first)?;

        Some(self.expand(snippet, values))
    }

    /// Finds the snippet whose trigger appears in the text, and expands it,
    /// reporting which snippet fired.
    ///
    /// The caller needs the identifier to count the use, and to be able to
    /// tell the user which snippet was expanded.
    pub fn expand_in_text_with_id(
        &self,
        text: &str,
        values: &HashMap<String, String>,
        app_name: Option<&str>,
        language: Option<&str>,
    ) -> Option<(String, Expansion)> {
        let applicable = self.applicable(app_name, language);
        if applicable.is_empty() {
            return None;
        }

        let rules: Vec<TextRule> = applicable.iter().map(|(rule, _)| rule.clone()).collect();
        let outcome = apply_rules(text, &rules);
        let first = outcome.applied.first()?;
        let (_, snippet) = applicable.iter().find(|(rule, _)| rule.id == *first)?;

        Some((snippet.id.clone(), self.expand(snippet, values)))
    }

    /// The snippets that apply to an application and a language, together
    /// with the rule that represents each of them.
    fn applicable(&self, app_name: Option<&str>, language: Option<&str>) -> Vec<(TextRule, Snippet)> {
        let mut candidates: Vec<Snippet> = self
            .list()
            .into_iter()
            .filter(|snippet| snippet.enabled)
            .filter(|snippet| applies_to_app(snippet, app_name))
            .filter(|snippet| applies_to_language(snippet, language))
            .collect();

        candidates.sort_by(resolution_order);

        candidates
            .iter()
            .enumerate()
            .map(|(index, snippet)| (rule_for(snippet, index), snippet.clone()))
            .collect()
    }

    /// The stored snippet with the given identifier.
    fn get(&self, id: &str) -> Option<Snippet> {
        self.list().into_iter().find(|snippet| snippet.id == id)
    }

    /// Writes the snippets to a JSON file.
    ///
    /// An existing file is only replaced when `overwrite` is set, so the
    /// caller has to ask the user first. Returns how many snippets were
    /// written.
    pub fn export_to_file(&self, path: &Path, overwrite: bool) -> Result<usize, String> {
        validate_export_path(path, overwrite)?;

        let payload = SnippetFile {
            snippets: self.list().iter().map(snippet_to_input).collect(),
        };

        let json = serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("Failed to serialize the snippets: {e}"))?;

        std::fs::write(path, json).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => format!(
                "'{}' cannot be written: the file is read-only or the folder is not writable.",
                path.display()
            ),
            _ => format!("Failed to write '{}': {}", path.display(), e),
        })?;

        Ok(payload.snippets.len())
    }

    /// Reads the snippets from a JSON file and adds them.
    ///
    /// The snippets that already exist are skipped, so importing the same
    /// file twice changes nothing. Returns how many snippets were actually
    /// added.
    pub fn import_from_file(&self, path: &Path) -> Result<usize, String> {
        validate_import_path(path)?;

        let text = std::fs::read_to_string(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => format!(
                "'{}' cannot be read: the file is locked or the user does not have permission.",
                path.display()
            ),
            _ => format!("Failed to read '{}': {}", path.display(), e),
        })?;

        let payload: SnippetFile = serde_json::from_str(&text)
            .map_err(|e| format!("'{}' is not a valid snippets file: {e}", path.display()))?;

        self.import(&payload.snippets)
    }

    /// Adds the given snippets, skipping the ones that already exist.
    ///
    /// Two snippets are the same when they have the same name, the same
    /// trigger and the same expansion. Returns how many snippets were
    /// actually added.
    pub fn import(&self, inputs: &[SnippetInput]) -> Result<usize, String> {
        let mut normalized = Vec::with_capacity(inputs.len());
        for input in inputs {
            normalized.push(normalize_input(input)?);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.connection()?;

        let mut known: Vec<(String, String, String)> = self
            .list()
            .iter()
            .map(|snippet| {
                (
                    snippet.name.clone(),
                    snippet.trigger_phrase.clone(),
                    snippet.expansion.clone(),
                )
            })
            .collect();

        let mut added = 0usize;
        for snippet in &normalized {
            let key = (
                snippet.name.clone(),
                snippet.trigger_phrase.clone(),
                snippet.expansion.clone(),
            );
            if known.contains(&key) {
                continue;
            }

            let id = new_id(now);
            conn.execute(
                "INSERT INTO snippets
                    (id, name, trigger_phrase, expansion, variables, category, app_names, language,
                     is_favorite, enabled, usage_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?11)",
                params![
                    id,
                    snippet.name,
                    snippet.trigger_phrase,
                    snippet.expansion,
                    serialize_variables(&snippet.variables),
                    snippet.category,
                    serialize_list(&snippet.app_names),
                    snippet.language,
                    snippet.is_favorite as i64,
                    snippet.enabled as i64,
                    now,
                ],
            )
            .map_err(|e| format!("Failed to import the snippet: {e}"))?;

            known.push(key);
            added += 1;
        }

        self.reload_snapshot()?;
        Ok(added)
    }
}

/// A piece of an expansion template.
enum Segment {
    /// Text that is written as it is.
    Literal(String),
    /// A `{name}` placeholder.
    Variable(String),
}

/// Splits an expansion into literal text and placeholders.
///
/// A `{...}` group whose content is not an identifier is literal text, so
/// an expansion may carry code such as `{"key": 1}`.
fn parse_template(template: &str) -> Vec<Segment> {
    let chars: Vec<char> = template.chars().collect();
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '{' {
            if let Some(close) = (index + 1..chars.len()).find(|&i| chars[i] == '}') {
                let name: String = chars[index + 1..close].iter().collect();
                if is_identifier(&name) {
                    if !literal.is_empty() {
                        segments.push(Segment::Literal(std::mem::take(&mut literal)));
                    }
                    segments.push(Segment::Variable(name));
                    index = close + 1;
                    continue;
                }
            }
        }
        literal.push(chars[index]);
        index += 1;
    }

    if !literal.is_empty() {
        segments.push(Segment::Literal(literal));
    }
    segments
}

/// Whether a name can be used as a variable.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Today's date, in the local time zone.
fn local_date() -> String {
    chrono::Local::now().format("%d/%m/%Y").to_string()
}

/// The current time, in the local time zone.
fn local_time() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

/// The priority of the rule of a snippet.
///
/// The table has no priority column, so the priority is derived from what
/// the user can express today: a starred snippet wins over a regular one
/// when both have the same trigger.
fn priority_of(snippet: &Snippet) -> i32 {
    if snippet.is_favorite {
        1
    } else {
        0
    }
}

/// Builds the rule of a snippet.
///
/// The rule identifier is the position of the snippet in the list of the
/// applicable rules, because the shared engine compares the priorities of
/// two rules by indexing the list with the identifier.
fn rule_for(snippet: &Snippet, index: usize) -> TextRule {
    TextRule {
        id: index as i64,
        needle: snippet.trigger_phrase.clone(),
        replacement: snippet.expansion.clone(),
        // A trigger is a phrase the user says, not a fragment of a word:
        // it must not fire inside "miTrigger" or "mi_trigger".
        whole_word: true,
        case_sensitive: false,
        priority: priority_of(snippet),
    }
}

/// Orders two snippets the way the engine resolves a collision.
///
/// The longest trigger wins, then the higher priority, then the
/// alphabetically first trigger. The creation time and the identifier break
/// the last tie, so the order is always the same for the same data.
fn resolution_order(a: &Snippet, b: &Snippet) -> std::cmp::Ordering {
    b.trigger_phrase
        .chars()
        .count()
        .cmp(&a.trigger_phrase.chars().count())
        .then_with(|| priority_of(b).cmp(&priority_of(a)))
        .then_with(|| {
            a.trigger_phrase
                .to_lowercase()
                .cmp(&b.trigger_phrase.to_lowercase())
        })
        .then_with(|| a.created_at.cmp(&b.created_at))
        .then_with(|| a.id.cmp(&b.id))
}

/// Whether a snippet applies to an application.
///
/// A snippet without applications applies everywhere. A snippet with
/// applications applies only to those, and the comparison is
/// case-insensitive. When the application is unknown the snippet cannot be
/// ruled out, so it still applies.
fn applies_to_app(snippet: &Snippet, app_name: Option<&str>) -> bool {
    if snippet.app_names.is_empty() {
        return true;
    }
    let Some(app_name) = app_name else {
        return true;
    };
    let app_name = normalize_app_name(app_name);
    snippet
        .app_names
        .iter()
        .any(|candidate| normalize_app_name(candidate) == app_name)
}

/// Normalizes an application name for the comparison.
///
/// The extension is dropped on both sides, so "chrome" and "chrome.exe"
/// are the same application.
fn normalize_app_name(name: &str) -> String {
    let lowered = name.trim().to_lowercase();
    match lowered.strip_suffix(".exe") {
        Some(stripped) => stripped.to_string(),
        None => lowered,
    }
}

/// Whether a snippet applies to a language.
///
/// A snippet without a language applies everywhere. A snippet with a
/// language applies when the language matches it, either exactly or as the
/// base language of a regional variant (`es-AR` matches `es`). When the
/// language is unknown the snippet cannot be ruled out, so it still
/// applies.
fn applies_to_language(snippet: &Snippet, language: Option<&str>) -> bool {
    let Some(tag) = snippet.language.as_deref() else {
        return true;
    };
    let Some(language) = language else {
        return true;
    };
    let tag = tag.to_lowercase();
    let language = language.to_lowercase();
    language == tag || language.starts_with(&format!("{tag}-"))
}

/// A validated, normalized snippet ready to be written.
struct NormalizedSnippet {
    name: String,
    trigger_phrase: String,
    expansion: String,
    variables: Vec<SnippetVariable>,
    category: Option<String>,
    app_names: Vec<String>,
    language: Option<String>,
    is_favorite: bool,
    enabled: bool,
}

/// Validates and normalizes user input.
fn normalize_input(input: &SnippetInput) -> Result<NormalizedSnippet, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("The snippet needs a name".to_string());
    }

    let trigger_phrase = input.trigger_phrase.trim().to_string();
    if trigger_phrase.is_empty() {
        return Err("The trigger phrase cannot be empty".to_string());
    }
    if trigger_phrase.chars().count() > MAX_TRIGGER_LENGTH {
        return Err(format!(
            "The trigger phrase cannot be longer than {MAX_TRIGGER_LENGTH} characters"
        ));
    }

    // The expansion is kept verbatim: it may be a signature with blank
    // lines, and trimming it would change what the user typed.
    let expansion = input.expansion.clone();
    if expansion.trim().is_empty() {
        return Err("The expansion cannot be empty".to_string());
    }
    if expansion.chars().count() > MAX_EXPANSION_LENGTH {
        return Err(format!(
            "The expansion cannot be longer than {MAX_EXPANSION_LENGTH} characters"
        ));
    }

    let mut variables: Vec<SnippetVariable> = Vec::with_capacity(input.variables.len());
    for variable in &input.variables {
        let variable_name = variable.name.trim().to_string();
        if variable_name.is_empty() {
            return Err("A variable needs a name".to_string());
        }
        if !is_identifier(&variable_name) {
            return Err(format!(
                "'{variable_name}' is not a valid variable name: use letters, digits and underscores"
            ));
        }
        // The date and the time are filled by the machine, never by the
        // user. A variable with one of those names would be declared and
        // then ignored, so it is refused instead of silently losing its
        // value.
        if variable_name == DATE_VARIABLE || variable_name == TIME_VARIABLE {
            return Err(format!(
                "'{variable_name}' is a reserved variable: it is filled with the current date and time"
            ));
        }
        if variables.iter().any(|v| v.name == variable_name) {
            return Err(format!("The variable '{variable_name}' is declared twice"));
        }
        variables.push(SnippetVariable {
            name: variable_name,
            required: variable.required,
            prompt: variable.prompt.clone(),
        });
    }

    // Every placeholder of the template has to be declared, or there would
    // be no way to know whether it is required. The two local variables are
    // always available, so they do not have to be declared.
    for segment in parse_template(&expansion) {
        if let Segment::Variable(name) = segment {
            if name == DATE_VARIABLE || name == TIME_VARIABLE {
                continue;
            }
            if !variables.iter().any(|variable| variable.name == name) {
                return Err(format!(
                    "The expansion uses the variable '{name}', which is not declared"
                ));
            }
        }
    }

    let mut app_names: Vec<String> = Vec::with_capacity(input.app_names.len());
    for candidate in &input.app_names {
        let candidate = candidate.trim().to_lowercase();
        if candidate.is_empty() {
            continue;
        }
        if !app_names.contains(&candidate) {
            app_names.push(candidate);
        }
    }

    Ok(NormalizedSnippet {
        name,
        trigger_phrase,
        expansion,
        variables,
        category: input
            .category
            .as_ref()
            .map(|category| category.trim().to_string())
            .filter(|category| !category.is_empty()),
        app_names,
        language: input
            .language
            .as_ref()
            .map(|language| language.trim().to_lowercase())
            .filter(|language| !language.is_empty()),
        is_favorite: input.is_favorite,
        enabled: input.enabled,
    })
}

/// The writable fields of a stored snippet.
fn snippet_to_input(snippet: &Snippet) -> SnippetInput {
    SnippetInput {
        name: snippet.name.clone(),
        trigger_phrase: snippet.trigger_phrase.clone(),
        expansion: snippet.expansion.clone(),
        variables: snippet.variables.clone(),
        category: snippet.category.clone(),
        app_names: snippet.app_names.clone(),
        language: snippet.language.clone(),
        is_favorite: snippet.is_favorite,
        enabled: snippet.enabled,
    }
}

/// Builds an identifier that no other snippet can have.
///
/// The counter is padded, so the identifiers of two snippets created in the
/// same millisecond still sort in the order they were created. That order
/// is the last tie breaker of the resolution rule.
fn new_id(now: i64) -> String {
    let counter = SNIPPET_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("snippet_{}_{now}_{counter:06}", std::process::id())
}

/// The default value of the `enabled` flag when a file does not carry it.
fn default_enabled() -> bool {
    true
}

/// Parses the stored variables.
fn parse_variables(text: &str) -> Vec<SnippetVariable> {
    serde_json::from_str::<Vec<SnippetVariable>>(text).unwrap_or_default()
}

/// Serializes the variables.
fn serialize_variables(variables: &[SnippetVariable]) -> String {
    serde_json::to_string(variables).unwrap_or_else(|_| "[]".to_string())
}

/// Parses a stored list of names.
fn parse_list(text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(text).unwrap_or_default()
}

/// Serializes a list of names.
fn serialize_list(names: &[String]) -> String {
    serde_json::to_string(names).unwrap_or_else(|_| "[]".to_string())
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
fn validate_export_path(path: &Path, overwrite: bool) -> Result<(), String> {
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
fn validate_import_path(path: &Path) -> Result<(), String> {
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
            "'{}' is a folder, not a snippets file",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
