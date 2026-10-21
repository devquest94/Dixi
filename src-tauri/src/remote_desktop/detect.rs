//! Detecting the remote desktop host.
//!
//! The facts the integration needs are narrow, and all of them come from the
//! window system, the process table and the installer registry:
//!
//! * Is the remote client installed, and which version?
//! * Is it running?
//! * Does it have the RDP client loaded?
//! * Does it have the focus?
//!
//! What is deliberately *not* read: the saved connections, the server
//! addresses, the user names and the credentials. The uninstall key carries
//! only the display name, the version and the folder, and that is the
//! entirety of what is read from the registry.
//!
//! How the RDP client is recognized
//! -------------------------------
//! The first version of this module looked for a window whose class had been
//! registered by `mstscax.dll` (the Microsoft Terminal Services ActiveX).
//! Measured against a real session, that assumption is wrong on both
//! counts:
//!
//! * The remote client does not use the Microsoft ActiveX. It speaks RDP
//!   through **Indigo**, a pure .NET implementation, so `mstscax.dll` is
//!   never loaded in its process. The `AxInterop.MSTSCLib.dll` that sits in
//!   the installation folder is only the interop assembly for the optional
//!   ActiveX mode.
//! * The RDP session renders inside a window of the client whose class is
//!   `IHWindowClass` (the Indigo Host), not inside a child of a
//!   WinForms-hosted control.
//!
//! What the live machine shows is the class `IHWindowClass` (Indigo Host),
//! `OPWindowClass` and `OPContainerClass` (the Indigo output pane), and
//! `UIMainClass` / `UIContainerClass` (the window of the client itself).
//! The session is the Indigo Host window, and counting those windows counts
//! the open sessions.

/// The remote client this integration targets.
#[cfg(target_os = "windows")]
pub const REMOTE_CLIENT_IMAGE: &str = "mremoteng.exe";

/// The class of the window where the remote client renders an RDP session:
/// the Indigo Host.
#[cfg(target_os = "windows")]
pub const RDP_SESSION_CLASS: &str = "IHWindowClass";

/// What holds the focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedSurface {
    /// Nothing has the focus.
    Nothing,
    /// The focus is inside the remote client, but not inside a session: the
    /// connection list, a dialog, the menu.
    ClientWindow,
    /// The focus is inside an open RDP session of the remote client.
    RdpSession,
    /// The focused window belongs to another application.
    #[default]
    OtherApplication,
}

impl FocusedSurface {
    /// Whether the focus is inside the remote client.
    #[allow(dead_code, reason = "parte del contrato de la deteccion")]
    pub fn is_client(self) -> bool {
        matches!(self, FocusedSurface::ClientWindow | FocusedSurface::RdpSession)
    }

    /// Whether the focus is inside an open RDP session.
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
    pub rdp_sessions: u32,
    /// What holds the focus.
    pub focused: FocusedSurface,
    /// The image name of the process that owns the focused window, e.g.
    /// "explorer.exe". No path, no window title, no session data.
    pub foreground_process: Option<String>,
}

