//! Tests for the transforms manager.
//!
//! The failure path is exercised without a network: the provider points at
//! `http://127.0.0.1:1`, a port nothing listens on, so the connection is
//! refused and the request never leaves the machine.

use super::*;
use tempfile::TempDir;

/// The ids of the shipped catalogue, in order.
const EXPECTED_BUILTIN_IDS: [&str; 14] = [
    "mejorar-redaccion",
    "corregir-ortografia",
    "resumir",
    "acortar",
    "expandir",
    "formalizar",
    "casual",
    "convertir-en-lista",
    "traducir",
    "explicar",
    "eliminar-muletillas",
    "convertir-en-tabla",
    "extraer-tareas",
    "titulo",
];

/// A manager over an isolated database.
///
/// The temporary folder is returned together with the manager because the
/// manager does not own it: the folder must outlive the manager.
fn manager() -> (TempDir, TransformManager) {
    let dir = TempDir::new().expect("a temporary folder");
    let manager = TransformManager::with_db_path(dir.path().join("transforms.db"))
        .expect("the manager opens");
    (dir, manager)
}

/// A provider whose endpoint refuses the connection.
///
/// Nothing listens on port 1, so the request fails at the transport level
/// without touching the network.
fn unreachable_provider() -> PostProcessProvider {
    PostProcessProvider {
        id: "unreachable".to_string(),
        label: "Unreachable".to_string(),
        base_url: "http://127.0.0.1:1".to_string(),
        allow_base_url_edit: true,
        models_endpoint: None,
        supports_structured_output: false,
        supports_reasoning_effort: false,
        api_format: crate::settings::PostProcessApiFormat::OpenAi,
    }
}

/// An input for a custom transform.
fn custom_input(name: &str, prompt: &str) -> TransformInput {
    TransformInput {
        name: name.to_string(),
        prompt: prompt.to_string(),
        provider_id: None,
        model: None,
        shortcut: None,
        category: None,
        enabled: true,
    }
}

#[test]
fn the_catalogue_has_the_fourteen_builtin_transforms() {
    let catalogue = TransformManager::builtin_catalogue();

    assert_eq!(catalogue.len(), 14);
    let ids: Vec<&str> = catalogue.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, EXPECTED_BUILTIN_IDS.to_vec());
    assert!(catalogue.iter().all(|t| t.kind == TransformKind::Builtin));
    assert!(catalogue.iter().all(|t| t.enabled));
    assert!(catalogue.iter().all(|t| !t.name.trim().is_empty()));
}

#[test]
fn every_builtin_instruction_is_a_spanish_sentence() {
    for transform in TransformManager::builtin_catalogue() {
        let prompt = transform.prompt.trim();
        assert!(!prompt.is_empty(), "{} has no instruction", transform.id);
        assert!(
            prompt.contains("texto"),
            "the instruction of {} is not written in Spanish: {prompt}",
            transform.id
        );
        assert_ne!(prompt, transform.name);
        assert!(
            prompt.split_whitespace().count() >= 5,
            "the instruction of {} is too short to be self-contained: {prompt}",
            transform.id
        );
    }
}

#[test]
fn the_catalogue_is_seeded_on_first_use() {
    let (_dir, manager) = manager();

    let transforms = manager.list();
    assert_eq!(transforms.len(), 14);
    assert!(transforms.iter().all(|t| t.kind == TransformKind::Builtin));

    // The catalogue order is preserved, so the UI shows the built-ins in the
    // order they are meant to be read.
    let ids: Vec<&str> = transforms.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, EXPECTED_BUILTIN_IDS.to_vec());
}

#[test]
fn seeding_the_catalogue_twice_does_not_duplicate_it() {
    let dir = TempDir::new().expect("a temporary folder");
    let path = dir.path().join("transforms.db");

    let first = TransformManager::with_db_path(path.clone()).expect("the manager opens");
    assert_eq!(first.list().len(), 14);
    drop(first);

    let second = TransformManager::with_db_path(path).expect("the manager reopens");
    assert_eq!(second.list().len(), 14);
}

#[test]
fn a_disabled_builtin_stays_disabled_after_reseeding() {
    let dir = TempDir::new().expect("a temporary folder");
    let path = dir.path().join("transforms.db");

    let manager = TransformManager::with_db_path(path.clone()).expect("the manager opens");
    manager
        .set_enabled("resumir", false)
        .expect("the transform is disabled");
    drop(manager);

    let manager = TransformManager::with_db_path(path).expect("the manager reopens");
    let resumir = manager.get("resumir").expect("the transform is still there");
    assert!(!resumir.enabled);
    assert_eq!(manager.list().len(), 14);
}

