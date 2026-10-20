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
//! The feature is Windows only. On the other platforms both functions report
//! that there is no selection.

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

/// Why a selection could not be read or replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// Nothing had the focus, or no text was selected.
    NothingSelected,
    /// The focused control is a password field. It is never read.
    PasswordField,
    /// The clipboard could not be used.
    Clipboard(String),
    /// The window or the selection changed while the transformation was running.
    SelectionChanged,
    /// The text could not be written.
    Write(String),
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

/// Replaces the selection, but only if it is still the one that was validated.
///
/// `expected` is the selection that was read before the transformation ran.
/// When the selection changed, nothing is written and `SelectionChanged` is
/// returned: the caller then offers to copy the result instead.
pub fn replace_selection(expected: &Selection, new_text: &str) -> Result<(), SelectionError> {
    #[cfg(target_os = "windows")]
    {
        platform::replace_selection(expected, new_text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (expected, new_text);
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

#[cfg(target_os = "windows")]
mod platform {
    use std::mem::size_of;
    use std::time::{Duration, Instant};

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW,
        GetWindowThreadProcessId, GUITHREADINFO, GWL_STYLE,
    };

    use super::{is_password_control, Selection, SelectionError};

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
    const PASTE_SETTLE: Duration = Duration::from_millis(250);
    /// How long the copy waits between two reads of the clipboard.
    const POLL_INTERVAL: Duration = Duration::from_millis(15);
    /// How many times opening the clipboard is retried: another application
    /// may be holding it for a moment.
    const CLIPBOARD_RETRIES: u32 = 6;
    /// How long the retries wait between each other.
    const CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(25);

    /// The clipboard content captured before Dixi touched it.
    enum SavedClipboard {
        /// The clipboard held text: it is restored exactly as it was.
        Text(String),
        /// The clipboard held something else, or nothing at all. The text
        /// format is restored as empty.
        NotText,
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

    /// Reads the current clipboard content, if it is text.
    unsafe fn snapshot_clipboard() -> Result<SavedClipboard, SelectionError> {
        let _clipboard = Clipboard::open()?;
        match read_clipboard_text() {
            Ok(Some(text)) => Ok(SavedClipboard::Text(text)),
            Ok(None) => Ok(SavedClipboard::NotText),
            Err(error) => Err(error),
        }
    }

    /// Puts the captured clipboard content back.
    unsafe fn restore_clipboard(saved: &SavedClipboard) {
        let Ok(_clipboard) = Clipboard::open() else {
            return;
        };
        let _ = EmptyClipboard();
        if let SavedClipboard::Text(text) = saved {
            let _ = set_clipboard_text(text);
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
        unsafe { refuse_password_field(control)? };

        let text = copy_selection()?;

        Ok(Selection {
            text,
            window: window.0 as isize,
            control: control.0 as isize,
            app_name: app_name(window),
        })
    }

    /// Replaces the selection, but only if it is still the one that was
    /// validated.
    pub fn replace_selection(expected: &Selection, new_text: &str) -> Result<(), SelectionError> {
        let (window, control) = focused_control()?;

        // The same window has to have the focus, and the same control has to
        // hold the selection inside it.
        if window.0 as isize != expected.window || control.0 as isize != expected.control {
            return Err(SelectionError::SelectionChanged);
        }

        // SAFETY: the control is the one that holds the keyboard focus and
        // is only queried for its class name and its style.
        unsafe { refuse_password_field(control)? };

        // The selection is read again and compared with the one that was
        // validated: the user may have typed or moved the selection while
        // the transformation was running. A selection that can no longer be
        // read is a changed selection, not a clipboard failure.
        let current = copy_selection().map_err(|error| match error {
            SelectionError::NothingSelected => SelectionError::SelectionChanged,
            other => other,
        })?;
        if current != expected.text {
            return Err(SelectionError::SelectionChanged);
        }

        paste_text(new_text)
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
mod tests {
    use super::*;

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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn there_is_no_selection_outside_windows() {
        assert_eq!(current_selection(), Err(SelectionError::NothingSelected));

        let selection = Selection {
            text: "hola".to_string(),
            window: 0,
            control: 0,
            app_name: None,
        };
        assert_eq!(
            replace_selection(&selection, "adios"),
            Err(SelectionError::NothingSelected)
        );
    }
}