impl Detection {
    /// Whether there is an RDP session open.
    #[allow(dead_code, reason = "parte del contrato de la deteccion")]
    pub fn has_rdp_session(&self) -> bool {
        self.rdp_sessions > 0
    }
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
    use super::{
        ClientInstall, Detection, FocusedSurface, RDP_SESSION_CLASS, REMOTE_CLIENT_IMAGE,
    };
    use std::collections::HashMap;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND, LPARAM, RECT};
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassLongPtrW, GetClassNameW, GetForegroundWindow,
        GetGUIThreadInfo, GetParent, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible, GCLP_HMODULE, GUITHREADINFO,
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
                windows::core::PWSTR(buffer.as_mut_ptr()),
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

    /// The class of a window.
    fn window_class_name(hwnd: HWND) -> String {
        unsafe {
            let mut buffer = [0u16; 256];
            let length = GetClassNameW(hwnd, &mut buffer);
            if length == 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buffer[..length as usize])
        }
    }

    /// The title of a window.
    #[allow(dead_code, reason = "diagnóstico: el volcado de ventanas lo usa")]
    fn window_title(hwnd: HWND) -> String {
        unsafe {
            let mut buffer = [0u16; 512];
            let length = GetWindowTextW(hwnd, &mut buffer);
            if length == 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buffer[..length as usize])
        }
    }

    /// The size of a window.
    #[allow(dead_code, reason = "diagnóstico: el volcado de ventanas lo usa")]
    fn window_size(hwnd: HWND) -> Option<(i32, i32)> {
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return None;
            }
            Some((rect.right - rect.left, rect.bottom - rect.top))
        }
    }

    /// The module that registered the window class.
    ///
    /// It is `None` for the classes of WinForms, which are registered
    /// without a module handle. That is why the RDP session is recognized by
    /// the class of its window and not by the module.
    #[allow(dead_code, reason = "diagnóstico: el volcado de ventanas lo usa")]
    fn class_module_name(hwnd: HWND) -> Option<String> {
        unsafe {
            let module = GetClassLongPtrW(hwnd, GCLP_HMODULE);
            if module == 0 {
                return None;
            }
            let mut buffer = [0u16; 260];
            let length = GetModuleFileNameW(Some(HMODULE(module as *mut core::ffi::c_void)), &mut buffer);
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

    /// The instances of the remote client that are running.
    fn client_pids() -> Vec<u32> {
        // One lookup per process, not one per window.
        let mut names: HashMap<u32, Option<String>> = HashMap::new();
        for window in top_level_windows() {
            let pid = window_process_id(window);
            names
                .entry(pid)
                .or_insert_with(|| process_image_name(pid));
        }
        names
            .iter()
            .filter(|(_, name)| {
                name.as_deref()
                    .map(|name| name.eq_ignore_ascii_case(REMOTE_CLIENT_IMAGE))
                    .unwrap_or(false)
            })
            .map(|(pid, _)| *pid)
            .collect()
    }

    /// The visible windows of the client that can hold a remote session.
    ///
    /// The remote client speaks RDP through Indigo, and the window where the
    /// session renders is the Indigo Host, whose class is `IHWindowClass`.
    /// Counting those windows counts the open sessions.
    fn rdp_view_candidates(pid: u32) -> Vec<HWND> {
        let mut found = Vec::new();
        for top in top_level_windows() {
            if window_process_id(top) != pid {
                continue;
            }
            for window in std::iter::once(top).chain(child_windows(top)) {
                if !unsafe { IsWindowVisible(window) }.as_bool() {
                    continue;
                }
                if window_class_name(window) == RDP_SESSION_CLASS {
                    found.push(window);
                }
            }
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

    /// The window that holds the keyboard focus inside the foreground
    /// application.
    fn focused_child(foreground: HWND) -> Option<HWND> {
        unsafe {
            let thread = GetWindowThreadProcessId(foreground, None);
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            if GetGUIThreadInfo(thread, &mut info).is_err() {
                return None;
            }
            if info.hwndFocus.0.is_null() {
                return None;
            }
            Some(info.hwndFocus)
        }
    }

    /// Reads the state of the environment.
    pub fn detect() -> Detection {
        let client_pids = client_pids();
        let mut sessions: Vec<HWND> = Vec::new();
        for pid in &client_pids {
            sessions.extend(rdp_view_candidates(*pid));
        }
        let rdp_sessions = sessions.len() as u32;

        let foreground = unsafe { GetForegroundWindow() };
        let foreground_pid = if foreground.0.is_null() {
            0
        } else {
            window_process_id(foreground)
        };
        let foreground_process = if foreground_pid == 0 {
            None
        } else {
            process_image_name(foreground_pid)
        };
        let is_client = foreground_process
            .as_deref()
            .map(|name| name.eq_ignore_ascii_case(REMOTE_CLIENT_IMAGE))
            .unwrap_or(false);

        let focused = if foreground.0.is_null() {
            FocusedSurface::Nothing
        } else if !is_client {
            FocusedSurface::OtherApplication
        } else {
            // The focus has to be inside one of the session windows for the
            // session to be the destination.
            let in_session = focused_child(foreground)
                .map(|child| {
                    sessions
                        .iter()
                        .any(|session| is_inside(child, *session) || is_inside(foreground, *session))
                })
                .unwrap_or(false);
            if in_session {
                FocusedSurface::RdpSession
            } else {
                FocusedSurface::ClientWindow
            }
        };

        Detection {
            client_running: !client_pids.is_empty(),
            rdp_sessions,
            focused,
            foreground_process,
        }
    }

    /// The window that holds the keyboard focus inside the foreground
    /// application, with its class and its size.
    ///
    /// The RDP client renders inside the main window of the remote client,
    /// so the window that really tells "the user is in the session" is the
    /// focused one, not the top-level one.
    #[allow(dead_code, reason = "diagnóstico: lo usa la prueba en vivo")]
    pub fn focused_window_info() -> Option<String> {
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground.0.is_null() {
                return None;
            }
            let mut pid = 0u32;
            let thread = GetWindowThreadProcessId(foreground, Some(&mut pid));
            let mut info = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            if GetGUIThreadInfo(thread, &mut info).is_err() {
                return None;
            }
            Some(format!(
                "foreground pid={pid} class={:?} titulo={:?} | focus class={:?} tam={:?} | active class={:?}",
                window_class_name(foreground),
                window_title(foreground),
                window_class_name(info.hwndFocus),
                window_size(info.hwndFocus),
                window_class_name(info.hwndActive),
            ))
        }
    }

    /// Vuelca las ventanas de un proceso: su clase, su tamaño y si están
    /// visibles.
    ///
    /// Es una herramienta de diagnóstico, no parte del flujo: sirve para
    /// comprobar contra la máquina real cómo se llaman las ventanas del
    /// cliente remoto.
    #[allow(dead_code, reason = "diagnóstico: lo usa la prueba en vivo")]
    pub fn describe_windows(image_name: &str) -> Vec<String> {
        let mut rows = Vec::new();
        for window in top_level_windows() {
            let pid = window_process_id(window);
            let Some(name) = process_image_name(pid) else {
                continue;
            };
            if !name.eq_ignore_ascii_case(image_name) {
                continue;
            }
            let visible = unsafe { IsWindowVisible(window) }.as_bool();
            rows.push(format!(
                "top-level pid={pid} visible={visible} class={:?} tam={:?} titulo={:?}",
                window_class_name(window),
                window_size(window),
                window_title(window)
            ));
            for child in child_windows(window) {
                let child_visible = unsafe { IsWindowVisible(child) }.as_bool();
                let child_pid = window_process_id(child);
                rows.push(format!(
                    "   child pid={child_pid} visible={child_visible} class={:?} tam={:?} module={:?}",
                    window_class_name(child),
                    window_size(child),
                    class_module_name(child)
                ));
            }
        }
        rows
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
            rdp_clients: 0,
            focused: FocusedSurface::OtherApplication,
            foreground_process: None,
        }
    }

    pub fn focused_window_info() -> Option<String> {
        None
    }

    pub fn describe_windows(_image_name: &str) -> Vec<String> {
        Vec::new()
    }

    pub fn client_install() -> Option<ClientInstall> {
        None
    }
}

/// Reads the state of the environment.
pub fn detect() -> Detection {
    platform::detect()
}

/// The window that holds the keyboard focus, for diagnostics.
#[allow(dead_code, reason = "diagnóstico: lo usa la prueba en vivo")]
pub fn focused_window_info() -> Option<String> {
    platform::focused_window_info()
}

/// Vuelca las ventanas de un proceso, con su clase y su tamaño.
#[allow(dead_code, reason = "diagnóstico: lo usa la prueba en vivo")]
pub fn describe_windows(image_name: &str) -> Vec<String> {
    platform::describe_windows(image_name)
}

/// Reads the installation information of the remote client.
pub fn client_install() -> Option<ClientInstall> {
    platform::client_install()
}
