//! Fachada de Dixi: una responsabilidad, un modulo.
//!
//! El nucleo heredado de Handy se conserva intacto y trabaja; esta capa solo
//! **organiza** el acceso por responsabilidad, tal como define el alcance de
//! la Fase 2A:
//!
//! | Responsabilidad | Modulo Dixi | Implementacion heredada |
//! |---|---|---|
//! | `audio` | [`audio`] | `managers::audio` + `audio_toolkit::vad` |
//! | `asr` | [`asr`] | **nuevo**: abstraccion de proveedores |
//! | `dictation` | [`dictation`] | **nuevo**: maquina de estados |
//! | `hotkeys` | [`hotkeys`] | `shortcut` + `settings::ShortcutBinding` |
//! | `paste` | [`paste`] | `paste_tx` + `clipboard` |
//! | `storage` | [`storage`] | `managers::history` (SQLite + migraciones) |
//! | `settings` | [`settings`] | `settings` |
//! | `ui` | [`ui`] | `tray` + `overlay` |
//!
//! No se reimplementa nada: cada modulo reexporta la implementacion que ya
//! funciona. Los dos modulos **nuevos** (`asr` y `dictation`) son codigo
//! propio de Dixi y concentran la logica que la auditoria identifico como
//! mas fragil: el cancelacion y los errores recuperables.

pub mod asr;
pub mod dictation;
pub mod evaluation;

/// Captura de microfono y control de grabacion.
pub mod audio {
    pub use crate::audio_toolkit::vad::{SpeechScorer, VoiceActivityDetector};
    pub use crate::managers::audio::{
        AudioRecordingManager, MicrophoneMode, RecordingReadiness, RecordingState,
    };
}

/// Atajos globales.
///
/// `handy_keys` es el crate de terceros que implementa la captura global; su
/// nombre se conserva por procedencia (ver `NOTICE.md`).
pub mod hotkeys {
    pub use crate::settings::ShortcutBinding;
    pub use crate::shortcut::{
        init_shortcuts, register_cancel_shortcut, register_shortcut, unregister_cancel_shortcut,
        unregister_shortcut,
    };
}

/// Inserccion de texto en la aplicacion enfocada y manejo del portapapeles.
pub mod paste {
    pub use crate::clipboard::paste;
}

/// Persistencia local: SQLite y migraciones.
pub mod storage {
    pub use crate::managers::history::{HistoryEntry, HistoryManager, PaginatedHistory};
}

/// Configuracion local.
pub mod settings {
    pub use crate::settings::{PasteMethod, ShortcutBinding, ShortcutActivation};
}

/// Interfaz: bandeja del sistema, overlay y barra flotante.
pub mod ui {
    pub use crate::overlay::{
        create_recording_overlay, emit_levels, emit_recording_ready, hide_recording_overlay,
        show_processing_overlay, show_recording_overlay, show_transcribing_overlay,
    };
    pub use crate::tray::{
        get_current_theme, get_icon_path, set_tray_state, set_tray_visibility, sync_tray,
        tray_tooltip, AppTheme, TrayIconState, TrayState,
    };
}
