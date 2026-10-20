use crate::managers::selection::{current_selection, replace_selection, SelectionError};
use tauri::AppHandle;

/// The text that is selected in the application that has the focus.
#[tauri::command]
#[specta::specta]
pub async fn capture_selection() -> Result<SelectionInfo, String> {
    match current_selection() {
        Ok(selection) => Ok(SelectionInfo {
            text: selection.text,
            window: selection.window,
            control: selection.control,
            app_name: selection.app_name,
        }),
        Err(SelectionError::NothingSelected) => Ok(SelectionInfo {
            text: String::new(),
            window: 0,
            control: 0,
            app_name: None,
        }),
        Err(error) => Err(describe(error)),
    }
}

/// Replaces the selection, but only if it is still the one that was read.
///
/// When the selection changed while the transformation was running, nothing
/// is written: the caller offers to copy the result instead.
#[tauri::command]
#[specta::specta]
pub async fn replace_text_selection(
    _app: AppHandle,
    expected_text: String,
    expected_window: isize,
    new_text: String,
) -> Result<bool, String> {
    let expected = crate::managers::selection::Selection {
        text: expected_text,
        window: expected_window,
        control: 0,
        app_name: None,
    };

    match replace_selection(&expected, &new_text) {
        Ok(()) => Ok(true),
        Err(SelectionError::SelectionChanged) => Ok(false),
        Err(error) => Err(describe(error)),
    }
}

/// A selection, ready to be shown.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SelectionInfo {
    /// The selected text. Empty when nothing was selected.
    pub text: String,
    /// The handle of the window that had the focus.
    pub window: isize,
    /// The handle of the focused control.
    pub control: isize,
    /// The name of the executable, when it could be read.
    pub app_name: Option<String>,
}

/// Turns a selection failure into a message the user can read.
fn describe(error: SelectionError) -> String {
    match error {
        SelectionError::NothingSelected => "There is no selected text".to_string(),
        SelectionError::PasswordField => {
            "A password field cannot be read or written".to_string()
        }
        SelectionError::Clipboard(detail) => {
            format!("The clipboard could not be used: {detail}")
        }
        SelectionError::SelectionChanged => {
            "The selection changed while the transformation was running".to_string()
        }
        SelectionError::Write(detail) => format!("The text could not be written: {detail}"),
    }
}
