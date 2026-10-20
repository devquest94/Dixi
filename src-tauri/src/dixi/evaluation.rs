//! Quality evaluation harness for ASR transcription accuracy.
//!
//! The dictation pipeline can transcribe, but until it can *measure* what it
//! produced, no change to the model, the language setting or the custom
//! dictionary can be proven to help. This module provides that measurement:
//! deterministic, dependency-free error rates over reference/hypothesis pairs.
//!
//! Design constraints:
//! - `std` only. No Unicode crate, no network, no audio processing.
//! - Every metric is computed over *normalized* text, so casing, accents and
//!   punctuation never count as transcription errors. Accents are folded to
//!   ASCII with an explicit Latin-1 / Latin Extended-A table instead of a
//!   Unicode normalization pass, which keeps the crate dependency-free.
//! - Substitutions, deletions and insertions are counted separately, not
//!   only as a total, because "the model replaced a word" and "the model
//!   dropped a word" point at different fixes.
//!
//! # Example
//!
//! ```
//! use dixi_app_lib::dixi::evaluation::{evaluate_case, EvalCase};
//!
//! let case = EvalCase {
//!     id: "es-001".to_string(),
//!     language: "es".to_string(),
//!     reference: "Hola, mundo".to_string(),
//!     hypothesis: "Hola, mundos".to_string(),
//!     audio: None,
//! };
//!
//! let result = evaluate_case(&case);
//! assert!(!result.exact_match);
//! assert_eq!(result.edits.substitutions, 1);
//! assert_eq!(result.edits.deletions, 0);
//! assert_eq!(result.edits.insertions, 0);
//! assert!((result.wer - 0.5).abs() < 1e-9);
//! ```

/// Edit operation counts for a single alignment.
///
/// The three counters are kept apart on purpose: a substitution means the
/// engine produced the wrong content, a deletion means it dropped something
/// and an insertion means it hallucinated something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditCounts {
    /// Reference tokens or characters replaced by a different one.
    pub substitutions: u32,
    /// Reference tokens or characters missing from the hypothesis.
    pub deletions: u32,
    /// Extra tokens or characters present only in the hypothesis.
    pub insertions: u32,
}

impl EditCounts {
    /// Total number of edit operations, regardless of their kind.
    pub fn total(&self) -> u32 {
        self.substitutions + self.deletions + self.insertions
    }
}

/// A single evaluation case: the expected text and what the engine produced.
#[derive(Debug, Clone)]
pub struct EvalCase {
    /// Stable identifier of the case.
    pub id: String,
    /// Language code of the case (`"es"`, `"en"`, `"es-en"`, ...).
    pub language: String,
    /// The expected transcription.
    pub reference: String,
    /// The transcription the engine actually produced.
    pub hypothesis: String,
    /// Optional relative path to the reference audio file, for a future
    /// end-to-end runner. Purely informational in this module.
    pub audio: Option<String>,
}

/// Full result for one case.
#[derive(Debug, Clone)]
pub struct EvalResult {
    /// Identifier of the evaluated case.
    pub id: String,
    /// Language code of the evaluated case.
    pub language: String,
    /// Word Error Rate of the case.
    pub wer: f64,
    /// Character Error Rate of the case.
    pub cer: f64,
    /// Word-level edit counts, i.e. the numerator of [`EvalResult::wer`].
    pub edits: EditCounts,
    /// `true` when the hypothesis equals the reference after normalization
    /// (case, accents and punctuation are ignored).
    pub exact_match: bool,
}

/// Aggregate across many cases.
#[derive(Debug, Clone)]
pub struct EvalSummary {
    /// Number of evaluated cases.
    pub total_cases: usize,
    /// Arithmetic mean of the per-case Word Error Rates.
    pub mean_wer: f64,
    /// Arithmetic mean of the per-case Character Error Rates.
    pub mean_cer: f64,
    /// Fraction of cases that matched the reference exactly.
    pub exact_match_rate: f64,
    /// Per-language breakdown: (language, cases, mean_wer, mean_cer).
    pub by_language: Vec<(String, usize, f64, f64)>,
}

