//! Writing profiles: how Dixi adapts the text to where it is being dictated.
//!
//! A *style* says **how** to write (formal, casual, enthusiastic, …). A
//! *profile* says **where** (a chat, an email, a personal message) and points
//! at a style, a cleanup level and a set of format rules. The application
//! that has the focus decides which profile applies, so the user does not
//! have to think about it.
//!
//! The profiles are not a separate system from the dictionary and the
//! snippets: they live in the same personal database and are applied in the
//! same transcription pass.
//!
//! # Precedence
//!
//! From the least to the most specific:
//!
//! 1. the global configuration,
//! 2. the profile that matches the detected application,
//! 3. a rule bound to that application,
//! 4. a manual override for the current session.
//!
//! A manual override always wins, and it is never persisted: it is a
//! decision for this session only.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::RwLock;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use super::personal_store::PersonalStore;

/// The profile used when nothing else matches.
pub const GENERAL_PROFILE_ID: &str = "general";

/// How aggressively the transcript is cleaned up.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CleanupLevel {
    /// Nothing is removed. The transcript is pasted as it came.
    None,
    /// Fillers and repetitions are removed, and nothing else changes.
    Light,
    /// Fillers, repetitions and the obvious punctuation mistakes are fixed.
    Medium,
}

impl Default for CleanupLevel {
    fn default() -> Self {
        CleanupLevel::Light
    }
}

impl CleanupLevel {
    /// The wire value.
    pub fn as_str(&self) -> &'static str {
        match self {
            CleanupLevel::None => "none",
            CleanupLevel::Light => "light",
            CleanupLevel::Medium => "medium",
        }
    }

    /// Parses a stored value, defaulting to the light level.
    pub fn parse(text: &str) -> Self {
        match text.trim().to_lowercase().as_str() {
            "none" => CleanupLevel::None,
            "medium" => CleanupLevel::Medium,
            _ => CleanupLevel::Light,
        }
    }
}

/// A deterministic rule applied to the text before it is pasted.
///
/// These rules are purely mechanical: they never need to understand the
/// meaning of the text. Anything that does is a *transform*, and it needs an
/// explicitly enabled LLM provider.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FormatRule {
    /// Capitalizes the first letter of every sentence.
    CapitalizeSentences,
    /// Makes sure the text ends with a full stop.
    EnsureFinalPeriod,
    /// Collapses runs of spaces into one.
    CollapseSpaces,
    /// Removes the space before a comma, a full stop or a closing bracket.
    SpaceBeforePunctuation,
}

/// Whether a style is one of the built-in ones.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProfileKind {
    Builtin,
    Custom,
}

/// A writing style.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct WritingStyle {
    pub id: String,
    pub name: String,
    pub kind: ProfileKind,
    /// The instruction sent to the model when the AI cleanup is enabled.
    pub base_prompt: String,
    pub cleanup_level: CleanupLevel,
    /// Whether the AI cleanup may be used with this style.
    pub ai_cleanup: bool,
    pub format_rules: Vec<FormatRule>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A writing profile: a set of applications plus how to write in them.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct WritingProfile {
    pub id: String,
    pub name: String,
    pub kind: ProfileKind,
    /// The applications this profile applies to, by executable name.
    /// Empty means "everywhere", which is what the fallback profile uses.
    pub app_names: Vec<String>,
    /// The preferred language, when the profile forces one.
    pub language: Option<String>,
    /// The style this profile points at.
    pub style_id: String,
    /// Overrides the cleanup level of the style when present.
    pub cleanup_level: Option<CleanupLevel>,
    /// Overrides the AI cleanup flag of the style when present.
    pub ai_cleanup: Option<bool>,
    /// Overrides the format rules of the style when present.
    pub format_rules: Option<Vec<FormatRule>>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A rule bound to one specific application.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct ProfileOverride {
    pub id: String,
    pub profile_id: String,
    /// What the rule is bound to. Only `app` is supported for now.
    pub scope_kind: String,
    /// The executable name the rule applies to.
    pub scope_value: String,
    /// The style this rule forces, when it forces one.
    pub style_id: Option<String>,
    /// The cleanup level this rule forces, when it forces one.
    pub cleanup_level: Option<CleanupLevel>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The writable fields of a writing style.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct WritingStyleInput {
    pub name: String,
    pub base_prompt: String,
    pub cleanup_level: CleanupLevel,
    pub ai_cleanup: bool,
    pub format_rules: Vec<FormatRule>,
    pub enabled: bool,
}

/// The writable fields of a writing profile.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct WritingProfileInput {
    pub name: String,
    pub app_names: Vec<String>,
    pub language: Option<String>,
    pub style_id: String,
    pub cleanup_level: Option<CleanupLevel>,
    pub ai_cleanup: Option<bool>,
    pub format_rules: Option<Vec<FormatRule>>,
    pub enabled: bool,
}

/// Where the effective configuration came from.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfileOrigin {
    /// A manual override for this session: the user chose it explicitly.
    ManualOverride,
    /// A rule bound to the detected application.
    AppOverride,
    /// The profile that matched the detected application.
    DetectedProfile,
    /// Nothing matched: the global configuration applies.
    Global,
}

