//! The butler moment.
//!
//! A worker finishes in a chat the user is NOT looking at. The right
//! behaviour is a colleague's, not a notification system's: finish the
//! sentence being spoken (the TTS queue guarantees that), then offer —
//! "no chat do pin, terminei a demanda. Quer que eu faça algo lá, ou
//! seguimos aqui?" — and route the user's NEXT sentence accordingly.
//!
//! The interpretation is pure and zero-token: the offer defines a tiny
//! reply grammar, and anything outside it means the user moved on — the
//! sentence falls through to the normal flow untouched.

use crate::domain::matching;

/// What the user's reply to the offer meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUp {
    /// "vai lá", "abre", "sim" — front that chat.
    GoThere,
    /// "continua aqui", "depois", "não" — drop the offer, stay.
    StayHere,
    /// "faz X lá" / "responde que Y" — dispatch into the finished chat.
    DoThere(String),
    /// Anything else: not an answer to the offer at all.
    Unrelated,
}

/// First tokens that accept the offer on their own.
const GO: &[&str] = &[
    "vai", "volta", "abre", "abra", "mostra", "bora", "sim", "yes", "go",
    "open", "show",
];
/// Refusals and "later" — the offer dies, the user stays.
const STAY: &[&str] = &[
    "nao", "not", "no", "depois", "later", "deixa", "esquece", "skip",
    "fica", "stay", "continua", "continue", "segue",
];
/// Words that aim the sentence at the OTHER chat ("lá").
const THERE: &[&str] = &["la", "there", "nele", "nela"];

/// Interpret the next utterance against a pending offer.
pub fn interpret(utterance: &str, lang_hint_label: &str) -> FollowUp {
    let tokens = matching::tokens(utterance);
    let words: Vec<&str> = tokens.iter().map(String::as_str).collect();
    if words.is_empty() {
        return FollowUp::Unrelated;
    }

    let mentions_there = words.iter().any(|w| THERE.contains(w));
    let mentions_label = {
        let label_words: std::collections::HashSet<String> =
            matching::tokens(lang_hint_label).into_iter().collect();
        matching::significant(utterance)
            .iter()
            .any(|t| label_words.contains(t))
    };

    // Short reply, first word decides — the same shape as the verdict
    // grammar, but scoped to this one offer.
    if words.len() <= 4 {
        if STAY.contains(&words[0]) {
            return FollowUp::StayHere;
        }
        if GO.contains(&words[0]) || (mentions_there && words.len() <= 2) {
            return FollowUp::GoThere;
        }
    }

    // A sentence with real work aimed at "lá" (or naming the finished
    // chat) dispatches THERE, in the user's own words.
    let has_verb = words
        .iter()
        .any(|w| crate::domain::intent::is_action_verb(w));
    if has_verb && (mentions_there || mentions_label) {
        return FollowUp::DoThere(utterance.trim().to_string());
    }

    FollowUp::Unrelated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_acceptances_go_there() {
        for say in ["vai lá", "sim", "abre", "volta lá", "mostra", "go there", "lá"] {
            assert_eq!(interpret(say, "pin"), FollowUp::GoThere, "{say:?}");
        }
    }

    #[test]
    fn refusals_and_later_stay_here() {
        for say in ["não", "continua aqui", "depois", "deixa", "esquece", "not now"] {
            assert_eq!(interpret(say, "pin"), FollowUp::StayHere, "{say:?}");
        }
    }

    #[test]
    fn work_aimed_at_the_finished_chat_dispatches_there() {
        assert_eq!(
            interpret("atualiza o changelog lá", "pin"),
            FollowUp::DoThere("atualiza o changelog lá".into())
        );
        assert_eq!(
            interpret("no pin, roda os testes de novo", "pin vigia chart"),
            FollowUp::DoThere("no pin, roda os testes de novo".into())
        );
    }

    #[test]
    fn everything_else_falls_through_untouched() {
        // The user moved on: a new question or work for the CURRENT chat
        // must reach the normal flow, not be captured by a stale offer.
        for say in [
            "quanto gastei hoje",
            "atualiza o artefato pro time",
            "e aí, como ficou o rollout?",
        ] {
            assert_eq!(interpret(say, "pin"), FollowUp::Unrelated, "{say:?}");
        }
    }
}
