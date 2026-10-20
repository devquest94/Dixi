//! Transforms: named, reusable LLM operations over a piece of text.
//!
//! A transform is a prompt with a name: "resumir", "traducir", "convertir en
//! lista"... The user picks one, the app sends the text to the provider with
//! the transform's instruction, and the answer replaces the text.
//!
//! Two kinds of transform live in the same table:
//!
//! * [`TransformKind::Builtin`] — the shipped catalogue. A built-in cannot be
//!   deleted or renamed. It can be disabled, and its provider, model and
//!   shortcut can be configured, because those are routing decisions and not
//!   part of the catalogue.
//! * [`TransformKind::Custom`] — written by the user. It can be edited and
//!   deleted.
//!
//! The manager keeps an in-memory snapshot because the UI asks for the whole
//! catalogue every time it opens, and because the hot path (running a
//! transform) must not read the disk.
//!
//! The catalogue is seeded the first time the manager opens the database, and
//! the seeding is idempotent: it inserts the built-ins that are missing and
//! never touches the ones that are already there, so a built-in the user
//! disabled stays disabled.
//!
//! Running a transform never destroys the text: when the provider fails, the
//! quota is exhausted, the connection drops or the answer comes back empty,
//! the outcome carries the original text and the reason. The reasoning of the
//! model is separated by the LLM client, so this module does not parse the
//! answer: it uses it as it arrives.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;

use super::personal_store::PersonalStore;
use crate::llm_client::{send_chat_completion, send_chat_completion_with_schema};
use crate::settings::{PostProcessApiFormat, PostProcessProvider};

/// What a transform is: part of the shipped catalogue, or written by the user.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransformKind {
    /// Shipped with the app. Can be disabled, never deleted or renamed.
    Builtin,
    /// Written by the user. Can be edited and deleted.
    Custom,
}

/// A stored transform.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Transform {
    pub id: String,
    pub name: String,
    pub kind: TransformKind,
    /// The instruction sent to the model.
    pub prompt: String,
    /// The provider this transform prefers. `None` means "the provider the
    /// caller passes".
    pub provider_id: Option<String>,
    /// The model this transform pins. `None` means
    /// [`DEFAULT_TRANSFORM_MODEL`].
    pub model: Option<String>,
    pub shortcut: Option<String>,
    pub category: Option<String>,
    pub enabled: bool,
    pub usage_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The writable fields of a transform.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TransformInput {
    pub name: String,
    pub prompt: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub shortcut: Option<String>,
    pub category: Option<String>,
    pub enabled: bool,
}

/// The result of running a transform.
///
/// On any failure the text is the original one: a transform that cannot run
/// never eats the user's words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TransformOutcome {
    /// The result, or the original text when the transform failed.
    pub text: String,
    /// Whether the text was actually modified.
    pub changed: bool,
    /// Why it failed, if it failed.
    pub error: Option<String>,
}

/// The model used when the transform does not pin one.
///
/// The wiring layer resolves the provider's configured model from the
/// settings and stores it on the transform; this constant is the fallback for
/// a transform that has no model of its own.
pub const DEFAULT_TRANSFORM_MODEL: &str = "gpt-4o-mini";

/// The role the model plays in every transformation.
///
/// It is in Spanish because it is sent to the model, and the app targets
/// Argentine Spanish.
const TRANSFORM_ROLE: &str = "Sos un asistente de redacción. Tu única tarea es \
aplicar la instrucción al texto que recibís y devolver únicamente el texto \
resultante, sin explicaciones, sin prefijos y sin comillas.";