/// The configuration that applies to one dictation.
#[derive(Clone, Debug)]
pub struct EffectiveProfile {
    pub profile: WritingProfile,
    pub style: WritingStyle,
    pub cleanup_level: CleanupLevel,
    pub ai_cleanup: bool,
    pub format_rules: Vec<FormatRule>,
    /// The preferred language of the profile, when it forces one.
    pub language: Option<String>,
    /// How this configuration was chosen.
    pub origin: ProfileOrigin,
}

/// Applies the deterministic format rules to a text.
///
/// The rules are mechanical on purpose: they never need to understand the
/// meaning of the text, so they are safe to apply without a model.
pub fn apply_format_rules(text: &str, rules: &[FormatRule]) -> String {
    let mut out = text.to_string();

    for rule in rules {
        out = match rule {
            FormatRule::CapitalizeSentences => capitalize_sentences(&out),
            FormatRule::EnsureFinalPeriod => ensure_final_period(&out),
            FormatRule::CollapseSpaces => collapse_spaces(&out),
            FormatRule::SpaceBeforePunctuation => remove_space_before_punctuation(&out),
        };
    }

    out
}

/// Capitalizes the first letter of every sentence.
fn capitalize_sentences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_sentence_start = true;

    for c in text.chars() {
        if at_sentence_start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            at_sentence_start = false;
            continue;
        }
        if matches!(c, '.' | '!' | '?' | '\n') {
            at_sentence_start = true;
        }
        out.push(c);
    }

    out
}

/// Makes sure the text ends with a full stop.
fn ensure_final_period(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return text.to_string();
    }
    if trimmed.ends_with(['.', '!', '?', ':', ';', ',']) {
        return trimmed.to_string();
    }
    format!("{trimmed}.")
}

/// Collapses runs of spaces into one, without touching the line breaks.
fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut previous_was_space = false;

    for c in text.chars() {
        if c == ' ' {
            if previous_was_space {
                continue;
            }
            previous_was_space = true;
        } else {
            previous_was_space = false;
        }
        out.push(c);
    }

    out
}

/// Removes the space that precedes a comma, a full stop or a closing bracket.
fn remove_space_before_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());

    for (index, c) in chars.iter().enumerate() {
        if *c == ' ' && index + 1 < chars.len() && is_trailing_punctuation(chars[index + 1]) {
            continue;
        }
        out.push(*c);
    }

    out
}

/// Whether the character is one that never follows a space.
fn is_trailing_punctuation(c: char) -> bool {
    matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
}

/// Owns the writing styles, the writing profiles and their overrides.
pub struct ProfileManager {
    store: PersonalStore,
    styles: RwLock<Vec<WritingStyle>>,
    profiles: RwLock<Vec<WritingProfile>>,
    overrides: RwLock<Vec<ProfileOverride>>,
}

