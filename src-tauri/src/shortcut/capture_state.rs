//! The real state of the capture shortcut.
//!
//! The interface used to be told, wrongly, that the capture shortcut could
//! not be registered because another application already had it. The
//! shortcut was registered and it did fire: the log of a real session shows
//! it being registered and dispatched six times.
//!
//! What was wrong was the bookkeeping. The flag that said "registered" was
//! only written by one of the routes that register a shortcut, and the route
//! that runs at startup —`init_shortcuts`— is not that one. So the state was
//! wrong from the very first moment, and the interface reported the wrong
//! reason for it.
//!
//! This module is the single place where the state is decided. Every attempt
//! to register the capture shortcut, from wherever it comes, goes through
//! it, so what the interface shows is what the system actually did.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;

/// The backend that registers the shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ShortcutBackend {
    /// The Tauri global-shortcut plugin.
    ///
    /// It asks the system to reserve the combination with `RegisterHotKey`,
    /// so the system can refuse it. This is the **only** backend where
    /// another application can be the reason of a failure.
    Tauri,
    /// handy-keys.
    ///
    /// It watches the keyboard with a low level hook and keeps its own list
    /// of combinations in memory. It never asks the system to reserve
    /// anything, so **no other application can conflict with it**: every
    /// failure here is Dixi's own.
    HandyKeys,
}

impl ShortcutBackend {
    /// The name of the backend, for the log and for the interface.
    pub fn as_str(self) -> &'static str {
        match self {
            ShortcutBackend::Tauri => "tauri",
            ShortcutBackend::HandyKeys => "handy_keys",
        }
    }
}

/// Why the last attempt to register the capture shortcut failed.
///
/// The codes are `snake_case`, like the rest of the codes the backend sends
/// to the interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationErrorCode {
    /// Another application holds the same combination.
    ///
    /// Only the Tauri backend can produce this: it asks the system to
    /// reserve the combination, and the system refuses when somebody else
    /// already has it.
    Conflict,
    /// Dixi itself had already registered the same combination.
    ///
    /// This is a duplicate inside Dixi, not a conflict with another
    /// application.
    Duplicate,
    /// The combination could not be interpreted.
    InvalidCombination,
    /// The backend was not ready: its state was never initialized.
    NotInitialized,
    /// The backend did not answer.
    BackendUnavailable,
    /// Anything else.
    Unknown,
}

impl RegistrationErrorCode {
    /// Whether the failure is another application's doing.
    ///
    /// The interface may only say "another application has it" when this is
    /// true. Saying it otherwise is a lie that sends the user to change the
    /// shortcut of a program that never had anything to do with it.
    pub fn blames_another_application(self) -> bool {
        matches!(self, RegistrationErrorCode::Conflict)
    }
}

/// The stable code of a registration failure.
///
/// The two backends only give a message, so the classification is made over
/// it. It is deliberately conservative: a text that does not clearly mean
/// one thing is `Unknown`, and `Unknown` never accuses anybody.
pub fn classify_registration_error(
    backend: ShortcutBackend,
    error: &str,
) -> RegistrationErrorCode {
    let text = error.to_lowercase();

    if text.contains("not initialized") || text.contains("no state") {
        return RegistrationErrorCode::NotInitialized;
    }

    if text.contains("parse")
        || text.contains("unrecognized")
        || text.contains("unexpected hotkey")
        || text.contains("unexpected empty token")
        || text.contains("invalid")
    {
        return RegistrationErrorCode::InvalidCombination;
    }

    if text.contains("already registered") {
        // The same words, two different meanings.
        //
        // The Tauri backend asks the system to reserve the combination, so
        // "already registered" is the system refusing it: that is a real
        // conflict, and the only way another application can be involved.
        //
        // handy-keys keeps its own list in memory and never asks the system
        // for anything, so the same words mean that Dixi registered that
        // same combination twice.
        return match backend {
            ShortcutBackend::Tauri => RegistrationErrorCode::Conflict,
            ShortcutBackend::HandyKeys => RegistrationErrorCode::Duplicate,
        };
    }

    if text.contains("failed to send")
        || text.contains("failed to receive")
        || text.contains("timed out")
        || text.contains("timeout")
        || text.contains("disconnected")
    {
        return RegistrationErrorCode::BackendUnavailable;
    }

    RegistrationErrorCode::Unknown
}

