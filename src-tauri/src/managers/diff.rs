//! A word-level diff between two texts.
//!
//! It is what the user sees before a transformation replaces something: which
//! words were added, which were removed and which stayed.
//!
//! The comparison is done on words, not on characters: a diff per character
//! is unreadable for prose, and a diff per paragraph hides the interesting
//! changes.
//!
//! The algorithm is the classic longest common subsequence, which gives the
//! smallest possible set of changes. It is quadratic in the number of words,
//! so a guard keeps it bounded: a very long pair is reported as a single
//! "the whole text changed" instead of spending minutes on it.

/// A part of the diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffSegmentKind {
    /// The text is in both versions.
    Equal,
    /// The text is only in the result.
    Inserted,
    /// The text is only in the original.
    Removed,
}

/// One part of the diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffSegment {
    pub kind: DiffSegmentKind,
    pub text: String,
}

/// The largest number of words for which the exact diff is computed.
pub const MAX_WORDS_FOR_EXACT_DIFF: usize = 2000;

/// The result of comparing two texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDiff {
    pub segments: Vec<DiffSegment>,
    /// How many words were added.
    pub inserted_words: usize,
    /// How many words were removed.
    pub removed_words: usize,
}

impl TextDiff {
    /// Whether the two texts are the same.
    pub fn is_identical(&self) -> bool {
        self.inserted_words == 0 && self.removed_words == 0
    }

    /// The text as it would read with the changes applied.
    pub fn result_text(&self) -> String {
        self.segments
            .iter()
            .filter(|segment| segment.kind != DiffSegmentKind::Removed)
            .map(|segment| segment.text.as_str())
            .collect()
    }
}

