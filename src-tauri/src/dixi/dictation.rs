//! Máquina de estados del dictado de Dixi.
//!
//! Coordina el ciclo completo de un dictado y concentrate en las reglas que
//! la auditoría de Wispr Flowisolate como el punto fragile del pipeline:
//! transiciones validas, cancelacion en cualquier etapa, y errores
//! **recuperables** (un fallo de ASR no puede dejar la aplicacion colgada).
//!
//! Es codigo puro (sin Tauri, sin tokio) para que sus pruebas sean rapidas
//! y deterministas: el estado se puede ejercitar entero sin microfono, sin
//! modelo y sin escritorio.
//!
//! Reglas:
//! - Cada dictado tiene un **id de sesion** monotono. Un evento que llega con
//!   un id antiguo (p. ej. el final de una grabacion que ya fue cancelada)
//!   se rechaza en lugar de corromper el estado actual.
//! - La cancelacion es valida en `Recording`, `Transcribing` e `Inserting`.
//!   En `Idle` no hay nada que cancelar y se devuelve un error explicito.
//! - Todo error es **recuperable**: desde `Failed` se puede iniciar otro
//!   dictado sin reiniciar la aplicacion.

use std::fmt;
use std::time::Duration;

/// Fase del ciclo de dictado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationPhase {
    /// Sin dictado en curso.
    Idle,
    /// Capturando audio del microfono.
    Recording,
    /// El audio se esta transcribiendo.
    Transcribing,
    /// El texto se esta insertando en la aplicacion enfocada.
    Inserting,
    /// El ultimo dictado fue cancelado por el usuario.
    Cancelled,
    /// El ultimo dictado termino en error.
    Failed,
}

impl fmt::Display for DictationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DictationPhase::Idle => "idle",
            DictationPhase::Recording => "recording",
            DictationPhase::Transcribing => "transcribing",
            DictationPhase::Inserting => "inserting",
            DictationPhase::Cancelled => "cancelled",
            DictationPhase::Failed => "failed",
        };
        f.write_str(s)
    }
}

/// Motivo por el que una transicion no es valida.
///
/// Todos estos errores son recuperables: describen una transicion rechazada o
/// un fallo de una etapa, nunca un estado terminal de la aplicacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationError {
    /// Se intento detener una grabacion que no estaba en curso.
    NotRecording,
    /// Se intento iniciar una grabacion con otra en curso.
    AlreadyRecording,
    /// Se intento transcribir sin audio grabado.
    NotTranscribing,
    /// Se intento insertar sin transcripcion lista.
    NotInserting,
    /// El ASR devolvio una transcripcion vacia.
    EmptyTranscript,
    /// El evento pertenece a un dictado anterior.
    StaleSession { expected: u64, got: u64 },
    /// No hay nada que cancelar.
    NothingToCancel,
    /// Fallo de captura de audio.
    CaptureFailed(String),
    /// Fallo del motor de ASR.
    AsrFailed(String),
    /// Fallo al insertar el texto en la aplicacion enfocada.
    PasteFailed(String),
}

impl fmt::Display for DictationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DictationError::NotRecording => f.write_str("no hay grabacion en curso"),
            DictationError::AlreadyRecording => f.write_str("ya hay una grabacion en curso"),
            DictationError::NotTranscribing => f.write_str("no hay audio para transcribir"),
            DictationError::NotInserting => f.write_str("no hay transcripcion para insertar"),
            DictationError::EmptyTranscript => f.write_str("la transcripcion esta vacia"),
            DictationError::StaleSession { expected, got } => {
                write!(f, "evento de una sesion antigua (esperada {expected}, recibida {got})")
            }
            DictationError::NothingToCancel => f.write_str("no hay nada que cancelar"),
            DictationError::CaptureFailed(m) => write!(f, "fallo de captura: {m}"),
            DictationError::AsrFailed(m) => write!(f, "fallo de ASR: {m}"),
            DictationError::PasteFailed(m) => write!(f, "fallo de insercion: {m}"),
        }
    }
}