/// What is known about the capture shortcut right now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CaptureShortcutStatus {
    /// Whether the shortcut is registered and in effect.
    pub registered: bool,
    /// Whether there was an attempt to register it at all.
    ///
    /// Before the first attempt there is nothing to report: the interface
    /// must not accuse anybody of anything.
    pub attempted: bool,
    /// The combination that is in effect, or the one that was tried.
    pub binding: String,
    /// The backend that was used.
    pub backend: String,
    /// The stable code of the failure, when there was one.
    pub error_code: Option<RegistrationErrorCode>,
    /// The message the backend gave.
    pub error: Option<String>,
    /// Whether the failure is another application's doing.
    pub conflicts_with_another_application: bool,
    /// When the last attempt happened, in milliseconds since the epoch.
    pub attempted_at: Option<i64>,
    /// Whether this system can capture the selection of another application
    /// at all.
    ///
    /// The capture of a selection only exists on Windows. On the other
    /// platforms the shortcut is not registered on purpose, and the
    /// interface must not offer it.
    pub available: bool,
}

impl CaptureShortcutStatus {
    /// A state that says nothing has been tried yet.
    pub fn not_tried() -> Self {
        Self {
            registered: false,
            attempted: false,
            binding: String::new(),
            backend: String::new(),
            error_code: None,
            error: None,
            conflicts_with_another_application: false,
            attempted_at: None,
            available: super::binding_is_available(CAPTURE_SHORTCUT_ID),
        }
    }
}

/// The identifier of the shortcut that captures the selection.
pub const CAPTURE_SHORTCUT_ID: &str = "capture_selection";

/// Where the state of the capture shortcut is kept.
#[derive(Debug)]
pub struct CaptureShortcutStore {
    inner: Mutex<Option<CaptureShortcutStatus>>,
}

impl CaptureShortcutStore {
    /// A store that has not seen any attempt yet.
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// Records the result of an attempt to register the shortcut.
    pub fn record(
        &self,
        binding: &str,
        backend: ShortcutBackend,
        result: &Result<(), String>,
    ) -> CaptureShortcutStatus {
        let status = match result {
            Ok(()) => CaptureShortcutStatus {
                registered: true,
                attempted: true,
                binding: binding.to_string(),
                backend: backend.as_str().to_string(),
                error_code: None,
                error: None,
                conflicts_with_another_application: false,
                attempted_at: Some(now_millis()),
                available: super::binding_is_available(CAPTURE_SHORTCUT_ID),
            },
            Err(error) => {
                let code = classify_registration_error(backend, error);
                CaptureShortcutStatus {
                    registered: false,
                    attempted: true,
                    binding: binding.to_string(),
                    backend: backend.as_str().to_string(),
                    error_code: Some(code),
                    error: Some(error.clone()),
                    conflicts_with_another_application: code.blames_another_application(),
                    attempted_at: Some(now_millis()),
                    available: super::binding_is_available(CAPTURE_SHORTCUT_ID),
                }
            }
        };

        self.store(status.clone());
        status
    }

    /// Records that the shortcut is not registered any more.
    ///
    /// The reason of the last failure is dropped: the shortcut is not
    /// registered, so there is no failure to report any more.
    pub fn release(&self) -> Option<CaptureShortcutStatus> {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let previous = guard.as_ref()?.clone();

        let released = CaptureShortcutStatus {
            registered: false,
            attempted: true,
            binding: previous.binding,
            backend: previous.backend,
            error_code: None,
            error: None,
            conflicts_with_another_application: false,
            attempted_at: previous.attempted_at,
            available: previous.available,
        };

        *guard = Some(released.clone());
        Some(released)
    }

