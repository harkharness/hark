//! What the microphone should EXPECT to hear.
//!
//! Whisper takes an "initial prompt" and biases its decoding toward words
//! it has just seen. Hark was feeding it a static, generic list — so a
//! session called "Gladius Hydrator" came back as "o gladus de dreater",
//! and "Hark" itself came back as "hack". The machine already knows the
//! right words: they are the names of the projects and the titles of the
//! sessions sitting in the local index. Free, and specific to one person.

use crate::domain::matching;

/// Compose the bias from what this machine talks about, newest last.
///
/// `configured` (config `vocab`) and `projects` come first: they are the
/// stable names. Session titles fill whatever budget is left. The cap
/// exists because whisper truncates its prompt — an overflowing list
/// silently loses the words that mattered.
pub fn speech_bias(
    configured: &[String],
    projects: &[String],
    titles: &[String],
    max_chars: usize,
) -> String {
    let mut seen: Vec<String> = Vec::new();
    let mut out = String::new();

    // Multi-word entries from the config are kept whole ("pull request");
    // titles are mined for their distinctive words — in their ORIGINAL
    // spelling. matching::significant() folds case and accents, which is
    // right for matching and wrong here: teaching whisper "ultima" and
    // "vigia" trains the misspelling we are trying to prevent.
    let stable = configured.iter().chain(projects.iter()).cloned();
    let mined = titles.iter().flat_map(|t| distinctive_words(t));

    for term in stable.chain(mined) {
        let term = term.trim().to_string();
        if term.chars().count() < 3 {
            continue;
        }
        let folded = matching::fold(&term);
        if seen.iter().any(|s| matching::fold(s) == folded) {
            continue;
        }
        let addition = if out.is_empty() { term.clone() } else { format!(", {term}") };
        if out.chars().count() + addition.chars().count() > max_chars {
            break;
        }
        out.push_str(&addition);
        seen.push(term);
    }
    out
}

/// Words from a title worth biasing toward, spelled as they were written.
/// Structural filler is dropped by comparing the FOLDED form against the
/// same stopword list the matcher uses.
fn distinctive_words(title: &str) -> Vec<String> {
    title
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.chars().count() >= 3)
        .filter(|w| !matching::significant(w).is_empty())
        .map(|w| w.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|i| i.to_string()).collect()
    }

    #[test]
    fn stable_names_come_first_and_titles_are_mined_for_topics() {
        let bias = speech_bias(
            &s(&["pull request"]),
            &s(&["workspace-alpha"]),
            &s(&["Pin Vigia chart na última versão"]),
            500,
        );
        assert!(bias.starts_with("pull request, workspace-alpha"), "{bias}");
        // Spelled the way it was written — capital H, and the accent on
        // "última". A bias that teaches the wrong spelling is worse than
        // no bias at all.
        assert!(bias.contains("Vigia"), "capitalisation survives: {bias}");
        assert!(bias.contains("última"), "the accent survives: {bias}");
        assert!(!bias.contains("ultima,"), "no folded duplicate: {bias}");
    }

    #[test]
    fn a_term_is_never_repeated_however_it_was_written() {
        let bias = speech_bias(&s(&["Vigia"]), &[], &s(&["vigia chart", "VIGIA rollout"]), 500);
        assert_eq!(bias.matches("eimdall").count(), 1, "{bias}");
    }

    #[test]
    fn the_budget_is_respected_without_cutting_a_word() {
        let bias = speech_bias(&s(&["alpha", "bravo", "charlie", "delta"]), &[], &[], 20);
        assert!(bias.chars().count() <= 20, "{bias}");
        assert!(!bias.ends_with(','), "{bias}");
        // Whatever fit is a whole word.
        for word in bias.split(", ") {
            assert!(["alpha", "bravo", "charlie", "delta"].contains(&word), "{word:?}");
        }
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert_eq!(speech_bias(&[], &[], &[], 500), "");
    }
}