impl std::error::Error for DictationError {}

/// Estado del dictado, con las transiciones validas entre fases.
#[derive(Debug, Clone)]
pub struct DictationMachine {
    phase: DictationPhase,
    session: u64,
    audio: Duration,
    /// Texto pendiente de insertar (presente en `Inserting`).
    text: Option<String>,
    /// Ultimo texto insertado con exito. Se conserva tras cancelar o fallar.
    last_text: Option<String>,
    /// Ultimo error producido.
    last_error: Option<DictationError>,
}

impl Default for DictationMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl DictationMachine {
    /// Crea una maquina en reposo, sin dictados previos.
    pub fn new() -> Self {
        Self {
            phase: DictationPhase::Idle,
            session: 0,
            audio: Duration::ZERO,
            text: None,
            last_text: None,
            last_error: None,
        }
    }

    /// Fase actual.
    pub fn phase(&self) -> DictationPhase {
        self.phase
    }

    /// Id de la sesion en curso. Se incrementa en cada `start_recording`.
    pub fn session(&self) -> u64 {
        self.session
    }

    /// Duracion del audio capturado en el dictado en curso.
    pub fn audio_duration(&self) -> Duration {
        self.audio
    }

    /// Texto pendiente de insertar, si lo hay.
    pub fn pending_text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Ultimo texto insertado con exito. Permite recuperar el ultimo dictado.
    pub fn last_text(&self) -> Option<&str> {
        self.last_text.as_deref()
    }

    /// Ultimo error producido, si lo hubo.
    pub fn last_error(&self) -> Option<&DictationError> {
        self.last_error.as_ref()
    }

