//! Receipt-sequenced clipboard paste ("reliable paste", debug-gated).
//!
//! The legacy clipboard paste (`clipboard::paste_via_clipboard`) restores the
//! previous clipboard after a fixed delay. The paste keystroke is only
//! *enqueued* at that point — the target application reads the clipboard
//! whenever its event loop gets to it, so any fixed delay can lose the race
//! and the user gets their old clipboard pasted back (#502).
//!
//! This module instead publishes the transcript as a *lazy promise* and waits
//! for the operating system to tell us that a consumer actually read the
//! clipboard — a "receipt" — before restoring:
//!
//! - Windows: delayed rendering (`SetClipboardData(CF_UNICODETEXT, NULL)`),
//!   the owner window receives `WM_RENDERFORMAT` on read.
//! - macOS: `declareTypes:owner:` with an owner object, the pasteboard calls
//!   `pasteboard:provideDataForType:` on read.
//!
//! Two rules make the receipt trustworthy:
//!
//! 1. Only receipts observed *after* the paste chord was injected count. A
//!    read before that is an eager third party (clipboard manager, antivirus)
//!    reacting to the clipboard change itself.
//! 2. Restoration only happens while we still own the clipboard
//!    (sequence number / changeCount unchanged, no ownership-lost event). If
//!    the user copied something else in the meantime, their action wins.
//!
//! The restore is additionally gated on a short quiet period after the *last*
//! receipt, because some applications read the clipboard several times per
//! paste (Chromium probes, then reads). A bounded timeout caps how long the
//! transcript may occupy the clipboard; the failure mode is always "the
//! transcript stays on the clipboard a bit longer", never "stale content gets
//! pasted".

// The shared transaction state is compiled on all platforms (for the unit
// tests), but only the macOS/Windows platform modules consume all of it.
#![cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]

use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// The clipboard lease for a paste the user performs by hand.
///
/// Only Windows exposes it: the remote-desktop integration it exists for is
/// a Windows client (mRemoteNG).
#[cfg(target_os = "windows")]
pub(crate) use windows::{clipboard_sequence, publish_for_manual_paste, ExternalPaste};

/// How long after the *last* observed read the transcript stays on the
/// clipboard before restoring. Covers applications that read the clipboard
/// several times per paste (e.g. Chromium probe-then-read).
pub(crate) const QUIET_PERIOD: Duration = Duration::from_millis(200);

/// Upper bound on how long the transcript may occupy the clipboard before we
/// restore regardless of receipts. Long enough that a realistically loaded
/// target always gets to read first; short enough that a lost keystroke does
/// not strand the transcript on the clipboard for long.
pub(crate) const RESTORE_TIMEOUT: Duration = Duration::from_secs(8);

/// When the chord could not be injected at all, no legitimate receipt can
/// arrive, so restore quickly instead of waiting out the full timeout.
pub(crate) const FAILED_INJECTION_TIMEOUT: Duration = Duration::from_millis(500);

/// How long a transcript published for a paste the user performs *by hand*
/// may stay on the clipboard before it is restored anyway.
///
/// The user has to switch to the remote session, click the field and press
/// Ctrl+V there, so this window is deliberately generous: restoring earlier
/// would replace the text before the remote side asked for it, and a fixed
/// delay proves nothing about the synchronization.
pub(crate) const DEFAULT_EXTERNAL_TIMEOUT: Duration = Duration::from_secs(180);

/// What turns a clipboard read into evidence that the transcript was
/// consumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TxTrigger {
    /// The caller injects the paste chord right after the publication, so
    /// only the reads that follow the chord count. A read before that is an
    /// eager third party reacting to the clipboard change itself.
    Chord,
    /// The text is pasted by hand somewhere else — typically a remote
    /// desktop reached through clipboard redirection. Every read after the
    /// publication counts, because there is no chord to compare against.
    Publication,
}

/// Shared, cross-thread record of one paste transaction.
#[derive(Debug)]
pub(crate) struct TxState {
    /// When the transcript was published to the clipboard.
    pub published_at: Instant,
    /// When the paste chord was injected. Only receipts *after* this count as
    /// evidence the target read the transcript — earlier reads are eager third
    /// parties reacting to the clipboard change itself.
    pub injected_at: Option<Instant>,
    /// The chord could not be sent; short-circuit the wait.
    pub injection_failed: bool,
    /// Times at which a consumer requested the clipboard data.
    pub receipts: Vec<Instant>,
    /// Someone else took clipboard ownership (user copied elsewhere, ...).
    pub ownership_lost: bool,
    /// A newer paste transaction settled this one early (see flush logic in
    /// the platform modules).
    pub cancelled: bool,
    /// The post-paste Enter (auto-submit) has been sent for this transaction.
    /// (Read on Windows; the macOS path settles via `MacPending::settled`.)
    #[allow(dead_code)]
    pub auto_submit_sent: bool,
    /// First post-injection receipt has been logged.
    pub logged_receipt: bool,
    /// What makes a read count as evidence that the transcript was consumed.
    pub trigger: TxTrigger,
    /// The caller asked to settle the transaction: the user confirmed the
    /// paste, or cancelled it.
    pub settle_requested: bool,
    /// How long the transaction may stay open before it settles on its own.
    pub timeout: Duration,
}

