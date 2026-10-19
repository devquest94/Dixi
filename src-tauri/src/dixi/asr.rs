//! Abstraccion de proveedores de reconocimiento de voz.
//!
//! Dixi separa **que** se transcribe de **quien** lo transcribe. El motor
//! local (`transcribe-cpp` + GGUF) es el proveedor por defecto; la misma
//! interfaz permite incorporate despues un proveedor remoto (MiniMax
//! `asr-1.0`) sin tocar la orquestacion del dictado.
//!
//! Reglas heredadas de la investigacion de MiniMax (doc 15) que se aplican
//! aqui desde el principio:
//! - El proveedor local **debe funcionar sin credenciales**: la integracion
//!   remota es opcional y explicita.
//! - La cancelacion se comprueba durante la transcripcion, no despues.
//! - Los errores se clasifican por tipo (`AsrError`) para que la UI pueda
//!   distinguir "recupera y reintenta" de "pide intervencion al usuario".
//!
//! En esta fase **no se realiza ninguna llamada de red**: solo se define el
//! contrato y el proveedor local.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Token de cancelacion cooperativo.
///
/// Se comprueba entre etapas del pipeline (y por el propio motor) para poder
/// abortar una transcripcion en curso sin matar el proceso.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// Token nuevo, sin cancelar.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marca el token como cancelado.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// `true` si se solicito la cancelacion.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Peticion de transcripcion.
///
/// El audio es PCM mono f32 a 16 kHz, el unico formato que aceptan tanto el
/// motor local como el endpoint de ASR remoto.
#[derive(Debug, Clone)]
pub struct TranscribeRequest {
    /// Muestras PCM mono f32 a 16 000 Hz.
    pub samples: Vec<f32>,
    /// Idioma forzado (`"es"`, `"en"`, ...). `None` deja que el modelo lo
    /// detecte.
    pub language: Option<String>,
    /// Texto de contexto para el decoding.
    pub prompt: Option<String>,
    /// Terminos propios que el modelo debe priorizar.
    ///
    /// Es el punto de extension del diccionario personalized (P0 en la
    /// matriz de paridad, todavia no implementado): el motor local ya
    /// acepta vocabulario, de modo que conectarlo no exige cambiar el
    /// pipeline.
    pub vocabulary: Vec<String>,
}

impl TranscribeRequest {
    /// Construye una peticion a partir de muestras PCM.
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            language: None,
            prompt: None,
            vocabulary: Vec::new(),
        }
    }

    /// Fija el idioma de la transcripcion.
    pub fn with_language(mut self, language: Option<String>) -> Self {
        self.language = language;
        self
    }

    /// Fija el texto de contexto.
    pub fn with_prompt(mut self, prompt: Option<String>) -> Self {
        self.prompt = prompt;
        self
    }

    /// Fija el vocabulario prioritario.
    pub fn with_vocabulary(mut self, vocabulary: Vec<String>) -> Self {
        self.vocabulary = vocabulary;
        self
    }

    /// Duracion del audio en milisegundos.
    pub fn duration_ms(&self) -> u64 {
        // 16 000 muestras por segundo.
        (self.samples.len() as u64 * 1000) / 16_000
    }
}

/// Segmento con marca de tiempo de la transcripcion.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptSegment {
    /// Texto del segmento.
    pub text: String,
    /// Inicio del segmento en milisegundos.
    pub start_ms: u64,
    /// Fin del segmento en milisegundos.
    pub end_ms: u64,
}

/// Resultado de una transcripcion.
#[derive(Debug, Clone)]
pub struct TranscribeResult {
    /// Texto transcrito.
    pub text: String,
    /// Idioma detectado, si el motor lo informa.
    pub language: Option<String>,
    /// Segmentos con marca de tiempo, si el motor los produce.
    pub segments: Vec<TranscriptSegment>,
    /// Id del proveedor que produjo el resultado.
    pub provider: String,
    /// Modelo usado.
    pub model: String,
    /// Tiempo de transcripcion medido.
    pub elapsed: Duration,
}

impl TranscribeResult {
    /// Resultado simple, sin segmentos ni idioma, para proveedores minimos.
    pub fn simple(text: String, provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            text,
            language: None,
            segments: Vec::new(),
            provider: provider.into(),
            model: model.into(),
            elapsed: Duration::ZERO,
        }
    }
}

