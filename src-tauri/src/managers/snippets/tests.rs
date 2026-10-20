//! Tests for the snippets manager.
//!
//! The manager writes to a throwaway SQLite file per test, so the tests
//! never touch the real database of the application.

use super::*;

use std::collections::HashMap;
use std::path::Path;

use tempfile::TempDir;

/// Opens a manager on a throwaway database.
fn manager() -> (TempDir, SnippetManager) {
    let dir = TempDir::new().expect("a temporary folder");
    let manager = SnippetManager::with_db_path(dir.path().join("dixi.db"))
        .expect("the manager opens the database");
    (dir, manager)
}

/// The writable fields of a plain snippet.
fn input(name: &str, trigger: &str, expansion: &str) -> SnippetInput {
    SnippetInput {
        name: name.to_string(),
        trigger_phrase: trigger.to_string(),
        expansion: expansion.to_string(),
        variables: Vec::new(),
        category: None,
        app_names: Vec::new(),
        language: None,
        is_favorite: false,
        enabled: true,
    }
}

/// The values handed to an expansion.
fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// The single snippet of a manager.
fn only(manager: &SnippetManager) -> Snippet {
    let mut snippets = manager.list();
    assert_eq!(snippets.len(), 1, "the manager holds exactly one snippet");
    snippets.remove(0)
}

// --- Storage ---------------------------------------------------------------

#[test]
fn adds_a_snippet_and_reads_it_back() {
    let (_dir, manager) = manager();

    let snippet = manager
        .add(input("Firma", "mi firma", "Juan Perez"))
        .expect("the snippet is added");

    assert_eq!(snippet.name, "Firma");
    assert_eq!(snippet.trigger_phrase, "mi firma");
    assert_eq!(snippet.expansion, "Juan Perez");
    assert!(snippet.enabled, "a new snippet is enabled");
    assert!(!snippet.is_favorite, "a new snippet is not starred");
    assert_eq!(snippet.usage_count, 0, "a new snippet was never used");
    assert!(snippet.id.starts_with("snippet_"), "the snippet has an id");

    let stored = only(&manager);
    assert_eq!(stored.id, snippet.id);
    assert_eq!(stored.trigger_phrase, "mi firma");
}

#[test]
fn keeps_the_variables_and_the_scope_of_a_snippet() {
    let (_dir, manager) = manager();

    let mut original = input("Saludo", "saludo", "Hola {nombre}");
    original.variables = vec![SnippetVariable {
        name: "nombre".to_string(),
        required: true,
        prompt: "¿Como te llamas?".to_string(),
    }];
    original.category = Some("correo".to_string());
    original.app_names = vec!["Chrome.exe".to_string()];
    original.language = Some("ES".to_string());

    manager.add(original).expect("the snippet is added");
    let stored = only(&manager);

    assert_eq!(stored.variables.len(), 1);
    assert_eq!(stored.variables[0].name, "nombre");
    assert!(stored.variables[0].required);
    assert_eq!(stored.variables[0].prompt, "¿Como te llamas?");
    assert_eq!(stored.category.as_deref(), Some("correo"));
    assert_eq!(
        stored.app_names,
        vec!["chrome.exe".to_string()],
        "the application names are normalized"
    );
    assert_eq!(
        stored.language.as_deref(),
        Some("es"),
        "the language is normalized"
    );
}

#[test]
fn updates_a_snippet() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");

    let mut changed = input("Firma larga", "mi firma", "Juan Perez\nIngeniero");
    changed.variables = vec![SnippetVariable {
        name: "empresa".to_string(),
        required: false,
        prompt: "¿Empresa?".to_string(),
    }];
    manager
        .update(&snippet.id, changed)
        .expect("the snippet is updated");

    let stored = only(&manager);
    assert_eq!(stored.name, "Firma larga");
    assert_eq!(stored.expansion, "Juan Perez\nIngeniero");
    assert_eq!(stored.variables.len(), 1);
    assert_eq!(stored.variables[0].name, "empresa");
}

#[test]
fn refuses_to_update_a_snippet_that_does_not_exist() {
    let (_dir, manager) = manager();
    assert!(manager
        .update("snippet_missing", input("A", "a", "b"))
        .is_err());
}

#[test]
fn deletes_a_snippet() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");

    manager.delete(&snippet.id).expect("the snippet is deleted");
    assert!(manager.list().is_empty(), "the snippet is gone");
    assert!(
        manager.delete(&snippet.id).is_err(),
        "deleting it twice is an error"
    );
}

