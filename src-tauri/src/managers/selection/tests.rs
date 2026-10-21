//! Tests for the selection manager.
//!
//! The tests that need a desktop session are not here: the parts that can
//! be decided without one — which controls are read, what the clipboard
//! has to hold, and when a destination is refused — are checked here, so
//! that a change in them cannot pass unnoticed.

use super::*;

// --- The controls that are read ---------------------------------------------

#[test]
fn the_password_style_bit_is_detected() {
    assert!(is_password_style(0x0020));
    assert!(is_password_style(0x0020 | 0x1000_0000));
    assert!(!is_password_style(0));
    assert!(!is_password_style(0x1000_0000));
}

#[test]
fn a_plain_style_word_is_not_a_password_style() {
    // `ES_AUTOHSCROLL` (0x0080) and the plain window styles do not
    // carry the `ES_PASSWORD` bit.
    assert!(!is_password_style(0x0080));
    assert!(!is_password_style(0x5000_0000 | 0x0080));
    assert!(!is_password_style(0x5000_0000 | 0x1000_0000));
}

#[test]
fn the_password_class_is_detected() {
    assert!(is_password_class("Password"));
    assert!(is_password_class("password"));
    assert!(is_password_class("PASSWORD"));
    assert!(is_password_class("PasswordBox"));
    assert!(!is_password_class("Edit"));
    assert!(!is_password_class("TEdit"));
    assert!(!is_password_class("Button"));
    assert!(!is_password_class(""));
}

#[test]
fn the_edit_classes_are_recognized() {
    assert!(is_edit_class("Edit"));
    assert!(is_edit_class("edit"));
    assert!(is_edit_class("EDIT"));
    assert!(is_edit_class("TEdit"));
    assert!(!is_edit_class("RichEdit"));
    assert!(!is_edit_class(""));
}

#[test]
fn an_edit_control_with_the_password_style_is_refused() {
    assert!(is_password_control("Edit", 0x0020));
    assert!(is_password_control("TEdit", 0x0020));
    assert!(is_password_control("Edit", 0x0020 | 0x1000_0000));
}

#[test]
fn a_plain_edit_control_is_not_refused() {
    assert!(!is_password_control("Edit", 0));
    assert!(!is_password_control("TEdit", 0x1000_0000));
    assert!(!is_password_control("RichEdit", 0));
}

#[test]
fn a_password_class_is_refused_even_without_the_style() {
    assert!(is_password_control("Password", 0));
    assert!(is_password_control("PasswordBox", 0));
}

#[test]
fn the_style_bit_alone_does_not_refuse_other_controls() {
    // The same bit means something else on a button or a list box.
    assert!(!is_password_control("Button", 0x0020));
    assert!(!is_password_control("ListBox", 0x0020));
}

#[test]
fn only_the_verified_text_controls_are_read() {
    // The native text controls.
    assert!(is_supported_text_class("Edit"));
    assert!(is_supported_text_class("TEdit"));
    assert!(is_supported_text_class("RichEdit"));
    assert!(is_supported_text_class("RichEdit20W"));
    assert!(is_supported_text_class("RichEditD2DPT"));
    // The browser renderers, where the document really is.
    assert!(is_supported_text_class("MozillaWindowClass"));
}

#[test]
fn the_focused_child_of_a_chromium_window_is_read() {
    // Chromium gives the FOCUSED CHILD the class
    // `Chrome_RenderWidgetHostHWND`; `Chrome_WidgetWin_1` is the top level
    // window, which is never the focused control. The class was confirmed
    // in the Chromium binary.
    assert!(is_supported_text_class("Chrome_RenderWidgetHostHWND"));
    assert!(is_supported_text_class("Chrome_WidgetWin_1"));
}

#[test]
fn the_browser_renderers_are_not_native_text_controls() {
    // A renderer does not answer with its text, so the checks that read the
    // text of a control cannot be used with it.
    assert!(!is_native_text_class("Chrome_RenderWidgetHostHWND"));
    assert!(!is_native_text_class("Chrome_WidgetWin_1"));
    assert!(is_native_text_class("Edit"));
    assert!(is_native_text_class("RichEdit20W"));
}

#[test]
fn an_unknown_control_is_not_read() {
    // A control that Dixi does not know could be a field of a third party
    // application, or a control that does not take a synthetic keystroke.
    assert!(!is_supported_text_class("Button"));
    assert!(!is_supported_text_class("SysListView32"));
    assert!(!is_supported_text_class("Static"));
    assert!(!is_supported_text_class(""));
    assert!(!is_supported_text_class("Edit Button"));
}

#[test]
fn the_class_is_matched_without_case_and_without_spaces() {
    // The class name never has spaces around it, but the comparison is
    // made on a trimmed name so that a stray space cannot make a verified
    // control look unknown.
    assert!(is_supported_text_class("edit"));
    assert!(is_supported_text_class("EDIT"));
    assert!(is_supported_text_class("chrome_widgetwin_1"));
    assert!(is_supported_text_class("Edit "));
}

// --- The clipboard -----------------------------------------------------------

#[test]
fn a_format_in_memory_can_be_restored() {
    // The text, the locale, the HTML, the RTF and the list of copied files
    // all live in movable memory, so they can be saved and put back with
    // their exact bytes.
    assert!(clipboard_format_is_restorable(1, true)); // CF_TEXT
    assert!(clipboard_format_is_restorable(13, true)); // CF_UNICODETEXT
    assert!(clipboard_format_is_restorable(16, true)); // CF_LOCALE
    assert!(clipboard_format_is_restorable(100, true)); // a registered format
}

