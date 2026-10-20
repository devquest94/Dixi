//! The shared text-matching engine.
//!
//! The dictionary, the snippets and the format rules of a writing profile all
//! need the same thing: find the rule that matches best at a position in the
//! text and replace it. This module is that one algorithm, so the three
//! features cannot drift apart in their behaviour.
//!
//! # Matching rules
//!
//! - **Longest wins.** A rule that matches more characters beats one that
//!   matches fewer, so `new york` is not rewritten as `new` followed by
//!   `york`.
//! - **Priority breaks ties.** Two rules of the same length are ordered by
//!   descending priority; if the priority is also equal, the order of the
//!   rules decides, so the resolution is deterministic.
//! - **One pass.** The engine walks the original text once. A replacement is
//!   never re-examined, so a rule cannot rewrite what another rule just
//!   wrote, and there is no way to loop forever.
//! - **Whole word.** A whole-word rule does not fire inside a longer word.
//! - **Case.** A case-insensitive rule matches in any case, and the casing
//!   of the matched text is preserved in the replacement.

/// A rule that can be matched against a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRule {
    /// The identifier of the rule, reported back when it fires.
    pub id: i64,
    /// The text that is looked for.
    pub needle: String,
    /// The text that is written in its place.
    pub replacement: String,
    /// Only match the needle as a complete word.
    pub whole_word: bool,
    /// Only match the needle with exactly this case.
    pub case_sensitive: bool,
    /// Higher priority wins between rules of the same length.
    pub priority: i32,
}

/// A rule that matched, with its offsets in the haystack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleMatch {
    pub rule_id: i64,
    /// Offset of the first matched character.
    pub start: usize,
    /// Offset just after the last matched character.
    pub end: usize,
}

/// The result of running a set of rules over a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleApplication {
    /// The rewritten text.
    pub text: String,
    /// The rules that fired, in the order they fired, without repeats.
    pub applied: Vec<i64>,
}

/// Finds the rule that matches best at or after `from`.
///
/// "Best" means the longest needle; ties are broken by descending priority
/// and then by the order of the rules.
pub fn find_longest_match(haystack: &[char], from: usize, rules: &[TextRule]) -> Option<RuleMatch> {
    // The best match and the priority that produced it: the priority has to
    // travel with the match, because the rules are not sorted and the id is
    // not the position.
    let mut best: Option<(RuleMatch, i32)> = None;

    for rule in rules {
        let needle: Vec<char> = rule.needle.chars().collect();
        if needle.is_empty() {
            continue;
        }
        if from + needle.len() > haystack.len() {
            continue;
        }
        if !matches_at(haystack, from, &needle, rule.case_sensitive) {
            continue;
        }
        if rule.whole_word && !has_word_boundaries(haystack, from, needle.len()) {
            continue;
        }

        let candidate = RuleMatch {
            rule_id: rule.id,
            start: from,
            end: from + needle.len(),
        };

        let better = match &best {
            None => true,
            Some((current, current_priority)) => {
                let current_len = current.end - current.start;
                let candidate_len = candidate.end - candidate.start;
                if candidate_len != current_len {
                    candidate_len > current_len
                } else {
                    rule.priority > *current_priority
                }
            }
        };

        if better {
            best = Some((candidate, rule.priority));
        }
    }

    best.map(|(found, _)| found)
}

/// Runs the rules over `text` in a single pass.
///
/// The result reports which rules fired, so the caller can count the uses and
/// honour the scopes.
pub fn apply_rules(text: &str, rules: &[TextRule]) -> RuleApplication {
    if text.is_empty() || rules.is_empty() {
        return RuleApplication {
            text: text.to_string(),
            applied: Vec::new(),
        };
    }

    let haystack: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut applied: Vec<i64> = Vec::new();
    let mut index = 0usize;

    while index < haystack.len() {
        match find_longest_match(&haystack, index, rules) {
            Some(found) => {
                let rule = rules
                    .iter()
                    .find(|rule| rule.id == found.rule_id)
                    .expect("the match comes from a rule");
                let matched: String = haystack[found.start..found.end].iter().collect();
                out.push_str(&apply_case_style(&rule.replacement, &matched));
                if !applied.contains(&found.rule_id) {
                    applied.push(found.rule_id);
                }
                index = found.end;
            }
            None => {
                out.push(haystack[index]);
                index += 1;
            }
        }
    }

    RuleApplication { text: out, applied }
}