impl ProfileManager {
    /// Opens the personal database and loads the profiles.
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        Self::with_store(PersonalStore::new(app_handle)?)
    }

    /// Opens the personal database at an explicit path.
    pub fn with_db_path(db_path: PathBuf) -> Result<Self, String> {
        Self::with_store(PersonalStore::with_db_path(db_path)?)
    }

    /// Uses an already opened personal database.
    pub fn with_store(store: PersonalStore) -> Result<Self, String> {
        let manager = Self {
            store,
            styles: RwLock::new(Vec::new()),
            profiles: RwLock::new(Vec::new()),
            overrides: RwLock::new(Vec::new()),
        };
        manager.reload()?;
        manager.seed_builtins()?;
        Ok(manager)
    }

    fn connection(&self) -> Result<Connection, String> {
        self.store.connection()
    }

    /// Re-reads everything from disk.
    fn reload(&self) -> Result<(), String> {
        let conn = self.connection()?;
        load_styles(&conn, &self.styles)?;
        load_profiles(&conn, &self.profiles)?;
        load_overrides(&conn, &self.overrides)?;
        Ok(())
    }

    // --- Styles ---------------------------------------------------------

    /// Every writing style.
    pub fn styles(&self) -> Vec<WritingStyle> {
        self.styles.read().map(|s| s.clone()).unwrap_or_default()
    }

    /// One writing style.
    pub fn style(&self, id: &str) -> Option<WritingStyle> {
        self.styles
            .read()
            .ok()
            .and_then(|styles| styles.iter().find(|style| style.id == id).cloned())
    }

    /// Adds a custom writing style.
    pub fn add_style(&self, input: WritingStyleInput) -> Result<WritingStyle, String> {
        let style = build_style(ProfileKind::Custom, input, None)?;
        let conn = self.connection()?;
        insert_style(&conn, &style)?;
        self.reload()?;
        Ok(style)
    }

    /// Updates a writing style. A built-in style keeps its id and kind.
    pub fn update_style(&self, id: &str, input: WritingStyleInput) -> Result<(), String> {
        let existing = self
            .style(id)
            .ok_or_else(|| format!("Writing style '{id}' not found"))?;
        let updated = build_style(existing.kind, input, Some(&existing.id))?;
        let conn = self.connection()?;
        update_style(&conn, &updated)?;
        self.reload()?;
        Ok(())
    }

    /// Deletes a writing style. A built-in style cannot be deleted.
    pub fn delete_style(&self, id: &str) -> Result<(), String> {
        let existing = self
            .style(id)
            .ok_or_else(|| format!("Writing style '{id}' not found"))?;
        if existing.kind == ProfileKind::Builtin {
            return Err("A built-in style cannot be deleted. Disable it instead.".to_string());
        }
        let conn = self.connection()?;
        conn.execute("DELETE FROM writing_styles WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete the writing style: {e}"))?;
        self.reload()?;
        Ok(())
    }

    // --- Profiles -------------------------------------------------------

    /// Every writing profile.
    pub fn profiles(&self) -> Vec<WritingProfile> {
        self.profiles
            .read()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// One writing profile.
    pub fn profile(&self, id: &str) -> Option<WritingProfile> {
        self.profiles
            .read()
            .ok()
            .and_then(|profiles| profiles.iter().find(|profile| profile.id == id).cloned())
    }

    /// Adds a custom writing profile.
    pub fn add_profile(&self, input: WritingProfileInput) -> Result<WritingProfile, String> {
        let profile = build_profile(ProfileKind::Custom, input, None)?;
        let conn = self.connection()?;
        insert_profile(&conn, &profile)?;
        self.reload()?;
        Ok(profile)
    }

    /// Updates a writing profile.
    pub fn update_profile(&self, id: &str, input: WritingProfileInput) -> Result<(), String> {
        let existing = self
            .profile(id)
            .ok_or_else(|| format!("Writing profile '{id}' not found"))?;
        let updated = build_profile(existing.kind, input, Some(&existing.id))?;
        let conn = self.connection()?;
        update_profile(&conn, &updated)?;
        self.reload()?;
        Ok(())
    }

    /// Deletes a writing profile. A built-in profile cannot be deleted.
    pub fn delete_profile(&self, id: &str) -> Result<(), String> {
        let existing = self
            .profile(id)
            .ok_or_else(|| format!("Writing profile '{id}' not found"))?;
        if existing.kind == ProfileKind::Builtin {
            return Err("A built-in profile cannot be deleted. Disable it instead.".to_string());
        }
        let conn = self.connection()?;
        conn.execute("DELETE FROM writing_profiles WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete the writing profile: {e}"))?;
        self.reload()?;
        Ok(())
    }

    /// Every rule bound to a specific application.
    pub fn overrides(&self) -> Vec<ProfileOverride> {
        self.overrides
            .read()
            .map(|o| o.clone())
            .unwrap_or_default()
    }

    /// Binds a rule to an application.
    pub fn add_override(
        &self,
        profile_id: &str,
        scope_value: &str,
        style_id: Option<String>,
        cleanup_level: Option<CleanupLevel>,
    ) -> Result<ProfileOverride, String> {
        if self.profile(profile_id).is_none() {
            return Err(format!("Writing profile '{profile_id}' not found"));
        }
        let scope_value = scope_value.trim().to_lowercase();
        if scope_value.is_empty() {
            return Err("The application name cannot be empty".to_string());
        }

        let now = chrono::Utc::now().timestamp_millis();
        let id = format!("override_{now}");
        let record = ProfileOverride {
            id: id.clone(),
            profile_id: profile_id.to_string(),
            scope_kind: "app".to_string(),
            scope_value,
            style_id,
            cleanup_level,
            created_at: now,
            updated_at: now,
        };

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO profile_overrides
                (id, profile_id, scope_kind, scope_value, style_id, cleanup_level, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.id,
                record.profile_id,
                record.scope_kind,
                record.scope_value,
                record.style_id,
                record.cleanup_level.map(|level| level.as_str().to_string()),
                record.created_at,
                record.updated_at,
            ],
        )
        .map_err(|e| format!("Failed to add the application rule: {e}"))?;

        self.reload()?;
        Ok(record)
    }

    /// Removes a rule bound to an application.
    pub fn delete_override(&self, id: &str) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute("DELETE FROM profile_overrides WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete the application rule: {e}"))?;
        if changed == 0 {
            return Err(format!("Application rule '{id}' not found"));
        }
        self.reload()?;
        Ok(())
    }

    // --- Resolution -----------------------------------------------------

    /// Resolves the configuration that applies to one dictation.
    ///
    /// `app_name` is the detected application, `manual_override` is a
    /// profile the user chose explicitly for this session.
    pub fn resolve(
        &self,
        app_name: Option<&str>,
        manual_override: Option<&str>,
    ) -> EffectiveProfile {
        // 4. A manual override for this session always wins.
        if let Some(id) = manual_override {
            if let Some(profile) = self.profile(id) {
                return self.effective(&profile, ProfileOrigin::ManualOverride);
            }
        }

        // 3. A rule bound to the detected application.
        if let Some(app) = app_name {
            let app = app.trim().to_lowercase();
            if !app.is_empty() {
                if let Some(rule) = self
                    .overrides()
                    .into_iter()
                    .find(|rule| rule.scope_kind == "app" && rule.scope_value == app)
                {
                    if let Some(profile) = self.profile(&rule.profile_id) {
                        let mut effective = self.effective(&profile, ProfileOrigin::AppOverride);
                        if let Some(style_id) = &rule.style_id {
                            if let Some(style) = self.style(style_id) {
                                effective.style = style;
                            }
                        }
                        if let Some(level) = rule.cleanup_level {
                            effective.cleanup_level = level;
                        }
                        return effective;
                    }
                }
            }
        }

        // 2. The profile that matches the detected application.
        if let Some(app) = app_name {
            let app = app.trim().to_lowercase();
            if !app.is_empty() {
                let matching = self
                    .profiles()
                    .into_iter()
                    .filter(|profile| profile.enabled)
                    .find(|profile| {
                        profile
                            .app_names
                            .iter()
                            .any(|name| name.trim().to_lowercase() == app)
                    });
                if let Some(profile) = matching {
                    return self.effective(&profile, ProfileOrigin::DetectedProfile);
                }
            }
        }

        // 1. The global configuration: the fallback profile.
        let general = self
            .profile(GENERAL_PROFILE_ID)
            .or_else(|| self.profiles().into_iter().next());
        match general {
            Some(profile) => self.effective(&profile, ProfileOrigin::Global),
            None => self.empty_effective(),
        }
    }

    /// Builds the effective configuration of a profile.
    fn effective(&self, profile: &WritingProfile, origin: ProfileOrigin) -> EffectiveProfile {
        let style = self
            .style(&profile.style_id)
            .unwrap_or_else(|| builtin_styles().into_iter().next().expect("a default style"));

        EffectiveProfile {
            profile: profile.clone(),
            cleanup_level: profile.cleanup_level.unwrap_or(style.cleanup_level),
            ai_cleanup: profile.ai_cleanup.unwrap_or(style.ai_cleanup),
            format_rules: profile
                .format_rules
                .clone()
                .unwrap_or_else(|| style.format_rules.clone()),
            language: profile.language.clone(),
            style,
            origin,
        }
    }

    /// The configuration used when there is no profile at all.
    fn empty_effective(&self) -> EffectiveProfile {
        let style = builtin_styles().into_iter().next().expect("a default style");
        let now = chrono::Utc::now().timestamp_millis();
        EffectiveProfile {
            profile: WritingProfile {
                id: GENERAL_PROFILE_ID.to_string(),
                name: "General".to_string(),
                kind: ProfileKind::Builtin,
                app_names: Vec::new(),
                language: None,
                style_id: style.id.clone(),
                cleanup_level: None,
                ai_cleanup: None,
                format_rules: None,
                enabled: true,
                created_at: now,
                updated_at: now,
            },
            cleanup_level: style.cleanup_level,
            ai_cleanup: style.ai_cleanup,
            format_rules: style.format_rules.clone(),
            language: None,
            style,
            origin: ProfileOrigin::Global,
        }
    }

    /// Seeds the built-in styles and profiles, once.
    fn seed_builtins(&self) -> Result<(), String> {
        let conn = self.connection()?;
        let now = chrono::Utc::now().timestamp_millis();

        for style in builtin_styles() {
            let mut style = style;
            style.created_at = now;
            style.updated_at = now;
            insert_style_if_missing(&conn, &style)?;
        }

        for profile in builtin_profiles() {
            let mut profile = profile;
            profile.created_at = now;
            profile.updated_at = now;
            insert_profile_if_missing(&conn, &profile)?;
        }

        self.reload()
    }
}