/// The shipped catalogue: id, display name, category and the instruction sent
/// to the model.
///
/// The instructions are written in Spanish (Argentine Spanish, "vos" forms)
/// because they are sent to the model verbatim, and the app is used by
/// Spanish speakers.
const BUILTIN_TRANSFORMS: &[(&str, &str, &str, &str)] = &[
    (
        "mejorar-redaccion",
        "Mejorar la redacción",
        "Redacción",
        "Mejorá la redacción del texto: hacéla más clara, más fácil de leer y mejor \
hilada. Conservá el significado, el idioma, los nombres, las cifras y los datos tal \
como están. No agregues información nueva, no la elimines y no escribas frases de \
relleno como «a continuación».",
    ),
    (
        "corregir-ortografia",
        "Corregir ortografía y gramática",
        "Redacción",
        "Corregí la ortografía, la gramática y la puntuación del texto. No cambies el \
estilo, el contenido ni el significado, y no agregues ni quites información: \
corregí solamente los errores.",
    ),
    (
        "resumir",
        "Resumir",
        "Resumen",
        "Resumí el texto en unas pocas oraciones (entre dos y cinco). Conservá los \
datos, las cifras, los nombres y las fechas importantes. Devolvé solo el resumen, sin \
explicaciones ni comentarios.",
    ),
    (
        "acortar",
        "Acortar",
        "Resumen",
        "Acortá el texto sin perder su significado. Conservá todos los datos \
concretos, las cifras, los nombres y las tareas pendientes. Preferí frases más \
directas y eliminá la redundancia, sin dejar afuera información importante.",
    ),
    (
        "expandir",
        "Expandir",
        "Resumen",
        "Expandí el texto con más detalle y contexto, manteniendo el mismo idioma. \
Podés explicarle al lector cada punto, pero no inventes datos, cifras, fechas ni \
hechos que no estén en el texto original.",
    ),
    (
        "formalizar",
        "Formalizar",
        "Estilo",
        "Reescribí el texto en un registro formal: profesional, cuidado y correcto. \
Mantené el contenido y los datos intactos y quitá las coloquialidades.",
    ),
    (
        "casual",
        "Casual",
        "Estilo",
        "Reescribí el texto en un registro casual y natural, como en una conversación. \
Mantené el contenido y los datos intactos.",
    ),
    (
        "convertir-en-lista",
        "Convertir en lista",
        "Formato",
        "Convertí el texto en una lista con viñetas («- »), un elemento por idea. \
Mantené el orden del texto original y no cambies el contenido.",
    ),
    (
        "traducir",
        "Traducir al inglés",
        "Idioma",
        "Traducí el texto al inglés. Conservá el formato, la estructura y el tono del \
original, y no agregues ni quites información.",
    ),
    (
        "explicar",
        "Explicar",
        "Estilo",
        "Explicá el texto con palabras sencillas, como si le hablaras a alguien que no \
es técnico. Conservá correctos los datos y las cifras, y no inventes información \
nueva.",
    ),
    (
        "eliminar-muletillas",
        "Eliminar muletillas",
        "Redacción",
        "Eliminá del texto las muletillas, las repeticiones y las palabras de relleno \
(por ejemplo «eh», «o sea», «bueno», «mirá», «che»), sin cambiar el significado ni los \
datos.",
    ),
    (
        "convertir-en-tabla",
        "Convertir en tabla",
        "Formato",
        "Convertí el texto en una tabla de markdown, con encabezados y filas, \
solamente si el contenido tiene datos que se puedan ordenar en columnas. Si el \
contenido no se puede ordenar en una tabla, devolvé el texto original sin cambios.",
    ),
    (
        "extraer-tareas",
        "Extraer tareas",
        "Acciones",
        "Extraé del texto las tareas o acciones pendientes y devolvelas como una lista \
con viñetas. De cada tarea indicá qué hay que hacer y, si el texto lo dice, quién la \
tiene a cargo y para cuándo. Si no hay tareas, devolvé «No hay tareas pendientes».",
    ),
    (
        "titulo",
        "Título",
        "Formato",
        "Escribí un título breve y descriptivo para el texto, de unas diez palabras \
como máximo. Devolvé solo el título, sin comillas ni explicaciones.",
    ),
];

/// Owns the transforms and the in-memory snapshot of them.
pub struct TransformManager {
    store: PersonalStore,
    transforms: RwLock<Vec<Transform>>,
}

