//! End-to-end tests of the personal dictionary against an isolated database.
//!
//! Every test works on its own temporary folder, so the tests never touch
//! the dictionary of a real installation. The manager is reopened over the
//! same file to reproduce what happens after the application restarts.

use super::*;
use tempfile::TempDir;

/// An isolated dictionary: its own database inside a temporary folder.
fn isolated() -> (TempDir, DictionaryManager) {
    let dir = TempDir::new().expect("a temporary folder");
    let manager = DictionaryManager::with_db_path(dir.path().join("dictionary.db"))
        .expect("the isolated dictionary must open");
    (dir, manager)
}

/// A substitution rule ready to be added.
fn substitution(term: &str, replacement: &str) -> DictionaryEntryInput {
    DictionaryEntryInput {
        term: term.to_string(),
        replacement: replacement.to_string(),
        kind: DictionaryEntryKind::Substitution,
        enabled: Some(true),
        scope: Some(GLOBAL_SCOPE.to_string()),
        case_sensitive: Some(false),
        whole_word: Some(true),
        priority: Some(0),
        language: None,
    }
}

/// A vocabulary hint ready to be added.
fn term_hint(term: &str) -> DictionaryEntryInput {
    DictionaryEntryInput {
        term: term.to_string(),
        replacement: String::new(),
        kind: DictionaryEntryKind::Term,
        enabled: Some(true),
        scope: Some(GLOBAL_SCOPE.to_string()),
        case_sensitive: Some(false),
        whole_word: Some(true),
        priority: Some(0),
        language: None,
    }
}

/// Runs the substitution rules of the dictionary over a text.
fn rewrite(manager: &DictionaryManager, text: &str) -> String {
    apply_substitutions(text, &manager.substitution_rules(GLOBAL_SCOPE), None).text
}

#[test]
fn adds_edits_and_deletes_entries() {
    let (_dir, manager) = isolated();

    let added = manager.add(&substitution("perro", "Dixi")).expect("the entry is added");
    assert_eq!(added.term, "perro");
    assert_eq!(added.replacement, "Dixi");
    assert!(added.enabled);
    assert_eq!(manager.list().len(), 1);

    // Edit.
    let mut edited = substitution("perro", "Dixi Pro");
    edited.whole_word = Some(false);
    manager
        .update(added.id, &edited)
        .expect("the entry is updated");
    let stored = manager
        .list()
        .into_iter()
        .find(|entry| entry.id == added.id)
        .expect("the entry is still there");
    assert_eq!(stored.replacement, "Dixi Pro");
    assert!(!stored.whole_word);

    // Delete.
    manager.delete(added.id).expect("the entry is deleted");
    assert!(manager.list().is_empty());
}

#[test]
fn a_duplicate_rule_is_refused() {
    let (_dir, manager) = isolated();
    manager.add(&substitution("perro", "Dixi")).expect("the entry is added");

    let error = manager
        .add(&substitution("perro", "Dixi"))
        .expect_err("the duplicate must be refused");
    assert!(
        error.contains("already exists"),
        "the error must say so, got: {error}"
    );

    // The same term as a vocabulary hint is a different kind and is allowed.
    manager
        .add(&term_hint("perro"))
        .expect("a hint with the same text is allowed");
    assert_eq!(manager.list().len(), 2);
}

#[test]
fn a_disabled_rule_does_not_apply() {
    let (_dir, manager) = isolated();
    let entry = manager.add(&substitution("perro", "Dixi")).expect("added");

    assert_eq!(rewrite(&manager, "el perro"), "el Dixi");

    manager.set_enabled(entry.id, false).expect("the rule is disabled");
    assert_eq!(rewrite(&manager, "el perro"), "el perro");

    manager.set_enabled(entry.id, true).expect("the rule is enabled");
    assert_eq!(rewrite(&manager, "el perro"), "el Dixi");
}

#[test]
fn the_priority_decides_between_two_rules_of_the_same_length() {
    let (_dir, manager) = isolated();

    let mut low = substitution("equipo", "equipo interno");
    low.priority = Some(0);
    let mut high = substitution("equipo", "equipo de la casa");
    high.priority = Some(10);

    manager.add(&low).expect("added");
    manager.add(&high).expect("added");

    assert_eq!(rewrite(&manager, "el equipo"), "el equipo de la casa");
}

#[test]
fn a_language_tag_limits_a_rule_to_that_language() {
    let (_dir, manager) = isolated();

    let mut english = substitution("server", "servidor");
    english.language = Some("en".to_string());
    manager.add(&english).expect("added");

    let rules = manager.substitution_rules(GLOBAL_SCOPE);
    assert_eq!(
        apply_substitutions("the server", &rules, Some("en")).text,
        "the servidor"
    );
    assert_eq!(
        apply_substitutions("el server", &rules, Some("es")).text,
        "el server",
        "an English rule must not touch a Spanish transcript"
    );
}

#[test]
fn the_entries_survive_a_restart() {
    let dir = TempDir::new().expect("a temporary folder");
    let db_path = dir.path().join("dictionary.db");

    {
        let manager = DictionaryManager::with_db_path(db_path.clone()).expect("opened");
        manager.add(&substitution("perro", "Dixi")).expect("added");
        manager.add(&term_hint("PostgreSQL")).expect("added");
    }

    // A new manager over the same file is what the application does on the
    // next launch.
    let reopened = DictionaryManager::with_db_path(db_path).expect("reopened");
    let entries = reopened.list();
    assert_eq!(entries.len(), 2);

    let vocabulary = reopened.vocabulary(GLOBAL_SCOPE);
    assert!(
        vocabulary.iter().any(|word| word == "PostgreSQL"),
        "the hint must be part of the vocabulary, got: {vocabulary:?}"
    );
    assert_eq!(rewrite(&reopened, "el perro"), "el Dixi");
}