    /// `true` mientras hay un dictado en curso (captura, ASR o insercion).
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            DictationPhase::Recording | DictationPhase::Transcribing | DictationPhase::Inserting
        )
    }

    /// `true` si la fase actual admite cancelacion.
    pub fn is_cancellable(&self) -> bool {
        self.is_active()
    }

    /// Inicia una grabacion. Devuelve el id de la nueva sesion.
    ///
    /// Se admite desde `Idle`, `Cancelled` y `Failed`: un error previo nunca
    /// deja la aplicacion bloqueada.
    pub fn start_recording(&mut self) -> Result<u64, DictationError> {
        if self.phase == DictationPhase::Recording {
            return Err(DictationError::AlreadyRecording);
        }
        self.session = self.session.wrapping_add(1);
        self.phase = DictationPhase::Recording;
        self.audio = Duration::ZERO;
        self.text = None;
        self.last_error = None;
        Ok(self.session)
    }

    /// Cierra la grabacion y pasa a la fase de transcripcion.
    pub fn stop_recording(&mut self, audio: Duration) -> Result<(), DictationError> {
        if self.phase != DictationPhase::Recording {
            return Err(DictationError::NotRecording);
        }
        self.audio = audio;
        self.phase = DictationPhase::Transcribing;
        Ok(())
    }

    /// Acepta la transcripcion y pasa a la fase de insercion.
    pub fn transcript_ready(&mut self, text: String) -> Result<(), DictationError> {
        if self.phase != DictationPhase::Transcribing {
            return Err(DictationError::NotTranscribing);
        }
        if text.trim().is_empty() {
            return Err(DictationError::EmptyTranscript);
        }
        self.text = Some(text);
        self.phase = DictationPhase::Inserting;
        Ok(())
    }

    /// Confirma la insercion del texto. El texto pasa a ser el ultimo
    /// dictado y la maquina vuelve a reposo.
    pub fn inserted(&mut self) -> Result<(), DictationError> {
        if self.phase != DictationPhase::Inserting {
            return Err(DictationError::NotInserting);
        }
        if let Some(text) = self.text.take() {
            self.last_text = Some(text);
        }
        self.phase = DictationPhase::Idle;
        Ok(())
    }

    /// Cancela el dictado en curso.
    ///
    /// Se puede cancelar durante la captura, la transcripcion o la
    /// insercion. El texto pendiente se descarta, pero el ultimo texto
    /// insertado con exito se conserva.
    pub fn cancel(&mut self) -> Result<(), DictationError> {
        if !self.is_cancellable() {
            return Err(DictationError::NothingToCancel);
        }
        self.text = None;
        self.phase = DictationPhase::Cancelled;
        Ok(())
    }

    /// Registra un fallo de una etapa. La maquina queda en `Failed` y sigue
    /// siendo utilizable: `start_recording` vuelve a funcionar.
    pub fn fail(&mut self, error: DictationError) -> Result<(), DictationError> {
        self.text = None;
        self.last_error = Some(error);
        self.phase = DictationPhase::Failed;
        Ok(())
    }

    /// Vuelve a reposo tras una cancelacion o un error.
    pub fn reset(&mut self) -> Result<(), DictationError> {
        match self.phase {
            DictationPhase::Cancelled | DictationPhase::Failed => {
                self.phase = DictationPhase::Idle;
                self.text = None;
                Ok(())
            }
            _ => Err(DictationError::NothingToCancel),
        }
    }

    /// Comprueba que un evento pertenece a la sesion en curso.
    ///
    /// Un id antiguo significa que el evento corresponde a un dictado ya
    /// cancelado o ya cerrado: se ignora en lugar de alterar el estado.
    pub fn check_session(&self, session: u64) -> Result<(), DictationError> {
        if session == self.session {
            Ok(())
        } else {
            Err(DictationError::StaleSession {
                expected: self.session,
                got: session,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empieza_en_reposo() {
        let m = DictationMachine::new();
        assert_eq!(m.phase(), DictationPhase::Idle);
        assert_eq!(m.session(), 0);
        assert!(!m.is_active());
        assert!(m.last_text().is_none());
        assert!(m.last_error().is_none());
    }

    #[test]
    fn ciclo_completo_exitoso() {
        let mut m = DictationMachine::new();
        let s = m.start_recording().unwrap();
        assert_eq!(s, 1);
        assert_eq!(m.phase(), DictationPhase::Recording);
        assert!(m.is_active());

        m.stop_recording(Duration::from_millis(2500)).unwrap();
        assert_eq!(m.phase(), DictationPhase::Transcribing);
        assert_eq!(m.audio_duration(), Duration::from_millis(2500));

        m.transcript_ready("hola que tal".into()).unwrap();
        assert_eq!(m.phase(), DictationPhase::Inserting);
        assert_eq!(m.pending_text(), Some("hola que tal"));

        m.inserted().unwrap();
        assert_eq!(m.phase(), DictationPhase::Idle);
        assert_eq!(m.last_text(), Some("hola que tal"));
        assert!(m.pending_text().is_none());
        assert!(!m.is_active());
    }

    #[test]
    fn no_se_puede_detener_sin_grabacion() {
        let mut m = DictationMachine::new();
        assert_eq!(
            m.stop_recording(Duration::ZERO),
            Err(DictationError::NotRecording)
        );
    }

    #[test]
    fn no_se_puede_iniciar_dos_grabaciones() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        assert_eq!(m.start_recording(), Err(DictationError::AlreadyRecording));
    }

    #[test]
    fn no_se_puede_transcribir_sin_audio() {
        let mut m = DictationMachine::new();
        assert_eq!(
            m.transcript_ready("texto".into()),
            Err(DictationError::NotTranscribing)
        );
    }

    #[test]
    fn no_se_puede_insertar_sin_transcripcion() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        m.stop_recording(Duration::from_millis(100)).unwrap();
        assert_eq!(m.inserted(), Err(DictationError::NotInserting));
    }

    #[test]
    fn transcripcion_vacia_se_rechaza_y_no_avanza_de_fase() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        m.stop_recording(Duration::from_millis(100)).unwrap();
        assert_eq!(
            m.transcript_ready("   ".into()),
            Err(DictationError::EmptyTranscript)
        );
        // Sigue en transcribing: se puede reintentar con una transcripcion real.
        assert_eq!(m.phase(), DictationPhase::Transcribing);
    }

    #[test]
    fn cancelacion_en_cada_etapa_activa() {
        for etapa in 0..3 {
            let mut m = DictationMachine::new();
            m.start_recording().unwrap();
            if etapa >= 1 {
                m.stop_recording(Duration::from_millis(100)).unwrap();
            }
            if etapa >= 2 {
                m.transcript_ready("texto".into()).unwrap();
            }
            assert!(m.is_cancellable(), "etapa {etapa} deberia ser cancelable");
            m.cancel().unwrap();
            assert_eq!(m.phase(), DictationPhase::Cancelled);
            assert!(!m.is_active());
            assert!(m.pending_text().is_none());
        }
    }

    #[test]
    fn cancelar_en_reposo_es_error() {
        let mut m = DictationMachine::new();
        assert_eq!(m.cancel(), Err(DictationError::NothingToCancel));
        assert!(!m.is_cancellable());
    }

    #[test]
    fn cancelar_conserva_el_ultimo_texto() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        m.stop_recording(Duration::from_millis(100)).unwrap();
        m.transcript_ready("primer dictado".into()).unwrap();
        m.inserted().unwrap();

        m.start_recording().unwrap();
        m.cancel().unwrap();
        assert_eq!(m.last_text(), Some("primer dictado"));
    }

    #[test]
    fn error_es_recuperable() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        m.fail(DictationError::CaptureFailed("micronfono ocupado".into()))
            .unwrap();
        assert_eq!(m.phase(), DictationPhase::Failed);
        assert!(matches!(
            m.last_error(),
            Some(DictationError::CaptureFailed(_))
        ));

        // Se puede volver a dictar sin reiniciar.
        let s = m.start_recording().unwrap();
        assert_eq!(s, 2);
        assert_eq!(m.phase(), DictationPhase::Recording);
    }

    #[test]
    fn reset_desde_cancelled_y_failed() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        m.cancel().unwrap();
        assert_eq!(m.phase(), DictationPhase::Cancelled);
        m.reset().unwrap();
        assert_eq!(m.phase(), DictationPhase::Idle);

        m.start_recording().unwrap();
        m.fail(DictationError::AsrFailed("modelo no cargado".into())).unwrap();
        m.reset().unwrap();
        assert_eq!(m.phase(), DictationPhase::Idle);
    }

    #[test]
    fn reset_desde_reposo_o_activo_es_error() {
        let mut m = DictationMachine::new();
        assert!(m.reset().is_err());
        m.start_recording().unwrap();
        assert!(m.reset().is_err());
    }

    #[test]
    fn se_rechazan_eventos_de_una_sesion_antigua() {
        let mut m = DictationMachine::new();
        let s1 = m.start_recording().unwrap();
        m.cancel().unwrap();
        let s2 = m.start_recording().unwrap();
        assert_ne!(s1, s2);

        assert!(m.check_session(s2).is_ok());
        let err = m.check_session(s1).unwrap_err();
        assert_eq!(
            err,
            DictationError::StaleSession {
                expected: s2,
                got: s1
            }
        );
    }

    #[test]
    fn el_id_de_sesion_es_monotono() {
        let mut m = DictationMachine::new();
        let a = m.start_recording().unwrap();
        m.cancel().unwrap();
        let b = m.start_recording().unwrap();
        m.cancel().unwrap();
        let c = m.start_recording().unwrap();
        assert!(a < b && b < c);
    }

    #[test]
    fn el_fallo_de_asr_registra_el_mensaje() {
        let mut m = DictationMachine::new();
        m.start_recording().unwrap();
        m.stop_recording(Duration::from_millis(10)).unwrap();
        m.fail(DictationError::AsrFailed("modelo no encontrado".into()))
            .unwrap();
        let msg = m.last_error().unwrap().to_string();
        assert!(msg.contains("modelo no encontrado"), "mensaje: {msg}");
    }
}