#[test]
fn reseeding_does_not_overwrite_a_configured_builtin() {
    let dir = TempDir::new().expect("a temporary folder");
    let path = dir.path().join("transforms.db");

    let manager = TransformManager::with_db_path(path.clone()).expect("the manager opens");
    let resumir = manager.get("resumir").expect("the built-in is there");
    manager
        .update(
            "resumir",
            TransformInput {
                name: resumir.name.clone(),
                prompt: resumir.prompt.clone(),
                provider_id: Some("minimax".to_string()),
                model: Some("MiniMax-M3".to_string()),
                shortcut: Some("Ctrl+Alt+R".to_string()),
                category: Some("Resumen".to_string()),
                enabled: true,
            },
        )
        .expect("the built-in is configured");
    drop(manager);

    let manager = TransformManager::with_db_path(path).expect("the manager reopens");
    let resumir = manager.get("resumir").expect("the built-in is still there");
    assert_eq!(resumir.provider_id.as_deref(), Some("minimax"));
    assert_eq!(resumir.model.as_deref(), Some("MiniMax-M3"));
    assert_eq!(resumir.shortcut.as_deref(), Some("Ctrl+Alt+R"));
}

#[test]
fn a_custom_transform_can_be_added_updated_and_deleted() {
    let (_dir, manager) = manager();

    let added = manager
        .add(TransformInput {
            name: "Mi transform".to_string(),
            prompt: "Hacé algo útil con el texto".to_string(),
            provider_id: Some("minimax".to_string()),
            model: Some("MiniMax-M3".to_string()),
            shortcut: Some("Ctrl+Alt+T".to_string()),
            category: Some("Custom".to_string()),
            enabled: true,
        })
        .expect("the transform is added");

    assert_eq!(added.kind, TransformKind::Custom);
    assert!(added.id.starts_with("custom-"));
    assert_eq!(added.usage_count, 0);
    assert_eq!(added.provider_id.as_deref(), Some("minimax"));
    assert_eq!(added.model.as_deref(), Some("MiniMax-M3"));
    assert_eq!(manager.list().len(), 15);

    manager
        .update(
            &added.id,
            TransformInput {
                name: "Mi transform editado".to_string(),
                prompt: "Hacé otra cosa con el texto".to_string(),
                provider_id: None,
                model: None,
                shortcut: None,
                category: None,
                enabled: false,
            },
        )
        .expect("the transform is updated");

    let updated = manager.get(&added.id).expect("the transform is still there");
    assert_eq!(updated.name, "Mi transform editado");
    assert_eq!(updated.prompt, "Hacé otra cosa con el texto");
    assert!(!updated.enabled);
    assert_eq!(updated.provider_id, None);
    assert_eq!(updated.model, None);

    manager.delete(&added.id).expect("the transform is deleted");
    assert!(manager.get(&added.id).is_none());
    assert_eq!(manager.list().len(), 14);
}

#[test]
fn a_builtin_transform_cannot_be_deleted() {
    let (_dir, manager) = manager();

    let error = manager
        .delete("resumir")
        .expect_err("a built-in transform cannot be deleted");
    assert!(
        error.contains("cannot be deleted"),
        "unexpected error: {error}"
    );
    assert!(manager.get("resumir").is_some());
    assert_eq!(manager.list().len(), 14);
}

#[test]
fn a_builtin_transform_cannot_be_renamed() {
    let (_dir, manager) = manager();
    let resumir = manager.get("resumir").expect("the built-in is there");

    let error = manager
        .update(
            "resumir",
            TransformInput {
                name: "Otro nombre".to_string(),
                prompt: resumir.prompt.clone(),
                provider_id: None,
                model: None,
                shortcut: None,
                category: None,
                enabled: true,
            },
        )
        .expect_err("a built-in transform cannot be renamed");
    assert!(
        error.contains("cannot be renamed"),
        "unexpected error: {error}"
    );

    let resumir = manager.get("resumir").expect("the built-in is still there");
    assert_eq!(resumir.name, "Resumir");
}

