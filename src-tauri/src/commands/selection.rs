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
///
/// The control that holds the selection is part of the identity of the
/// selection: the same text in another control is not the same selection.
#[tauri::command]
#[specta::specta]
pub async fn replace_text_selection(
    _app: AppHandle,
    expected_text: String,
    expected_window: isize,
    expected_control: isize,
    new_text: String,
) -> Result<bool, String> {
    let expected = expected_selection(expected_text, expected_window, expected_control);

    match replace_selection(&expected, &new_text) {
        Ok(()) => Ok(true),
        Err(SelectionError::SelectionChanged) => Ok(false),
        Err(error) => Err(describe(error)),
    }
}

/// Builds the selection that the caller expects to still be in place.
///
/// The control is kept: the same text inside another control is a different
/// selection, and the check that refuses to write over a selection that
/// moved compares the control as well.
fn expected_selection(
    expected_text: String,
    expected_window: isize,
    expected_control: isize,
) -> crate::managers::selection::Selection {
    crate::managers::selection::Selection {
        text: expected_text,
        window: expected_window,
        control: expected_control,
        app_name: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_expected_selection_keeps_the_control_that_holds_it() {
        // El control forma parte de la identidad de la seleccion: si se
        // pierde, la comprobacion que se niega a escribir sobre una
        // seleccion que ha cambiado nunca puede coincidir.
        let expected = expected_selection("hola".to_string(), 1234, 5678);
        assert_eq!(expected.text, "hola");
        assert_eq!(expected.window, 1234);
        assert_eq!(expected.control, 5678);
    }
}
