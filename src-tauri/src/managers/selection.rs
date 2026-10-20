//! Reading and replacing the text selection of the focused application.
//!
//! Dixi reads the selection with the clipboard: it asks the focused window
//! for a copy with `Ctrl+C`, reads the text and puts the clipboard back the
//! way it was. A password field is refused before the copy is asked for, so
//! a password is never read.
//!
//! Replacing a selection is the risky half. The new text has to land on the
//! very same selection that was read, so the selection is read again right
//! before the write and compared with the one that was validated. When they
//! differ, nothing is written and the caller offers the result as a copy
//! instead.
//!
//! **The focus is the hard part.** The selection only exists in the
//! application that has the focus, so the capture has to happen while that
//! application is in front. A button inside Dixi cannot do it: clicking the
//! button gives Dixi the focus and the selection of the other application is
//! already lost. That is why the capture is done from a global shortcut,
//! which does not move the focus, and why the capture keeps the *context*
//! of the selection — the window, the control and the text — so that the
//! write can go back to the same place later.
//!
//! The feature is Windows only. On the other platforms both functions report
//! that there is no selection.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;

/// What was selected in the application that had the focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The selected text.
    pub text: String,
    /// The handle of the window that had the focus.
    pub window: isize,
    /// The handle of the focused control.
    pub control: isize,
    /// The name of the application, when it could be read.
    pub app_name: Option<String>,
}

/// The context of a captured selection: where it was, and what it said.
///
/// Without the context there is no way to write safely: the only thing that
/// can be done with a text is to paste it into whatever has the focus at
/// that moment, and that is never what the user meant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SelectionContext {
    /// The selected text, as it was read.
    pub text: String,
    /// The handle of the window that had the focus.
    pub window: isize,
    /// The handle of the focused control.
    pub control: isize,
    /// The name of the application, when it could be read.
    pub app_name: Option<String>,
    /// When the selection was captured, in milliseconds since the epoch.
    pub captured_at: i64,
}

impl SelectionContext {
    /// The context of a selection, stamped with the current time.
    pub fn new(selection: Selection) -> Self {
        Self {
            text: selection.text,
            window: selection.window,
            control: selection.control,
            app_name: selection.app_name,
            captured_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// The last captured selection, so that the interface can show it and the
/// write can go back to the same place.
#[derive(Default)]
pub struct SelectionContextStore {
    inner: Mutex<Option<SelectionContext>>,
}

impl SelectionContextStore {
    /// A store with nothing captured yet.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// The captured selection, if there is one.
    pub fn get(&self) -> Option<SelectionContext> {
        self.inner.lock().ok().and_then(|context| context.clone())
    }

    /// Remembers a captured selection.
    pub fn set(&self, context: SelectionContext) {
        if let Ok(mut slot) = self.inner.lock() {
            *slot = Some(context);
        }
    }

    /// Forgets the captured selection.
    pub fn clear(&self) {
        if let Ok(mut slot) = self.inner.lock() {
            *slot = None;
        }
    }
}

/// Why a selection could not be read or replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// Nothing had the focus, or no text was selected.
    NothingSelected,
    /// The focused control is a password field. It is never read.
    PasswordField,
    /// The focused control is not one of the verified text controls, so its
    /// content is not read and not written.
    UnsupportedControl,
    /// The clipboard holds something that cannot be saved and restored
    /// exactly, so the operation is refused before anything is modified.
    ClipboardNotPreservable,
    /// The clipboard could not be used.
    Clipboard(String),
    /// The window or the selection changed while the transformation was running.
    SelectionChanged,
    /// The application where the selection was read could not be brought
    /// back to the front.
    DestinationUnavailable(String),
    /// The window or the control of the selection no longer exists.
    DestinationGone,
    /// The text could not be written.
    Write(String),
}

/// Why a selection could not be read or written, in a shape the interface
/// can show.
///
/// The code is stable, so the interface can translate the reason; the
/// message is in English and is meant for the logs.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SelectionFailure {
    /// A stable identifier of the reason.
    pub code: String,
    /// The reason in English.
    pub message: String,
}

/// Turns an error of the selection manager into a failure the interface can
/// show.
pub fn failure(error: SelectionError) -> SelectionFailure {
    let (code, message) = match error {
        SelectionError::NothingSelected => {
            ("nothing_selected", "there is no selected text".to_string())
        }
        SelectionError::PasswordField => (
            "password_field",
            "the focused control is a password field".to_string(),
        ),
        SelectionError::UnsupportedControl => (
            "unsupported_control",
            "the focused control is not a text control that Dixi knows".to_string(),
        ),
        SelectionError::ClipboardNotPreservable => (
            "clipboard_not_preservable",
            "the clipboard holds content that cannot be restored exactly, so it was not touched"
                .to_string(),
        ),
        SelectionError::Clipboard(message) => {
            ("clipboard", format!("the clipboard failed: {message}"))
        }
        SelectionError::SelectionChanged => (
            "selection_changed",
            "the selection changed since it was read, so nothing was written".to_string(),
        ),
        SelectionError::DestinationUnavailable(message) => {
            ("destination_unavailable", message)
        }
        SelectionError::DestinationGone => (
            "destination_gone",
            "the application where the text was read no longer exists".to_string(),
        ),
        SelectionError::Write(message) => ("write", message),
    };
    SelectionFailure {
        code: code.to_string(),
        message,
    }
}

/// The text that is selected right now, if any.
pub fn current_selection() -> Result<Selection, SelectionError> {
    #[cfg(target_os = "windows")]
    {
        platform::current_selection()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(SelectionError::NothingSelected)
    }
}

/// Captures the selection of the application that has the focus, together
/// with the context needed to write it back later.
///
/// The application that has the focus must be the one the user selected the
/// text in: a global shortcut does not move the focus, so this is the only
/// moment where the selection is still reachable.
pub fn capture_context() -> Result<SelectionContext, SelectionError> {
    #[cfg(target_os = "windows")]
    {
        platform::capture_context()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(SelectionError::NothingSelected)
    }
}

/// Writes text over a selection that was captured before.
///
/// The write only happens when everything still matches: the same window,
/// the same control, the same selected text and a destination that is not a
/// protected field. When any of that fails, nothing is written.
pub fn write_to_context(
    context: &SelectionContext,
    expected_text: &str,
    new_text: &str,
) -> Result<(), SelectionError> {
    #[cfg(target_os = "windows")]
    {
        platform::write_to_context(context, expected_text, new_text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (context, expected_text, new_text);
        Err(SelectionError::NothingSelected)
    }
}


/// The `ES_PASSWORD` style bit (`0x0020`): the control masks what is typed.
///
/// The bit only means something on an edit control, so it is always read
/// together with the class name of the control.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const ES_PASSWORD_STYLE: isize = 0x0020;

/// Whether the style word marks the control as a password field.
///
/// The helper is `pub(crate)` so that the tests can reach it without a
/// desktop session.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn is_password_style(style: isize) -> bool {
    style & ES_PASSWORD_STYLE != 0
}

/// Whether the class name marks the control as a password field.
///
/// The check is a substring match on purpose: the password controls that do
/// not use the plain `Password` class are refused too (`PasswordBox` and the
/// WPF wrapper windows that mention it). Refusing a control that is not
/// really a password field only costs the user one transform; reading a
/// password costs much more.
///
/// The helper is `pub(crate)` so that the tests can reach it without a
/// desktop session.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn is_password_class(class: &str) -> bool {
    class.to_ascii_lowercase().contains("password")
}