/// The built-in writing styles.
pub fn builtin_styles() -> Vec<WritingStyle> {
    let now = 0;
    vec![
        WritingStyle {
            id: "default".to_string(),
            name: "Default".to_string(),
            kind: ProfileKind::Builtin,
            base_prompt: String::new(),
            cleanup_level: CleanupLevel::Light,
            ai_cleanup: false,
            format_rules: vec![FormatRule::CollapseSpaces],
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingStyle {
            id: "formal".to_string(),
            name: "Formal".to_string(),
            kind: ProfileKind::Builtin,
            base_prompt: "Escribí de forma formal y profesional, sin perder el contenido.".to_string(),
            cleanup_level: CleanupLevel::Light,
            ai_cleanup: true,
            format_rules: vec![
                FormatRule::CapitalizeSentences,
                FormatRule::CollapseSpaces,
                FormatRule::SpaceBeforePunctuation,
                FormatRule::EnsureFinalPeriod,
            ],
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingStyle {
            id: "casual".to_string(),
            name: "Casual".to_string(),
            kind: ProfileKind::Builtin,
            base_prompt: "Escribí de forma casual y natural, como en una conversación.".to_string(),
            cleanup_level: CleanupLevel::Light,
            ai_cleanup: true,
            format_rules: vec![FormatRule::CollapseSpaces],
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingStyle {
            id: "very-casual".to_string(),
            name: "Very casual".to_string(),
            kind: ProfileKind::Builtin,
            base_prompt: "Escribí de forma muy casual y relajada, con un tono cercano.".to_string(),
            cleanup_level: CleanupLevel::Light,
            ai_cleanup: true,
            format_rules: vec![FormatRule::CollapseSpaces],
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingStyle {
            id: "enthusiastic".to_string(),
            name: "Enthusiastic".to_string(),
            kind: ProfileKind::Builtin,
            base_prompt: "Escribí con entusiasmo y energía, sin exagerar.".to_string(),
            cleanup_level: CleanupLevel::Light,
            ai_cleanup: true,
            format_rules: vec![
                FormatRule::CapitalizeSentences,
                FormatRule::CollapseSpaces,
                FormatRule::EnsureFinalPeriod,
            ],
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingStyle {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            kind: ProfileKind::Builtin,
            base_prompt: String::new(),
            cleanup_level: CleanupLevel::Light,
            ai_cleanup: true,
            format_rules: Vec::new(),
            enabled: true,
            created_at: now,
            updated_at: now,
        },
    ]
}

/// The built-in writing profiles.
pub fn builtin_profiles() -> Vec<WritingProfile> {
    let now = 0;
    vec![
        WritingProfile {
            id: "work".to_string(),
            name: "Work".to_string(),
            kind: ProfileKind::Builtin,
            app_names: vec!["slack.exe".into(), "teams.exe".into(), "discord.exe".into()],
            language: None,
            style_id: "formal".to_string(),
            cleanup_level: None,
            ai_cleanup: None,
            format_rules: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingProfile {
            id: "email".to_string(),
            name: "Email".to_string(),
            kind: ProfileKind::Builtin,
            app_names: vec![
                "outlook.exe".into(),
                "olk.exe".into(),
                "thunderbird.exe".into(),
                "mail.exe".into(),
            ],
            language: None,
            style_id: "formal".to_string(),
            cleanup_level: None,
            ai_cleanup: None,
            format_rules: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingProfile {
            id: "personal".to_string(),
            name: "Personal".to_string(),
            kind: ProfileKind::Builtin,
            app_names: vec!["whatsapp.exe".into(), "telegram.exe".into(), "messenger.exe".into()],
            language: None,
            style_id: "casual".to_string(),
            cleanup_level: None,
            ai_cleanup: None,
            format_rules: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        },
        WritingProfile {
            id: GENERAL_PROFILE_ID.to_string(),
            name: "General".to_string(),
            kind: ProfileKind::Builtin,
            app_names: Vec::new(),
            language: None,
            style_id: "default".to_string(),
            cleanup_level: None,
            ai_cleanup: None,
            format_rules: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        },
    ]
}

/// Builds a writing style, validating the input.
fn build_style(
    kind: ProfileKind,
    input: WritingStyleInput,
    id: Option<&str>,
) -> Result<WritingStyle, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("The style name cannot be empty".to_string());
    }

    let now = chrono::Utc::now().timestamp_millis();
    Ok(WritingStyle {
        id: id
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("style_{now}")),
        name,
        kind,
        base_prompt: input.base_prompt.trim().to_string(),
        cleanup_level: input.cleanup_level,
        ai_cleanup: input.ai_cleanup,
        format_rules: input.format_rules,
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
    })
}

/// Builds a writing profile, validating the input.
fn build_profile(
    kind: ProfileKind,
    input: WritingProfileInput,
    id: Option<&str>,
) -> Result<WritingProfile, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("The profile name cannot be empty".to_string());
    }

    let mut app_names: Vec<String> = Vec::new();
    for candidate in &input.app_names {
        let candidate = candidate.trim().to_lowercase();
        if candidate.is_empty() {
            continue;
        }
        if !app_names.contains(&candidate) {
            app_names.push(candidate);
        }
    }

    let now = chrono::Utc::now().timestamp_millis();
    Ok(WritingProfile {
        id: id
            .map(|value| value.to_string())
            .unwrap_or_else(|| format!("profile_{now}")),
        name,
        kind,
        app_names,
        language: input
            .language
            .as_ref()
            .map(|language| language.trim().to_lowercase())
            .filter(|language| !language.is_empty()),
        style_id: input.style_id,
        cleanup_level: input.cleanup_level,
        ai_cleanup: input.ai_cleanup,
        format_rules: input.format_rules,
        enabled: input.enabled,
        created_at: now,
        updated_at: now,
    })
}

// --- Persistence --------------------------------------------------------

fn load_styles(conn: &Connection, out: &RwLock<Vec<WritingStyle>>) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, kind, base_prompt, cleanup_level, ai_cleanup, format_rules, enabled, created_at, updated_at
             FROM writing_styles",
        )
        .map_err(|e| format!("Failed to prepare the writing styles query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(WritingStyle {
                id: row.get("id")?,
                name: row.get("name")?,
                kind: parse_kind(&row.get::<_, String>("kind")?),
                base_prompt: row.get("base_prompt")?,
                cleanup_level: CleanupLevel::parse(&row.get::<_, String>("cleanup_level")?),
                ai_cleanup: row.get::<_, i64>("ai_cleanup")? != 0,
                format_rules: parse_rules(&row.get::<_, String>("format_rules")?),
                enabled: row.get::<_, i64>("enabled")? != 0,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })
        .map_err(|e| format!("Failed to read the writing styles: {e}"))?;

    let mut styles = Vec::new();
    for row in rows {
        styles.push(row.map_err(|e| format!("Failed to read a writing style: {e}"))?);
    }
    *out.write().map_err(|_| "Writing style lock poisoned".to_string())? = styles;
    Ok(())
}