/// Whether `needle` matches at `start`, honouring case sensitivity.
fn matches_at(haystack: &[char], start: usize, needle: &[char], case_sensitive: bool) -> bool {
    for (offset, expected) in needle.iter().enumerate() {
        let actual = haystack[start + offset];
        if case_sensitive {
            if actual != *expected {
                return false;
            }
        } else if !actual.eq_ignore_ascii_case(expected)
            && !actual.to_lowercase().eq(expected.to_lowercase())
        {
            return false;
        }
    }
    true
}

/// Whether the match is not glued to a word character on either side.
fn has_word_boundaries(haystack: &[char], start: usize, len: usize) -> bool {
    let before_ok = start == 0 || !is_word_char(haystack[start - 1]);
    let after_index = start + len;
    let after_ok = after_index >= haystack.len() || !is_word_char(haystack[after_index]);
    before_ok && after_ok
}

/// Whether the character is part of a word.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Writes `replacement` with the casing style of `matched`.
///
/// An all-caps match yields an all-caps replacement, a title-case match
/// yields a title-case replacement, and anything else yields the replacement
/// verbatim.
pub fn apply_case_style(replacement: &str, matched: &str) -> String {
    let letters: Vec<char> = matched.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return replacement.to_string();
    }

    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return replacement.to_uppercase();
    }

    let first_upper = letters[0].is_uppercase();
    let rest_lower = letters[1..].iter().all(|c| c.is_lowercase());
    if first_upper && rest_lower {
        let mut chars = replacement.chars();
        return match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => replacement.to_string(),
        };
    }

    replacement.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, needle: &str, replacement: &str) -> TextRule {
        TextRule {
            id,
            needle: needle.to_string(),
            replacement: replacement.to_string(),
            whole_word: true,
            case_sensitive: false,
            priority: 0,
        }
    }

    #[test]
    fn the_longest_rule_wins() {
        let rules = vec![rule(1, "new", "nuevo"), rule(2, "new york", "Nueva York")];
        let result = apply_rules("voy a new york", &rules);
        assert_eq!(result.text, "voy a Nueva York");
        assert_eq!(result.applied, vec![2]);
    }

    #[test]
    fn the_priority_breaks_a_tie() {
        let mut preferred = rule(1, "equipo", "equipo interno");
        preferred.priority = 10;
        let fallback = rule(2, "equipo", "equipo de la casa");
        let result = apply_rules("el equipo", &[fallback, preferred]);
        assert_eq!(result.text, "el equipo interno");
    }

    #[test]
    fn a_whole_word_rule_does_not_fire_inside_a_word() {
        let result = apply_rules("el perrote", &[rule(1, "perro", "Dixi")]);
        assert_eq!(result.text, "el perrote");
        assert!(result.applied.is_empty());
    }

    #[test]
    fn a_substring_rule_fires_inside_a_word() {
        let mut substring = rule(1, "js", "JavaScript");
        substring.whole_word = false;
        let result = apply_rules("un jsx", &[substring]);
        assert_eq!(result.text, "un JavaScriptx");
    }

    #[test]
    fn the_replacement_is_never_rewritten_again() {
        // "a" -> "ab" would loop forever if the engine cascaded.
        let result = apply_rules("a", &[rule(1, "a", "ab")]);
        assert_eq!(result.text, "ab");
    }

    #[test]
    fn the_case_of_the_match_is_preserved() {
        let rules = vec![rule(1, "dixi", "Dixi")];
        assert_eq!(apply_rules("dixi", &rules).text, "Dixi");
        assert_eq!(apply_rules("DIXI", &rules).text, "DIXI");
        assert_eq!(apply_rules("Dixi", &rules).text, "Dixi");
    }

    #[test]
    fn a_case_sensitive_rule_ignores_other_cases() {
        let mut sensitive = rule(1, "Dixi", "Dixi");
        sensitive.case_sensitive = true;
        let result = apply_rules("dixi y Dixi", &[sensitive]);
        assert_eq!(result.text, "dixi y Dixi");
    }

    #[test]
    fn an_empty_rule_set_leaves_the_text_untouched() {
        let result = apply_rules("sin cambios", &[]);
        assert_eq!(result.text, "sin cambios");
        assert!(result.applied.is_empty());
    }

    #[test]
    fn an_empty_needle_never_matches() {
        let result = apply_rules("texto", &[rule(1, "", "x")]);
        assert_eq!(result.text, "texto");
    }

    #[test]
    fn multibyte_text_is_handled() {
        let result = apply_rules("el ñandú corre", &[rule(1, "ñandú", "ñandú")]);
        assert_eq!(result.text, "el ñandú corre");
    }
}