/// Error de transcripcion, clasificado por su recuperabilidad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsrError {
    /// El audio es demasiado corto para transcribir.
    AudioTooShort { duration_ms: u64 },
    /// No hay ningun modelo cargado.
    ModelNotLoaded,
    /// La operacion fue cancelada.
    Cancelled,
    /// El motor fallo (modelo invalido, fichero corrupto, error interno).
    Engine(String),
    /// El proveedor remoto rechazo la peticion.
    Provider { kind: ProviderErrorKind, message: String },
    /// El idioma solicitado no lo admite el modelo.
    UnsupportedLanguage(String),
}

/// Clasificacion de un error de proveedor remoto.
///
/// Se separa de `AsrError::Provider` porque las reglas de reintento son
/// distintas: un `Auth` no se reintenta, un `RateLimit` si.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorKind {
    /// Credenciales invalidas o ausentes.
    Auth,
    /// Limite de peticiones alcanzado.
    RateLimit,
    /// Cuota del plan agotada.
    PlanQuota,
    /// Saldo insuficiente.
    Billing,
    /// Desbordamiento de contexto.
    ContextOverflow,
    /// Filtro de contenido.
    ContentFilter,
    /// Fallo de red.
    Network,
    /// Error no clasificado.
    Unknown,
}

impl ProviderErrorKind {
    /// `true` si reintentar el mismo request tiene sentido.
    ///
    /// La investigacion de MiniMax es explicita: `2056` (cuota), `1004`
    /// (clave invalida), `1026/1027` (filtro) y `402` (saldo) **no** se
    /// reintentan.
    pub fn is_retryable(&self) -> bool {
        matches!(self, ProviderErrorKind::RateLimit | ProviderErrorKind::Network)
    }
}

impl fmt::Display for AsrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsrError::AudioTooShort { duration_ms } => {
                write!(f, "audio demasiado corto ({duration_ms} ms)")
            }
            AsrError::ModelNotLoaded => f.write_str("no hay ningun modelo ASR cargado"),
            AsrError::Cancelled => f.write_str("transcripcion cancelada"),
            AsrError::Engine(m) => write!(f, "fallo del motor de ASR: {m}"),
            AsrError::Provider { kind, message } => {
                write!(f, "error del proveedor ({kind:?}): {message}")
            }
            AsrError::UnsupportedLanguage(l) => write!(f, "idioma no soportado: {l}"),
        }
    }
}

impl std::error::Error for AsrError {}

/// Proveedor de reconocimiento de voz.
///
/// Implementaciones: `LocalAsrProvider` (motor local GGUF) y, en fases
/// posteriores, un proveedor remoto compatible con la API de MiniMax.
pub trait AsrProvider: Send + Sync {
    /// Id estable del proveedor (`"local"`, `"minimax"`, ...).
    fn id(&self) -> &str;

    /// Nombre visible para la UI.
    fn display_name(&self) -> &str;

    /// Transcribe el audio. Debe consultar `cancel` durante el trabajo para
    /// poder abortar.
    fn transcribe(
        &self,
        request: &TranscribeRequest,
        cancel: &CancelToken,
    ) -> Result<TranscribeResult, AsrError>;
}

/// Motor de transcripcion local basado en `transcribe-cpp`.
///
/// El modelo se carga una vez y se reutiliza entre dictados. `unload` libera
/// la memoria del modelo de forma explicita, con el mismo proposito que el
/// `FinishGuard` de Handy: no dejar el pico de RAM del modelo residiendo en
/// el proceso entre dictados.
///
/// La cancelacion **en vuelo** de una transcripcion local la resuelve el
/// `EngineSupervisor` de la aplicacion (que trabaja con un worker aislado);
/// este provider comprueba el token antes y despues de la llamada, que es
/// suficiente para el contrato de la abstraccion.
pub struct LocalAsrProvider {
    model: std::sync::Mutex<Option<(transcribe_cpp::Model, transcribe_cpp::Session)>>,
    model_path: std::path::PathBuf,
    model_id: String,
    gpu_allowed: bool,
}

impl LocalAsrProvider {
    /// Crea el proveedor apuntando a un fichero de modelo GGUF.
    pub fn new(model_path: impl Into<std::path::PathBuf>, model_id: impl Into<String>) -> Self {
        Self {
            model: std::sync::Mutex::new(None),
            model_path: model_path.into(),
            model_id: model_id.into(),
            gpu_allowed: false,
        }
    }

    /// Permite o prohibe el uso de la GPU. En esta fase el build es solo CPU.
    pub fn with_gpu(mut self, allowed: bool) -> Self {
        self.gpu_allowed = allowed;
        self
    }

