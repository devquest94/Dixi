use crate::remote_desktop::{self, RemoteDesktopDetection, RemoteTransferStatus};
use tauri::AppHandle;

/// What the integration sees in the environment: whether the remote client is
/// installed, whether it runs, how many RDP sessions it has open and which
/// window has the focus.
///
/// Nothing sensitive is read: no saved connection, no server address, no
/// credential.
#[tauri::command]
#[specta::specta]
pub fn detect_remote_desktop(app: AppHandle) -> RemoteDesktopDetection {
    remote_desktop::detect(&app)
}

/// Prepares the last transcription so the user can paste it in the remote
/// session.
///
/// The transcript goes to the local clipboard and stays there: the RDP
/// redirection carries it to the remote machine when the user presses Ctrl+V
/// there. Nothing is typed and nothing is sent.
#[tauri::command]
#[specta::specta]
pub fn prepare_remote_paste(app: AppHandle) -> Result<RemoteTransferStatus, String> {
    remote_desktop::prepare(&app)
}

/// The user confirmed that the text was pasted in the remote session.
///
/// The previous clipboard content is restored, when the setting asks for it.
#[tauri::command]
#[specta::specta]
pub fn confirm_remote_paste(app: AppHandle) -> RemoteTransferStatus {
    remote_desktop::confirm(&app)
}

/// The user cancelled the transfer.
#[tauri::command]
#[specta::specta]
pub fn cancel_remote_transfer(app: AppHandle) -> RemoteTransferStatus {
    remote_desktop::cancel(&app)
}

/// The state of the transfer.
#[tauri::command]
#[specta::specta]
pub fn remote_transfer_status(app: AppHandle) -> RemoteTransferStatus {
    remote_desktop::status(&app)
}

/// Inserts the transcript in the remote session by itself.
///
/// It only acts when every condition is verified: the integration and the
/// automatic mode are on, the remote client runs, a single RDP session is
/// open and it holds the focus. Otherwise it refuses and the manual mode
/// takes over.
#[tauri::command]
#[specta::specta]
pub fn insert_remote_transcription(app: AppHandle) -> Result<RemoteTransferStatus, String> {
    remote_desktop::insert_automatically(&app)
}

/// Enables or disables the integration with the remote desktop.
///
/// The shortcut that prepares the transcription follows the switch: it is
/// registered while the integration is on and unregistered while it is off.
#[tauri::command]
#[specta::specta]
pub fn change_remote_desktop_enabled_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.remote_desktop_enabled = enabled;
    crate::settings::write_settings(&app, settings);

    crate::shortcut::refresh_remote_desktop_shortcut(&app, enabled);
    Ok(())
}

/// Turns the manual paste on or off.
#[tauri::command]
#[specta::specta]
pub fn change_remote_desktop_manual_mode_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.remote_desktop_manual_mode = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

/// Turns the automatic insertion on or off.
///
/// It is off by default and it only acts when every condition is verified.
#[tauri::command]
#[specta::specta]
pub fn change_remote_desktop_auto_mode_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.remote_desktop_auto_mode = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

/// Restores the previous clipboard content when the transfer settles.
#[tauri::command]
#[specta::specta]
pub fn change_remote_desktop_restore_clipboard_setting(
    app: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.remote_desktop_restore_clipboard = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

/// The notification that tells the user the text is ready to paste.
#[tauri::command]
#[specta::specta]
pub fn change_remote_desktop_notify_setting(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.remote_desktop_notify = enabled;
    crate::settings::write_settings(&app, settings);
    Ok(())
}

/// How long the transcript may stay on the clipboard waiting for the paste,
/// in milliseconds.
#[tauri::command]
#[specta::specta]
pub fn change_remote_desktop_timeout_setting(app: AppHandle, ms: u64) -> Result<(), String> {
    let mut settings = crate::settings::get_settings(&app);
    settings.remote_desktop_timeout_ms = ms;
    crate::settings::write_settings(&app, settings);
    Ok(())
}
