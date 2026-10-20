//! Detecting the application that has the focus.
//!
//! A writing profile is chosen by the application the user is dictating into:
//! a formal message in a chat is not the same text as a formal message in an
//! email. To know which one, Dixi asks Windows which process owns the
//! foreground window.
//!
//! What is read is deliberately minimal: the **executable name** and the
//! window title. No document content, no clipboard, no URL. The window title
//! is optional and only used to refine a match; a profile never depends on
//! it, so a title that leaks a message never reaches a rule.
//!
//! On other platforms there is no detection and the caller falls back to the
//! general profile.

/// The application that currently has the focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForegroundApp {
    /// The executable name, lowercased: `chrome.exe`, `outlook.exe`.
    pub process_name: String,
    /// The full path of the executable, when it could be read.
    pub executable_path: Option<String>,
    /// The title of the window, when it could be read.
    pub window_title: Option<String>,
}

impl ForegroundApp {
    /// Whether the process name matches `name`, ignoring the extension and
    /// the case.
    ///
    /// `chrome` matches `chrome.exe` and `CHROME.EXE`.
    pub fn matches(&self, name: &str) -> bool {
        let wanted = name.trim().trim_end_matches(".exe").to_lowercase();
        if wanted.is_empty() {
            return false;
        }
        let current = self.process_name.trim_end_matches(".exe").to_lowercase();
        current == wanted
    }
}

/// The application that has the focus, if it can be determined.
#[cfg(target_os = "windows")]
pub fn foreground_app() -> Option<ForegroundApp> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    // SAFETY: every call below is a plain Win32 query on the foreground
    // window. The handles are closed before returning, and the buffers are
    // sized by the API itself.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if process_id == 0 {
            return None;
        }

        // The window title, read before the handle dance: it is the cheapest
        // signal and it is optional.
        let mut title_buffer = [0_u16; 512];
        let title_length = GetWindowTextW(hwnd, &mut title_buffer);
        let window_title = if title_length > 0 {
            let title = String::from_utf16_lossy(&title_buffer[..title_length as usize]);
            let title = title.trim();
            if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            }
        } else {
            None
        };

        let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
            Ok(handle) => handle,
            Err(_) => {
                return Some(ForegroundApp {
                    process_name: String::new(),
                    executable_path: None,
                    window_title,
                })
            }
        };

        let mut path_buffer = [0_u16; 1024];
        let mut path_length: u32 = path_buffer.len() as u32;
        let path = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path_buffer.as_mut_ptr()),
            &mut path_length,
        )
        .ok()
        .map(|_| {
            let path = String::from_utf16_lossy(&path_buffer[..path_length as usize]);
            let path = path.trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(path)
            }
        })
        .unwrap_or(None);

        let _ = CloseHandle(process);

        let executable_path = path.clone();
        let process_name = path
            .as_deref()
            .and_then(|path| {
                path.rsplit(['\\', '/'])
                    .find(|segment| !segment.is_empty())
                    .map(|name| name.to_lowercase())
            })
            .unwrap_or_default();

        Some(ForegroundApp {
            process_name,
            executable_path,
            window_title,
        })
    }
}

/// The application that has the focus, if it can be determined.
///
/// Without the Windows API there is no reliable way to know, so nothing is
/// reported and the caller uses the fallback profile.
#[cfg(not(target_os = "windows"))]
pub fn foreground_app() -> Option<ForegroundApp> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str) -> ForegroundApp {
        ForegroundApp {
            process_name: name.to_string(),
            executable_path: Some(format!("C:\\Program Files\\{name}")),
            window_title: None,
        }
    }

    #[test]
    fn the_process_name_matches_ignoring_the_extension_and_the_case() {
        let chrome = app("chrome.exe");
        assert!(chrome.matches("chrome"));
        assert!(chrome.matches("CHROME"));
        assert!(chrome.matches("chrome.exe"));
        assert!(chrome.matches("  chrome  "));
    }

    #[test]
    fn a_different_process_does_not_match() {
        let chrome = app("chrome.exe");
        assert!(!chrome.matches("outlook"));
        assert!(!chrome.matches("fire"));
    }

    #[test]
    fn an_empty_name_never_matches() {
        let empty = app("");
        assert!(!empty.matches("chrome"));
        assert!(!empty.matches(""));
    }
}