    /// Ruta del modelo que este proveedor carga.
    pub fn model_path(&self) -> &std::path::Path {
        &self.model_path
    }

    /// Carga el modelo si aun no esta cargado.
    pub fn ensure_loaded(&self) -> Result<(), AsrError> {
        let mut guard = self
            .model
            .lock()
            .map_err(|_| AsrError::Engine("estado del motor envenenado".into()))?;
        if guard.is_some() {
            return Ok(());
        }
        if !self.model_path.exists() {
            return Err(AsrError::Engine(format!(
                "no existe el fichero de modelo: {}",
                self.model_path.display()
            )));
        }
        let options = transcribe_cpp::ModelOptions {
            backend: if self.gpu_allowed {
                transcribe_cpp::Backend::Auto
            } else {
                transcribe_cpp::Backend::Cpu
            },
            device: None,
        };
        let model = transcribe_cpp::Model::load_with(&self.model_path, &options)
            .map_err(|e| AsrError::Engine(format!("no se pudo cargar el modelo: {e}")))?;
        let session = model
            .session()
            .map_err(|e| AsrError::Engine(format!("no se pudo abrir la sesion: {e}")))?;
        *guard = Some((model, session));
        Ok(())
    }

    /// Libera la memoria del modelo.
    pub fn unload(&self) {
        if let Ok(mut guard) = self.model.lock() {
            *guard = None;
        }
    }

    /// `true` si hay un modelo cargado.
    pub fn is_loaded(&self) -> bool {
        self.model.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

impl AsrProvider for LocalAsrProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn display_name(&self) -> &str {
        "Local (GGUF)"
    }

    fn transcribe(
        &self,
        request: &TranscribeRequest,
        cancel: &CancelToken,
    ) -> Result<TranscribeResult, AsrError> {
        if cancel.is_cancelled() {
            return Err(AsrError::Cancelled);
        }
        if request.samples.is_empty() {
            return Err(AsrError::AudioTooShort { duration_ms: 0 });
        }

        let started = std::time::Instant::now();
        let mut guard = self
            .model
            .lock()
            .map_err(|_| AsrError::Engine("estado del motor envenenado".into()))?;
        let (_model, session) = guard
            .as_mut()
            .ok_or(AsrError::ModelNotLoaded)?;

        let mut run = transcribe_cpp::RunOptions::default();
        run.language = request.language.clone();
        run.prompt = request.prompt.clone();
        run.vocabulary = request.vocabulary.clone();

        let transcript = session
            .run(&request.samples, &run)
            .map_err(|e| AsrError::Engine(format!("{e}")))?;

        if cancel.is_cancelled() {
            return Err(AsrError::Cancelled);
        }

        let segments = transcript
            .segments
            .iter()
            .map(|s| TranscriptSegment {
                text: s.text.clone(),
                start_ms: s.t0_ms.max(0) as u64,
                end_ms: s.t1_ms.max(0) as u64,
            })
            .collect();

        Ok(TranscribeResult {
            text: transcript.text,
            language: transcript.language,
            segments,
            provider: "local".into(),
            model: self.model_id.clone(),
            elapsed: started.elapsed(),
        })
    }
}

/// Proveedor de pruebas: devuelve una transcripcion fija o un error.
///
/// Permite ejercitar la orquestacion del dictado (y sus tests) sin modelo,
/// sin microfono y sin disco.
pub struct StaticAsrProvider {
    id: String,
    display_name: String,
    behaviour: Box<dyn Fn(&TranscribeRequest) -> Result<TranscribeResult, AsrError> + Send + Sync>,
}

impl StaticAsrProvider {
    /// Proveedor que siempre devuelve el mismo texto.
    pub fn fixed(id: &str, text: &str) -> Self {
        let text = text.to_string();
        Self {
            id: id.to_string(),
            display_name: id.to_string(),
            behaviour: Box::new(move |_| Ok(TranscribeResult::simple(text.clone(), id, "static"))),
        }
    }

    /// Proveedor que siempre falla con el error indicado.
    pub fn failing(id: &str, error: AsrError) -> Self {
        Self {
            id: id.to_string(),
            display_name: id.to_string(),
            behaviour: Box::new(move |_| Err(error.clone())),
        }
    }

    /// Proveedor con comportamiento arbitrario.
    pub fn custom(
        id: &str,
        behaviour: impl Fn(&TranscribeRequest) -> Result<TranscribeResult, AsrError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            id: id.to_string(),
            display_name: id.to_string(),
            behaviour: Box::new(behaviour),
        }
    }
}

