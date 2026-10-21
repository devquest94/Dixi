//! Detecting the remote desktop host.
//!
//! The facts the integration needs are narrow, and all of them come from the
//! window system and the installer registry:
//!
//! * Is the remote client installed, and which version?
//! * Is it running?
//! * Does it have an RDP session open, and how many?
//! * Does that session hold the focus?
//!
//! What is deliberately *not* read: the saved connections, the server
//! addresses, the user names and the credentials. The uninstall key carries
//! only the display name, the version and the folder, and that is the
//! entirety of what is read from the registry.
//!
//! The RDP surface is recognized by the module that registered the window
//! class, not by a hard-coded class name: the Microsoft Terminal Services
//! ActiveX (`mstscax.dll`) is what mRemoteNG embeds to speak RDP, so a
//! window whose class belongs to that module is an RDP client surface and
//! nothing else is.

/// The remote client this integration targets.
#[cfg(target_os = "windows")]
pub const REMOTE_CLIENT_IMAGE: &str = "mremoteng.exe";

/// The Microsoft Terminal Services ActiveX: the RDP client mRemoteNG
/// embeds.
#[cfg(target_os = "windows")]
pub const RDP_CLIENT_MODULE: &str = "mstscax.dll";

/// What holds the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedSurface {
    /// Nothing has the focus.
    Nothing,
    /// The focused window belongs to the remote client but is not an RDP
    /// session: its main window, a dialog, the connection list...
    ClientWindow,
    /// The focused window is an RDP session of the remote client.
    RdpSession,
    /// The focused window belongs to another application.
    #[default]
    OtherApplication,
}

impl FocusedSurface {
    /// Whether the focus is inside an RDP session.
    pub fn is_rdp_session(self) -> bool {
        matches!(self, FocusedSurface::RdpSession)
    }
}

/// Everything the integration needs to know about the environment.
#[derive(Debug, Clone, Default)]
pub struct Detection {
    /// The remote client is running.
    pub client_running: bool,
    /// How many RDP sessions the remote client has open.
    pub rdp_session_count: u32,
    /// What holds the focus.
    pub focused: FocusedSurface,
    /// The image name of the process that owns the focused window, e.g.
    /// "explorer.exe". No path, no window title, no session data.
    pub foreground_process: Option<String>,
}

/// The installation information of the remote client.
#[derive(Debug, Clone, Default)]
pub struct ClientInstall {
    /// The version, as the installer reports it.
    pub version: Option<String>,
    /// The folder it is installed in.
    pub location: Option<String>,
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform {
    use super::{ClientInstall, Detection, FocusedSurface, RDP_CLIENT_MODULE, REMOTE_CLIENT_IMAGE};
    use std::collections::HashMap;
    use windows::core::{BOOL, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND, LPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassLongPtrW, GetForegroundWindow, GetParent,
        GetWindowThreadProcessId, GCLP_HMODULE,
    };
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    /// The process that owns the window.
    fn window_process_id(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        pid
    }

    /// The image name of a process, e.g. "mremoteng.exe".
    ///
    /// Only the file name is kept: the full path would carry information
    /// about the machine layout that the integration has no use for.
    fn process_image_name(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = [0u16; 1024];
            let mut size = buffer.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(handle);
            result.ok()?;
            let length = buffer
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(buffer.len());
            let path = String::from_utf16_lossy(&buffer[..length]);
            let name = path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(path.as_str())
                .to_string();
            Some(name)
        }
    }

