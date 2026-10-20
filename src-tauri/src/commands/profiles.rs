use crate::managers::app_detection::foreground_app;
use crate::managers::profiles::{
    CleanupLevel, FormatRule, ProfileManager, ProfileOverride, WritingProfile, WritingProfileInput,
    WritingStyle, WritingStyleInput,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// The application that has the focus, when it can be determined.
#[tauri::command]
#[specta::specta]
pub async fn get_foreground_app() -> Result<Option<ForegroundAppInfo>, String> {
    Ok(foreground_app().map(|app| ForegroundAppInfo {
        process_name: app.process_name,
        window_title: app.window_title,
    }))
}

/// What is known about the application that has the focus.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ForegroundAppInfo {
    /// The executable name, lowercased.
    pub process_name: String,
    /// The title of the window, when it could be read.
    pub window_title: Option<String>,
}

/// Every writing style.
#[tauri::command]
#[specta::specta]
pub async fn list_writing_styles(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
) -> Result<Vec<WritingStyle>, String> {
    Ok(profile_manager.styles())
}

/// Adds a custom writing style.
#[tauri::command]
#[specta::specta]
pub async fn add_writing_style(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    style: WritingStyleInput,
) -> Result<WritingStyle, String> {
    profile_manager.add_style(style)
}

/// Updates a writing style.
#[tauri::command]
#[specta::specta]
pub async fn update_writing_style(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    id: String,
    style: WritingStyleInput,
) -> Result<(), String> {
    profile_manager.update_style(&id, style)
}

/// Deletes a custom writing style.
#[tauri::command]
#[specta::specta]
pub async fn delete_writing_style(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    id: String,
) -> Result<(), String> {
    profile_manager.delete_style(&id)
}

/// Every writing profile.
#[tauri::command]
#[specta::specta]
pub async fn list_writing_profiles(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
) -> Result<Vec<WritingProfile>, String> {
    Ok(profile_manager.profiles())
}

/// Adds a custom writing profile.
#[tauri::command]
#[specta::specta]
pub async fn add_writing_profile(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    profile: WritingProfileInput,
) -> Result<WritingProfile, String> {
    profile_manager.add_profile(profile)
}

/// Updates a writing profile.
#[tauri::command]
#[specta::specta]
pub async fn update_writing_profile(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    id: String,
    profile: WritingProfileInput,
) -> Result<(), String> {
    profile_manager.update_profile(&id, profile)
}

/// Deletes a custom writing profile.
#[tauri::command]
#[specta::specta]
pub async fn delete_writing_profile(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    id: String,
) -> Result<(), String> {
    profile_manager.delete_profile(&id)
}

/// Every rule bound to a specific application.
#[tauri::command]
#[specta::specta]
pub async fn list_profile_overrides(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
) -> Result<Vec<ProfileOverride>, String> {
    Ok(profile_manager.overrides())
}

/// Binds a rule to an application.
#[tauri::command]
#[specta::specta]
pub async fn add_profile_override(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    profile_id: String,
    scope_value: String,
    style_id: Option<String>,
    cleanup_level: Option<CleanupLevel>,
) -> Result<ProfileOverride, String> {
    profile_manager.add_override(&profile_id, &scope_value, style_id, cleanup_level)
}

/// Removes a rule bound to an application.
#[tauri::command]
#[specta::specta]
pub async fn delete_profile_override(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    id: String,
) -> Result<(), String> {
    profile_manager.delete_override(&id)
}

/// The configuration that applies to one dictation.
///
/// `app_name` is the detected application; `manual_override` is a profile
/// the user chose for this session only.
#[tauri::command]
#[specta::specta]
pub async fn resolve_writing_profile(
    _app: AppHandle,
    profile_manager: State<'_, Arc<ProfileManager>>,
    app_name: Option<String>,
    manual_override: Option<String>,
) -> Result<EffectiveProfileInfo, String> {
    let effective = profile_manager.resolve(app_name.as_deref(), manual_override.as_deref());
    Ok(EffectiveProfileInfo {
        profile_id: effective.profile.id,
        profile_name: effective.profile.name,
        style_id: effective.style.id,
        style_name: effective.style.name,
        cleanup_level: effective.cleanup_level,
        ai_cleanup: effective.ai_cleanup,
        format_rules: effective.format_rules,
        language: effective.language,
        origin: effective.origin,
    })
}

/// The configuration that applies to one dictation.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct EffectiveProfileInfo {
    pub profile_id: String,
    pub profile_name: String,
    pub style_id: String,
    pub style_name: String,
    pub cleanup_level: CleanupLevel,
    pub ai_cleanup: bool,
    pub format_rules: Vec<FormatRule>,
    pub language: Option<String>,
    pub origin: crate::managers::profiles::ProfileOrigin,
}