/// Normalize text for metric comparison: lowercase, strip accents, replace
/// every non-alphanumeric character with a space, collapse runs of
/// whitespace, trim.
///
/// Accent folding covers the Latin-1 Supplement and Latin Extended-A ranges
/// (`á` -> `a`, `ñ` -> `n`, `ü` -> `u`, `é` -> `e`) and drops combining
/// marks, so pre-decomposed input such as `a` + U+0301 folds like the
/// precomposed `á`. Characters outside those ranges that are still
/// alphanumeric (for example CJK) are preserved as they are.
pub fn normalize_for_comparison(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());

    for ch in text.chars() {
        for lower in ch.to_lowercase() {
            if is_combining_mark(lower) {
                // A combining mark carries no content of its own; dropping it
                // is what a "strip accents" pass does.
                continue;
            }
            match ascii_fold(lower) {
                Some(replacement) => folded.push_str(replacement),
                None if lower.is_alphanumeric() => folded.push(lower),
                None => folded.push(' '),
            }
        }
    }

    collapse_whitespace(&folded)
}

/// Word-level edit distance on already-normalized token sequences.
///
/// The tokens are compared verbatim, so the caller is responsible for
/// normalizing them first.
pub fn word_edit_distance(reference: &[String], hypothesis: &[String]) -> EditCounts {
    let reference: Vec<&str> = reference.iter().map(String::as_str).collect();
    let hypothesis: Vec<&str> = hypothesis.iter().map(String::as_str).collect();
    edit_counts(&reference, &hypothesis)
}

/// Character-level edit distance on already-normalized strings.
///
/// The characters are compared verbatim, so the caller is responsible for
/// normalizing the strings first.
pub fn character_edit_distance(reference: &str, hypothesis: &str) -> EditCounts {
    let reference: Vec<char> = reference.chars().collect();
    let hypothesis: Vec<char> = hypothesis.chars().collect();
    edit_counts(&reference, &hypothesis)
}

/// Word Error Rate = total word edits / reference word count.
///
/// Returns `0.0` when the reference is empty and the hypothesis is empty, and
/// `1.0` when the reference is empty but the hypothesis is not. Both texts are
/// normalized internally, so raw text can be passed directly.
pub fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let reference_tokens = tokenize(&normalize_for_comparison(reference));
    let hypothesis_tokens = tokenize(&normalize_for_comparison(hypothesis));

    if reference_tokens.is_empty() {
        return if hypothesis_tokens.is_empty() { 0.0 } else { 1.0 };
    }

    let edits = word_edit_distance(&reference_tokens, &hypothesis_tokens);
    edits.total() as f64 / reference_tokens.len() as f64
}

/// Character Error Rate = total character edits / reference character count.
///
/// Returns `0.0` when the reference is empty and the hypothesis is empty, and
/// `1.0` when the reference is empty but the hypothesis is not. Both texts are
/// normalized internally, so raw text can be passed directly.
pub fn character_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let reference = normalize_for_comparison(reference);
    let hypothesis = normalize_for_comparison(hypothesis);

    let reference_length = reference.chars().count();
    if reference_length == 0 {
        return if hypothesis.is_empty() { 0.0 } else { 1.0 };
    }

    let edits = character_edit_distance(&reference, &hypothesis);
    edits.total() as f64 / reference_length as f64
}

/// Evaluates one case and returns its metrics.
///
/// The word-level edit counts are reported in [`EvalResult::edits`]; the
/// character-level breakdown is not exposed because the CER numerator is
/// derived from the same normalized texts.
pub fn evaluate_case(case: &EvalCase) -> EvalResult {
    let reference = normalize_for_comparison(&case.reference);
    let hypothesis = normalize_for_comparison(&case.hypothesis);

    let reference_tokens = tokenize(&reference);
    let hypothesis_tokens = tokenize(&hypothesis);
    let edits = word_edit_distance(&reference_tokens, &hypothesis_tokens);

    let wer = if reference_tokens.is_empty() {
        if hypothesis_tokens.is_empty() {
            0.0
        } else {
            1.0
        }
    } else {
        edits.total() as f64 / reference_tokens.len() as f64
    };

    let reference_length = reference.chars().count();
    let cer = if reference_length == 0 {
        if hypothesis.is_empty() {
            0.0
        } else {
            1.0
        }
    } else {
        character_edit_distance(&reference, &hypothesis).total() as f64 / reference_length as f64
    };

    EvalResult {
        id: case.id.clone(),
        language: case.language.clone(),
        wer,
        cer,
        edits,
        exact_match: reference == hypothesis,
    }
}