// --- Flags and counters ----------------------------------------------------

#[test]
fn toggles_the_favourite_flag() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");

    manager.set_favorite(&snippet.id, true).expect("it is starred");
    assert!(only(&manager).is_favorite);

    manager
        .set_favorite(&snippet.id, false)
        .expect("it is unstarred");
    assert!(!only(&manager).is_favorite);
}

#[test]
fn toggles_the_enabled_flag() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");

    manager.set_enabled(&snippet.id, false).expect("it is disabled");
    assert!(!only(&manager).enabled);
    assert!(
        manager.rules_for(None, None).is_empty(),
        "a disabled snippet does not produce a rule"
    );

    manager.set_enabled(&snippet.id, true).expect("it is enabled");
    assert!(only(&manager).enabled);
    assert_eq!(manager.rules_for(None, None).len(), 1);
}

#[test]
fn counts_the_uses() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");

    manager.record_use(&snippet.id).expect("the use is recorded");
    manager.record_use(&snippet.id).expect("the use is recorded");
    assert_eq!(only(&manager).usage_count, 2);

    assert!(
        manager.record_use("snippet_missing").is_err(),
        "an unknown snippet cannot be counted"
    );
}

#[test]
fn lists_the_triggers_as_a_vocabulary() {
    let (_dir, manager) = manager();
    manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");
    manager
        .add(input("Saludo", "hola", "Hola"))
        .expect("the snippet is added");

    let vocabulary = manager.trigger_vocabulary(None, None);
    assert_eq!(vocabulary.len(), 2, "both triggers are listed");
    assert!(vocabulary.contains(&"mi firma".to_string()));
    assert!(vocabulary.contains(&"hola".to_string()));
}

#[test]
fn the_vocabulary_skips_the_disabled_snippets() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Firma", "mi firma", "Juan"))
        .expect("the snippet is added");
    manager
        .add(input("Saludo", "hola", "Hola"))
        .expect("the snippet is added");

    manager.set_enabled(&snippet.id, false).expect("it is disabled");

    let vocabulary = manager.trigger_vocabulary(None, None);
    assert_eq!(vocabulary.len(), 1);
    assert_eq!(vocabulary[0], "hola");
}

// --- Variables -------------------------------------------------------------

#[test]
fn resolves_a_variable_from_the_values() {
    let (_dir, manager) = manager();
    let mut original = input("Saludo", "saludo", "Hola {nombre}");
    original.variables = vec![SnippetVariable {
        name: "nombre".to_string(),
        required: true,
        prompt: "¿Como te llamas?".to_string(),
    }];
    let snippet = manager.add(original).expect("the snippet is added");

    let Expansion::Text(text) = manager.expand(&snippet, &values(&[("nombre", "Mario")])) else {
        panic!("the expansion is resolved");
    };
    assert_eq!(text, "Hola Mario");
}

#[test]
fn asks_for_a_missing_required_variable() {
    let (_dir, manager) = manager();
    let mut original = input("Saludo", "saludo", "Hola {nombre}");
    original.variables = vec![SnippetVariable {
        name: "nombre".to_string(),
        required: true,
        prompt: "¿Como te llamas?".to_string(),
    }];
    let snippet = manager.add(original).expect("the snippet is added");

    let Expansion::NeedsInput {
        snippet_id,
        variable,
    } = manager.expand(&snippet, &HashMap::new())
    else {
        panic!("the expansion asks for the value");
    };
    assert_eq!(snippet_id, snippet.id);
    assert_eq!(variable.name, "nombre");
    assert_eq!(variable.prompt, "¿Como te llamas?");
    assert!(variable.required);
}

#[test]
fn a_missing_required_variable_does_not_expand_the_text() {
    let (_dir, manager) = manager();
    let mut original = input("Saludo", "saludo", "Hola {nombre}");
    original.variables = vec![SnippetVariable {
        name: "nombre".to_string(),
        required: true,
        prompt: "¿Como te llamas?".to_string(),
    }];
    manager.add(original).expect("the snippet is added");

    let Some(Expansion::NeedsInput { .. }) =
        manager.expand_in_text("saludo", &HashMap::new(), None, None)
    else {
        panic!("the expansion asks for the value instead of expanding");
    };
}

