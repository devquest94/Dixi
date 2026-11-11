//! Integration with a remote desktop reached through clipboard redirection.
//!
//! What this is for
//! ----------------
//! Dixi transcribes locally. The user wants to use that text inside a remote
//! session opened with mRemoteNG (RDP) on their work machine, without
//! installing anything there and without touching the corporate security
//! policy.
//!
//! How it works
//! ------------
//! mRemoteNG embeds the Microsoft Terminal Services ActiveX (`mstscax.dll`).
//! That client synchronizes the local clipboard with the remote session
//! through the redirection channel the server authorizes. Writing the
//! transcript to the *local* clipboard is therefore enough for the remote
//! side to receive it when the user presses Ctrl+V there.
//!
//! That is why the manual mode needs no keystroke at all: the redirection
//! already carries the text, the only thing left to do is press Ctrl+V
//! inside the remote session.
//!
//! What this module deliberately does not do
//! -----------------------------------------
//! * It never sends audio, files, credentials or the content of other
//!   windows. The only thing that ever reaches the clipboard is the text the
//!   user dictated, or explicitly approved.
//! * It never touches the remote machine's processes (`rdpclip.exe`), the
//!   corporate policy, or the saved mRemoteNG profiles.
//! * It never treats a remote handle as if it were a local one: the remote
//!   session is reached through the clipboard, not through a window handle.
//! * It never restores the clipboard on a fixed delay. The synchronization is
//!   asynchronous, and a fixed delay proves nothing about it: the text
//!   stays on the clipboard until the user confirms the paste, cancels it, or
//!   the transfer expires.

pub mod detect;

use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// The event the frontend listens to for the state of the transfer.
pub const TRANSFER_EVENT: &str = "remote-transfer-status";

/// The lowest timeout the transfer accepts: a paste that takes longer than
/// this is not a paste the user is still working on.
const MIN_TIMEOUT_MS: u64 = 5_000;

/// The highest timeout the transfer accepts.
const MAX_TIMEOUT_MS: u64 = 3_600_000;

/// The phase of a transfer to the remote desktop.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type, Default)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTransferPhase {
    /// Nothing is pending.
    #[default]
    Idle,
    /// The transcript is on the local clipboard, waiting for the user to
    /// paste it in the remote session.
    ReadyToPaste,
    /// Something read the clipboard after the publication: the RDP client
    /// picked the text up.
    RemoteRead,
    /// The user confirmed the paste.
    Pasted,
    /// The user cancelled the transfer.
    Cancelled,
    /// The transfer expired without a confirmation.
    Expired,
    /// The transfer could not be started.
    Failed,
}

/// The state of the transfer, as the UI sees it.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct RemoteTransferStatus {
    /// Where the transfer stands.
    pub phase: RemoteTransferPhase,
    /// Whether a transcript is still waiting to be pasted.
    pub pending: bool,
    /// The length of the pending transcript.
    ///
    /// The text itself never travels through the status: it would end up in
    /// the UI state and in the logs.
    pub char_count: usize,
    /// Whether something already read the clipboard after the publication.
    pub remote_read: bool,
    /// A non-sensitive technical note, e.g. the clipboard formats that
    /// cannot be restored without loss.
    pub warning: Option<String>,
}

impl RemoteTransferStatus {
    /// The state of an application with no transfer in flight.
    pub fn idle() -> Self {
        Self {
            phase: RemoteTransferPhase::Idle,
            pending: false,
            char_count: 0,
            remote_read: false,
            warning: None,
        }
    }
}