/// Evaluates a set of cases and aggregates the results.
///
/// The per-language breakdown is sorted by language code so the output is
/// deterministic for the same input set.
pub fn evaluate_cases(cases: &[EvalCase]) -> EvalSummary {
    let results: Vec<EvalResult> = cases.iter().map(evaluate_case).collect();

    if results.is_empty() {
        return EvalSummary {
            total_cases: 0,
            mean_wer: 0.0,
            mean_cer: 0.0,
            exact_match_rate: 0.0,
            by_language: Vec::new(),
        };
    }

    let total = results.len() as f64;
    let mean_wer = results.iter().map(|result| result.wer).sum::<f64>() / total;
    let mean_cer = results.iter().map(|result| result.cer).sum::<f64>() / total;
    let exact_match_rate = results
        .iter()
        .filter(|result| result.exact_match)
        .count() as f64
        / total;

    let mut languages: Vec<String> = results.iter().map(|result| result.language.clone()).collect();
    languages.sort();
    languages.dedup();

    let by_language = languages
        .iter()
        .map(|language| {
            let group: Vec<&EvalResult> =
                results.iter().filter(|r| r.language == *language).collect();
            let count = group.len();
            let language_wer = group.iter().map(|r| r.wer).sum::<f64>() / count as f64;
            let language_cer = group.iter().map(|r| r.cer).sum::<f64>() / count as f64;
            (language.clone(), count, language_wer, language_cer)
        })
        .collect();

    EvalSummary {
        total_cases: results.len(),
        mean_wer,
        mean_cer,
        exact_match_rate,
        by_language,
    }
}

/// Computes Levenshtein edit counts over two sequences.
///
/// Ties are broken deterministically: a diagonal step (match or substitution)
/// wins over a deletion, and a deletion wins over an insertion. The same
/// inputs therefore always produce the same breakdown, which is what makes
/// two evaluation runs comparable.
fn edit_counts<T: PartialEq>(reference: &[T], hypothesis: &[T]) -> EditCounts {
    let n = reference.len();
    let m = hypothesis.len();

    if n == 0 {
        return EditCounts {
            substitutions: 0,
            deletions: 0,
            insertions: m as u32,
        };
    }
    if m == 0 {
        return EditCounts {
            substitutions: 0,
            deletions: n as u32,
            insertions: 0,
        };
    }

    let mut cost = vec![vec![0u32; m + 1]; n + 1];
    let mut ops = vec![vec![EditCounts::default(); m + 1]; n + 1];

    for i in 1..=n {
        cost[i][0] = i as u32;
        ops[i][0].deletions = i as u32;
    }
    for j in 1..=m {
        cost[0][j] = j as u32;
        ops[0][j].insertions = j as u32;
    }

    for i in 1..=n {
        for j in 1..=m {
            // Diagonal first: a match is free and a substitution is the
            // preferred way to pay for a mismatch.
            let diagonal_cost = cost[i - 1][j - 1];
            let mut best_cost = diagonal_cost + 1;
            let mut best_ops = ops[i - 1][j - 1];
            best_ops.substitutions += 1;

            if reference[i - 1] == hypothesis[j - 1] {
                if diagonal_cost <= best_cost {
                    best_cost = diagonal_cost;
                    best_ops = ops[i - 1][j - 1];
                }
            }

            // Deletion: a reference element the hypothesis does not have.
            let deletion_cost = cost[i - 1][j] + 1;
            if deletion_cost < best_cost {
                best_cost = deletion_cost;
                best_ops = ops[i - 1][j];
                best_ops.deletions += 1;
            }

            // Insertion: an extra element the hypothesis invented.
            let insertion_cost = cost[i][j - 1] + 1;
            if insertion_cost < best_cost {
                best_cost = insertion_cost;
                best_ops = ops[i][j - 1];
                best_ops.insertions += 1;
            }

            cost[i][j] = best_cost;
            ops[i][j] = best_ops;
        }
    }

    ops[n][m]
}