impl TransformManager {
    /// Opens (or creates) the personal database, seeds the catalogue and
    /// loads the snapshot.
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        Self::with_store(PersonalStore::new(app)?)
    }

    /// Opens (or creates) the personal database at an explicit path.
    ///
    /// The path-based constructor is the seam that keeps the manager
    /// independent of the Tauri application handle, and it is what lets the
    /// tests work on an isolated database.
    pub fn with_db_path(db_path: PathBuf) -> Result<Self, String> {
        Self::with_store(PersonalStore::with_db_path(db_path)?)
    }

    /// Uses an already opened personal database.
    fn with_store(store: PersonalStore) -> Result<Self, String> {
        let manager = Self {
            store,
            transforms: RwLock::new(Vec::new()),
        };
        manager.seed_catalogue()?;
        manager.reload_snapshot()?;
        Ok(manager)
    }

    fn connection(&self) -> Result<Connection, String> {
        self.store.connection()
    }

    /// The built-in catalogue, seeded on first use.
    pub fn builtin_catalogue() -> Vec<Transform> {
        BUILTIN_TRANSFORMS
            .iter()
            .map(|(id, name, category, prompt)| Transform {
                id: id.to_string(),
                name: name.to_string(),
                kind: TransformKind::Builtin,
                prompt: prompt.to_string(),
                provider_id: None,
                model: None,
                shortcut: None,
                category: Some(category.to_string()),
                enabled: true,
                usage_count: 0,
                created_at: 0,
                updated_at: 0,
            })
            .collect()
    }

    /// Every transform: the built-ins in catalogue order, then the custom ones
    /// by name.
    pub fn list(&self) -> Vec<Transform> {
        self.transforms
            .read()
            .map(|transforms| transforms.clone())
            .unwrap_or_default()
    }

    /// The stored transform with the given id.
    pub fn get(&self, id: &str) -> Option<Transform> {
        self.transforms
            .read()
            .ok()
            .and_then(|transforms| transforms.iter().find(|t| t.id == id).cloned())
    }

    /// Adds a custom transform and returns the stored version.
    pub fn add(&self, input: TransformInput) -> Result<Transform, String> {
        self.insert_custom(&generate_custom_id(), input)
    }

    /// Updates the writable fields of a transform.
    ///
    /// A built-in cannot be renamed and its instruction cannot be changed:
    /// both are part of the shipped catalogue. Its provider, model, shortcut
    /// and category can be configured, because those are routing decisions.
    pub fn update(&self, id: &str, input: TransformInput) -> Result<(), String> {
        let existing = self
            .get(id)
            .ok_or_else(|| format!("Transform '{id}' not found"))?;
        let normalized = normalize_input(input)?;

        if existing.kind == TransformKind::Builtin {
            if normalized.name != existing.name {
                return Err(format!(
                    "The built-in transform '{id}' cannot be renamed; create a custom \
transform instead"
                ));
            }
            if normalized.prompt != existing.prompt {
                return Err(format!(
                    "The built-in transform '{id}' cannot be edited; create a custom \
transform instead"
                ));
            }
        }

        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE transforms
                    SET name = ?1, prompt = ?2, provider_id = ?3, model = ?4,
                        shortcut = ?5, category = ?6, enabled = ?7, updated_at = ?8
                  WHERE id = ?9",
                params![
                    normalized.name,
                    normalized.prompt,
                    normalized.provider_id,
                    normalized.model,
                    normalized.shortcut,
                    normalized.category,
                    normalized.enabled as i64,
                    now,
                    id,
                ],
            )
            .map_err(|e| format!("Failed to update the transform: {e}"))?;

        if changed == 0 {
            return Err(format!("Transform '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Deletes a custom transform.
    ///
    /// A built-in cannot be deleted: it belongs to the shipped catalogue and
    /// can only be disabled.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let existing = self
            .get(id)
            .ok_or_else(|| format!("Transform '{id}' not found"))?;
        if existing.kind == TransformKind::Builtin {
            return Err(format!(
                "The built-in transform '{id}' cannot be deleted; disable it instead"
            ));
        }

        let conn = self.connection()?;
        let changed = conn
            .execute("DELETE FROM transforms WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete the transform: {e}"))?;

        if changed == 0 {
            return Err(format!("Transform '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Enables or disables a transform.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE transforms SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![enabled as i64, chrono::Utc::now().timestamp_millis(), id],
            )
            .map_err(|e| format!("Failed to update the transform: {e}"))?;

        if changed == 0 {
            return Err(format!("Transform '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Records that the transform was used.
    pub fn record_use(&self, id: &str) -> Result<(), String> {
        let conn = self.connection()?;
        let changed = conn
            .execute(
                "UPDATE transforms SET usage_count = usage_count + 1 WHERE id = ?1",
                params![id],
            )
            .map_err(|e| format!("Failed to record the use of the transform: {e}"))?;

        if changed == 0 {
            return Err(format!("Transform '{id}' not found"));
        }
        self.reload_snapshot()
    }

    /// Runs a transform over a piece of text.
    ///
    /// The provider is the one the caller resolved: a transform that pins a
    /// `provider_id` is meant to be run with that provider, and a transform
    /// without one runs with whatever provider the caller passes. The model is
    /// the one the transform pins, or [`DEFAULT_TRANSFORM_MODEL`].
    ///
    /// The original text is preserved on every failure: when the provider
    /// errors, the quota is exhausted, the connection drops or the answer
    /// comes back empty, the outcome carries the original text and the reason.
    pub async fn run(
        &self,
        transform: &Transform,
        text: &str,
        provider: &PostProcessProvider,
        api_key: &str,
        effort: Option<&str>,
    ) -> TransformOutcome {
        let original = text.to_string();

        // The counter counts the invocations, not the successes: a transform
        // that keeps failing is a transform the user keeps reaching for.
        if let Err(e) = self.record_use(&transform.id) {
            log::warn!(
                "Failed to record the use of the transform '{}': {e}",
                transform.id
            );
        }

        if original.trim().is_empty() {
            return TransformOutcome {
                text: original,
                changed: false,
                error: Some("There is no text to transform".to_string()),
            };
        }

        let instruction = transform.prompt.trim();
        if instruction.is_empty() {
            return TransformOutcome {
                text: original,
                changed: false,
                error: Some("The transform has no instruction".to_string()),
            };
        }

        let model = transform
            .model
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or(DEFAULT_TRANSFORM_MODEL);

        // The same providers that skip the reasoning in the post-processing
        // pipeline skip it here: a transformation is not a reasoning task.
        let disable_reasoning = matches!(provider.id.as_str(), "custom" | "openrouter");
        let system_prompt = build_system_prompt(instruction);

        let response = if provider.api_format == PostProcessApiFormat::Anthropic {
            // The Anthropic wire format carries the instruction and the text
            // in a single user message.
            send_chat_completion(
                provider,
                api_key.to_string(),
                model,
                format!("{system_prompt}\n\n{original}"),
                disable_reasoning,
                effort,
            )
            .await
        } else {
            send_chat_completion_with_schema(
                provider,
                api_key.to_string(),
                model,
                original.clone(),
                Some(system_prompt),
                None,
                disable_reasoning,
                effort,
            )
            .await
        };

        match response {
            Ok(content) => outcome_from_answer(content, &original),
            Err(e) => {
                log::warn!("The transform '{}' failed: {e}", transform.id);
                TransformOutcome {
                    text: original,
                    changed: false,
                    error: Some(e),
                }
            }
        }
    }

    /// Inserts a custom transform under a given id.
    ///
    /// [`TransformManager::add`] generates the id; this entry point keeps the
    /// rule "a custom transform cannot take the id of a built-in" in one
    /// place.
    fn insert_custom(&self, id: &str, input: TransformInput) -> Result<Transform, String> {
        if is_builtin_id(id) {
            return Err(format!(
                "'{id}' is the id of a built-in transform and cannot be used by a custom one"
            ));
        }

        let normalized = normalize_input(input)?;
        let now = chrono::Utc::now().timestamp_millis();

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO transforms
                (id, name, kind, prompt, provider_id, model, shortcut, category,
                 enabled, usage_count, created_at, updated_at)
             VALUES (?1, ?2, 'custom', ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9)",
            params![
                id,
                normalized.name,
                normalized.prompt,
                normalized.provider_id,
                normalized.model,
                normalized.shortcut,
                normalized.category,
                normalized.enabled as i64,
                now,
            ],
        )
        .map_err(|e| {
            if is_unique_violation(&e) {
                format!("A transform with the id '{id}' already exists")
            } else {
                format!("Failed to add the transform: {e}")
            }
        })?;

        self.reload_snapshot()?;
        self.get(id)
            .ok_or_else(|| "The transform was added but could not be read back".to_string())
    }

    /// Inserts the built-ins that are missing.
    ///
    /// The built-ins that are already stored are left untouched, so a built-in
    /// the user disabled stays disabled and a built-in the user configured
    /// keeps its provider and its model.
    fn seed_catalogue(&self) -> Result<(), String> {
        let conn = self.connection()?;
        let now = chrono::Utc::now().timestamp_millis();

        for (id, name, category, prompt) in BUILTIN_TRANSFORMS {
            let stored: Option<String> = conn
                .query_row(
                    "SELECT id FROM transforms WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| format!("Failed to read the transform '{id}': {e}"))?;
            if stored.is_some() {
                continue;
            }

            conn.execute(
                "INSERT INTO transforms
                    (id, name, kind, prompt, provider_id, model, shortcut, category,
                     enabled, usage_count, created_at, updated_at)
                 VALUES (?1, ?2, 'builtin', ?3, NULL, NULL, NULL, ?4, 1, 0, ?5, ?5)",
                params![id, name, prompt, category, now],
            )
            .map_err(|e| format!("Failed to seed the transform '{id}': {e}"))?;
        }
        Ok(())
    }

    /// Re-reads every transform from disk into the snapshot.
    fn reload_snapshot(&self) -> Result<(), String> {
        let conn = self.connection()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, kind, prompt, provider_id, model, shortcut, category,
                        enabled, usage_count, created_at, updated_at
                 FROM transforms",
            )
            .map_err(|e| format!("Failed to prepare the transforms query: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let kind_text: String = row.get("kind")?;
                Ok(Transform {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    kind: parse_kind(&kind_text),
                    prompt: row.get("prompt")?,
                    provider_id: row.get("provider_id")?,
                    model: row.get("model")?,
                    shortcut: row.get("shortcut")?,
                    category: row.get("category")?,
                    enabled: row.get::<_, i64>("enabled")? != 0,
                    usage_count: row.get("usage_count")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })
            .map_err(|e| format!("Failed to read the transforms: {e}"))?;

        let mut transforms = Vec::new();
        for row in rows {
            transforms.push(row.map_err(|e| format!("Failed to read a transform: {e}"))?);
        }
        sort_transforms(&mut transforms);

        *self
            .transforms
            .write()
            .map_err(|_| "Transforms lock poisoned".to_string())? = transforms;
        Ok(())
    }
}

/// Builds the system prompt of a transformation.
fn build_system_prompt(instruction: &str) -> String {
    format!("{TRANSFORM_ROLE}\n\nInstrucción: {instruction}")
}

/// Turns the answer of the provider into an outcome.
///
/// The answer is used as it arrives: the reasoning is already separated by
/// the LLM client, so this module must not add a second parsing layer that
/// could mangle the text. An empty answer is a failure, not a result.
fn outcome_from_answer(content: Option<String>, original: &str) -> TransformOutcome {
    let Some(content) = content else {
        return TransformOutcome {
            text: original.to_string(),
            changed: false,
            error: Some("The provider returned no content".to_string()),
        };
    };

    let result = content.trim();
    if result.is_empty() {
        return TransformOutcome {
            text: original.to_string(),
            changed: false,
            error: Some("The provider returned an empty response".to_string()),
        };
    }

    TransformOutcome {
        text: result.to_string(),
        changed: result != original,
        error: None,
    }
}

/// Sorts the built-ins first, in catalogue order, and the custom ones by
/// name.
fn sort_transforms(transforms: &mut [Transform]) {
    transforms.sort_by(|a, b| {
        let rank = |transform: &Transform| match transform.kind {
            TransformKind::Builtin => (0u8, builtin_rank(&transform.id).unwrap_or(usize::MAX)),
            TransformKind::Custom => (1u8, 0usize),
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// The position of a built-in in the catalogue.
fn builtin_rank(id: &str) -> Option<usize> {
    BUILTIN_TRANSFORMS
        .iter()
        .position(|(builtin_id, _, _, _)| *builtin_id == id)
}

/// Whether the id belongs to the shipped catalogue.
fn is_builtin_id(id: &str) -> bool {
    builtin_rank(id).is_some()
}

/// Reads the kind of a stored row.
fn parse_kind(text: &str) -> TransformKind {
    match text {
        "builtin" => TransformKind::Builtin,
        _ => TransformKind::Custom,
    }
}

/// Trims and validates the writable fields of a transform.
fn normalize_input(input: TransformInput) -> Result<TransformInput, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("The transform needs a name".to_string());
    }

    let prompt = input.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("The transform needs an instruction".to_string());
    }

    let optional = |value: Option<String>| {
        value
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };

    Ok(TransformInput {
        name,
        prompt,
        provider_id: optional(input.provider_id),
        model: optional(input.model),
        shortcut: optional(input.shortcut),
        category: optional(input.category),
        enabled: input.enabled,
    })
}

/// A unique id for a custom transform.
///
/// The app has no UUID dependency: a timestamp plus a per-process counter is
/// enough to keep the primary key unique.
fn generate_custom_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("custom-{}-{}", chrono::Utc::now().timestamp_millis(), sequence)
}

/// Whether the error is a primary key conflict.
fn is_unique_violation(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message)) => {
            message.contains("UNIQUE constraint failed")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