/// What the integration sees in the environment.
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct RemoteDesktopDetection {
    /// Whether this platform supports the integration at all.
    pub supported: bool,
    /// The remote client is installed on this machine.
    pub client_installed: bool,
    /// The version the installer reports.
    pub client_version: Option<String>,
    /// The folder it is installed in.
    pub client_location: Option<String>,
    /// The remote client is running right now.
    pub client_running: bool,
    /// How many RDP sessions the remote client has open.
    pub rdp_sessions: u32,
    /// The focus is inside an open RDP session of the remote client.
    pub remote_session_focused: bool,
    /// The process that owns the focused window.
    pub foreground_process: Option<String>,
    /// Whether the transcript could be inserted automatically right now.
    pub can_insert_automatically: bool,
    /// Why it could not, when it could not.
    ///
    /// It is a technical code, not a sentence: the panel shows it next to
    /// a translated label, and a sentence in one language inside a
    /// translated interface would be a lie.
    pub blocked_reason: Option<AutoBlock>,
}

/// Why the automatic insertion is not allowed.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum AutoBlock {
    /// The integration is disabled.
    IntegrationDisabled,
    /// The automatic mode is off.
    AutomaticModeOff,
    /// The remote client is not running.
    ClientNotRunning,
    /// The remote client has no RDP client loaded.
    NoRdpClient,
    /// More than one remote client has an RDP client: the destination is
    /// ambiguous.
    AmbiguousDestination,
    /// The remote client does not have the focus.
    SessionNotFocused,
}

impl AutoBlock {
    /// A stable technical code for the reason.
    pub fn code(self) -> &'static str {
        match self {
            AutoBlock::IntegrationDisabled => "integration_disabled",
            AutoBlock::AutomaticModeOff => "automatic_mode_off",
            AutoBlock::ClientNotRunning => "client_not_running",
            AutoBlock::NoRdpClient => "no_rdp_client",
            AutoBlock::AmbiguousDestination => "ambiguous_destination",
            AutoBlock::SessionNotFocused => "session_not_focused",
        }
    }
}

/// Decides whether the transcript may be inserted automatically.
///
/// The rules are deliberately strict: when anything cannot be confirmed, the
/// answer is "no" and the manual mode takes over. A transcription pasted
/// into the wrong window is worse than one extra keystroke.
pub fn decide_auto_insert(
    settings: &AppSettings,
    detection: &detect::Detection,
) -> Result<(), AutoBlock> {
    if !settings.remote_desktop_enabled {
        return Err(AutoBlock::IntegrationDisabled);
    }
    if !settings.remote_desktop_auto_mode {
        return Err(AutoBlock::AutomaticModeOff);
    }
    if !detection.client_running {
        return Err(AutoBlock::ClientNotRunning);
    }
    if detection.rdp_sessions == 0 {
        return Err(AutoBlock::NoRdpClient);
    }
    if detection.rdp_sessions > 1 {
        return Err(AutoBlock::AmbiguousDestination);
    }
    if !detection.focused.is_rdp_session() {
        return Err(AutoBlock::SessionNotFocused);
    }
    Ok(())
}

/// A transcript published on the clipboard, waiting for the user to paste it
/// in the remote session.
pub struct PendingPaste {
    #[cfg(target_os = "windows")]
    paste: crate::paste_tx::ExternalPaste,
    /// How long the transcript is.
    char_count: usize,
    /// The clipboard formats that cannot be restored without loss.
    warning: Option<String>,
    /// When the transcript was published.
    started_at: Instant,
    /// Set when the transaction settled on its own, without the user
    /// confirming or cancelling it.
    expired: Arc<AtomicBool>,
}

