//! The commands that read and replace the text selection of another
//! application.
//!
//! The capture only works while the application the text was selected in
//! has the focus, so it is done from the global shortcut, which does not
//! move the focus. The result is kept with the context of the selection —
//! the window, the control and the text — and that context is what the
//! write needs to put the result back in exactly the same place.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::managers::selection::{
    self, SelectionContext, SelectionContextStore, SelectionFailure,
};
use crate::managers::text_history::{HistoryKind, TextHistoryManager};
use crate::settings::get_settings;

/// The state of the capture shortcut.
///
/// A global shortcut that another application already holds does not fire,
/// and the user would have no way of knowing why. The state is exposed so the
/// interface can say it.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct CaptureShortcutState {
    /// Whether the shortcut could be registered.
    pub registered: bool,
    /// The combination it is registered with.
    pub binding: String,
    /// Whether this system can capture the selection of another application
    /// at all.
    pub available: bool,
}

/// The state of the capture shortcut.
#[tauri::command]
#[specta::specta]
pub async fn get_capture_shortcut_state(app: AppHandle) -> Result<CaptureShortcutState, String> {
    let settings = get_settings(&app);
    let binding = settings
        .bindings
        .get("capture_selection")
        .map(|binding| binding.current_binding.clone())
        .unwrap_or_default();

    Ok(CaptureShortcutState {
        registered: crate::shortcut::capture_shortcut_registered(),
        binding,
        available: cfg!(target_os = "windows"),
    })
}

/// Captures the selection of the application that has the focus.
///
/// The application that has the focus has to be the one the text was
/// selected in. A click on a button of Dixi gives Dixi the focus, so from
/// the interface this only works when Dixi does not have the focus; the
/// reliable way is the global shortcut, which leaves the focus where it is.
#[tauri::command]
#[specta::specta]
pub async fn capture_selection(app: AppHandle) -> Result<SelectionContext, SelectionFailure> {
    let context = selection::capture_context().map_err(selection::failure)?;
    app.state::<SelectionContextStore>().set(context.clone());
    Ok(context)
}

/// The selection captured the last time, if there is one.
#[tauri::command]
#[specta::specta]
pub async fn get_captured_selection(
    app: AppHandle,
) -> Result<Option<SelectionContext>, SelectionFailure> {
    Ok(app.state::<SelectionContextStore>().get())
}

/// Forgets the captured selection.
#[tauri::command]
#[specta::specta]
pub async fn clear_captured_selection(app: AppHandle) -> Result<(), SelectionFailure> {
    app.state::<SelectionContextStore>().clear();
    Ok(())
}

/// Writes text over the captured selection.
///
/// Nothing is written unless the window, the control and the selected text
/// are still the ones that were captured, and the destination is not a
/// protected field. When the selection moved, the failure says so and
/// nothing is written: the caller then offers the text as a copy instead of
/// writing it somewhere else.
///
/// The change is recorded in the history with the place where it was made,
/// so that it can be put back later.
#[tauri::command]
#[specta::specta]
pub async fn apply_to_captured_selection(
    app: AppHandle,
    history_manager: State<'_, Arc<TextHistoryManager>>,
    new_text: String,
    source_id: Option<String>,
) -> Result<Option<i64>, SelectionFailure> {
    let store = app.state::<SelectionContextStore>();
    let context = store
        .get()
        .ok_or_else(|| selection::failure(selection::SelectionError::NothingSelected))?;

    let expected = context.text.clone();

    // The change is recorded BEFORE the write, with the place where it is
    // made: without that place, putting the original text back would mean
    // writing it wherever the focus happens to be. If the write fails, the
    // entry is removed, so the history only holds what really happened.
    let entry = history_manager
        .record_with_context(
            HistoryKind::Transform,
            source_id.as_deref(),
            context.app_name.as_deref(),
            Some(&context),
            &expected,
            &new_text,
        )
        .map_err(|error| selection::failure(selection::SelectionError::Write(error)))?;

    match selection::write_to_context(&context, &expected, &new_text) {
        Ok(()) => Ok(entry),
        Err(error) => {
            if let Some(id) = entry {
                let _ = history_manager.remove(id);
            }
            // A selection that can no longer be read is a changed
            // selection: the caller says so and offers to copy.
            match error {
                selection::SelectionError::NothingSelected => Err(selection::failure(
                    selection::SelectionError::SelectionChanged,
                )),
                other => Err(selection::failure(other)),
            }
        }
    }
}