impl TxState {
    pub fn new() -> Self {
        Self {
            published_at: Instant::now(),
            injected_at: None,
            injection_failed: false,
            receipts: Vec::new(),
            ownership_lost: false,
            cancelled: false,
            auto_submit_sent: false,
            logged_receipt: false,
            trigger: TxTrigger::Chord,
            settle_requested: false,
            timeout: RESTORE_TIMEOUT,
        }
    }

    /// A transaction for a paste the user performs by hand.
    ///
    /// It has no chord, so it is settled by the caller (the user confirmed
    /// the paste, or cancelled) and never by the reads going quiet.
    pub fn for_publication(timeout: Duration) -> Self {
        Self {
            trigger: TxTrigger::Publication,
            timeout,
            ..Self::new()
        }
    }

    /// Records a read receipt, logging the first one that counts as evidence.
    pub fn record_receipt(&mut self, at: Instant) {
        self.receipts.push(at);
        if !self.logged_receipt {
            if let Some(trigger) = self.trigger_at() {
                if at >= trigger {
                    self.logged_receipt = true;
                    log::info!(
                        "[reliable-paste] clipboard read {}ms after the trigger",
                        at.duration_since(trigger).as_millis()
                    );
                }
            }
        }
    }

    /// The instant from which a read counts as evidence.
    ///
    /// For a chord transaction that is the moment the chord was injected; for
    /// a transaction the user pastes by hand it is the publication itself.
    pub fn trigger_at(&self) -> Option<Instant> {
        match self.trigger {
            TxTrigger::Chord => self.injected_at,
            TxTrigger::Publication => Some(self.published_at),
        }
    }

    pub fn last_receipt_after_injection(&self) -> Option<Instant> {
        let injected = self.injected_at?;
        self.receipts.iter().copied().rev().find(|t| *t >= injected)
    }

    pub fn any_receipt_after_injection(&self) -> bool {
        self.last_receipt_after_injection().is_some()
    }

    /// The last read that counts as evidence for this transaction's trigger.
    pub fn last_receipt_after_trigger(&self) -> Option<Instant> {
        let trigger = self.trigger_at()?;
        self.receipts.iter().copied().rev().find(|t| *t >= trigger)
    }

    /// Whether a consumer read the clipboard after the trigger.
    pub fn any_receipt_after_trigger(&self) -> bool {
        self.last_receipt_after_trigger().is_some()
    }
}

pub(crate) enum WaitDecision {
    KeepWaiting,
    /// Stop waiting; settle the transaction (auto-submit + guarded restore).
    Finish,
}

/// Pure decision: given the current transaction state, keep waiting for the
/// target to read, or finish now. Both platform event loops call this.
pub(crate) fn evaluate(state: &TxState, now: Instant) -> WaitDecision {
    if state.ownership_lost || state.cancelled || state.settle_requested {
        return WaitDecision::Finish;
    }
    match state.trigger {
        TxTrigger::Chord => {
            if let Some(last) = state.last_receipt_after_injection() {
                if now.duration_since(last) >= QUIET_PERIOD {
                    return WaitDecision::Finish;
                }
            }
            let deadline = if state.injection_failed {
                FAILED_INJECTION_TIMEOUT
            } else {
                state.timeout
            };
            if now.duration_since(state.published_at) >= deadline {
                return WaitDecision::Finish;
            }
        }
        TxTrigger::Publication => {
            // The user pastes by hand, in the remote session. The transcript
            // stays on the clipboard until the caller settles it (the user
            // confirmed the paste, or cancelled it) or the transfer expires.
            //
            // A read never ends this transaction early: the remote side can
            // still ask for the clipboard when the user presses Ctrl+V there,
            // and the synchronization is asynchronous. Restoring on a read
            // would be exactly the fixed-delay race this module exists to
            // avoid.
            if now.duration_since(state.published_at) >= state.timeout {
                return WaitDecision::Finish;
            }
        }
    }
    WaitDecision::KeepWaiting
}

/// Modifier hold for the receipt-sequenced chord, kept at parity with the
/// legacy path (100ms) for the beta: that hold was added in #165 because real
/// users' systems dropped chords released too quickly, and the beta should
/// validate the receipt mechanism without changing a second variable.
///
/// Once receipts are proven in the field this becomes a safe tuning knob — a
/// chord the target never recognizes produces no receipt and is logged ("no
/// read within timeout") rather than failing silently, so a shorter hold
/// (measured working at 10ms on a fast machine, cutting visible latency from
/// ~110ms to ~20ms) can be tried as its own experiment later.
const CHORD_HOLD_MS: u64 = 100;