/// Whether the class name is the one of a plain edit control, the control
/// that can carry the `ES_PASSWORD` style.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn is_edit_class(class: &str) -> bool {
    let class = class.trim();
    class.eq_ignore_ascii_case("edit") || class.eq_ignore_ascii_case("tedit")
}

/// The decision that protects a password field.
///
/// A control is a password field when its class name says so, or when it is
/// an edit control that carries the `ES_PASSWORD` style. The style bit is
/// not read on other controls: on a button or a list box the same bit means
/// something else, so reading it there would refuse controls that are not
/// masked.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn is_password_control(class: &str, style: isize) -> bool {
    is_password_class(class) || (is_edit_class(class) && is_password_style(style))
}

/// The control classes Dixi is willing to read from and to write to.
///
/// The list is a whitelist on purpose. Windows offers no reliable way to
/// tell that a control holds a protected field without Windows UI
/// Automation, and a control that is not recognized could be anything:
/// a password field of a third party application, a control that reacts
/// badly to a synthetic keystroke, a control that is not a text field at
/// all. Refusing an unknown control costs the user one transformation;
/// reading or overwriting the wrong control costs much more.
///
/// The browser renderers are in the list because the text of a document is
/// really there, but a masked field inside a renderer is not detectable
/// from Win32: the browsers themselves refuse to hand out the content of a
/// masked field, so the read is safe even there.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const SUPPORTED_TEXT_CLASSES: &[&str] = &[
    // Native text controls.
    "edit",
    "tedit",
    "richedit",
    "richedit20w",
    "richedit50w",
    "richeditd2dpt",
    "richeditd2dptx",
    // Browser renderers, where the document lives.
    "chrome_widgetwin_1",
    "mozillawindowclass",
];

/// Whether the class name is one of the verified text controls.
///
/// The helper is `pub(crate)` so that the tests can reach it without a
/// desktop session.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn is_supported_text_class(class: &str) -> bool {
    let class = class.trim().to_ascii_lowercase();
    SUPPORTED_TEXT_CLASSES.contains(&class.as_str())
}