#[test]
fn a_builtin_transform_can_be_configured_but_not_edited() {
    let (_dir, manager) = manager();
    let resumir = manager.get("resumir").expect("the built-in is there");

    manager
        .update(
            "resumir",
            TransformInput {
                name: resumir.name.clone(),
                prompt: resumir.prompt.clone(),
                provider_id: Some("minimax".to_string()),
                model: Some("MiniMax-M3".to_string()),
                shortcut: Some("Ctrl+Alt+R".to_string()),
                category: Some("Resumen".to_string()),
                enabled: true,
            },
        )
        .expect("a built-in transform can be configured");

    let configured = manager.get("resumir").expect("the built-in is still there");
    assert_eq!(configured.provider_id.as_deref(), Some("minimax"));
    assert_eq!(configured.model.as_deref(), Some("MiniMax-M3"));
    assert_eq!(configured.shortcut.as_deref(), Some("Ctrl+Alt+R"));
}

#[test]
fn a_custom_transform_cannot_take_a_builtin_id() {
    let (_dir, manager) = manager();

    let error = manager
        .insert_custom("resumir", custom_input("Impostor", "Otro prompt"))
        .expect_err("a custom transform cannot take the id of a built-in");
    assert!(error.contains("built-in"), "unexpected error: {error}");

    let resumir = manager.get("resumir").expect("the built-in is still there");
    assert_eq!(resumir.name, "Resumir");
    assert_eq!(resumir.prompt, TransformManager::builtin_catalogue()[2].prompt);
    assert_eq!(manager.list().len(), 14);
}

#[test]
fn a_transform_needs_a_name_and_an_instruction() {
    let (_dir, manager) = manager();

    assert!(manager.add(custom_input("   ", "Un prompt")).is_err());
    assert!(manager.add(custom_input("Un nombre", "   ")).is_err());
    assert_eq!(manager.list().len(), 14);
}

#[tokio::test]
async fn the_usage_counter_increases_when_the_transform_runs() {
    let (_dir, manager) = manager();
    let transform = manager.get("resumir").expect("the built-in is there");
    assert_eq!(transform.usage_count, 0);

    let provider = unreachable_provider();
    // The provider refuses the connection, but the transform did run.
    let outcome = manager.run(&transform, "un texto", &provider, "key", None).await;
    assert!(outcome.error.is_some());

    let after = manager.get("resumir").expect("the built-in is still there");
    assert_eq!(after.usage_count, 1);

    manager
        .run(&transform, "otro texto", &provider, "key", None)
        .await;
    let after = manager.get("resumir").expect("the built-in is still there");
    assert_eq!(after.usage_count, 2);
}

#[tokio::test]
async fn a_failed_run_keeps_the_original_text_and_reports_the_error() {
    let (_dir, manager) = manager();
    let transform = manager.get("resumir").expect("the built-in is there");

    let original = "El texto original del usuario, con acentos: ñandú y compatriotas.";
    let provider = unreachable_provider();

    let outcome = manager.run(&transform, original, &provider, "key", None).await;

    assert_eq!(outcome.text, original);
    assert!(!outcome.changed);
    let error = outcome.error.expect("the failure is reported");
    assert!(!error.trim().is_empty());
}

#[tokio::test]
async fn a_run_uses_the_provider_it_is_given() {
    let (_dir, manager) = manager();
    let transform = manager.get("resumir").expect("the built-in is there");
    let provider = unreachable_provider();

    let outcome = manager.run(&transform, "hola", &provider, "key", None).await;

    let error = outcome.error.expect("the failure is reported");
    assert!(
        error.contains("127.0.0.1"),
        "the error must name the provider that was used: {error}"
    );
}

#[tokio::test]
async fn an_empty_text_fails_without_calling_the_provider() {
    let (_dir, manager) = manager();
    let transform = manager.get("resumir").expect("the built-in is there");
    let provider = unreachable_provider();

    let outcome = manager.run(&transform, "   ", &provider, "key", None).await;

    assert_eq!(outcome.text, "   ");
    assert!(!outcome.changed);
    let error = outcome.error.expect("the failure is reported");
    assert!(error.contains("no text"), "unexpected error: {error}");
}

#[test]
fn the_module_uses_the_answer_of_the_provider_verbatim() {
    // The reasoning is separated by the LLM client, so this module must not
    // touch the answer: no stripping, no rewriting, no second parsing layer.
    let outcome = outcome_from_answer(Some("  Texto final.  ".to_string()), "original");
    assert_eq!(outcome.text, "Texto final.");
    assert!(outcome.changed);
    assert!(outcome.error.is_none());

    // An empty answer is a failure, not a result.
    let empty = outcome_from_answer(Some("   ".to_string()), "original");
    assert_eq!(empty.text, "original");
    assert!(!empty.changed);
    assert!(empty.error.is_some());

    let none = outcome_from_answer(None, "original");
    assert_eq!(none.text, "original");
    assert!(!none.changed);
    assert!(none.error.is_some());
}