/// Sends the platform paste chord for the configured method.
pub(crate) fn send_chord(
    enigo: &mut enigo::Enigo,
    paste_method: &crate::settings::PasteMethod,
) -> Result<(), String> {
    use crate::settings::PasteMethod;
    match paste_method {
        PasteMethod::CtrlV => crate::input::send_paste_ctrl_v(enigo, CHORD_HOLD_MS),
        PasteMethod::CtrlShiftV => crate::input::send_paste_ctrl_shift_v(enigo, CHORD_HOLD_MS),
        PasteMethod::ShiftInsert => crate::input::send_paste_shift_insert(enigo, CHORD_HOLD_MS),
        other => Err(format!(
            "Invalid paste method for clipboard paste: {:?}",
            other
        )),
    }
}

/// Attempts the receipt-sequenced paste. Returns `Err` before anything has
/// been published when the platform transaction cannot start, in which case
/// the caller should fall back to the legacy paste path. On `Ok`, publishing
/// and chord injection have completed and the guarded restore (plus
/// auto-submit) finishes asynchronously.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn try_reliable_paste(
    text: &str,
    app_handle: &tauri::AppHandle,
    paste_method: &crate::settings::PasteMethod,
    enigo: &mut enigo::Enigo,
    auto_submit: bool,
    auto_submit_key: crate::settings::AutoSubmitKey,
    clipboard_handling: crate::settings::ClipboardHandling,
) -> Result<(), String> {
    platform::run(
        text,
        app_handle,
        paste_method,
        enigo,
        auto_submit,
        auto_submit_key,
        clipboard_handling,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_after_publish(published_ago: Duration) -> TxState {
        let mut s = TxState::new();
        s.published_at = Instant::now() - published_ago;
        s
    }

    #[test]
    fn keeps_waiting_without_receipt_within_timeout() {
        let s = state_after_publish(Duration::from_millis(100));
        assert!(matches!(
            evaluate(&s, Instant::now()),
            WaitDecision::KeepWaiting
        ));
    }

    #[test]
    fn finishes_after_quiet_period_once_read() {
        let mut s = state_after_publish(Duration::from_millis(300));
        s.injected_at = Some(Instant::now() - Duration::from_millis(250));
        s.receipts.push(Instant::now() - QUIET_PERIOD);
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }

    #[test]
    fn waits_through_quiet_period_after_recent_read() {
        let mut s = state_after_publish(Duration::from_millis(300));
        s.injected_at = Some(Instant::now() - Duration::from_millis(100));
        s.receipts.push(Instant::now() - Duration::from_millis(50));
        assert!(matches!(
            evaluate(&s, Instant::now()),
            WaitDecision::KeepWaiting
        ));
    }

    #[test]
    fn pre_injection_receipt_does_not_count() {
        let mut s = state_after_publish(Duration::from_millis(300));
        s.receipts.push(Instant::now() - Duration::from_millis(200));
        s.injected_at = Some(Instant::now() - Duration::from_millis(100));
        assert!(!s.any_receipt_after_injection());
        assert!(matches!(
            evaluate(&s, Instant::now()),
            WaitDecision::KeepWaiting
        ));
    }

    #[test]
    fn finishes_on_timeout_without_receipt() {
        let s = state_after_publish(RESTORE_TIMEOUT);
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }

    #[test]
    fn failed_injection_uses_short_timeout() {
        let mut s = state_after_publish(FAILED_INJECTION_TIMEOUT);
        s.injection_failed = true;
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }

    #[test]
    fn ownership_loss_finishes_immediately() {
        let mut s = state_after_publish(Duration::from_millis(10));
        s.ownership_lost = true;
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }

    fn publication_state(timeout: Duration) -> TxState {
        let mut s = TxState::for_publication(timeout);
        s.published_at = Instant::now() - Duration::from_millis(100);
        s
    }

    #[test]
    fn publication_keeps_waiting_even_after_the_reads_go_quiet() {
        // The user pastes by hand in the remote session: a read does not
        // mean the remote side already asked for the clipboard.
        let mut s = publication_state(Duration::from_secs(180));
        s.receipts.push(Instant::now() - QUIET_PERIOD - Duration::from_millis(50));
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::KeepWaiting));
    }

    #[test]
    fn publication_counts_reads_from_the_publication() {
        let mut s = publication_state(Duration::from_secs(180));
        s.receipts.push(Instant::now() - Duration::from_millis(10));
        assert!(s.any_receipt_after_trigger());
        // The chord was never injected, so the chord-only accessor stays empty.
        assert!(!s.any_receipt_after_injection());
    }

    #[test]
    fn publication_finishes_when_the_caller_settles_it() {
        let mut s = publication_state(Duration::from_secs(180));
        s.settle_requested = true;
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }

    #[test]
    fn publication_finishes_when_the_transfer_expires() {
        let s = publication_state(Duration::from_millis(50));
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }

    #[test]
    fn publication_finishes_when_the_clipboard_ownership_is_lost() {
        let mut s = publication_state(Duration::from_secs(180));
        s.ownership_lost = true;
        assert!(matches!(evaluate(&s, Instant::now()), WaitDecision::Finish));
    }
}