/// Splits already-normalized text into comparison tokens.
fn tokenize(normalized: &str) -> Vec<String> {
    normalized
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// Collapses runs of whitespace into single spaces and trims the result.
fn collapse_whitespace(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    let mut pending_space = false;

    for ch in text.chars() {
        if ch.is_whitespace() {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
            }
            pending_space = false;
            collapsed.push(ch);
        }
    }

    collapsed
}

/// Whether the character is a combining diacritical mark.
fn is_combining_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{FE20}'..='\u{FE2F}')
}

/// ASCII fold for the Latin-1 Supplement and Latin Extended-A ranges.
///
/// Returns `None` for characters that need no folding. Characters outside
/// these ranges keep their original form, except for the combining marks
/// that [`is_combining_mark`] already drops.
fn ascii_fold(c: char) -> Option<&'static str> {
    match c {
        // Latin-1 Supplement, uppercase.
        'À'..='Å' => Some("a"),
        'Æ' => Some("ae"),
        'Ç' => Some("c"),
        'È'..='Ë' => Some("e"),
        'Ì'..='Ï' => Some("i"),
        'Ð' => Some("d"),
        'Ñ' => Some("n"),
        'Ò'..='Ö' => Some("o"),
        'Ø' => Some("o"),
        'Ù'..='Ü' => Some("u"),
        'Ý' | 'Ÿ' => Some("y"),
        'Þ' => Some("th"),
        'ß' => Some("ss"),
        // Latin-1 Supplement, lowercase.
        'à'..='å' => Some("a"),
        'æ' => Some("ae"),
        'ç' => Some("c"),
        'è'..='ë' => Some("e"),
        'ì'..='ï' => Some("i"),
        'ð' => Some("d"),
        'ñ' => Some("n"),
        'ò'..='ö' => Some("o"),
        'ø' => Some("o"),
        'ù'..='ü' => Some("u"),
        'ý' | 'ÿ' => Some("y"),
        'þ' => Some("th"),
        // Latin Extended-A.
        'Ā' | 'Ă' | 'Ą' => Some("a"),
        'ā' | 'ă' | 'ą' => Some("a"),
        'Ć' | 'Ĉ' | 'Ċ' | 'Č' => Some("c"),
        'ć' | 'ĉ' | 'ċ' | 'č' => Some("c"),
        'Ď' | 'Đ' => Some("d"),
        'ď' | 'đ' => Some("d"),
        'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => Some("e"),
        'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => Some("e"),
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => Some("g"),
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => Some("g"),
        'Ĥ' | 'Ħ' => Some("h"),
        'ĥ' | 'ħ' => Some("h"),
        'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => Some("i"),
        'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => Some("i"),
        'Ĳ' => Some("ij"),
        'ĳ' => Some("ij"),
        'Ĵ' => Some("j"),
        'ĵ' => Some("j"),
        'Ķ' => Some("k"),
        'ķ' => Some("k"),
        'ĸ' => Some("k"),
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => Some("l"),
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => Some("l"),
        'Ń' | 'Ņ' | 'Ň' | 'Ŋ' => Some("n"),
        'ń' | 'ņ' | 'ň' | 'ŋ' => Some("n"),
        'Ō' | 'Ŏ' | 'Ő' => Some("o"),
        'ō' | 'ŏ' | 'ő' => Some("o"),
        'Œ' => Some("oe"),
        'œ' => Some("oe"),
        'Ŕ' | 'Ŗ' | 'Ř' => Some("r"),
        'ŕ' | 'ŗ' | 'ř' => Some("r"),
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => Some("s"),
        'ś' | 'ŝ' | 'ş' | 'š' => Some("s"),
        'Ţ' | 'Ť' | 'Ŧ' => Some("t"),
        'ţ' | 'ť' | 'ŧ' => Some("t"),
        'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => Some("u"),
        'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => Some("u"),
        'Ŵ' => Some("w"),
        'ŵ' => Some("w"),
        'Ŷ' => Some("y"),
        'ŷ' => Some("y"),
        'Ź' | 'Ż' | 'Ž' => Some("z"),
        'ź' | 'ż' | 'ž' => Some("z"),
        'ſ' => Some("s"),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
