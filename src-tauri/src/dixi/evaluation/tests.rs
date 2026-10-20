//! Tests for the ASR quality evaluation harness.
//!
//! Every expected value in this file is computed by hand and the arithmetic
//! is spelled out next to the assertion, so a future change in the
//! normalization or in the tie-breaking rules shows up as a clear diff
//! instead of a mysterious failure.

use super::*;

/// Ten reference words, used by the single-error WER tests.
const TEN_WORDS: &str = "uno dos tres cuatro cinco seis siete ocho nueve diez";

#[test]
fn normalize_for_comparison_strips_accents_punctuation_and_case() {
    // "  ¡Hola, MUNDO! ¿Qué tal?  "
    //   lowercased -> "  ¡hola, mundo! ¿qué tal?  "
    //   accents     -> "  ¡hola, mundo! ¿que tal?  "
    //   punctuation -> "   hola  mundo   que tal   " (every non-alphanumeric
    //                 character becomes a space)
    //   collapsed   -> " hola mundo que tal " (runs of spaces merged)
    //   trimmed     -> "hola mundo que tal"
    assert_eq!(
        normalize_for_comparison("  ¡Hola, MUNDO! ¿Qué tal?  "),
        "hola mundo que tal"
    );
}

#[test]
fn normalize_for_comparison_folds_accents_to_ascii() {
    // Uppercase Latin-1 folds to ASCII: A, E, I, O, U, U, N.
    assert_eq!(normalize_for_comparison("ÁÉÍÓÚÜÑ"), "aeiouun");
    // Precomposed and pre-decomposed accents fold the same way.
    assert_eq!(normalize_for_comparison("café"), "cafe");
    assert_eq!(normalize_for_comparison("cafe\u{0301}"), "cafe");
    // The inverted question mark is punctuation, the letters are not.
    assert_eq!(normalize_for_comparison("¿Qué tal?"), "que tal");
}

#[test]
fn word_error_rate_is_zero_for_identical_text() {
    // 0 edits / 7 reference words = 0.0
    assert_eq!(word_error_rate(TEN_WORDS, TEN_WORDS), 0.0);
}

#[test]
fn word_error_rate_counts_one_substitution_in_ten_words() {
    // Reference has 10 words, the hypothesis differs in one of them
    // ("diez" -> "once"), so the alignment is 1 substitution.
    // WER = 1 edit / 10 reference words = 0.1
    let hypothesis = "uno dos tres cuatro cinco seis siete ocho nueve once";
    assert!((word_error_rate(TEN_WORDS, hypothesis) - 0.1).abs() < 1e-9);

    let reference_tokens = tokenize(&normalize_for_comparison(TEN_WORDS));
    let hypothesis_tokens = tokenize(&normalize_for_comparison(hypothesis));
    assert_eq!(
        word_edit_distance(&reference_tokens, &hypothesis_tokens),
        EditCounts {
            substitutions: 1,
            deletions: 0,
            insertions: 0
        }
    );
}

#[test]
fn word_error_rate_counts_one_deletion_in_ten_words() {
    // The hypothesis drops the last word ("diez"), so the alignment is
    // 1 deletion.
    // WER = 1 edit / 10 reference words = 0.1
    let hypothesis = "uno dos tres cuatro cinco seis siete ocho nueve";
    assert!((word_error_rate(TEN_WORDS, hypothesis) - 0.1).abs() < 1e-9);

    let reference_tokens = tokenize(&normalize_for_comparison(TEN_WORDS));
    let hypothesis_tokens = tokenize(&normalize_for_comparison(hypothesis));
    assert_eq!(
        word_edit_distance(&reference_tokens, &hypothesis_tokens),
        EditCounts {
            substitutions: 0,
            deletions: 1,
            insertions: 0
        }
    );
}

