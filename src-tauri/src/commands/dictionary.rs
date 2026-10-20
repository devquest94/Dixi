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

/// Writes the dictionary to a JSON file the user chose.
///
/// The backend owns the file format and the filesystem access, so the
/// webview never touches the file itself. The path always comes from the
/// native save dialog.
///
/// An existing file is only replaced when `overwrite` is set: the caller has
/// to ask the user first.
#[tauri::command]
#[specta::specta]
pub async fn export_dictionary_entries(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    path: String,
    overwrite: bool,
) -> Result<usize, String> {
    dictionary_manager.export_to_file(std::path::Path::new(&path), overwrite)
}

/// Reads a dictionary from a JSON file the user chose and adds its entries.
///
/// Entries that already exist are skipped. Returns how many entries were
/// actually added.
#[tauri::command]
#[specta::specta]
pub async fn import_dictionary_entries_from_file(
    _app: AppHandle,
    dictionary_manager: State<'_, Arc<DictionaryManager>>,
    path: String,
) -> Result<usize, String> {
    dictionary_manager.import_from_file(std::path::Path::new(&path))
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