/// Splits a text into words, keeping the separators.
///
/// The words and the spaces are both preserved so that joining the pieces
/// reproduces the original text exactly.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        if c.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(c.to_string());
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Compares two texts word by word.
pub fn diff_words(original: &str, result: &str) -> TextDiff {
    if original == result {
        return TextDiff {
            segments: vec![DiffSegment {
                kind: DiffSegmentKind::Equal,
                text: original.to_string(),
            }],
            inserted_words: 0,
            removed_words: 0,
        };
    }

    let before = tokenize(original);
    let after = tokenize(result);

    if before.len() + after.len() > MAX_WORDS_FOR_EXACT_DIFF {
        // Too long to compare exactly: report the change as a whole.
        return TextDiff {
            segments: vec![
                DiffSegment {
                    kind: DiffSegmentKind::Removed,
                    text: original.to_string(),
                },
                DiffSegment {
                    kind: DiffSegmentKind::Inserted,
                    text: result.to_string(),
                },
            ],
            inserted_words: count_word_tokens(&after),
            removed_words: count_word_tokens(&before),
        };
    }

    // The longest common subsequence table.
    let mut table = vec![vec![0_usize; after.len() + 1]; before.len() + 1];
    for i in (0..before.len()).rev() {
        for j in (0..after.len()).rev() {
            table[i][j] = if before[i] == after[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }

    let mut segments: Vec<DiffSegment> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;

    while i < before.len() && j < after.len() {
        if before[i] == after[j] {
            push_segment(&mut segments, DiffSegmentKind::Equal, &before[i]);
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            push_segment(&mut segments, DiffSegmentKind::Removed, &before[i]);
            i += 1;
        } else {
            push_segment(&mut segments, DiffSegmentKind::Inserted, &after[j]);
            j += 1;
        }
    }
    while i < before.len() {
        push_segment(&mut segments, DiffSegmentKind::Removed, &before[i]);
        i += 1;
    }
    while j < after.len() {
        push_segment(&mut segments, DiffSegmentKind::Inserted, &after[j]);
        j += 1;
    }

    // The counts are WORDS, not tokens: the spaces are tokens too, and a
    // diff that counts them would report two changes where there is one.
    let inserted = segments
        .iter()
        .filter(|segment| segment.kind == DiffSegmentKind::Inserted)
        .map(|segment| count_words(&segment.text))
        .sum();
    let removed = segments
        .iter()
        .filter(|segment| segment.kind == DiffSegmentKind::Removed)
        .map(|segment| count_words(&segment.text))
        .sum();

    TextDiff {
        segments,
        inserted_words: inserted,
        removed_words: removed,
    }
}

/// Appends a piece of text to the last segment when it has the same kind.
fn push_segment(segments: &mut Vec<DiffSegment>, kind: DiffSegmentKind, text: &str) {
    if let Some(last) = segments.last_mut() {
        if last.kind == kind {
            last.text.push_str(text);
            return;
        }
    }
    segments.push(DiffSegment {
        kind,
        text: text.to_string(),
    });
}

/// Counts the words of a text, ignoring the spaces.
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Counts the word tokens of a tokenized text.
fn count_word_tokens(tokens: &[String]) -> usize {
    tokens.iter().filter(|token| !token.trim().is_empty()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inserted(diff: &TextDiff) -> String {
        diff.segments
            .iter()
            .filter(|s| s.kind == DiffSegmentKind::Inserted)
            .map(|s| s.text.as_str())
            .collect::<String>()
            .trim()
            .to_string()
    }

    fn removed(diff: &TextDiff) -> String {
        diff.segments
            .iter()
            .filter(|s| s.kind == DiffSegmentKind::Removed)
            .map(|s| s.text.as_str())
            .collect::<String>()
            .trim()
            .to_string()
    }

    #[test]
    fn identical_texts_have_no_changes() {
        let diff = diff_words("hola mundo", "hola mundo");
        assert!(diff.is_identical());
        assert_eq!(diff.inserted_words, 0);
        assert_eq!(diff.removed_words, 0);
    }

    #[test]
    fn an_added_word_is_reported() {
        let diff = diff_words("hola mundo", "hola gran mundo");
        assert_eq!(inserted(&diff), "gran");
        assert_eq!(diff.inserted_words, 1);
        assert_eq!(diff.removed_words, 0);
    }

    #[test]
    fn a_removed_word_is_reported() {
        let diff = diff_words("hola gran mundo", "hola mundo");
        assert_eq!(removed(&diff), "gran");
        assert_eq!(diff.removed_words, 1);
    }

    #[test]
    fn a_changed_word_is_a_removal_and_an_insertion() {
        let diff = diff_words("el perro", "el gato");
        assert_eq!(removed(&diff), "perro");
        assert_eq!(inserted(&diff), "gato");
    }

    #[test]
    fn the_result_text_is_the_new_text() {
        let original = "el perro ladra mucho";
        let result = "el gato ladra poco";
        let diff = diff_words(original, result);
        assert_eq!(diff.result_text(), result);
    }

    #[test]
    fn the_diff_is_readable_in_segments() {
        let diff = diff_words("uno dos tres", "uno cuatro tres");
        let kinds: Vec<DiffSegmentKind> = diff.segments.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&DiffSegmentKind::Equal));
        assert!(kinds.contains(&DiffSegmentKind::Inserted));
        assert!(kinds.contains(&DiffSegmentKind::Removed));
    }

    #[test]
    fn an_empty_original_is_all_inserted() {
        let diff = diff_words("", "hola");
        assert_eq!(inserted(&diff), "hola");
        assert_eq!(diff.removed_words, 0);
    }

    #[test]
    fn an_empty_result_is_all_removed() {
        let diff = diff_words("hola", "");
        assert_eq!(removed(&diff), "hola");
        assert_eq!(diff.inserted_words, 0);
    }

    #[test]
    fn a_very_long_pair_is_reported_as_a_whole_change() {
        let long = "palabra ".repeat(MAX_WORDS_FOR_EXACT_DIFF);
        let diff = diff_words(&long, "otra cosa");
        assert_eq!(diff.segments.len(), 2);
        assert_eq!(diff.removed_words, MAX_WORDS_FOR_EXACT_DIFF);
    }

    #[test]
    fn the_punctuation_and_the_spaces_are_preserved() {
        let original = "Hola, mundo.  Adios.";
        let diff = diff_words(original, original);
        assert_eq!(diff.result_text(), original);
    }
}