#[test]
fn word_error_rate_counts_one_insertion_in_ten_words() {
    // The hypothesis adds one word ("once") at the end, so the alignment is
    // 1 insertion.
    // WER = 1 edit / 10 reference words = 0.1
    let hypothesis = "uno dos tres cuatro cinco seis siete ocho nueve diez once";
    assert!((word_error_rate(TEN_WORDS, hypothesis) - 0.1).abs() < 1e-9);

    let reference_tokens = tokenize(&normalize_for_comparison(TEN_WORDS));
    let hypothesis_tokens = tokenize(&normalize_for_comparison(hypothesis));
    assert_eq!(
        word_edit_distance(&reference_tokens, &hypothesis_tokens),
        EditCounts {
            substitutions: 0,
            deletions: 0,
            insertions: 1
        }
    );
}

#[test]
fn word_error_rate_is_one_when_the_reference_is_empty() {
    // The reference has no words to align against, so a non-empty hypothesis
    // is a full error by definition: WER = 1.0
    assert_eq!(word_error_rate("", "hola mundo"), 1.0);
}

#[test]
fn word_error_rate_is_zero_when_both_sides_are_empty() {
    // Nothing to align and nothing produced: WER = 0.0
    assert_eq!(word_error_rate("", ""), 0.0);
    // Whitespace-only text normalizes to the empty string.
    assert_eq!(word_error_rate("   ", "  "), 0.0);
}

#[test]
fn character_error_rate_measures_a_single_character_change() {
    // "hola mundo" is 10 characters (h,o,l,a,space,m,u,n,d,o).
    // Identical text: 0 edits / 10 = 0.0
    assert_eq!(character_error_rate("hola mundo", "hola mundo"), 0.0);
    // One changed character ("mundo" -> "munco"): 1 substitution.
    // CER = 1 edit / 10 reference characters = 0.1
    assert!((character_error_rate("hola mundo", "hola munco") - 0.1).abs() < 1e-9);
    // Trailing punctuation is normalized away, so it is not an error.
    assert_eq!(character_error_rate("hola mundo", "hola mundo!"), 0.0);
}

#[test]
fn evaluate_case_reports_exact_match_and_edit_counts() {
    let identical = EvalCase {
        id: "es-001".to_string(),
        language: "es".to_string(),
        reference: "Hola, ¿cómo estás hoy?".to_string(),
        hypothesis: "Hola, ¿cómo estás hoy?".to_string(),
        audio: Some("../../validation-phase2a/audio-referencia/F1.wav".to_string()),
    };

    let result = evaluate_case(&identical);
    // Both sides normalize to "hola como estas hoy" (4 words, 17 characters).
    assert!(result.exact_match);
    assert_eq!(
        result.edits,
        EditCounts {
            substitutions: 0,
            deletions: 0,
            insertions: 0
        }
    );
    assert_eq!(result.wer, 0.0);
    assert_eq!(result.cer, 0.0);
    assert_eq!(result.id, "es-001");
    assert_eq!(result.language, "es");

    // One word replaced by another one: at the word level this is a
    // substitution (2 reference words against 2 hypothesis words).
    let one_substitution = EvalCase {
        id: "es-002".to_string(),
        language: "es".to_string(),
        reference: "hola mundo".to_string(),
        hypothesis: "mola mundo".to_string(),
        audio: None,
    };

    let result = evaluate_case(&one_substitution);
    assert!(!result.exact_match);
    assert_eq!(
        result.edits,
        EditCounts {
            substitutions: 1,
            deletions: 0,
            insertions: 0
        }
    );
    // WER = 1 edit / 2 reference words = 0.5
    assert!((result.wer - 0.5).abs() < 1e-9);
    // CER = 1 edit ("h" -> "m") / 10 reference characters = 0.1
    assert!((result.cer - 0.1).abs() < 1e-9);

    // One word missing from the hypothesis: at the word level this is a
    // deletion (2 reference words against 1 hypothesis word).
    let one_deletion = EvalCase {
        id: "es-003".to_string(),
        language: "es".to_string(),
        reference: "hola mundo".to_string(),
        hypothesis: "hola".to_string(),
        audio: None,
    };

    let result = evaluate_case(&one_deletion);
    assert!(!result.exact_match);
    assert_eq!(
        result.edits,
        EditCounts {
            substitutions: 0,
            deletions: 1,
            insertions: 0
        }
    );
    // WER = 1 edit / 2 reference words = 0.5
    assert!((result.wer - 0.5).abs() < 1e-9);
    // CER = 6 deleted characters (" mundo") / 10 reference characters = 0.6
    assert!((result.cer - 6.0 / 10.0).abs() < 1e-9);
}