    /// The module that registered the window class.
    fn class_module_name(hwnd: HWND) -> Option<String> {
        unsafe {
            let module = GetClassLongPtrW(hwnd, GCLP_HMODULE);
            if module == 0 {
                return None;
            }
            let mut buffer = [0u16; 260];
            let length = GetModuleFileNameW(
                Some(HMODULE(module as *mut core::ffi::c_void)),
                &mut buffer,
            );
            if length == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buffer[..length as usize]);
            let name = path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(path.as_str())
                .to_string();
            Some(name)
        }
    }

    /// Collects a window into the list the caller left in the `lparam`.
    ///
    /// The pointer is the one the caller created, and it outlives the
    /// enumeration.
    unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let list = &mut *(lparam.0 as *mut Vec<HWND>);
        list.push(hwnd);
        BOOL(1)
    }

    /// Every top-level window.
    fn top_level_windows() -> Vec<HWND> {
        let mut found: Vec<HWND> = Vec::new();
        let counter = LPARAM(&mut found as *mut Vec<HWND> as isize);
        unsafe {
            let _ = EnumWindows(Some(collect_window), counter);
        }
        found
    }

    /// Every descendant of a window.
    fn child_windows(parent: HWND) -> Vec<HWND> {
        let mut found: Vec<HWND> = Vec::new();
        let counter = LPARAM(&mut found as *mut Vec<HWND> as isize);
        unsafe {
            let _ = EnumChildWindows(Some(parent), Some(collect_window), counter);
        }
        found
    }

    /// Whether the window is the surface or lives inside it.
    fn is_inside(window: HWND, surface: HWND) -> bool {
        if window == surface {
            return true;
        }
        let mut current = window;
        // The chain is short; the guard keeps a broken parent chain from
        // looping forever.
        for _ in 0..32 {
            let parent = match unsafe { GetParent(current) } {
                Ok(parent) => parent,
                Err(_) => return false,
            };
            if parent.0.is_null() {
                return false;
            }
            if parent == surface {
                return true;
            }
            current = parent;
        }
        false
    }

    /// The RDP sessions open inside the remote client.
    fn rdp_surfaces(client_pid: u32) -> Vec<HWND> {
        let mut surfaces = Vec::new();
        for top in top_level_windows() {
            if window_process_id(top) != client_pid {
                continue;
            }
            let candidates = std::iter::once(top).chain(child_windows(top));
            for window in candidates {
                let is_rdp = class_module_name(window)
                    .map(|name| name.eq_ignore_ascii_case(RDP_CLIENT_MODULE))
                    .unwrap_or(false);
                if is_rdp {
                    surfaces.push(window);
                }
            }
        }
        surfaces
    }

    /// Reads the state of the environment.
    pub fn detect() -> Detection {
        let windows = top_level_windows();

        // One lookup per process, not one per window.
        let mut names: HashMap<u32, Option<String>> = HashMap::new();
        for window in &windows {
            let pid = window_process_id(*window);
            names
                .entry(pid)
                .or_insert_with(|| process_image_name(pid));
        }

        let client_pids: Vec<u32> = names
            .iter()
            .filter(|(_, name)| {
                name.as_deref()
                    .map(|name| name.eq_ignore_ascii_case(REMOTE_CLIENT_IMAGE))
                    .unwrap_or(false)
            })
            .map(|(pid, _)| *pid)
            .collect();

        let mut surfaces = Vec::new();
        for pid in &client_pids {
            surfaces.extend(rdp_surfaces(*pid));
        }

        let foreground = unsafe { GetForegroundWindow() };
        let foreground_pid = if foreground.0.is_null() {
            0
        } else {
            window_process_id(foreground)
        };
        let foreground_process = if foreground_pid == 0 {
            None
        } else {
            names
                .get(&foreground_pid)
                .cloned()
                .flatten()
                .or_else(|| process_image_name(foreground_pid))
        };
        let is_client = foreground_process
            .as_deref()
            .map(|name| name.eq_ignore_ascii_case(REMOTE_CLIENT_IMAGE))
            .unwrap_or(false);

        let focused = if foreground.0.is_null() {
            FocusedSurface::Nothing
        } else if is_client {
            if surfaces.iter().any(|surface| is_inside(foreground, *surface)) {
                FocusedSurface::RdpSession
            } else {
                FocusedSurface::ClientWindow
            }
        } else {
            FocusedSurface::OtherApplication
        };

        Detection {
            client_running: !client_pids.is_empty(),
            rdp_session_count: surfaces.len() as u32,
            focused,
            foreground_process,
        }
    }

    /// Reads the installation information of the remote client.
    ///
    /// Only the uninstall keys are read, and only three values of each entry:
    /// the display name, the version and the folder. The saved connections
    /// live in a different file and are never opened.
    pub fn client_install() -> Option<ClientInstall> {
        let uninstall = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
        let roots = [
            (HKEY_LOCAL_MACHINE, uninstall.to_string()),
            (
                HKEY_LOCAL_MACHINE,
                "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall".to_string(),
            ),
            (HKEY_CURRENT_USER, uninstall.to_string()),
        ];

        for (root, path) in roots {
            let Ok(key) = RegKey::predef(root).open_subkey(path.as_str()) else {
                continue;
            };
            for name in key.enum_keys().flatten() {
                let Ok(entry) = key.open_subkey(&name) else {
                    continue;
                };
                let display: String = entry
                    .get_value("DisplayName")
                    .unwrap_or_default();
                if !display.eq_ignore_ascii_case("mRemoteNG") {
                    continue;
                }
                return Some(ClientInstall {
                    version: entry.get_value("DisplayVersion").ok(),
                    location: entry.get_value("InstallLocation").ok(),
                });
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Other platforms
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::{ClientInstall, Detection, FocusedSurface};

    /// The integration only exists on Windows: the remote client it targets
    /// is a Windows application.
    pub fn detect() -> Detection {
        Detection {
            client_running: false,
            rdp_session_count: 0,
            focused: FocusedSurface::OtherApplication,
            foreground_process: None,
        }
    }

    pub fn client_install() -> Option<ClientInstall> {
        None
    }
}

/// Reads the state of the environment.
pub fn detect() -> Detection {
    platform::detect()
}

/// Reads the installation information of the remote client.
pub fn client_install() -> Option<ClientInstall> {
    platform::client_install()
}
