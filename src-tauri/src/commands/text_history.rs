use crate::managers::diff::{diff_words, DiffSegmentKind};
use crate::managers::text_history::{HistoryKind, TextHistoryEntry, TextHistoryManager};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// The local history of the text changes Dixi made.
#[tauri::command]
#[specta::specta]
pub async fn list_text_history(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
    limit: Option<usize>,
) -> Result<Vec<TextHistoryEntry>, String> {
    Ok(history_manager.list(limit.unwrap_or(50)))
}

/// Records a change, so it can be reverted.
#[tauri::command]
#[specta::specta]
pub async fn record_text_history(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
    kind: HistoryKind,
    source_id: Option<String>,
    app_name: Option<String>,
    original_text: String,
    result_text: String,
) -> Result<Option<i64>, String> {
    history_manager.record(
        kind,
        source_id.as_deref(),
        app_name.as_deref(),
        &original_text,
        &result_text,
    )
}

/// The most recent change that can still be reverted.
#[tauri::command]
#[specta::specta]
pub async fn last_undoable_change(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
) -> Result<Option<TextHistoryEntry>, String> {
    Ok(history_manager.last_undoable())
}

/// Marks a change as reverted.
#[tauri::command]
#[specta::specta]
pub async fn mark_text_change_undone(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
    id: i64,
) -> Result<(), String> {
    history_manager.mark_undone(id)
}

/// Empties the local history.
#[tauri::command]
#[specta::specta]
pub async fn clear_text_history(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
) -> Result<(), String> {
    history_manager.clear()
}

/// How long the history is kept, in days.
#[tauri::command]
#[specta::specta]
pub async fn get_text_history_retention(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
) -> Result<i64, String> {
    Ok(history_manager.retention_days())
}

/// Sets how long the history is kept, in days. Zero means "until it is cleared".
#[tauri::command]
#[specta::specta]
pub async fn set_text_history_retention(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
    days: i64,
) -> Result<(), String> {
    history_manager.set_retention_days(days)
}

/// Removes the entries that are older than the retention.
#[tauri::command]
#[specta::specta]
pub async fn purge_text_history(
    _app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
) -> Result<usize, String> {
    history_manager.purge_expired()
}

/// Compares two texts word by word.
///
/// This is what the user sees before a transformation replaces something.
#[tauri::command]
#[specta::specta]
pub async fn diff_text(original: String, result: String) -> Result<TextDiffInfo, String> {
    let diff = diff_words(&original, &result);
    let inserted_words = diff.inserted_words;
    let removed_words = diff.removed_words;
    let identical = diff.is_identical();
    Ok(TextDiffInfo {
        segments: diff
            .segments
            .into_iter()
            .map(|segment| DiffSegmentInfo {
                kind: match segment.kind {
                    DiffSegmentKind::Equal => "equal",
                    DiffSegmentKind::Inserted => "inserted",
                    DiffSegmentKind::Removed => "removed",
                }
                .to_string(),
                text: segment.text,
            })
            .collect(),
        inserted_words,
        removed_words,
        identical,
    })
}

/// A word-level diff, ready to be shown.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct TextDiffInfo {
    pub segments: Vec<DiffSegmentInfo>,
    pub inserted_words: usize,
    pub removed_words: usize,
    pub identical: bool,
}

/// One part of a diff.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DiffSegmentInfo {
    /// `equal`, `inserted` or `removed`.
    pub kind: String,
    pub text: String,
}