#[test]
fn evaluate_cases_groups_by_language_and_averages_wer() {
    let cases = vec![
        EvalCase {
            id: "es-001".to_string(),
            language: "es".to_string(),
            reference: "hola mundo".to_string(),
            hypothesis: "hola mundo".to_string(),
            audio: None,
        },
        EvalCase {
            id: "es-002".to_string(),
            language: "es".to_string(),
            reference: "hola mundo".to_string(),
            hypothesis: "mola mundo".to_string(),
            audio: None,
        },
        EvalCase {
            id: "es-003".to_string(),
            language: "es".to_string(),
            reference: "uno dos tres".to_string(),
            hypothesis: "uno dos tres".to_string(),
            audio: None,
        },
        EvalCase {
            id: "en-001".to_string(),
            language: "en".to_string(),
            reference: "hello world".to_string(),
            hypothesis: "hello world".to_string(),
            audio: None,
        },
        EvalCase {
            id: "en-002".to_string(),
            language: "en".to_string(),
            reference: "the quick brown fox".to_string(),
            hypothesis: "the quick brown fax".to_string(),
            audio: None,
        },
    ];

    let results: Vec<EvalResult> = cases.iter().map(evaluate_case).collect();
    let summary = evaluate_cases(&cases);

    assert_eq!(summary.total_cases, 5);

    // Per-case WER: es-001 0.0, es-002 1/2 = 0.5, es-003 0.0,
    //               en-001 0.0, en-002 1/4 = 0.25
    // mean_wer = (0.0 + 0.5 + 0.0 + 0.0 + 0.25) / 5 = 0.75 / 5 = 0.15
    assert!((summary.mean_wer - 0.15).abs() < 1e-9);

    // Per-case CER: es-001 0.0, es-002 1/10 = 0.1, es-003 0.0, en-001 0.0,
    //               en-002 1/19 = 0.052631... ("the quick brown fox" is 19 chars)
    // mean_cer = (0.0 + 0.1 + 0.0 + 0.0 + 1/19) / 5
    assert!((summary.mean_cer - (0.1 + 1.0 / 19.0) / 5.0).abs() < 1e-9);

    // es-001, es-003 and en-001 match exactly: 3 / 5 = 0.6
    assert!((summary.exact_match_rate - 0.6).abs() < 1e-9);

    // Two languages, sorted by language code: "en" before "es".
    assert_eq!(summary.by_language.len(), 2);

    // "en": 2 cases, WER (0.0 + 0.25) / 2 = 0.125, CER (0.0 + 1/19) / 2 = 1/38
    assert_eq!(summary.by_language[0].0, "en");
    assert_eq!(summary.by_language[0].1, 2);
    assert!((summary.by_language[0].2 - 0.125).abs() < 1e-9);
    assert!((summary.by_language[0].3 - 1.0 / 38.0).abs() < 1e-9);

    // "es": 3 cases, WER (0.0 + 0.5 + 0.0) / 3 = 0.5 / 3,
    //       CER (0.0 + 0.1 + 0.0) / 3 = 0.1 / 3
    assert_eq!(summary.by_language[1].0, "es");
    assert_eq!(summary.by_language[1].1, 3);
    assert!((summary.by_language[1].2 - 0.5 / 3.0).abs() < 1e-9);
    assert!((summary.by_language[1].3 - 0.1 / 3.0).abs() < 1e-9);

    // The aggregate is the arithmetic mean of the individual case metrics.
    let mean_of_cases = results.iter().map(|r| r.wer).sum::<f64>() / results.len() as f64;
    assert!((summary.mean_wer - mean_of_cases).abs() < 1e-9);
}

#[test]
fn evaluate_cases_on_an_empty_set_is_a_safe_zero() {
    let summary = evaluate_cases(&[]);
    assert_eq!(summary.total_cases, 0);
    assert_eq!(summary.mean_wer, 0.0);
    assert_eq!(summary.mean_cer, 0.0);
    assert_eq!(summary.exact_match_rate, 0.0);
    assert!(summary.by_language.is_empty());
}
