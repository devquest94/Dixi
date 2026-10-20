use crate::managers::snippets::{Expansion, Snippet, SnippetInput, SnippetManager};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Every snippet, favourites first.
#[tauri::command]
#[specta::specta]
pub async fn list_snippets(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
) -> Result<Vec<Snippet>, String> {
    Ok(snippet_manager.list())
}

/// Adds a snippet.
#[tauri::command]
#[specta::specta]
pub async fn add_snippet(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    snippet: SnippetInput,
) -> Result<Snippet, String> {
    snippet_manager.add(snippet)
}

/// Updates a snippet.
#[tauri::command]
#[specta::specta]
pub async fn update_snippet(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    id: String,
    snippet: SnippetInput,
) -> Result<(), String> {
    snippet_manager.update(&id, snippet)
}

/// Deletes a snippet.
#[tauri::command]
#[specta::specta]
pub async fn delete_snippet(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    id: String,
) -> Result<(), String> {
    snippet_manager.delete(&id)
}

/// Stars or unstars a snippet.
#[tauri::command]
#[specta::specta]
pub async fn set_snippet_favorite(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    id: String,
    favorite: bool,
) -> Result<(), String> {
    snippet_manager.set_favorite(&id, favorite)
}

/// Enables or disables a snippet.
#[tauri::command]
#[specta::specta]
pub async fn set_snippet_enabled(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    snippet_manager.set_enabled(&id, enabled)
}

/// The trigger phrases of the snippets that apply.
///
/// These are the phrases the ASR engine hears as vocabulary, so it can
/// transcribe them correctly.
#[tauri::command]
#[specta::specta]
pub async fn get_snippet_vocabulary(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    app_name: Option<String>,
    language: Option<String>,
) -> Result<Vec<String>, String> {
    Ok(snippet_manager.trigger_vocabulary(
        app_name.as_deref(),
        language.as_deref(),
    ))
}

/// Expands a snippet in a piece of text.
///
/// When the snippet needs a variable the user has not supplied, the answer
/// says which one is missing: nothing is expanded until it is answered, so a
/// fragment is never pasted half resolved.
#[tauri::command]
#[specta::specta]
pub async fn expand_snippet(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    text: String,
    values: HashMap<String, String>,
    app_name: Option<String>,
    language: Option<String>,
) -> Result<SnippetExpansionResult, String> {
    let expansion = snippet_manager.expand_in_text(
        &text,
        &values,
        app_name.as_deref(),
        language.as_deref(),
    );

    Ok(match expansion {
        None => SnippetExpansionResult {
            text,
            expanded: false,
            needs_input: None,
        },
        Some(Expansion::Text(expanded)) => {
            SnippetExpansionResult {
                text: expanded,
                expanded: true,
                needs_input: None,
            }
        }
        Some(Expansion::NeedsInput {
            snippet_id,
            variable,
        }) => SnippetExpansionResult {
            text,
            expanded: false,
            needs_input: Some(SnippetInputRequest {
                snippet_id,
                variable_name: variable.name,
                prompt: variable.prompt,
                required: variable.required,
            }),
        },
    })
}

/// The result of expanding a snippet.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SnippetExpansionResult {
    /// The expanded text, or the original text when nothing was expanded.
    pub text: String,
    /// Whether a snippet was expanded.
    pub expanded: bool,
    /// The variable the user has to answer before the expansion can happen.
    pub needs_input: Option<SnippetInputRequest>,
}

/// A variable the user has to answer.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SnippetInputRequest {
    pub snippet_id: String,
    pub variable_name: String,
    /// The text to show the user when asking for the value.
    pub prompt: String,
    pub required: bool,
}

/// The snippets that compete for the same trigger phrase.
///
/// The winner is the first entry: the longest trigger, then the highest
/// priority, then the alphabetically first. The UI shows the rest so the
/// user can see which rule is winning.
#[tauri::command]
#[specta::specta]
pub async fn list_competing_snippets(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    trigger: String,
) -> Result<Vec<String>, String> {
    Ok(snippet_manager
        .competing_rules(&trigger)
        .into_iter()
        .map(|rule| rule.needle)
        .collect())
}

/// Writes the snippets to a JSON file the user chose.
#[tauri::command]
#[specta::specta]
pub async fn export_snippets(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    path: String,
    overwrite: bool,
) -> Result<usize, String> {
    snippet_manager.export_to_file(std::path::Path::new(&path), overwrite)
}

/// Adds the snippets contained in a JSON file the user chose.
#[tauri::command]
#[specta::specta]
pub async fn import_snippets_from_file(
    _app: AppHandle,
    snippet_manager: State<'_, Arc<SnippetManager>>,
    path: String,
) -> Result<usize, String> {
    snippet_manager.import_from_file(std::path::Path::new(&path))
}