fn load_profiles(conn: &Connection, out: &RwLock<Vec<WritingProfile>>) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, kind, app_names, language, style_id, cleanup_level, ai_cleanup, format_rules, enabled, created_at, updated_at
             FROM writing_profiles",
        )
        .map_err(|e| format!("Failed to prepare the writing profiles query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(WritingProfile {
                id: row.get("id")?,
                name: row.get("name")?,
                kind: parse_kind(&row.get::<_, String>("kind")?),
                app_names: parse_list(&row.get::<_, String>("app_names")?),
                language: row.get("language")?,
                style_id: row.get("style_id")?,
                cleanup_level: row
                    .get::<_, Option<String>>("cleanup_level")?
                    .map(|level| CleanupLevel::parse(&level)),
                ai_cleanup: row
                    .get::<_, Option<i64>>("ai_cleanup")?
                    .map(|value| value != 0),
                format_rules: row
                    .get::<_, Option<String>>("format_rules")?
                    .map(|rules| parse_rules(&rules)),
                enabled: row.get::<_, i64>("enabled")? != 0,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })
        .map_err(|e| format!("Failed to read the writing profiles: {e}"))?;

    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|e| format!("Failed to read a writing profile: {e}"))?);
    }
    *out.write().map_err(|_| "Writing profile lock poisoned".to_string())? = profiles;
    Ok(())
}