#[test]
fn a_missing_optional_variable_becomes_an_empty_string() {
    let (_dir, manager) = manager();
    let mut original = input("Saludo", "saludo", "Hola {nombre}!");
    original.variables = vec![SnippetVariable {
        name: "nombre".to_string(),
        required: false,
        prompt: String::new(),
    }];
    let snippet = manager.add(original).expect("the snippet is added");

    let Expansion::Text(text) = manager.expand(&snippet, &HashMap::new()) else {
        panic!("the expansion is resolved");
    };
    assert_eq!(text, "Hola !");
}

#[test]
fn resolves_the_date_and_the_time_locally() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Ahora", "ahora", "{fecha} {hora}"))
        .expect("the snippet is added");

    let Expansion::Text(text) = manager.expand(&snippet, &HashMap::new()) else {
        panic!("the expansion is resolved");
    };

    let (date_part, time_part) = text
        .split_once(' ')
        .expect("the expansion carries the date and the time");

    let date = chrono::NaiveDate::parse_from_str(date_part, "%d/%m/%Y")
        .expect("the date is written as dd/mm/yyyy");
    let today = chrono::Local::now().date_naive();
    assert!(
        (date - today).num_days().abs() <= 1,
        "the date is today, got {date} and today is {today}"
    );

    let time = chrono::NaiveTime::parse_from_str(time_part, "%H:%M")
        .expect("the time is written as hh:mm");
    let now = chrono::Local::now().time();
    assert!(
        (time - now).num_minutes().abs() <= 2,
        "the time is now, got {time} and now is {now}"
    );
}

#[test]
fn the_date_and_the_time_need_no_value() {
    let (_dir, manager) = manager();
    // The placeholders are not declared, and the snippet is still accepted.
    let snippet = manager
        .add(input("Ahora", "ahora", "{fecha} {hora}"))
        .expect("the snippet is added");

    let Expansion::Text(text) = manager.expand(&snippet, &HashMap::new()) else {
        panic!("the expansion is resolved");
    };
    assert!(!text.is_empty(), "the expansion is not empty");
}

