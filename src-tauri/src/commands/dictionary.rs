use crate::managers::dictionary::{
    apply_substitutions, DictionaryEntry, DictionaryEntryInput, DictionaryManager, GLOBAL_SCOPE,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Resolves the scope a request applies to.
fn resolve_scope(scope: &Option<String>) -> String {
    scope
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(GLOBAL_SCOPE)
        .to_string()
}

/// Every dictionary entry, longest term first.
#[tauri::command]
#[specta::specta]
pub async fn list_dictionary_entries(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    scope: Option<String>,
) -> Result<Vec<DictionaryEntry>, String> {
    let scope = resolve_scope(&scope);
    Ok(dictionary_manager.list_for_scope(&scope))
}

/// Adds a dictionary entry.
#[tauri::command]
#[specta::specta]
pub async fn add_dictionary_entry(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    entry: DictionaryEntryInput,
) -> Result<DictionaryEntry, String> {
    dictionary_manager.add(&entry)
}

/// Updates a dictionary entry.
#[tauri::command]
#[specta::specta]
pub async fn update_dictionary_entry(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    id: i64,
    entry: DictionaryEntryInput,
) -> Result<(), String> {
    dictionary_manager.update(id, &entry)
}

/// Deletes a dictionary entry.
#[tauri::command]
#[specta::specta]
pub async fn delete_dictionary_entry(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    id: i64,
) -> Result<(), String> {
    dictionary_manager.delete(id)
}

/// Enables or disables a dictionary entry.
#[tauri::command]
#[specta::specta]
pub async fn set_dictionary_entry_enabled(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    dictionary_manager.set_enabled(id, enabled)
}

/// Adds several entries at once, skipping duplicates.
/// Returns how many entries were actually added.
#[tauri::command]
#[specta::specta]
pub async fn import_dictionary_entries(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    entries: Vec<DictionaryEntryInput>,
) -> Result<usize, String> {
    dictionary_manager.import(&entries)
}

/// The vocabulary the ASR engine would receive for the given scope.
#[tauri::command]
#[specta::specta]
pub async fn get_dictionary_vocabulary(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    scope: Option<String>,
) -> Result<Vec<String>, String> {
    let scope = resolve_scope(&scope);
    Ok(dictionary_manager.vocabulary(&scope))
}

/// Applies the substitution rules to a piece of text and returns the result.
///
/// Used by the settings screen to preview a rule before enabling it, and by
/// the tests to exercise the engine end to end.
#[tauri::command]
#[specta::specta]
pub async fn apply_dictionary_substitutions(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    text: String,
    scope: Option<String>,
) -> Result<String, String> {
    let scope = resolve_scope(&scope);
    let rules = dictionary_manager.substitution_rules(&scope);
    Ok(apply_substitutions(&text, &rules, None).text)
}

/// The on-disk shape of an exported dictionary.
#[derive(serde::Serialize, serde::Deserialize)]
struct DictionaryExport {
    entries: Vec<DictionaryEntryInput>,
}

/// Writes the dictionary to a JSON file the user chose.
///
/// The file is written by the backend on purpose: the frontend's filesystem
/// plugin is scoped to the application data directory, and widening that
/// scope to every place a user might save a file would hand the webview
/// more reach than the feature needs. The path always comes from the
/// native save dialog, which is the user choosing the destination.
#[tauri::command]
#[specta::specta]
pub async fn export_dictionary_entries(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    path: String,
) -> Result<usize, String> {
    // A backup carries every entry, including the scoped ones, so that
    // restoring it on another machine reproduces the same dictionary.
    let entries = dictionary_manager.list();

    let payload = DictionaryExport {
        entries: entries
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

    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write the dictionary file '{path}': {e}"))?;

    Ok(payload.entries.len())
}

/// Reads a dictionary from a JSON file the user chose and adds its entries.
///
/// Entries that already exist are skipped, so importing the same file twice
/// changes nothing. Returns how many entries were actually added.
#[tauri::command]
#[specta::specta]
pub async fn import_dictionary_entries_from_file(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    path: String,
) -> Result<usize, String> {
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read the dictionary file '{path}': {e}"))?;

    let payload: DictionaryExport = serde_json::from_str(&text)
        .map_err(|e| format!("'{path}' is not a valid dictionary file: {e}"))?;

    dictionary_manager.import(&payload.entries)
}