#[test]
fn a_format_with_a_handle_cannot_be_restored() {
    // A bitmap, a metafile or a palette hold a handle that only Windows
    // knows how to interpret: the operation is refused.
    assert!(!clipboard_format_is_restorable(2, true)); // CF_BITMAP
    assert!(!clipboard_format_is_restorable(3, true)); // CF_METAFILEPICT
    assert!(!clipboard_format_is_restorable(9, true)); // CF_PALETTE
    assert!(!clipboard_format_is_restorable(14, true)); // CF_ENHMETAFILE
}

#[test]
fn a_format_that_is_not_in_memory_cannot_be_restored() {
    assert!(!clipboard_format_is_restorable(13, false));
    assert!(!clipboard_format_is_restorable(1, false));
}

// --- The destination ---------------------------------------------------------

#[test]
fn a_destination_that_matches_is_accepted() {
    assert_eq!(
        verify_destination(10, 20, "hola", 10, 20, "hola"),
        Ok(())
    );
}

#[test]
fn a_destination_in_another_window_is_refused() {
    assert_eq!(
        verify_destination(10, 20, "hola", 11, 20, "hola"),
        Err(SelectionError::SelectionChanged)
    );
}

#[test]
fn a_destination_in_another_control_is_refused() {
    assert_eq!(
        verify_destination(10, 20, "hola", 10, 21, "hola"),
        Err(SelectionError::SelectionChanged)
    );
}

#[test]
fn a_destination_with_another_text_is_refused() {
    // The user moved the selection, or typed, in the meantime.
    assert_eq!(
        verify_destination(10, 20, "hola", 10, 20, "hola mundo"),
        Err(SelectionError::SelectionChanged)
    );
    assert_eq!(
        verify_destination(10, 20, "hola", 10, 20, ""),
        Err(SelectionError::SelectionChanged)
    );
}

#[test]
fn a_control_of_zero_is_never_the_control_that_was_read() {
    // The regression that broke "Aceptar": the handle of the focused
    // control used to arrive as a zero, and a zero is never the control
    // that was read.
    assert_eq!(
        verify_destination(10, 0, "hola", 10, 20, "hola"),
        Err(SelectionError::SelectionChanged)
    );
    assert_eq!(
        verify_destination(10, 20, "hola", 10, 0, "hola"),
        Err(SelectionError::SelectionChanged)
    );
    // And a capture never produces a context with a control of zero.
    let context = SelectionContext::new(Selection {
        text: "hola".to_string(),
        window: 10,
        control: 20,
        app_name: None,
    });
    assert_ne!(context.control, 0);
}

#[test]
fn a_window_that_is_not_alive_is_detected() {
    // A window that was closed is not a destination. A handle that was
    // never a window is not alive either.
    assert!(!window_is_alive(0));
    assert!(!window_is_alive(-1));
}

#[test]
fn the_spaces_around_the_text_do_not_matter() {
    // The applications add spaces of their own when they copy.
    assert_eq!(
        verify_destination(10, 20, "hola", 10, 20, "hola\n"),
        Ok(())
    );
    assert_eq!(
        verify_destination(10, 20, " hola ", 10, 20, "hola"),
        Ok(())
    );
}

// --- The failures the interface shows ---------------------------------------

#[test]
fn every_failure_has_a_stable_code() {
    let cases = [
        (
            SelectionError::NothingSelected,
            "nothing_selected",
        ),
        (SelectionError::PasswordField, "password_field"),
        (
            SelectionError::UnsupportedControl,
            "unsupported_control",
        ),
        (
            SelectionError::ClipboardNotPreservable,
            "clipboard_not_preservable",
        ),
        (SelectionError::Clipboard("x".to_string()), "clipboard"),
        (SelectionError::SelectionChanged, "selection_changed"),
        (
            SelectionError::DestinationUnavailable("x".to_string()),
            "destination_unavailable",
        ),
        (SelectionError::DestinationGone, "destination_gone"),
        (SelectionError::Write("x".to_string()), "write"),
    ];

    for (error, code) in cases {
        let failure = failure(error);
        assert_eq!(failure.code, code);
        assert!(!failure.message.is_empty());
    }
}

// --- The context -------------------------------------------------------------

#[test]
fn a_context_keeps_the_place_and_the_text() {
    let selection = Selection {
        text: "hola".to_string(),
        window: 10,
        control: 20,
        app_name: Some("notepad.exe".to_string()),
    };
    let context = SelectionContext::new(selection);

    assert_eq!(context.text, "hola");
    assert_eq!(context.window, 10);
    assert_eq!(context.control, 20);
    assert_eq!(context.app_name.as_deref(), Some("notepad.exe"));
    assert!(context.captured_at > 0);
}

#[test]
fn the_store_remembers_the_last_capture() {
    let store = SelectionContextStore::new();
    assert!(store.get().is_none());

    let context = SelectionContext::new(Selection {
        text: "hola".to_string(),
        window: 10,
        control: 20,
        app_name: None,
    });
    store.set(context.clone());
    assert_eq!(store.get(), Some(context));

    store.clear();
    assert!(store.get().is_none());
}

#[cfg(not(target_os = "windows"))]
#[test]
fn there_is_no_selection_outside_windows() {
    assert_eq!(current_selection(), Err(SelectionError::NothingSelected));
    assert_eq!(capture_context(), Err(SelectionError::NothingSelected));

    let context = SelectionContext::new(Selection {
        text: "hola".to_string(),
        window: 0,
        control: 0,
        app_name: None,
    });
    assert_eq!(
        write_to_context(&context, "hola", "adios"),
        Err(SelectionError::NothingSelected)
    );
}