#[test]
fn a_group_that_is_not_an_identifier_stays_literal() {
    let (_dir, manager) = manager();
    let snippet = manager
        .add(input("Json", "json", r#"{"clave": 1}"#))
        .expect("the snippet is added");

    let Expansion::Text(text) = manager.expand(&snippet, &HashMap::new()) else {
        panic!("the expansion is resolved");
    };
    assert_eq!(text, r#"{"clave": 1}"#);
}

// --- Trigger matching ------------------------------------------------------

#[test]
fn expands_the_snippet_whose_trigger_is_in_the_text() {
    let (_dir, manager) = manager();
    manager
        .add(input("Firma", "mi firma", "Juan Perez"))
        .expect("the snippet is added");

    let Some(Expansion::Text(text)) =
        manager.expand_in_text("Hola, mi firma.", &HashMap::new(), None, None)
    else {
        panic!("the snippet expands");
    };
    assert_eq!(
        text, "Juan Perez",
        "the expansion of the snippet that fired is returned"
    );
}

#[test]
fn the_rules_rewrite_the_text_around_the_trigger() {
    let (_dir, manager) = manager();
    manager
        .add(input("Firma", "mi firma", "Juan Perez"))
        .expect("the snippet is added");

    // The matching itself is the shared engine's, so the text around the
    // trigger stays untouched.
    let rules = manager.rules_for(None, None);
    let result = apply_rules("Hola, mi firma.", &rules);
    assert_eq!(result.text, "Hola, Juan Perez.");
    assert_eq!(result.applied, vec![0], "the only rule that fired");
}

#[test]
fn does_not_expand_when_no_trigger_is_in_the_text() {
    let (_dir, manager) = manager();
    manager
        .add(input("Firma", "mi firma", "Juan Perez"))
        .expect("the snippet is added");

    assert!(manager
        .expand_in_text("Hola, buenos dias.", &HashMap::new(), None, None)
        .is_none());
}

#[test]
fn the_longest_trigger_wins() {
    let (_dir, manager) = manager();
    manager
        .add(input("Corta", "firma", "Corta"))
        .expect("the snippet is added");
    manager
        .add(input("Larga", "mi firma", "Larga"))
        .expect("the snippet is added");

    let Some(Expansion::Text(text)) =
        manager.expand_in_text("mi firma", &HashMap::new(), None, None)
    else {
        panic!("the snippet expands");
    };
    assert_eq!(text, "Larga", "the longest trigger wins");
}

#[test]
fn does_not_loop_on_a_self_referencing_snippet() {
    let (_dir, manager) = manager();
    manager
        .add(input("Bucle", "bucle", "esto es un bucle"))
        .expect("the snippet is added");

    let Some(Expansion::Text(text)) =
        manager.expand_in_text("bucle", &HashMap::new(), None, None)
    else {
        panic!("the snippet expands");
    };
    assert_eq!(
        text, "esto es un bucle",
        "the replacement is not examined again"
    );

    // Expanding the result again does not grow it either.
    let Some(Expansion::Text(again)) =
        manager.expand_in_text(&text, &HashMap::new(), None, None)
    else {
        panic!("the snippet expands");
    };
    assert_eq!(again, "esto es un bucle");
}

#[test]
fn does_not_fire_a_trigger_inside_a_longer_identifier() {
    let (_dir, manager) = manager();
    manager
        .add(input("Trigger", "trigger", "Dixi"))
        .expect("the snippet is added");

    let rules = manager.rules_for(None, None);
    let result = apply_rules("miTrigger", &rules);
    assert_eq!(result.text, "miTrigger", "the trigger stays inside the word");
    assert!(result.applied.is_empty(), "the rule did not fire");

    let result = apply_rules("mi_trigger", &rules);
    assert_eq!(
        result.text, "mi_trigger",
        "the trigger stays inside the identifier"
    );
    assert!(result.applied.is_empty(), "the rule did not fire");

    assert!(
        manager
            .expand_in_text("miTrigger", &HashMap::new(), None, None)
            .is_none(),
        "no snippet expands inside an identifier"
    );
}

// --- Collisions ------------------------------------------------------------

#[test]
fn the_starred_snippet_wins_a_collision() {
    let (_dir, manager) = manager();
    let regular = manager
        .add(input("Regular", "firma", "Texto A"))
        .expect("the snippet is added");
    let starred = manager
        .add(input("Favorita", "firma", "Texto B"))
        .expect("the snippet is added");
    manager
        .set_favorite(&starred.id, true)
        .expect("it is starred");

    let competing = manager.competing_rules("firma");
    assert_eq!(competing.len(), 2, "both snippets compete");
    assert_eq!(
        competing[0].replacement, "Texto B",
        "the starred snippet has the higher priority"
    );
    assert_eq!(competing[1].replacement, "Texto A");

    let Some(Expansion::Text(text)) =
        manager.expand_in_text("firma", &HashMap::new(), None, None)
    else {
        panic!("the snippet expands");
    };
    assert_eq!(text, "Texto B");
    assert_ne!(regular.id, starred.id);
}

#[test]
fn the_order_of_a_collision_is_deterministic() {
    let (_dir, manager) = manager();
    manager
        .add(input("Primera", "firma", "Texto A"))
        .expect("the snippet is added");
    manager
        .add(input("Segunda", "firma", "Texto B"))
        .expect("the snippet is added");

    let first = manager.competing_rules("firma");
    let second = manager.competing_rules("firma");
    assert_eq!(first.len(), 2);
    assert_eq!(
        first.iter().map(|r| r.replacement.clone()).collect::<Vec<_>>(),
        second
            .iter()
            .map(|r| r.replacement.clone())
            .collect::<Vec<_>>(),
        "the same data always gives the same order"
    );

    // Without a priority and without a difference in the trigger, the
    // oldest snippet wins, and it is the first rule of the list.
    let Some(Expansion::Text(text)) =
        manager.expand_in_text("firma", &HashMap::new(), None, None)
    else {
        panic!("the snippet expands");
    };
    assert_eq!(text, "Texto A", "the first snippet of the list wins");
    assert_eq!(first[0].replacement, "Texto A");
}

#[test]
fn the_rules_are_ordered_by_the_resolution_rule() {
    let (_dir, manager) = manager();
    manager
        .add(input("Corta", "firma", "Corta"))
        .expect("the snippet is added");
    manager
        .add(input("Larga", "mi firma", "Larga"))
        .expect("the snippet is added");

    let rules = manager.rules_for(None, None);
    assert_eq!(rules.len(), 2);
    assert_eq!(
        rules[0].needle, "mi firma",
        "the longest trigger comes first"
    );
    assert_eq!(rules[1].needle, "firma");
}

// --- Scopes ----------------------------------------------------------------

#[test]
fn a_scoped_snippet_only_fires_in_its_application() {
    let (_dir, manager) = manager();
    let mut scoped = input("Chrome", "firma", "Juan Perez");
    scoped.app_names = vec!["chrome".to_string()];
    manager.add(scoped).expect("the snippet is added");

    assert_eq!(
        manager.rules_for(Some("chrome"), None).len(),
        1,
        "the snippet applies to its application"
    );
    assert_eq!(
        manager.rules_for(Some("CHROME"), None).len(),
        1,
        "the application is compared without the case"
    );
    assert_eq!(
        manager.rules_for(Some("chrome.exe"), None).len(),
        1,
        "the extension does not change the application"
    );
    assert_eq!(
        manager.rules_for(Some("notepad"), None).len(),
        0,
        "the snippet does not apply to another application"
    );
}

#[test]
fn a_snippet_without_applications_fires_everywhere() {
    let (_dir, manager) = manager();
    manager
        .add(input("Firma", "firma", "Juan Perez"))
        .expect("the snippet is added");

    assert_eq!(manager.rules_for(Some("chrome"), None).len(), 1);
    assert_eq!(manager.rules_for(Some("notepad"), None).len(), 1);
    assert_eq!(manager.rules_for(None, None).len(), 1);
}

#[test]
fn a_language_tagged_snippet_only_fires_in_that_language() {
    let (_dir, manager) = manager();
    let mut tagged = input("Servidor", "servidor", "server");
    tagged.language = Some("es".to_string());
    manager.add(tagged).expect("the snippet is added");

    assert_eq!(manager.rules_for(None, Some("es")).len(), 1);
    assert_eq!(
        manager.rules_for(None, Some("es-AR")).len(),
        1,
        "the base language matches a regional variant"
    );
    assert_eq!(
        manager.rules_for(None, Some("en")).len(),
        0,
        "the snippet does not fire in another language"
    );
}

#[test]
fn a_snippet_without_a_language_fires_in_every_language() {
    let (_dir, manager) = manager();
    manager
        .add(input("Firma", "firma", "Juan Perez"))
        .expect("the snippet is added");

    assert_eq!(manager.rules_for(None, Some("es")).len(), 1);
    assert_eq!(manager.rules_for(None, Some("en")).len(), 1);
    assert_eq!(manager.rules_for(None, None).len(), 1);
}

// --- Validation ------------------------------------------------------------

#[test]
fn rejects_an_empty_trigger() {
    let (_dir, manager) = manager();
    assert!(manager.add(input("Firma", "   ", "Juan")).is_err());
}

#[test]
fn rejects_a_trigger_longer_than_the_limit() {
    let (_dir, manager) = manager();
    let trigger = "a".repeat(MAX_TRIGGER_LENGTH + 1);
    assert!(manager.add(input("Firma", &trigger, "Juan")).is_err());

    let trigger = "a".repeat(MAX_TRIGGER_LENGTH);
    assert!(
        manager.add(input("Firma", &trigger, "Juan")).is_ok(),
        "a trigger of exactly the limit is accepted"
    );
}

#[test]
fn rejects_an_empty_expansion() {
    let (_dir, manager) = manager();
    assert!(manager.add(input("Firma", "firma", "   ")).is_err());
}

#[test]
fn rejects_an_expansion_longer_than_the_limit() {
    let (_dir, manager) = manager();
    let expansion = "a".repeat(MAX_EXPANSION_LENGTH + 1);
    assert!(manager.add(input("Firma", "firma", &expansion)).is_err());

    let expansion = "a".repeat(MAX_EXPANSION_LENGTH);
    assert!(
        manager.add(input("Firma", "firma", &expansion)).is_ok(),
        "an expansion of exactly the limit is accepted"
    );
}

#[test]
fn rejects_a_variable_without_a_name() {
    let (_dir, manager) = manager();
    let mut broken = input("Saludo", "saludo", "Hola {nombre}");
    broken.variables = vec![SnippetVariable {
        name: "  ".to_string(),
        required: true,
        prompt: String::new(),
    }];
    assert!(manager.add(broken).is_err());
}

#[test]
fn rejects_a_variable_with_an_invalid_name() {
    let (_dir, manager) = manager();
    let mut broken = input("Saludo", "saludo", "Hola {nombre}");
    broken.variables = vec![SnippetVariable {
        name: "nombre con espacios".to_string(),
        required: true,
        prompt: String::new(),
    }];
    assert!(manager.add(broken).is_err());
}

#[test]
fn rejects_a_variable_used_but_not_declared() {
    let (_dir, manager) = manager();
    let broken = input("Saludo", "saludo", "Hola {nombre}");
    assert!(manager.add(broken).is_err());
}

#[test]
fn accepts_a_declared_variable_that_is_not_used() {
    let (_dir, manager) = manager();
    let mut generous = input("Saludo", "saludo", "Hola");
    generous.variables = vec![SnippetVariable {
        name: "nombre".to_string(),
        required: true,
        prompt: "¿Como te llamas?".to_string(),
    }];
    assert!(
        manager.add(generous).is_ok(),
        "a variable that is not used is allowed"
    );
}

// --- Import and export -----------------------------------------------------

#[test]
fn exports_and_imports_a_round_trip() {
    let (dir, manager) = manager();

    let mut original = input("Firma", "mi firma", "Juan Perez");
    original.variables = vec![SnippetVariable {
        name: "empresa".to_string(),
        required: false,
        prompt: "¿Empresa?".to_string(),
    }];
    original.category = Some("correo".to_string());
    original.app_names = vec!["chrome".to_string()];
    original.language = Some("es".to_string());
    original.is_favorite = true;
    manager.add(original).expect("the snippet is added");

    let path = dir.path().join("snippets.json");
    let exported = manager
        .export_to_file(&path, false)
        .expect("the export works");
    assert_eq!(exported, 1, "one snippet was written");

    let other_dir = TempDir::new().expect("a temporary folder");
    let other = SnippetManager::with_db_path(other_dir.path().join("dixi.db"))
        .expect("the manager opens the database");

    let imported = other.import_from_file(&path).expect("the import works");
    assert_eq!(imported, 1, "one snippet was added");

    let restored = only(&other);
    assert_eq!(restored.name, "Firma");
    assert_eq!(restored.trigger_phrase, "mi firma");
    assert_eq!(restored.expansion, "Juan Perez");
    assert_eq!(restored.variables.len(), 1);
    assert_eq!(restored.variables[0].name, "empresa");
    assert_eq!(restored.variables[0].prompt, "¿Empresa?");
    assert_eq!(restored.category.as_deref(), Some("correo"));
    assert_eq!(restored.app_names, vec!["chrome".to_string()]);
    assert_eq!(restored.language.as_deref(), Some("es"));
    assert!(restored.is_favorite);
    assert!(restored.enabled);
    assert_eq!(restored.usage_count, 0, "the counter starts again");
}

#[test]
fn importing_the_same_file_twice_adds_nothing() {
    let (dir, manager) = manager();
    manager
        .add(input("Firma", "mi firma", "Juan Perez"))
        .expect("the snippet is added");

    let path = dir.path().join("snippets.json");
    manager
        .export_to_file(&path, false)
        .expect("the export works");

    let other_dir = TempDir::new().expect("a temporary folder");
    let other = SnippetManager::with_db_path(other_dir.path().join("dixi.db"))
        .expect("the manager opens the database");

    assert_eq!(other.import_from_file(&path).expect("the import works"), 1);
    assert_eq!(
        other
            .import_from_file(&path)
            .expect("the second import works"),
        0,
        "the second import adds nothing"
    );
    assert_eq!(other.list().len(), 1, "there is still one snippet");
}

#[test]
fn refuses_to_replace_an_existing_export_without_overwrite() {
    let (dir, manager) = manager();
    manager
        .add(input("Firma", "mi firma", "Juan Perez"))
        .expect("the snippet is added");

    let path = dir.path().join("snippets.json");
    manager
        .export_to_file(&path, false)
        .expect("the first export works");

    let error = manager
        .export_to_file(&path, false)
        .expect_err("the second export is refused");
    assert!(
        error.contains("already exists"),
        "the error explains the refusal: {error}"
    );

    manager
        .export_to_file(&path, true)
        .expect("the export with overwrite works");
}

#[test]
fn refuses_a_relative_path() {
    let (_dir, manager) = manager();

    let error = manager
        .export_to_file(Path::new("snippets.json"), false)
        .expect_err("a relative destination is refused");
    assert!(
        error.contains("absolute path"),
        "the error explains the refusal: {error}"
    );

    let error = manager
        .import_from_file(Path::new("snippets.json"))
        .expect_err("a relative source is refused");
    assert!(
        error.contains("absolute path"),
        "the error explains the refusal: {error}"
    );
}

#[test]
fn refuses_to_import_a_file_that_does_not_exist() {
    let (dir, manager) = manager();
    let path = dir.path().join("missing.json");
    assert!(manager.import_from_file(&path).is_err());
}