impl PendingPaste {
    /// Whether something read the clipboard after the publication.
    pub fn was_read(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.paste.was_read()
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    /// Whether the transaction settled on its own.
    pub fn expired(&self) -> bool {
        self.expired.load(Ordering::SeqCst)
    }

    /// Whether the clipboard still holds this transaction's text.
    ///
    /// When it does not, somebody else copied something and their content
    /// wins: the restore is skipped.
    pub fn still_ours(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.paste.still_ours()
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    /// Ends the transaction: the previous clipboard content is restored when
    /// we still own the clipboard.
    fn settle(&self) {
        #[cfg(target_os = "windows")]
        {
            self.paste.settle();
        }
    }
}

/// The transfer that is open, if any.
#[derive(Default)]
pub struct RemoteDesktopState {
    current: Mutex<Option<PendingPaste>>,
    /// The outcome of the last settled transfer, so the UI can show it after
    /// the transfer is gone.
    last_phase: Mutex<RemoteTransferPhase>,
}

impl RemoteDesktopState {
    /// The status the UI sees.
    pub fn status(&self) -> RemoteTransferStatus {
        // The lock is held for the read: `PendingPaste` owns the clipboard
        // lease and is deliberately not `Clone`.
        let guard = self.current.lock();
        let pending = match &guard {
            Ok(locked) => locked.as_ref(),
            Err(_) => None,
        };
        match pending {
            Some(pending) => {
                let expired = pending.expired();
                let read = pending.was_read();
                RemoteTransferStatus {
                    phase: if expired {
                        RemoteTransferPhase::Expired
                    } else if read {
                        RemoteTransferPhase::RemoteRead
                    } else {
                        RemoteTransferPhase::ReadyToPaste
                    },
                    pending: !expired,
                    char_count: pending.char_count,
                    remote_read: read,
                    warning: pending.warning.clone(),
                }
            }
            None => RemoteTransferStatus {
                phase: self.last_phase(),
                ..RemoteTransferStatus::idle()
            },
        }
    }

    /// The outcome of the last settled transfer.
    fn last_phase(&self) -> RemoteTransferPhase {
        self.last_phase
            .lock()
            .map(|phase| *phase)
            .unwrap_or(RemoteTransferPhase::Idle)
    }

    /// Takes the open transfer, if any.
    fn take_current(&self) -> Option<PendingPaste> {
        match self.current.lock() {
            Ok(mut slot) => slot.take(),
            Err(_) => None,
        }
    }

    /// Records the outcome of a settled transfer.
    fn set_last_phase(&self, phase: RemoteTransferPhase) {
        if let Ok(mut slot) = self.last_phase.lock() {
            *slot = phase;
        }
    }
}

/// How long a transcript may stay on the clipboard waiting for the paste.
pub fn transfer_timeout(settings: &AppSettings) -> Duration {
    Duration::from_millis(settings.remote_desktop_timeout_ms.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS))
}

/// Reads the environment.
pub fn detect(app: &AppHandle) -> RemoteDesktopDetection {
    let detection = detect::detect();
    let install = detect::client_install();
    let settings = crate::settings::get_settings(app);

    let block = decide_auto_insert(&settings, &detection);

    RemoteDesktopDetection {
        supported: cfg!(target_os = "windows"),
        client_installed: install.is_some(),
        client_version: install.as_ref().and_then(|i| i.version.clone()),
        client_location: install.as_ref().and_then(|i| i.location.clone()),
        client_running: detection.client_running,
        rdp_sessions: detection.rdp_sessions,
        remote_session_focused: detection.focused.is_rdp_session(),
        foreground_process: detection.foreground_process.clone(),
        can_insert_automatically: block.is_ok(),
        blocked_reason: block.err(),
    }
}

/// The last transcription, ready to be pasted.
///
/// The local corrections enabled by the user are already applied: this is the
/// text as the history recorded it.
fn last_transcription(app: &AppHandle) -> Result<String, String> {
    use crate::managers::history::HistoryManager;

    let history_manager = app.state::<Arc<HistoryManager>>();
    let entry = history_manager
        .get_latest_completed_entry()
        .map_err(|e| format!("Could not read the transcription history: {e}"))?
        .ok_or_else(|| "There is no completed transcription yet".to_string())?;

    let text = crate::tray::last_transcript_text(&entry).to_string();
    if text.trim().is_empty() {
        return Err("The last transcription is empty".to_string());
    }
    Ok(text)
}

/// Publishes the last transcription for the user to paste in the remote
/// session.
///
/// Nothing is typed and nothing is sent: the transcript goes to the local
/// clipboard and stays there until the user confirms the paste, cancels it,
/// or the transfer expires.
#[cfg(target_os = "windows")]
pub fn prepare(app: &AppHandle) -> Result<RemoteTransferStatus, String> {
    let settings = crate::settings::get_settings(app);
    if !settings.remote_desktop_enabled {
        return Err("The remote desktop integration is disabled".to_string());
    }
    if !settings.remote_desktop_manual_mode {
        return Err("The manual paste mode is off".to_string());
    }

    let text = last_transcription(app)?;
    let timeout = transfer_timeout(&settings);
    let restore = settings.remote_desktop_restore_clipboard;

    // The flag the worker sets when the transaction settles on its own.
    let expired = Arc::new(AtomicBool::new(false));
    let flag_for_worker = expired.clone();

    let paste = crate::paste_tx::publish_for_manual_paste(
        &text,
        app,
        timeout,
        restore,
        Some(Box::new(move || {
            flag_for_worker.store(true, Ordering::SeqCst);
        })),
    )?;

    let unrestorable = paste.unrestorable_formats();
    let warning = if unrestorable.is_empty() {
        None
    } else {
        Some(format!(
            "These clipboard formats cannot be restored without loss: {}",
            unrestorable.join(", ")
        ))
    };

    let pending = PendingPaste {
        paste,
        char_count: text.chars().count(),
        warning,
        started_at: Instant::now(),
        expired,
    };

    let state = app.state::<RemoteDesktopState>();
    // A transfer that is still open is settled before a new one starts: the
    // clipboard lease belongs to one transaction at a time.
    if let Some(previous) = state.take_current() {
        previous.settle();
    }
    if let Ok(mut slot) = state.current.lock() {
        *slot = Some(pending);
    }
    state.set_last_phase(RemoteTransferPhase::ReadyToPaste);

    log::info!(
        "[remote-desktop] transcript prepared for a manual paste ({} characters)",
        text.chars().count()
    );

    let status = state.status();
    let _ = app.emit(TRANSFER_EVENT, &status);
    Ok(status)
}

/// Publishes the last transcription for the user to paste in the remote
/// session.
#[cfg(not(target_os = "windows"))]
pub fn prepare(app: &AppHandle) -> Result<RemoteTransferStatus, String> {
    let _ = app;
    Err("The remote desktop integration is only available on Windows".to_string())
}

/// The user confirmed that the text was pasted in the remote session.
///
/// The previous clipboard content is restored, when the setting asks for it
/// and the clipboard still belongs to the transfer.
pub fn confirm(app: &AppHandle) -> RemoteTransferStatus {
    let state = app.state::<RemoteDesktopState>();
    if let Some(pending) = state.take_current() {
        // When the clipboard no longer holds the transcript, somebody else
        // copied something: their content wins and the restore is skipped.
        if pending.still_ours() {
            log::info!(
                "[remote-desktop] the user confirmed the paste after {}s",
                pending.started_at.elapsed().as_secs()
            );
        } else {
            log::info!(
                "[remote-desktop] the user confirmed the paste; the clipboard changed in the meantime, so it is left untouched"
            );
        }
        pending.settle();
        state.set_last_phase(RemoteTransferPhase::Pasted);
    }
    let status = state.status();
    let _ = app.emit(TRANSFER_EVENT, &status);
    status
}

/// The user cancelled the transfer.
pub fn cancel(app: &AppHandle) -> RemoteTransferStatus {
    let state = app.state::<RemoteDesktopState>();
    if let Some(pending) = state.take_current() {
        pending.settle();
        state.set_last_phase(RemoteTransferPhase::Cancelled);
        log::info!("[remote-desktop] the user cancelled the transfer");
    }
    let status = state.status();
    let _ = app.emit(TRANSFER_EVENT, &status);
    status
}

/// Settles a pending transfer when the app is about to exit.
///
/// The transcript is published with delayed rendering, so the text lives in
/// this process. If the process dies with a transfer pending, Windows
/// discards the delayed content and the user's clipboard ends up **empty**,
/// instead of holding what it held before. Settling here puts the previous
/// content back before the process goes away.
pub fn settle_on_exit(app: &AppHandle) {
    let state = app.state::<RemoteDesktopState>();
    let Some(pending) = state.take_current() else {
        return;
    };

    #[cfg(target_os = "windows")]
    {
        let before = crate::paste_tx::clipboard_sequence();
        pending.settle();
        // Give the worker the moment it needs to put the previous content
        // back. The wait is short: blocking the exit for long is worse than
        // a clipboard that was not restored.
        let deadline = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < deadline {
            if crate::paste_tx::clipboard_sequence() != before {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        pending.settle();
    }

    log::info!("[remote-desktop] the pending transfer was settled on exit");
}

/// The state of the transfer.
pub fn status(app: &AppHandle) -> RemoteTransferStatus {
    app.state::<RemoteDesktopState>().status()
}

/// Inserts the transcript in the remote session by itself.
///
/// Every condition is verified again right before the keystroke is sent: the
/// focus can change between the decision and the action, and a keystroke
/// that lands in another application is exactly what this integration must
/// never do.
#[cfg(target_os = "windows")]
pub fn insert_automatically(app: &AppHandle) -> Result<RemoteTransferStatus, String> {
    let settings = crate::settings::get_settings(app);
    let detection = detect::detect();

    decide_auto_insert(&settings, &detection).map_err(|block| {
        format!(
            "The automatic insertion is not allowed: {}",
            block.code()
        )
    })?;

    // The focus is checked once more immediately before the action: it may
    // have changed since the decision.
    let before = detect::detect();
    if !before.focused.is_rdp_session() {
        return Err("The RDP session lost the focus; nothing was written".to_string());
    }

    let text = last_transcription(app)?;
    let timeout = transfer_timeout(&settings);
    let restore = settings.remote_desktop_restore_clipboard;

    let expired = Arc::new(AtomicBool::new(false));
    let flag_for_worker = expired.clone();

    let paste = crate::paste_tx::publish_for_manual_paste(
        &text,
        app,
        timeout,
        restore,
        Some(Box::new(move || {
            flag_for_worker.store(true, Ordering::SeqCst);
        })),
    )?;

    let pending = PendingPaste {
        paste,
        char_count: text.chars().count(),
        warning: None,
        started_at: Instant::now(),
        expired,
    };

    let state = app.state::<RemoteDesktopState>();
    if let Some(previous) = state.take_current() {
        previous.settle();
    }
    if let Ok(mut slot) = state.current.lock() {
        *slot = Some(pending);
    }

    // The last check before the keystroke leaves: the focused window still
    // has to be the RDP session.
    let now = detect::detect();
    if !now.focused.is_rdp_session() {
        let _ = cancel(app);
        return Err("The RDP session lost the focus; nothing was written".to_string());
    }

    let chord = app.state::<crate::input::EnigoState>();
    let mut enigo = chord
        .0
        .lock()
        .map_err(|e| format!("The keyboard is busy: {e}"))?;
    crate::input::send_paste_ctrl_v(&mut enigo, 100)
        .map_err(|e| format!("Could not send the paste keystroke: {e}"))?;
    drop(enigo);

    log::info!("[remote-desktop] the paste keystroke was sent to the RDP session");

    let status = state.status();
    let _ = app.emit(TRANSFER_EVENT, &status);
    Ok(status)
}

/// Inserts the transcript in the remote session by itself.
#[cfg(not(target_os = "windows"))]
pub fn insert_automatically(app: &AppHandle) -> Result<RemoteTransferStatus, String> {
    let _ = app;
    Err("The remote desktop integration is only available on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;

    fn settings(
        enabled: bool,
        auto: bool,
    ) -> AppSettings {
        let mut settings = crate::settings::get_default_settings();
        settings.remote_desktop_enabled = enabled;
        settings.remote_desktop_auto_mode = auto;
        settings
    }

    fn detection(
        running: bool,
        rdp_sessions: u32,
        focused: detect::FocusedSurface,
    ) -> detect::Detection {
        detect::Detection {
            client_running: running,
            rdp_sessions,
            focused,
            foreground_process: Some("mremoteng.exe".to_string()),
        }
    }

    #[test]
    fn the_integration_is_off_by_default() {
        let settings = settings(false, false);
        let detection = detection(true, 1, detect::FocusedSurface::RdpSession);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::IntegrationDisabled)
        );
    }

    #[test]
    fn the_automatic_mode_is_off_by_default() {
        let settings = settings(true, false);
        let detection = detection(true, 1, detect::FocusedSurface::RdpSession);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::AutomaticModeOff)
        );
    }