fn load_overrides(conn: &Connection, out: &RwLock<Vec<ProfileOverride>>) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, profile_id, scope_kind, scope_value, style_id, cleanup_level, created_at, updated_at
             FROM profile_overrides",
        )
        .map_err(|e| format!("Failed to prepare the application rules query: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ProfileOverride {
                id: row.get("id")?,
                profile_id: row.get("profile_id")?,
                scope_kind: row.get("scope_kind")?,
                scope_value: row.get("scope_value")?,
                style_id: row.get("style_id")?,
                cleanup_level: row
                    .get::<_, Option<String>>("cleanup_level")?
                    .map(|level| CleanupLevel::parse(&level)),
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })
        .map_err(|e| format!("Failed to read the application rules: {e}"))?;

    let mut overrides = Vec::new();
    for row in rows {
        overrides.push(row.map_err(|e| format!("Failed to read an application rule: {e}"))?);
    }
    *out.write().map_err(|_| "Application rule lock poisoned".to_string())? = overrides;
    Ok(())
}

fn insert_style(conn: &Connection, style: &WritingStyle) -> Result<(), String> {
    conn.execute(
        "INSERT INTO writing_styles
            (id, name, kind, base_prompt, cleanup_level, ai_cleanup, format_rules, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            style.id,
            style.name,
            kind_text(style.kind),
            style.base_prompt,
            style.cleanup_level.as_str(),
            style.ai_cleanup as i64,
            serialize_rules(&style.format_rules),
            style.enabled as i64,
            style.created_at,
            style.updated_at,
        ],
    )
    .map_err(|e| format!("Failed to add the writing style: {e}"))?;
    Ok(())
}