impl AsrProvider for StaticAsrProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn transcribe(
        &self,
        request: &TranscribeRequest,
        cancel: &CancelToken,
    ) -> Result<TranscribeResult, AsrError> {
        if cancel.is_cancelled() {
            return Err(AsrError::Cancelled);
        }
        (self.behaviour)(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_token_de_cancelacion_es_coherente() {
        let t = CancelToken::new();
        assert!(!t.is_cancelled());
        t.cancel();
        assert!(t.is_cancelled());

        // Una copia comparte el estado.
        let c = t.clone();
        assert!(c.is_cancelled());
    }

    #[test]
    fn la_duracion_se_calcula_sobre_16_khz() {
        let req = TranscribeRequest::new(vec![0.0; 16_000]);
        assert_eq!(req.duration_ms(), 1000);
        let req = TranscribeRequest::new(vec![0.0; 8_000]);
        assert_eq!(req.duration_ms(), 500);
    }

    #[test]
    fn un_proveedor_estatico_devuelve_texto() {
        let p = StaticAsrProvider::fixed("test", "hola mundo");
        let r = p
            .transcribe(&TranscribeRequest::new(vec![0.0; 16_000]), &CancelToken::new())
            .unwrap();
        assert_eq!(r.text, "hola mundo");
        assert_eq!(r.provider, "test");
    }

    #[test]
    fn un_proveedor_estatico_propaga_el_error() {
        let p = StaticAsrProvider::failing("test", AsrError::ModelNotLoaded);
        let e = p
            .transcribe(&TranscribeRequest::new(vec![0.0; 16_000]), &CancelToken::new())
            .unwrap_err();
        assert_eq!(e, AsrError::ModelNotLoaded);
    }

    #[test]
    fn la_cancelacion_precede_a_la_transcripcion() {
        let p = StaticAsrProvider::fixed("test", "no deberia llegar aqui");
        let cancel = CancelToken::new();
        cancel.cancel();
        let e = p
            .transcribe(&TranscribeRequest::new(vec![0.0; 16_000]), &cancel)
            .unwrap_err();
        assert_eq!(e, AsrError::Cancelled);
    }

    #[test]
    fn el_proveedor_local_rechaza_audio_vacio() {
        let p = LocalAsrProvider::new("no-existe.gguf", "test");
        let e = p
            .transcribe(&TranscribeRequest::new(Vec::new()), &CancelToken::new())
            .unwrap_err();
        assert_eq!(e, AsrError::AudioTooShort { duration_ms: 0 });
    }

    #[test]
    fn el_proveedor_local_falla_si_el_modelo_no_existe() {
        let p = LocalAsrProvider::new("no-existe.gguf", "test");
        let e = p.ensure_loaded().unwrap_err();
        match e {
            AsrError::Engine(m) => assert!(m.contains("no existe el fichero")),
            other => panic!("error inesperado: {other:?}"),
        }
        assert!(!p.is_loaded());
    }

    #[test]
    fn el_proveedor_local_sin_modelo_cargado_falla() {
        let p = LocalAsrProvider::new("no-existe.gguf", "test");
        let e = p
            .transcribe(&TranscribeRequest::new(vec![0.0; 16_000]), &CancelToken::new())
            .unwrap_err();
        assert_eq!(e, AsrError::ModelNotLoaded);
    }

    #[test]
    fn solo_los_errores_transitorios_se_reintentan() {
        assert!(ProviderErrorKind::RateLimit.is_retryable());
        assert!(ProviderErrorKind::Network.is_retryable());

        // La investigacion de MiniMax es explicita: estos NO se reintentan.
        assert!(!ProviderErrorKind::Auth.is_retryable());
        assert!(!ProviderErrorKind::PlanQuota.is_retryable());
        assert!(!ProviderErrorKind::Billing.is_retryable());
        assert!(!ProviderErrorKind::ContentFilter.is_retryable());
        assert!(!ProviderErrorKind::ContextOverflow.is_retryable());
    }

    #[test]
    fn el_error_de_proveedor_conserva_tipo_y_mensaje() {
        let e = AsrError::Provider {
            kind: ProviderErrorKind::Auth,
            message: "clave invalida".into(),
        };
        let msg = e.to_string();
        assert!(msg.contains("Auth"), "mensaje: {msg}");
        assert!(msg.contains("clave invalida"), "mensaje: {msg}");
    }
}