#[test]
fn exports_a_json_file_that_can_be_imported_again() {
    let (dir, manager) = isolated();
    manager.add(&substitution("perro", "Dixi")).expect("added");
    manager.add(&term_hint("PostgreSQL")).expect("added");

    let path = dir.path().join("dictionary.json");
    let written = manager
        .export_to_file(&path, false)
        .expect("the export must succeed");
    assert_eq!(written, 2);
    assert!(path.exists(), "the backend must have written the file");

    // The file is a real JSON document with the documented shape.
    let text = std::fs::read_to_string(&path).expect("the file must be readable");
    let parsed: serde_json::Value =
        serde_json::from_str(&text).expect("the file must be valid JSON");
    let entries = parsed
        .get("entries")
        .and_then(|value| value.as_array())
        .expect("the file must carry the entries");
    assert_eq!(entries.len(), 2);

    // Importing the same file into a fresh dictionary reproduces it.
    let (other_dir, other) = isolated();
    let imported = other
        .import_from_file(&path)
        .expect("the import must succeed");
    assert_eq!(imported, 2);
    assert_eq!(other.list().len(), 2);

    // Importing it a second time changes nothing.
    let again = other
        .import_from_file(&path)
        .expect("the second import must succeed");
    assert_eq!(again, 0, "the entries already exist");
    assert_eq!(other.list().len(), 2);

    let _ = other_dir;
}

#[test]
fn an_existing_file_is_not_replaced_without_being_asked() {
    let (dir, manager) = isolated();
    manager.add(&substitution("perro", "Dixi")).expect("added");

    let path = dir.path().join("dictionary.json");
    manager.export_to_file(&path, false).expect("the first export");

    // A second export refuses, so an accidental double click cannot destroy
    // a file the user had edited.
    let error = manager
        .export_to_file(&path, false)
        .expect_err("the existing file must be protected");
    assert!(
        error.contains("already exists"),
        "the error must say so, got: {error}"
    );

    // The original content survived the refused export.
    let text = std::fs::read_to_string(&path).expect("the file must still be there");
    assert!(text.contains("perro"));

    // Asked explicitly, it is replaced.
    manager
        .export_to_file(&path, true)
        .expect("the confirmed export must succeed");
}

#[test]
fn the_import_and_export_paths_are_validated() {
    let (dir, manager) = isolated();

    // A relative path is refused: it would resolve against the working
    // directory of the process, not against what the user sees.
    let relative = manager.export_to_file(std::path::Path::new("dictionary.json"), false);
    assert!(relative.is_err(), "a relative path must be refused");

    let missing = manager.import_from_file(&dir.path().join("nope.json"));
    assert!(missing.is_err(), "a missing file must be refused");

    let folder = manager.import_from_file(dir.path());
    assert!(folder.is_err(), "a folder must be refused");

    // A file that is not a dictionary is refused with a clear message.
    let bogus = dir.path().join("bogus.json");
    std::fs::write(&bogus, "esto no es un diccionario").expect("the file must be written");
    let error = manager
        .import_from_file(&bogus)
        .expect_err("a bogus file must be refused");
    assert!(
        error.contains("not a valid dictionary file"),
        "the error must say so, got: {error}"
    );
}

#[test]
fn substitutes_the_technical_terms_an_engine_gets_wrong() {
    let (_dir, manager) = isolated();

    manager
        .add(&substitution("asp dot net", "ASP.NET"))
        .expect("added");
    manager
        .add(&substitution("post gre sql", "PostgreSQL"))
        .expect("added");
    manager
        .add(&substitution("entity framework", "Entity Framework"))
        .expect("added");
    manager.add(&substitution("git hub", "GitHub")).expect("added");

    let text = "hoy deployamos asp dot net contra post gre sql \
                con entity framework y subimos a git hub";

    assert_eq!(
        rewrite(&manager, text),
        "hoy deployamos ASP.NET contra PostgreSQL con Entity Framework y subimos a GitHub"
    );
}

#[test]
fn the_vocabulary_carries_the_terms_the_engine_has_to_hear() {
    let (_dir, manager) = isolated();
    manager.add(&term_hint("PostgreSQL")).expect("added");
    manager.add(&term_hint("Entity Framework")).expect("added");
    manager
        .add(&substitution("git hub", "GitHub"))
        .expect("added");

    let vocabulary = manager.vocabulary(GLOBAL_SCOPE);
    assert!(vocabulary.iter().any(|word| word == "PostgreSQL"));
    assert!(vocabulary.iter().any(|word| word == "Entity Framework"));
    assert!(
        vocabulary.iter().any(|word| word == "git hub"),
        "the search text of a substitution also reaches the engine, got: {vocabulary:?}"
    );
    assert_eq!(
        vocabulary.iter().filter(|word| *word == "PostgreSQL").count(),
        1,
        "a term must not be repeated"
    );
}

#[test]
fn a_hit_counter_records_the_use_of_a_rule() {
    let (_dir, manager) = isolated();
    let entry = manager.add(&substitution("perro", "Dixi")).expect("added");

    let outcome = apply_substitutions("el perro", &manager.substitution_rules(GLOBAL_SCOPE), None);
    manager.record_hits(&outcome.applied);

    let stored = manager
        .list()
        .into_iter()
        .find(|e| e.id == entry.id)
        .expect("the entry is still there");
    assert_eq!(stored.hit_count, 1);
}