/// The clipboard formats that Dixi is able to save and restore exactly.
///
/// Everything else — an image, files, HTML, RTF — would be lost when the
/// clipboard is emptied, so the operation is refused before that happens.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) const RESTORABLE_CLIPBOARD_FORMATS: &[u32] = &[1, 13, 16];

/// Whether the clipboard holds only formats that can be restored exactly.
///
/// The helper takes the list of formats so that the tests can reach it
/// without a desktop session.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn clipboard_is_restorable(formats: &[u32]) -> bool {
    formats
        .iter()
        .all(|format| RESTORABLE_CLIPBOARD_FORMATS.contains(format))
}

/// The checks that have to pass before anything is written.
///
/// The window and the control have to be the same ones that were read, and
/// the selected text has to be the text that was read. A different text
/// means the user moved the selection in the meantime, and writing over it
/// would destroy something the user did not ask to lose.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn verify_destination(
    expected_window: isize,
    expected_control: isize,
    expected_text: &str,
    observed_window: isize,
    observed_control: isize,
    observed_text: &str,
) -> Result<(), SelectionError> {
    if expected_window != observed_window || expected_control != observed_control {
        return Err(SelectionError::SelectionChanged);
    }
    if expected_text.trim() != observed_text.trim() {
        return Err(SelectionError::SelectionChanged);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
mod platform {
    use std::mem::size_of;
    use std::time::{Duration, Instant};

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, WPARAM,
    };
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::{CF_LOCALE, CF_TEXT, CF_UNICODETEXT};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW,
        GetWindowThreadProcessId, IsIconic, IsWindow, SendMessageW, SetForegroundWindow,
        ShowWindow, GUITHREADINFO, GWL_STYLE, SW_RESTORE,
    };

    use super::{
        clipboard_is_restorable, is_password_control, is_supported_text_class, verify_destination,
        Selection, SelectionContext, SelectionError,
    };

    // `SendInput` lives in the `Win32_UI_Input_KeyboardAndMouse` feature of
    // the `windows` crate, which is not enabled: enabling it would rebuild
    // that very large crate. The declaration below mirrors the Win32
    // `SendInput` exactly and links against the same `user32` import
    // library, so no cargo feature has to change.
    #[link(name = "user32")]
    extern "system" {
        fn SendInput(c_inputs: u32, p_inputs: *const Input, cb_size: i32) -> u32;
    }

    /// The Win32 `INPUT` structure: the type of the event and the event
    /// itself. The layout mirrors the Win32 one, so `size_of::<Input>()` is
    /// the `sizeof(INPUT)` that `SendInput` expects.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Input {
        r#type: u32,
        data: InputData,
    }

    /// The event union of the Win32 `INPUT` structure.
    #[repr(C)]
    #[derive(Clone, Copy)]
    union InputData {
        mouse: MouseInput,
        keyboard: KeyboardInput,
        hardware: HardwareInput,
    }

    /// The Win32 `MOUSEINPUT` structure. Only its size matters here: it is
    /// the biggest member of the union, so it fixes the size of `INPUT`.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct MouseInput {
        dx: i32,
        dy: i32,
        mouse_data: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    /// The Win32 `KEYBDINPUT` structure.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct KeyboardInput {
        virtual_key: u16,
        scan_code: u16,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    /// The Win32 `HARDWAREINPUT` structure.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct HardwareInput {
        message: u32,
        param_low: u16,
        param_high: u16,
    }

    /// The `INPUT_KEYBOARD` event type.
    const INPUT_KEYBOARD: u32 = 1;
    /// The `KEYEVENTF_KEYUP` event flag: the key goes up instead of down.
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    /// The virtual key code of the control key.
    const VK_CONTROL: u16 = 0x11;
    /// The virtual key code of the `C` key.
    const VK_C: u16 = 0x43;
    /// The virtual key code of the `V` key.
    const VK_V: u16 = 0x56;

    /// How long the copy waits for the focused application to answer the
    /// `Ctrl+C` chord.
    const COPY_TIMEOUT: Duration = Duration::from_millis(750);
    /// How long the paste waits before the clipboard is restored, so that the
    /// focused application has read the text it is going to paste.
    ///
    /// The application reads the clipboard on its own thread, so restoring
    /// too early would replace the text before it is pasted. The wait is not
    /// the guarantee: the write is checked afterwards, and if the
    /// application did not take the text the operation says so.
    const PASTE_SETTLE: Duration = Duration::from_millis(400);
    /// How long the wait for a window to come back to the front lasts.
    const FOCUS_TIMEOUT: Duration = Duration::from_millis(800);
    /// How long the copy waits between two reads of the clipboard.
    const POLL_INTERVAL: Duration = Duration::from_millis(15);
    /// How many times opening the clipboard is retried: another application
    /// may be holding it for a moment.
    const CLIPBOARD_RETRIES: u32 = 6;
    /// How long the retries wait between each other.
    const CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(25);

    /// The clipboard content captured before Dixi touched it.
    enum SavedClipboard {
        /// The clipboard was empty.
        Empty,
        /// The clipboard held text. Every format it held is saved, so it can
        /// be put back exactly as it was: the same text in the same formats,
        /// with the same locale.
        Text {
            /// The text in `CF_UNICODETEXT`.
            unicode: Option<String>,
            /// The raw bytes of `CF_TEXT`, terminator included.
            ansi: Option<Vec<u8>>,
            /// The locale of `CF_LOCALE`.
            locale: Option<u32>,
        },
    }

    /// Restores the captured clipboard content when it goes out of scope.
    ///
    /// The guard is what makes the restore happen on the error paths too:
    /// every function that touches the clipboard keeps one alive, so the
    /// clipboard is put back whether the operation succeeded or not.
    struct ClipboardGuard {
        saved: SavedClipboard,
    }

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            // SAFETY: restoring the clipboard only calls Win32 clipboard
            // functions on data this module owns. A failure here is ignored
            // on purpose: the clipboard may be locked by another application
            // and the original error must not be masked by it.
            unsafe {
                restore_clipboard(&self.saved);
            }
        }
    }

    /// The open clipboard. It is closed when it goes out of scope, so no
    /// path can leave the clipboard locked.
    struct Clipboard;

    impl Clipboard {
        /// Opens the clipboard, retrying while another application holds it.
        unsafe fn open() -> Result<Self, SelectionError> {
            let mut last_error = String::new();
            for _ in 0..CLIPBOARD_RETRIES {
                match OpenClipboard(None) {
                    Ok(()) => return Ok(Clipboard),
                    Err(error) => {
                        last_error = error.to_string();
                        std::thread::sleep(CLIPBOARD_RETRY_DELAY);
                    }
                }
            }
            Err(SelectionError::Clipboard(format!(
                "could not open the clipboard: {last_error}"
            )))
        }
    }

    impl Drop for Clipboard {
        fn drop(&mut self) {
            // SAFETY: the clipboard was opened by `Clipboard::open`, and the
            // clipboard can only be closed by the thread that opened it.
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    /// The window that has the focus and the control inside it that holds
    /// the keyboard focus.
    fn focused_control() -> Result<(HWND, HWND), SelectionError> {
        // SAFETY: plain Win32 queries on the foreground window. The handles
        // are only read, never stored and never freed.
        unsafe {
            let window = GetForegroundWindow();
            if window.0.is_null() {
                return Err(SelectionError::NothingSelected);
            }

            let thread = GetWindowThreadProcessId(window, None);
            if thread == 0 {
                return Err(SelectionError::NothingSelected);
            }

            let mut info = GUITHREADINFO {
                cbSize: size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            if GetGUIThreadInfo(thread, &mut info).is_err() {
                return Err(SelectionError::NothingSelected);
            }

            // The focus may be on a control of another thread, in which case
            // Windows reports no focused control and the active window is
            // the closest thing to one.
            let control = if info.hwndFocus.0.is_null() {
                info.hwndActive
            } else {
                info.hwndFocus
            };
            if control.0.is_null() {
                return Err(SelectionError::NothingSelected);
            }

            Ok((window, control))
        }
    }

    /// The class name of a window.
    unsafe fn class_name(window: HWND) -> String {
        let mut buffer = [0_u16; 256];
        let length = GetClassNameW(window, &mut buffer);
        if length <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..length as usize])
    }

    /// The style word of a window.
    unsafe fn window_style(window: HWND) -> isize {
        GetWindowLongPtrW(window, GWL_STYLE)
    }

    /// Refuses to touch a password field.
    ///
    /// The focused control is never read and never written: the check runs
    /// before the copy is asked for and before anything is pasted.
    unsafe fn refuse_password_field(control: HWND) -> Result<(), SelectionError> {
        let class = class_name(control);
        let style = window_style(control);
        if is_password_control(&class, style) {
            return Err(SelectionError::PasswordField);
        }
        Ok(())
    }

    /// Refuses to touch a control that is not a verified text control.
    ///
    /// Windows offers no reliable way to tell that a control holds a
    /// protected field without Windows UI Automation, so the decision is
    /// made with the class name: only the controls that are known to carry
    /// text are read, and everything else is refused.
    ///
    /// The focused control is never read and never written: the check runs
    /// before the copy is asked for and before anything is pasted.
    unsafe fn refuse_unsupported_control(control: HWND) -> Result<(), SelectionError> {
        if !is_supported_text_class(&class_name(control)) {
            return Err(SelectionError::UnsupportedControl);
        }
        Ok(())
    }

    /// The name of the executable that owns the window, when it can be read.
    fn app_name(window: HWND) -> Option<String> {
        // SAFETY: plain Win32 queries on the process that owns the window.
        // The process handle is closed before returning.
        unsafe {
            let mut process_id: u32 = 0;
            GetWindowThreadProcessId(window, Some(&mut process_id));
            if process_id == 0 {
                return None;
            }

            let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
                Ok(handle) => handle,
                Err(_) => return None,
            };

            let mut buffer = [0_u16; 1024];
            let mut length: u32 = buffer.len() as u32;
            let path = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            );
            let _ = CloseHandle(process);

            path.ok()?;
            let full_path = String::from_utf16_lossy(&buffer[..length as usize]);
            let name = full_path
                .rsplit(['\\', '/'])
                .find(|part| !part.is_empty())?;
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        }
    }

    /// Reads the text of the clipboard. The clipboard must already be open.
    unsafe fn read_clipboard_text() -> Result<Option<String>, SelectionError> {
        let Ok(handle) = GetClipboardData(CF_UNICODETEXT.0 as u32) else {
            // The format is not available: the clipboard holds no text.
            return Ok(None);
        };

        let global = HGLOBAL(handle.0);
        let pointer = GlobalLock(global) as *const u16;
        if pointer.is_null() {
            return Err(SelectionError::Clipboard(
                "could not lock the clipboard text".to_string(),
            ));
        }

        // The text is a null terminated string, but the size of the memory
        // block is what keeps the read inside the buffer.
        let units = GlobalSize(global) / 2;
        let mut text = Vec::new();
        for index in 0..units {
            // SAFETY: the pointer is the locked buffer and `index` stays
            // below the number of units the buffer holds.
            let unit = unsafe { *pointer.add(index) };
            if unit == 0 {
                break;
            }
            text.push(unit);
        }
        let _ = GlobalUnlock(global);

        Ok(Some(String::from_utf16_lossy(&text)))
    }

    /// The text of the clipboard, opening and closing the clipboard.
    unsafe fn clipboard_text() -> Option<String> {
        let _clipboard = match Clipboard::open() {
            Ok(clipboard) => clipboard,
            Err(_) => return None,
        };
        read_clipboard_text().ok().flatten()
    }

    /// Writes text to the clipboard. The clipboard must already be open and
    /// empty.
    unsafe fn set_clipboard_text(text: &str) -> Result<(), SelectionError> {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let global = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).map_err(|error| {
            SelectionError::Clipboard(format!("could not allocate the clipboard: {error}"))
        })?;

        let pointer = GlobalLock(global) as *mut u16;
        if pointer.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(SelectionError::Clipboard(
                "could not lock the clipboard memory".to_string(),
            ));
        }
        // SAFETY: the buffer holds the whole wide string and the lock is
        // held for the copy.
        unsafe {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer, wide.len());
        }
        let _ = GlobalUnlock(global);

        if SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0))).is_err() {
            let _ = GlobalFree(Some(global));
            return Err(SelectionError::Clipboard(
                "could not set the clipboard text".to_string(),
            ));
        }
        Ok(())
    }

    /// Writes raw bytes to a clipboard format. The clipboard must already be
    /// open and empty.
    unsafe fn set_clipboard_bytes(format: u32, bytes: &[u8]) -> Result<(), SelectionError> {
        let global = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|error| {
            SelectionError::Clipboard(format!("could not allocate the clipboard: {error}"))
        })?;

        let pointer = GlobalLock(global) as *mut u8;
        if pointer.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(SelectionError::Clipboard(
                "could not lock the clipboard memory".to_string(),
            ));
        }
        // SAFETY: the buffer holds the whole content and the lock is held
        // for the write.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
        }
        let _ = GlobalUnlock(global);

        if SetClipboardData(format, Some(HANDLE(global.0))).is_err() {
            let _ = GlobalFree(Some(global));
            return Err(SelectionError::Clipboard(
                "could not set the clipboard content".to_string(),
            ));
        }
        Ok(())
    }

    /// Writes the locale to the clipboard. The clipboard must already be
    /// open and empty.
    unsafe fn set_clipboard_locale(value: u32) -> Result<(), SelectionError> {
        let global = GlobalAlloc(GMEM_MOVEABLE, 4).map_err(|error| {
            SelectionError::Clipboard(format!("could not allocate the clipboard: {error}"))
        })?;

        let pointer = GlobalLock(global) as *mut u32;
        if pointer.is_null() {
            let _ = GlobalFree(Some(global));
            return Err(SelectionError::Clipboard(
                "could not lock the clipboard memory".to_string(),
            ));
        }
        // SAFETY: the pointer is the locked buffer of a `CF_LOCALE` value.
        unsafe {
            *pointer = value;
        }
        let _ = GlobalUnlock(global);

        if SetClipboardData(CF_LOCALE.0 as u32, Some(HANDLE(global.0))).is_err() {
            let _ = GlobalFree(Some(global));
            return Err(SelectionError::Clipboard(
                "could not set the clipboard locale".to_string(),
            ));
        }
        Ok(())
    }

    /// Checks that the clipboard can be used without damaging it.
    ///
    /// The clipboard is only used when it holds text, because that is the
    /// only thing Dixi is able to put back exactly. A clipboard with an
    /// image, with copied files, with HTML or with RTF is refused here,
    /// before anything is modified.
    unsafe fn ensure_clipboard_is_usable() -> Result<(), SelectionError> {
        let _clipboard = Clipboard::open()?;
        if !clipboard_is_restorable(&clipboard_formats()) {
            return Err(SelectionError::ClipboardNotPreservable);
        }
        Ok(())
    }

    /// The formats the clipboard holds right now.
    ///
    /// The clipboard has to be open.
    unsafe fn clipboard_formats() -> Vec<u32> {        let mut formats = Vec::new();
        let mut current = 0_u32;
        loop {
            // SAFETY: the clipboard is open and the call only enumerates the
            // formats it holds.
            current = EnumClipboardFormats(current);
            if current == 0 {
                break;
            }
            formats.push(current);
        }
        formats
    }

    /// The raw bytes of a clipboard format, when it is available.
    ///
    /// The clipboard has to be open.
    unsafe fn read_clipboard_bytes(format: u32) -> Option<Vec<u8>> {
        let Ok(handle) = GetClipboardData(format) else {
            return None;
        };

        let global = HGLOBAL(handle.0);
        let pointer = GlobalLock(global) as *const u8;
        if pointer.is_null() {
            return None;
        }
        let size = GlobalSize(global);
        // SAFETY: the pointer is the locked buffer and the size is the size
        // of that buffer, so the slice stays inside it.
        let bytes = unsafe { std::slice::from_raw_parts(pointer, size) }.to_vec();
        let _ = GlobalUnlock(global);
        Some(bytes)
    }

    /// The locale of the clipboard, when it has one.
    ///
    /// The clipboard has to be open.
    unsafe fn read_clipboard_locale() -> Option<u32> {
        let Ok(handle) = GetClipboardData(CF_LOCALE.0 as u32) else {
            return None;
        };

        let global = HGLOBAL(handle.0);
        let pointer = GlobalLock(global) as *const u32;
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the pointer is the locked buffer of a `CF_LOCALE` value,
        // which is a single locale identifier.
        let value = unsafe { *pointer };
        let _ = GlobalUnlock(global);
        Some(value)
    }

    /// Reads the current clipboard content, if it is text.
    ///
    /// A clipboard that holds something else — an image, copied files, HTML,
    /// RTF — is refused here, **before** anything is modified: emptying the
    /// clipboard would destroy that content, and a faithful restore of it
    /// would need the OLE clipboard object, which is out of the reach of
    /// this module. Refusing is the only way to promise that the clipboard
    /// of the user is never damaged.
    unsafe fn snapshot_clipboard() -> Result<SavedClipboard, SelectionError> {
        let _clipboard = Clipboard::open()?;

        let formats = clipboard_formats();
        if !clipboard_is_restorable(&formats) {
            return Err(SelectionError::ClipboardNotPreservable);
        }

        let unicode = read_clipboard_text()?;
        let ansi = read_clipboard_bytes(CF_TEXT.0 as u32);
        let locale = read_clipboard_locale();

        if unicode.is_none() && ansi.is_none() && locale.is_none() {
            return Ok(SavedClipboard::Empty);
        }
        Ok(SavedClipboard::Text {
            unicode,
            ansi,
            locale,
        })
    }

    /// Puts the captured clipboard content back.
    unsafe fn restore_clipboard(saved: &SavedClipboard) {
        let Ok(_clipboard) = Clipboard::open() else {
            return;
        };
        let _ = EmptyClipboard();
        match saved {
            SavedClipboard::Empty => {}
            SavedClipboard::Text {
                unicode,
                ansi,
                locale,
            } => {
                if let Some(text) = unicode {
                    let _ = set_clipboard_text(text);
                }
                if let Some(bytes) = ansi {
                    let _ = set_clipboard_bytes(CF_TEXT.0 as u32, &bytes);
                }
                if let Some(value) = locale {
                    let _ = set_clipboard_locale(*value);
                }
            }
        }
    }

    /// Builds one keyboard `INPUT` event.
    ///
    /// The structure is zeroed first, so no padding byte carries garbage
    /// into the call.
    fn keyboard_input(virtual_key: u16, release: bool) -> Input {
        let mut input: Input = unsafe { std::mem::zeroed() };
        input.r#type = INPUT_KEYBOARD;
        // Writing a union field is safe: the event becomes the keyboard
        // event that `r#type` announces.
        input.data.keyboard = KeyboardInput {
            virtual_key,
            scan_code: 0,
            flags: if release { KEYEVENTF_KEYUP } else { 0 },
            time: 0,
            extra_info: 0,
        };
        input
    }

    /// Sends the `Ctrl+<key>` chord to the focused application.
    fn send_chord(key: u16) -> Result<(), SelectionError> {
        let inputs = [
            keyboard_input(VK_CONTROL, false),
            keyboard_input(key, false),
            keyboard_input(key, true),
            keyboard_input(VK_CONTROL, true),
        ];

        // SAFETY: the array is a fully initialized buffer of `INPUT`
        // structures and `cb_size` is the size of one of them, which is what
        // `SendInput` expects.
        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                size_of::<Input>() as i32,
            )
        };
        if sent != inputs.len() as u32 {
            return Err(SelectionError::Write(
                "the focused application did not accept the keyboard input".to_string(),
            ));
        }
        Ok(())
    }

    /// Copies the current selection with `Ctrl+C` and returns the copied
    /// text. The clipboard is left exactly as it was found.
    fn copy_selection() -> Result<String, SelectionError> {
        // SAFETY: the clipboard is opened and closed inside the snapshot.
        let saved = unsafe { snapshot_clipboard() }?;
        let _guard = ClipboardGuard { saved };

        // The clipboard is emptied before the copy is asked for, so a copy
        // that copies nothing is not confused with the text that was already
        // there.
        {
            // SAFETY: the clipboard is opened here and closed when the block
            // ends.
            let _clipboard = unsafe { Clipboard::open() }?;
            unsafe { EmptyClipboard() }.map_err(|error| {
                SelectionError::Clipboard(format!("could not clear the clipboard: {error}"))
            })?;
        }

        send_chord(VK_C).map_err(|error| {
            SelectionError::Clipboard(format!("could not copy the selection: {error:?}"))
        })?;

        // The focused application answers the chord on its own thread, so
        // the clipboard is polled until the text shows up.
        let deadline = Instant::now() + COPY_TIMEOUT;
        loop {
            // SAFETY: the clipboard is opened and closed inside the read.
            if let Some(text) = unsafe { clipboard_text() } {
                if !text.is_empty() {
                    return Ok(text);
                }
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(POLL_INTERVAL);
        }

        Err(SelectionError::NothingSelected)
    }

    /// Pastes text over the current selection with `Ctrl+V`. The clipboard
    /// is left exactly as it was found.
    fn paste_text(text: &str) -> Result<(), SelectionError> {
        // SAFETY: the clipboard is opened and closed inside the snapshot.
        let saved = unsafe { snapshot_clipboard() }?;
        let _guard = ClipboardGuard { saved };

        {
            // SAFETY: the clipboard is opened here and closed when the block
            // ends.
            let _clipboard = unsafe { Clipboard::open() }?;
            unsafe { EmptyClipboard() }.map_err(|error| {
                SelectionError::Clipboard(format!("could not clear the clipboard: {error}"))
            })?;
            // SAFETY: the clipboard is open and empty.
            unsafe { set_clipboard_text(text) }?;
        }

        send_chord(VK_V).map_err(|error| {
            SelectionError::Write(format!("could not paste the text: {error:?}"))
        })?;

        // The focused application reads the clipboard when it handles the
        // paste, so the text is left in place for a moment before the
        // clipboard is restored.
        std::thread::sleep(PASTE_SETTLE);
        Ok(())
    }

    /// The text that is selected right now, if any.
    pub fn current_selection() -> Result<Selection, SelectionError> {
        let (window, control) = focused_control()?;

        // SAFETY: the control is the one that holds the keyboard focus and
        // is only queried for its class name and its style.
        unsafe {
            refuse_password_field(control)?;
            refuse_unsupported_control(control)?;
        }

        let text = copy_selection()?;

        Ok(Selection {
            text,
            window: window.0 as isize,
            control: control.0 as isize,
            app_name: app_name(window),
        })
    }

    /// Captures the selection of the application that has the focus, together
    /// with the context needed to write it back later.
    pub fn capture_context() -> Result<SelectionContext, SelectionError> {
        Ok(SelectionContext::new(current_selection()?))
    }

    /// Brings a window back to the front and waits until it has the focus.
    ///
    /// The write is made with synthetic keystrokes, and those go to
    /// whatever has the focus, so the window where the text was read has to
    /// be the one in front before anything is written.
    fn focus_window(window: HWND) -> Result<(), SelectionError> {
        // SAFETY: the window is only queried, shown and brought to the
        // front. No handle is stored and none is freed.
        unsafe {
            if !IsWindow(Some(window)).as_bool() {
                return Err(SelectionError::DestinationGone);
            }
            if IsIconic(window).as_bool() {
                let _ = ShowWindow(window, SW_RESTORE);
            }
            let _ = SetForegroundWindow(window);
        }

        // The foreground change is not immediate, so it is waited for.
        let deadline = Instant::now() + FOCUS_TIMEOUT;
        while Instant::now() < deadline {
            // SAFETY: the call only reads the foreground window.
            if unsafe { GetForegroundWindow() } == window {
                return Ok(());
            }
            std::thread::sleep(POLL_INTERVAL);
        }

        Err(SelectionError::DestinationUnavailable(
            "the application could not be brought back to the front".to_string(),
        ))
    }

    /// Writes text over a selection that was captured before.
    ///
    /// Nothing is written unless everything still matches: the same window,
    /// the same control, the same selected text and a destination that is
    /// not a protected field.
    pub fn write_to_context(
        context: &SelectionContext,
        expected_text: &str,
        new_text: &str,
    ) -> Result<(), SelectionError> {
        // The clipboard is checked before anything else: if it holds
        // something that cannot be restored exactly, the operation is
        // refused here, without bringing the application of the user to
        // the front for nothing.
        unsafe { ensure_clipboard_is_usable()? };

        let window = HWND(context.window as *mut std::ffi::c_void);
        focus_window(window)?;

        let (current_window, control) = focused_control()?;

        // SAFETY: the control is the one that holds the keyboard focus and
        // is only queried for its class name and its style.
        unsafe {
            refuse_password_field(control)?;
            refuse_unsupported_control(control)?;
        }

        // The selection is read again and compared with the one that was
        // captured: the user may have typed or moved the selection in the
        // meantime. A selection that can no longer be read is a changed
        // selection, not a clipboard failure.
        let current = copy_selection().map_err(|error| match error {
            SelectionError::NothingSelected => SelectionError::SelectionChanged,
            other => other,
        })?;

        verify_destination(
            context.window,
            context.control,
            expected_text,
            current_window.0 as isize,
            control.0 as isize,
            &current,
        )?;

        paste_text(new_text)?;

        // The write is checked afterwards: the destination has to hold the
        // new text, or the selection has to be gone, which is what a paste
        // leaves behind.
        if !write_took_effect(control, expected_text, new_text) {
            return Err(SelectionError::Write(
                "the application did not take the new text".to_string(),
            ));
        }

        Ok(())
    }

    /// The whole text of a control, when the control answers with it.
    ///
    /// The native text controls answer with `WM_GETTEXT`; a browser renderer
    /// does not, and `None` says so.
    unsafe fn read_control_text(control: HWND) -> Option<String> {
        /// The `WM_GETTEXT` message.
        const WM_GETTEXT: u32 = 0x000D;
        /// The `WM_GETTEXTLENGTH` message.
        const WM_GETTEXTLENGTH: u32 = 0x000E;

        // SAFETY: the messages are sent to a control that exists, and the
        // buffer is sized with the length the control reports.
        unsafe {
            let length = SendMessageW(control, WM_GETTEXTLENGTH, None, None).0 as usize;
            if length == 0 {
                return Some(String::new());
            }

            let mut buffer = vec![0_u16; length + 1];
            let copied = SendMessageW(
                control,
                WM_GETTEXT,
                Some(WPARAM(buffer.len())),
                Some(LPARAM(buffer.as_mut_ptr() as isize)),
            )
            .0 as usize;
            if copied == 0 {
                return None;
            }
            buffer.truncate(copied);
            Some(String::from_utf16_lossy(&buffer))
        }
    }

    /// Whether the destination took the text that was written.
    fn write_took_effect(control: HWND, expected_text: &str, new_text: &str) -> bool {
        // SAFETY: the control is the one that holds the focus; it is only
        // asked for its text.
        if let Some(text) = unsafe { read_control_text(control) } {
            return text.contains(new_text.trim());
        }

        // The control does not answer with its text, so the only thing that
        // can be checked is that the selection is gone: a paste replaces the
        // selection and leaves the caret behind it.
        match copy_selection() {
            Ok(text) => text.trim() != expected_text.trim(),
            Err(SelectionError::NothingSelected) => true,
            Err(_) => false,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_input_structure_has_the_win32_size() {
            // `SendInput` is told how big one `INPUT` is, so the structure
            // has to be exactly the Win32 one: 40 bytes on 64 bits and 28
            // bytes on 32 bits.
            let expected = if cfg!(target_pointer_width = "64") {
                40
            } else {
                28
            };
            assert_eq!(size_of::<Input>(), expected);
            assert_eq!(
                size_of::<KeyboardInput>(),
                if cfg!(target_pointer_width = "64") { 24 } else { 16 }
            );
        }
    }
}


#[cfg(test)]
mod tests;