    /// The state, if there was an attempt.
    pub fn get(&self) -> Option<CaptureShortcutStatus> {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// The state, or a state that says nothing has been tried yet.
    pub fn get_or_not_tried(&self) -> CaptureShortcutStatus {
        self.get().unwrap_or_else(CaptureShortcutStatus::not_tried)
    }

    fn store(&self, status: CaptureShortcutStatus) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        *guard = Some(status);
    }
}

/// The state of the capture shortcut of the running application.
pub static CAPTURE_SHORTCUT: CaptureShortcutStore = CaptureShortcutStore::new();

/// The current time in milliseconds since the epoch.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_registration_leaves_the_shortcut_registered() {
        let store = CaptureShortcutStore::new();

        let status = store.record("ctrl+alt+c", ShortcutBackend::HandyKeys, &Ok(()));

        assert!(status.registered);
        assert!(status.attempted);
        assert_eq!(status.binding, "ctrl+alt+c");
        assert_eq!(status.backend, "handy_keys");
        assert!(status.error_code.is_none());
        assert!(status.error.is_none());
        assert!(!status.conflicts_with_another_application);
        assert!(status.attempted_at.is_some());
    }

    #[test]
    fn the_state_that_is_read_is_the_state_that_was_recorded() {
        let store = CaptureShortcutStore::new();

        store.record("ctrl+alt+c", ShortcutBackend::Tauri, &Ok(()));

        let read = store.get_or_not_tried();
        assert!(read.registered);
        assert_eq!(read.binding, "ctrl+alt+c");
    }

    #[test]
    fn a_real_conflict_is_reported_as_a_conflict() {
        // The Tauri backend asks the system to reserve the combination, so
        // this is the one case where another application is to blame.
        let store = CaptureShortcutStore::new();

        let status = store.record(
            "ctrl+alt+c",
            ShortcutBackend::Tauri,
            &Err("HotKey already registered: HotKey { modifiers: CTRL, key: KeyC }".to_string()),
        );

        assert!(!status.registered);
        assert!(status.attempted);
        assert_eq!(
            status.error_code,
            Some(RegistrationErrorCode::Conflict),
            "the system refused the combination, so that is a conflict"
        );
        assert!(status.conflicts_with_another_application);
    }

    #[test]
    fn the_duplicate_of_handy_keys_is_not_a_conflict_with_another_application() {
        // handy-keys keeps its own list in memory and never asks the system
        // to reserve anything: the same words mean a duplicate inside Dixi.
        let store = CaptureShortcutStore::new();

        let status = store.record(
            "ctrl+alt+c",
            ShortcutBackend::HandyKeys,
            &Err("Failed to register hotkey: Hotkey already registered: ctrl+alt+c (id: HotkeyId(3))".to_string()),
        );

        assert!(!status.registered);
        assert_eq!(
            status.error_code,
            Some(RegistrationErrorCode::Duplicate),
            "Dixi registered the same combination twice, nobody else is involved"
        );
        assert!(
            !status.conflicts_with_another_application,
            "a duplicate inside Dixi must not be blamed on another application"
        );
    }

    #[test]
    fn an_initialization_failure_is_not_reported_as_a_conflict() {
        let store = CaptureShortcutStore::new();

        let status = store.record(
            "ctrl+alt+c",
            ShortcutBackend::HandyKeys,
            &Err("HandyKeysState not initialized".to_string()),
        );

        assert_eq!(
            status.error_code,
            Some(RegistrationErrorCode::NotInitialized)
        );
        assert!(
            !status.conflicts_with_another_application,
            "a backend that was not ready is Dixi's problem, not another application's"
        );
    }

    #[test]
    fn an_invalid_combination_is_told_apart_from_a_conflict() {
        let store = CaptureShortcutStore::new();

        let status = store.record(
            "ctrl+alt",
            ShortcutBackend::Tauri,
            &Err("Unexpected hotkey string format: \"ctrl+alt\"".to_string()),
        );

        assert_eq!(
            status.error_code,
            Some(RegistrationErrorCode::InvalidCombination)
        );
        assert!(!status.conflicts_with_another_application);
    }

    #[test]
    fn an_unknown_failure_accuses_nobody() {
        let store = CaptureShortcutStore::new();

        let status = store.record(
            "ctrl+alt+c",
            ShortcutBackend::Tauri,
            &Err("something nobody expected".to_string()),
        );

        assert_eq!(status.error_code, Some(RegistrationErrorCode::Unknown));
        assert!(!status.conflicts_with_another_application);
    }

    #[test]
    fn changing_the_combination_updates_the_state() {
        let store = CaptureShortcutStore::new();

        store.record("ctrl+alt+c", ShortcutBackend::HandyKeys, &Ok(()));
        let status = store.record("ctrl+alt+d", ShortcutBackend::HandyKeys, &Ok(()));

        assert!(status.registered);
        assert_eq!(status.binding, "ctrl+alt+d");
        assert_eq!(store.get_or_not_tried().binding, "ctrl+alt+d");
    }

    #[test]
    fn before_the_first_attempt_there_is_nothing_to_report() {
        let store = CaptureShortcutStore::new();

        let status = store.get_or_not_tried();

        assert!(!status.attempted);
        assert!(!status.registered);
        assert!(status.error_code.is_none());
        assert!(
            !status.conflicts_with_another_application,
            "without an attempt there is nothing to blame anybody for"
        );
    }

    #[test]
    fn releasing_the_shortcut_marks_it_as_not_registered() {
        let store = CaptureShortcutStore::new();
        store.record("ctrl+alt+c", ShortcutBackend::HandyKeys, &Ok(()));

        let released = store.release().expect("there was a registration");

        assert!(!released.registered);
        assert!(released.attempted);
        assert!(released.error_code.is_none());
        assert!(!store.get_or_not_tried().registered);
    }

    #[test]
    fn releasing_a_shortcut_that_was_never_registered_says_nothing() {
        let store = CaptureShortcutStore::new();

        assert!(store.release().is_none());
        assert!(!store.get_or_not_tried().attempted);
    }

    #[test]
    fn changing_the_combination_releases_the_previous_one() {
        // This is what happens when the shortcut is changed from the
        // settings: the old one is released and the new one is registered,
        // and the state has to end up saying the new one.
        let store = CaptureShortcutStore::new();
        store.record("ctrl+alt+c", ShortcutBackend::HandyKeys, &Ok(()));

        store.release();
        let status = store.record("ctrl+alt+d", ShortcutBackend::HandyKeys, &Ok(()));

        assert!(status.registered);
        assert_eq!(status.binding, "ctrl+alt+d");
        assert_eq!(store.get_or_not_tried().binding, "ctrl+alt+d");
    }

    #[test]
    fn a_failed_change_falls_back_to_the_previous_combination() {
        // The change fails and the previous combination is registered again:
        // the state has to say that the shortcut works with the old one, and
        // that nothing is in conflict any more.
        let store = CaptureShortcutStore::new();
        store.record("ctrl+alt+c", ShortcutBackend::Tauri, &Ok(()));

        store.release();
        let failed = store.record(
            "ctrl+alt+d",
            ShortcutBackend::Tauri,
            &Err("HotKey already registered: HotKey { modifiers: CTRL, key: KeyD }".to_string()),
        );
        assert!(!failed.registered);
        assert!(failed.conflicts_with_another_application);

        let restored = store.record("ctrl+alt+c", ShortcutBackend::Tauri, &Ok(()));

        assert!(restored.registered);
        assert_eq!(restored.binding, "ctrl+alt+c");
        assert!(
            !restored.conflicts_with_another_application,
            "the conflict is gone once the previous combination is back"
        );
    }
}
