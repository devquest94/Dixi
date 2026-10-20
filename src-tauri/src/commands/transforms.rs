use crate::managers::transforms::{Transform, TransformInput, TransformManager};
use crate::settings::{get_settings, PostProcessProvider};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Every transform: the built-in catalogue and the custom ones.
#[tauri::command]
#[specta::specta]
pub async fn list_transforms(
    _app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
) -> Result<Vec<Transform>, String> {
    Ok(transform_manager.list())
}

/// The built-in catalogue of transforms.
///
/// It is returned apart so the UI can offer the built-ins as a reference
/// while the user writes their own.
#[tauri::command]
#[specta::specta]
pub async fn list_builtin_transforms() -> Result<Vec<Transform>, String> {
    Ok(TransformManager::builtin_catalogue())
}

/// Adds a custom transform.
#[tauri::command]
#[specta::specta]
pub async fn add_transform(
    _app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
    transform: TransformInput,
) -> Result<Transform, String> {
    transform_manager.add(transform)
}

/// Updates a transform.
#[tauri::command]
#[specta::specta]
pub async fn update_transform(
    _app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
    id: String,
    transform: TransformInput,
) -> Result<(), String> {
    transform_manager.update(&id, transform)
}

/// Deletes a custom transform. A built-in one can only be disabled.
#[tauri::command]
#[specta::specta]
pub async fn delete_transform(
    _app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
    id: String,
) -> Result<(), String> {
    transform_manager.delete(&id)
}

/// Enables or disables a transform.
#[tauri::command]
#[specta::specta]
pub async fn set_transform_enabled(
    _app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    transform_manager.set_enabled(&id, enabled)
}

/// Runs a transform over a piece of text.
///
/// The original text is returned unchanged when anything goes wrong: the
/// provider failed, the quota ran out, the connection dropped or the answer
/// came back empty. The reason is reported apart, and never mixed into the
/// text.
#[tauri::command]
#[specta::specta]
pub async fn run_transform(
    app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
    id: String,
    text: String,
) -> Result<TransformRunResult, String> {
    let Some(mut transform) = transform_manager.get(&id) else {
        return Err(format!("Transform '{id}' not found"));
    };

    let settings = get_settings(&app);

    // The provider: the one the transform pins, or the one the user chose.
    let provider_id = transform
        .provider_id
        .clone()
        .or_else(|| Some(settings.post_process_provider_id.clone()))
        .unwrap_or_else(|| "openai".to_string());

    let Some(provider) = settings
        .post_process_providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned()
    else {
        return Ok(TransformRunResult {
            text: text.clone(),
            changed: false,
            error: Some(format!("The provider '{provider_id}' is not configured")),
        });
    };

    let api_key = settings
        .post_process_api_keys
        .get(&provider_id)
        .cloned()
        .unwrap_or_default();

    // A transform that does not pin a model uses the one configured for its
    // provider.
    if transform
        .model
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        transform.model = settings.post_process_models.get(&provider_id).cloned();
    }

    let effort = settings.post_process_efforts.get(&provider_id).cloned();

    let outcome = transform_manager
        .run(&transform, &text, &provider, &api_key, effort.as_deref())
        .await;

    Ok(TransformRunResult {
        text: outcome.text,
        changed: outcome.changed,
        error: outcome.error,
    })
}

/// The result of running a transform.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TransformRunResult {
    /// The transformed text, or the original text when it failed.
    pub text: String,
    /// Whether the text was actually modified.
    pub changed: bool,
    /// Why it failed, when it failed.
    pub error: Option<String>,
}

/// The provider a transform would use, so the UI can show it.
#[tauri::command]
#[specta::specta]
pub async fn get_transform_provider(
    app: AppHandle,
    transform_manager: State<'_, Arc<TransformManager>>,
    id: String,
) -> Result<Option<PostProcessProvider>, String> {
    let settings = get_settings(&app);
    let Some(transform) = transform_manager.get(&id) else {
        return Ok(None);
    };
    let provider_id = transform
        .provider_id
        .clone()
        .unwrap_or_else(|| settings.post_process_provider_id.clone());
    Ok(settings
        .post_process_providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .cloned())
}