fn insert_style_if_missing(conn: &Connection, style: &WritingStyle) -> Result<(), String> {
    let existing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM writing_styles WHERE id = ?1",
            params![style.id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to check the writing styles: {e}"))?;
    if existing > 0 {
        return Ok(());
    }
    insert_style(conn, style)
}

fn update_style(conn: &Connection, style: &WritingStyle) -> Result<(), String> {
    conn.execute(
        "UPDATE writing_styles
            SET name = ?1, base_prompt = ?2, cleanup_level = ?3, ai_cleanup = ?4,
                format_rules = ?5, enabled = ?6, updated_at = ?7
          WHERE id = ?8",
        params![
            style.name,
            style.base_prompt,
            style.cleanup_level.as_str(),
            style.ai_cleanup as i64,
            serialize_rules(&style.format_rules),
            style.enabled as i64,
            style.updated_at,
            style.id,
        ],
    )
    .map_err(|e| format!("Failed to update the writing style: {e}"))?;
    Ok(())
}

fn insert_profile(conn: &Connection, profile: &WritingProfile) -> Result<(), String> {
    conn.execute(
        "INSERT INTO writing_profiles
            (id, name, kind, app_names, language, style_id, cleanup_level, ai_cleanup, format_rules, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            profile.id,
            profile.name,
            kind_text(profile.kind),
            serialize_list(&profile.app_names),
            profile.language,
            profile.style_id,
            profile.cleanup_level.map(|level| level.as_str().to_string()),
            profile.ai_cleanup.map(|value| value as i64),
            profile
                .format_rules
                .as_ref()
                .map(|rules| serialize_rules(rules))
                .unwrap_or_else(|| "null".to_string()),
            profile.enabled as i64,
            profile.created_at,
            profile.updated_at,
        ],
    )
    .map_err(|e| format!("Failed to add the writing profile: {e}"))?;
    Ok(())
}

fn insert_profile_if_missing(conn: &Connection, profile: &WritingProfile) -> Result<(), String> {
    let existing: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM writing_profiles WHERE id = ?1",
            params![profile.id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to check the writing profiles: {e}"))?;
    if existing > 0 {
        return Ok(());
    }
    insert_profile(conn, profile)
}

fn update_profile(conn: &Connection, profile: &WritingProfile) -> Result<(), String> {
    conn.execute(
        "UPDATE writing_profiles
            SET name = ?1, app_names = ?2, language = ?3, style_id = ?4, cleanup_level = ?5,
                ai_cleanup = ?6, format_rules = ?7, enabled = ?8, updated_at = ?9
          WHERE id = ?10",
        params![
            profile.name,
            serialize_list(&profile.app_names),
            profile.language,
            profile.style_id,
            profile.cleanup_level.map(|level| level.as_str().to_string()),
            profile.ai_cleanup.map(|value| value as i64),
            profile
                .format_rules
                .as_ref()
                .map(|rules| serialize_rules(rules))
                .unwrap_or_else(|| "null".to_string()),
            profile.enabled as i64,
            profile.updated_at,
            profile.id,
        ],
    )
    .map_err(|e| format!("Failed to update the writing profile: {e}"))?;
    Ok(())
}

/// Parses a stored kind, defaulting to a custom one.
fn parse_kind(text: &str) -> ProfileKind {
    match text {
        "builtin" => ProfileKind::Builtin,
        _ => ProfileKind::Custom,
    }
}

/// The wire value of a kind.
fn kind_text(kind: ProfileKind) -> &'static str {
    match kind {
        ProfileKind::Builtin => "builtin",
        ProfileKind::Custom => "custom",
    }
}

/// Parses a stored list of names.
fn parse_list(text: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(text).unwrap_or_default()
}

/// Serializes a list of names.
fn serialize_list(names: &[String]) -> String {
    serde_json::to_string(names).unwrap_or_else(|_| "[]".to_string())
}

/// Parses the stored format rules.
fn parse_rules(text: &str) -> Vec<FormatRule> {
    serde_json::from_str::<Vec<FormatRule>>(text).unwrap_or_default()
}

/// Serializes the format rules.
fn serialize_rules(rules: &[FormatRule]) -> String {
    serde_json::to_string(rules).unwrap_or_else(|_| "[]".to_string())
}

/// The distinct application names known to the profiles.
pub fn known_app_names(profiles: &[WritingProfile]) -> Vec<String> {
    let mut names: HashSet<String> = HashSet::new();
    for profile in profiles {
        for name in &profile.app_names {
            names.insert(name.trim().to_lowercase());
        }
    }
    let mut sorted: Vec<String> = names.into_iter().collect();
    sorted.sort();
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn manager() -> (TempDir, ProfileManager) {
        let dir = TempDir::new().expect("a temporary folder");
        let manager = ProfileManager::with_db_path(dir.path().join("dixi.db")).expect("the manager opens");
        (dir, manager)
    }

    #[test]
    fn the_built_in_styles_and_profiles_are_seeded() {
        let (_dir, manager) = manager();
        let styles = manager.styles();
        for id in ["default", "formal", "casual", "very-casual", "enthusiastic", "custom"] {
            assert!(
                styles.iter().any(|style| style.id == id),
                "the style {id} must exist"
            );
        }

        let profiles = manager.profiles();
        for id in ["work", "email", "personal", GENERAL_PROFILE_ID] {
            assert!(
                profiles.iter().any(|profile| profile.id == id),
                "the profile {id} must exist"
            );
        }
    }

    #[test]
    fn seeding_is_idempotent() {
        let dir = TempDir::new().expect("a temporary folder");
        let path = dir.path().join("dixi.db");
        let first = ProfileManager::with_db_path(path.clone()).expect("opened");
        let count = first.profiles().len();
        drop(first);

        let second = ProfileManager::with_db_path(path).expect("reopened");
        assert_eq!(second.profiles().len(), count, "the profiles must not be duplicated");
    }

    #[test]
    fn a_disabled_built_in_stays_disabled_after_seeding() {
        let dir = TempDir::new().expect("a temporary folder");
        let path = dir.path().join("dixi.db");
        {
            let manager = ProfileManager::with_db_path(path.clone()).expect("opened");
            let mut input = WritingProfileInput {
                name: "Work".to_string(),
                app_names: Vec::new(),
                language: None,
                style_id: "formal".to_string(),
                cleanup_level: None,
                ai_cleanup: None,
                format_rules: None,
                enabled: false,
            };
            input.name = "Work".to_string();
            manager.update_profile("work", input).expect("updated");
        }
        let reopened = ProfileManager::with_db_path(path).expect("reopened");
        let work = reopened.profile("work").expect("the profile exists");
        assert!(!work.enabled, "seeding must not re-enable a disabled profile");
    }

    #[test]
    fn the_general_profile_is_used_when_nothing_matches() {
        let (_dir, manager) = manager();
        let effective = manager.resolve(Some("unknown-app.exe"), None);
        assert_eq!(effective.origin, ProfileOrigin::Global);
        assert_eq!(effective.profile.id, GENERAL_PROFILE_ID);
    }

    #[test]
    fn the_profile_of_the_detected_application_is_used() {
        let (_dir, manager) = manager();
        let effective = manager.resolve(Some("slack.exe"), None);
        assert_eq!(effective.origin, ProfileOrigin::DetectedProfile);
        assert_eq!(effective.profile.id, "work");
        assert_eq!(effective.style.id, "formal");
    }

    #[test]
    fn a_manual_override_wins_over_everything() {
        let (_dir, manager) = manager();
        let effective = manager.resolve(Some("slack.exe"), Some("personal"));
        assert_eq!(effective.origin, ProfileOrigin::ManualOverride);
        assert_eq!(effective.profile.id, "personal");
    }

    #[test]
    fn an_application_rule_wins_over_the_profile() {
        let (_dir, manager) = manager();
        manager
            .add_override("personal", "slack.exe", Some("casual".to_string()), None)
            .expect("the rule is added");

        let effective = manager.resolve(Some("slack.exe"), None);
        assert_eq!(effective.origin, ProfileOrigin::AppOverride);
        assert_eq!(effective.style.id, "casual");
    }

    #[test]
    fn a_disabled_profile_does_not_match() {
        let (_dir, manager) = manager();
        manager
            .update_profile(
                "work",
                WritingProfileInput {
                    name: "Work".to_string(),
                    app_names: vec!["slack.exe".to_string()],
                    language: None,
                    style_id: "formal".to_string(),
                    cleanup_level: None,
                    ai_cleanup: None,
                    format_rules: None,
                    enabled: false,
                },
            )
            .expect("updated");

        let effective = manager.resolve(Some("slack.exe"), None);
        assert_eq!(effective.origin, ProfileOrigin::Global);
    }

    #[test]
    fn a_built_in_profile_cannot_be_deleted_and_a_custom_one_can() {
        let (_dir, manager) = manager();
        assert!(manager.delete_profile("work").is_err());

        let custom = manager
            .add_profile(WritingProfileInput {
                name: "Mine".to_string(),
                app_names: vec!["myapp.exe".to_string()],
                language: None,
                style_id: "casual".to_string(),
                cleanup_level: None,
                ai_cleanup: None,
                format_rules: None,
                enabled: true,
            })
            .expect("the profile is added");
        manager.delete_profile(&custom.id).expect("a custom profile can be deleted");
    }

    #[test]
    fn a_profile_with_an_empty_name_is_refused() {
        let (_dir, manager) = manager();
        let error = manager
            .add_profile(WritingProfileInput {
                name: "   ".to_string(),
                app_names: Vec::new(),
                language: None,
                style_id: "casual".to_string(),
                cleanup_level: None,
                ai_cleanup: None,
                format_rules: None,
                enabled: true,
            })
            .expect_err("an empty name must be refused");
        assert!(error.contains("name"));
    }

    #[test]
    fn the_format_rules_are_deterministic() {
        assert_eq!(
            apply_format_rules("hola , mundo", &[FormatRule::SpaceBeforePunctuation]),
            "hola, mundo"
        );
        assert_eq!(
            apply_format_rules("hola   mundo", &[FormatRule::CollapseSpaces]),
            "hola mundo"
        );
        assert_eq!(
            apply_format_rules("hola mundo", &[FormatRule::CapitalizeSentences]),
            "Hola mundo"
        );
        assert_eq!(
            apply_format_rules("hola mundo", &[FormatRule::EnsureFinalPeriod]),
            "hola mundo."
        );
    }

    #[test]
    fn the_format_rules_keep_the_line_breaks() {
        let text = "primera  linea\nsegunda  linea";
        let result = apply_format_rules(text, &[FormatRule::CollapseSpaces]);
        assert_eq!(result, "primera linea\nsegunda linea");
    }

    #[test]
    fn the_cleanup_levels_parse_and_render() {
        assert_eq!(CleanupLevel::parse("none"), CleanupLevel::None);
        assert_eq!(CleanupLevel::parse("MEDIUM"), CleanupLevel::Medium);
        assert_eq!(CleanupLevel::parse("whatever"), CleanupLevel::Light);
        assert_eq!(CleanupLevel::Medium.as_str(), "medium");
    }
}