    #[test]
    fn a_client_that_is_not_running_blocks_the_insertion() {
        let settings = settings(true, true);
        let detection = detection(false, 0, detect::FocusedSurface::OtherApplication);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::ClientNotRunning)
        );
    }

    #[test]
    fn a_connection_that_is_not_rdp_blocks_the_insertion() {
        let settings = settings(true, true);
        let detection = detection(true, 0, detect::FocusedSurface::ClientWindow);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::NoRdpClient)
        );
    }

    #[test]
    fn a_remote_window_without_the_focus_blocks_the_insertion() {
        let settings = settings(true, true);
        let detection = detection(true, 1, detect::FocusedSurface::ClientWindow);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::SessionNotFocused)
        );
    }

    #[test]
    fn two_open_sessions_block_the_insertion() {
        let settings = settings(true, true);
        let detection = detection(true, 2, detect::FocusedSurface::RdpSession);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::AmbiguousDestination)
        );
    }

    #[test]
    fn a_single_focused_rdp_session_allows_the_insertion() {
        let settings = settings(true, true);
        let detection = detection(true, 1, detect::FocusedSurface::RdpSession);
        assert_eq!(decide_auto_insert(&settings, &detection), Ok(()));
    }

    #[test]
    fn the_focus_of_another_application_blocks_the_insertion() {
        let settings = settings(true, true);
        let detection = detection(true, 1, detect::FocusedSurface::OtherApplication);
        assert_eq!(
            decide_auto_insert(&settings, &detection),
            Err(AutoBlock::SessionNotFocused)
        );
    }

    #[test]
    fn the_timeout_is_bounded() {
        let mut settings = crate::settings::get_default_settings();
        settings.remote_desktop_timeout_ms = 0;
        assert_eq!(transfer_timeout(&settings), Duration::from_millis(MIN_TIMEOUT_MS));
        settings.remote_desktop_timeout_ms = u64::MAX;
        assert_eq!(transfer_timeout(&settings), Duration::from_millis(MAX_TIMEOUT_MS));
        settings.remote_desktop_timeout_ms = 60_000;
        assert_eq!(transfer_timeout(&settings), Duration::from_millis(60_000));
    }

    /// Diagnóstico contra la máquina real.
    ///
    /// Se ignora por defecto porque su resultado depende del estado del
    /// equipo: qué aplicación tiene el foco y si el cliente remoto está
    /// corriendo. Es la forma de verificar la detección contra una sesión
    /// RDP de verdad:
    ///
    /// ```text
    /// cargo test --lib live_detection -- --ignored --nocapture
    /// ```
    ///
    /// Con la sesión RDP enfocada dentro de mRemoteNG tiene que informar
    /// `client_running: true`, `rdp_session_count: 1` y
    /// `focused: RdpSession`.
    #[test]
    #[ignore = "depende del estado del equipo; se ejecuta a mano"]
    fn live_detection() {
        let detection = detect::detect();
        println!("deteccion en vivo: {detection:#?}");
    }

    /// Diagnóstico: vuelca las ventanas del cliente remoto.
    ///
    /// ```text
    /// cargo test --lib live_windows -- --ignored --nocapture
    /// ```
    ///
    /// Sirve para ver, contra la máquina real, cómo se llaman las ventanas
    /// del cliente y cuál tiene el foco.
    #[test]
    #[ignore = "depende del estado del equipo; se ejecuta a mano"]
    fn live_windows() {
        for row in detect::describe_windows("mremoteng.exe") {
            println!("{row}");
        }
        if let Some(info) = detect::focused_window_info() {
            println!("foco: {info}");
        }
    }
}
