//! Spoken verdicts on anything the app is waiting on: dispatch confirms,
//! permission cards, candidate pickers, actionable warnings. STT is fuzzy,
//! so the grammar is forgiving on wording but conservative on outcome —
//! noise must never confirm.

use crate::domain::intent::ACTION_VERBS;
use crate::domain::matching;

/// What the utterance decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Go ahead ("sim", "pode seguir"); `always` records a standing rule.
    Confirm { always: bool },
    /// Stop ("não", "cancela").
    Deny,
    /// One of the offered options, zero-based ("a primeira", "número dois",
    /// or enough of the option's own name).
    Pick(usize),
    /// A registered action phrase ("compacta antes") — returns the id.
    Action(String),
    /// A replacement/new instruction (the user rephrased the message).
    Instruction(String),
    /// Unintelligible for this decision: never guess.
    Unknown,
}

/// One offered action: (id, spoken trigger phrases, folded).
pub type ActionPhrases<'a> = (&'a str, &'a [&'a str]);

/// Interpret an utterance against a pending decision surface.
/// `option_labels` are the visible choices (empty = no picker); `actions`
/// are extra verbs the surface offers (empty = none).
/// First tokens that green-light on their own ("ok" is out: STT noise).
const ALLOW: &[&str] = &[
    "sim", "pode", "bora", "vai", "manda", "confirma", "confirmo", "confirmado",
    "permite", "permito", "permitir", "autoriza", "autorizar", "autorizado",
    "aprova", "aprovo", "aprovar", "aprovado", "libera", "liberar", "liberado",
    "segue", "seguir", "isso", "exato", "positivo", "beleza", "fechado", "fechou",
    // English. "ok" stays out in both languages: STT hears it in noise.
    "yes", "yeah", "yep", "yup", "sure", "confirm", "confirmed", "approve",
    "approved", "allow", "allowed", "permit", "proceed", "correct", "exactly",
    "perfect", "affirmative",
];
/// Two-token greenlights: the verb alone is too common to trust ("go to the
/// board" is not consent), the pair is unmistakable.
const ALLOW_PHRASES: &[[&str; 2]] = &[
    ["go", "ahead"], ["do", "it"], ["sounds", "good"], ["looks", "good"],
    ["that", "works"], ["thats", "right"], ["go", "for"],
];
/// Prefixes that turn a confirm into a standing rule.
const ALWAYS_STARTS: &[&str] = &["sempre", "always"];
/// First tokens that refuse on their own. English "no" is NOT here: it is
/// also Portuguese for "in the", so it refuses only as the whole utterance.
const DENY_STARTS: &[&str] = &[
    "nao", "nega", "negar", "cancela", "cancelar", "bloqueia", "bloquear",
    "esquece", "aborta", "abortar", "para", "pare",
    "nope", "nah", "cancel", "stop", "abort", "deny", "denied", "reject",
    "forget", "skip",
];
/// Two-token refusals, for the same reason ALLOW_PHRASES exists.
const DENY_PHRASES: &[[&str; 2]] = &[["no", "way"], ["no", "thanks"]];
/// Words that poison a confirm anywhere in the sentence.
const NEGATION: &[&str] = &["nao", "nunca", "jamais", "nem", "not", "never"];

/// Ordinal/number the utterance points at (zero-based), if any.
fn spoken_index(words: &[&str]) -> Option<usize> {
    const ORDINALS: &[(&str, usize)] = &[
        ("primeiro", 0), ("primeira", 0), ("segundo", 1), ("segunda", 1),
        ("terceiro", 2), ("terceira", 2), ("quarto", 3), ("quarta", 3),
        ("quinto", 4), ("quinta", 4),
        ("first", 0), ("second", 1), ("third", 2), ("fourth", 3), ("fifth", 4),
    ];
    // Cardinals only right after "número"/"opção"/"number" — "dois" and
    // "two" alone are prose.
    const CARDINALS: &[(&str, usize)] = &[
        ("dois", 1), ("duas", 1), ("tres", 2), ("quatro", 3), ("cinco", 4),
        ("one", 0), ("two", 1), ("three", 2), ("four", 3), ("five", 4),
    ];
    for (i, w) in words.iter().enumerate() {
        if let Some((_, n)) = ORDINALS.iter().find(|(o, _)| o == w) {
            return Some(*n);
        }
        if let Ok(d) = w.parse::<usize>() {
            return d.checked_sub(1);
        }
        let after_marker =
            i > 0 && matches!(words[i - 1], "numero" | "opcao" | "number" | "option");
        if after_marker {
            if let Some((_, n)) = CARDINALS.iter().find(|(c, _)| c == w) {
                return Some(*n);
            }
        }
    }
    None
}

pub fn interpret(
    utterance: &str,
    option_labels: &[&str],
    actions: &[ActionPhrases],
) -> Verdict {
    let folded = matching::fold(utterance);
    // tokens(): punctuation-proof — STT and typing produce "sim, pode".
    let tokens = matching::tokens(utterance);
    let words: Vec<&str> = tokens.iter().map(String::as_str).collect();
    if words.is_empty() {
        return Verdict::Unknown;
    }

    // 1. Action phrases the surface itself offered win outright.
    for (id, phrases) in actions {
        if phrases.iter().any(|p| folded.contains(&matching::fold(p))) {
            return Verdict::Action((*id).to_string());
        }
    }

    // 2. Short utterances are verdicts (whole tokens: "assim" ≠ "sim").
    let negated = words.iter().any(|w| NEGATION.contains(w));
    let pair = |set: &[[&str; 2]]| {
        words.len() >= 2 && set.iter().any(|p| p[0] == words[0] && p[1] == words[1])
    };
    if words.len() <= 4 {
        // A bare "no" refuses; "no projeto X" addresses a target.
        let refused = negated
            || DENY_STARTS.contains(&words[0])
            || (words[0] == "no" && words.len() == 1)
            || pair(DENY_PHRASES);
        if refused {
            return Verdict::Deny;
        }
        let (always, rest) = match ALWAYS_STARTS.contains(&words[0]) {
            true => (true, &words[1..]),
            false => (false, &words[..]),
        };
        if rest.first().is_some_and(|w| ALLOW.contains(w)) || pair(ALLOW_PHRASES) {
            return Verdict::Confirm { always };
        }
    }

    // 3. Picking one of the offered options, by position or by name.
    if !option_labels.is_empty() {
        if let Some(i) = spoken_index(&words) {
            return if i < option_labels.len() { Verdict::Pick(i) } else { Verdict::Unknown };
        }
        if words.len() <= 6 {
            let terms = matching::significant(utterance);
            if !terms.is_empty() {
                let hits: Vec<usize> = option_labels
                    .iter()
                    .enumerate()
                    .filter(|(_, label)| {
                        let label_words: std::collections::HashSet<String> =
                            matching::tokens(label).into_iter().collect();
                        terms.iter().all(|t| label_words.contains(t))
                    })
                    .map(|(i, _)| i)
                    .collect();
                match hits.as_slice() {
                    [only] => return Verdict::Pick(*only),
                    [_, _, ..] => return Verdict::Unknown, // ambiguous pick
                    [] => {}
                }
            }
        }
    }

    // 4. A real sentence is the user rephrasing the message.
    let has_verb = words
        .iter()
        .any(|w| ACTION_VERBS.iter().any(|v| matching::fold(v) == **w));
    if words.len() >= 5 || has_verb {
        return Verdict::Instruction(utterance.trim().to_string());
    }
    Verdict::Unknown
}

/// Permission-card shortcut: Some((allow, always)) when the utterance is a
/// clean verdict, None otherwise (the message is normal conversation).
pub fn interpret_permission(utterance: &str) -> Option<(bool, bool)> {
    match interpret(utterance, &[], &[]) {
        Verdict::Confirm { always } => Some((true, always)),
        Verdict::Deny => Some((false, false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(utterance: &str) -> Verdict {
        interpret(utterance, &[], &[])
    }

    #[test]
    fn sim_confirms_and_assim_does_not() {
        assert_eq!(v("sim"), Verdict::Confirm { always: false });
        assert_eq!(v("Sim, pode mandar"), Verdict::Confirm { always: false });
        assert_eq!(v("pode seguir"), Verdict::Confirm { always: false });
        assert_eq!(v("confirmado pode seguir"), Verdict::Confirm { always: false });
        // "assim" contains "sim" — token match only, never substring.
        assert_eq!(v("assim"), Verdict::Unknown);
        assert_eq!(v("assim que der"), Verdict::Unknown);
    }

    #[test]
    fn sempre_pode_confirms_with_standing_rule() {
        assert_eq!(v("sempre pode"), Verdict::Confirm { always: true });
        assert_eq!(v("sempre permite"), Verdict::Confirm { always: true });
        assert_eq!(interpret_permission("sempre pode"), Some((true, true)));
        assert_eq!(interpret_permission("nega"), Some((false, false)));
        assert_eq!(interpret_permission("bom dia"), None);
    }

    #[test]
    fn nao_and_cancela_deny() {
        assert_eq!(v("não"), Verdict::Deny);
        assert_eq!(v("nao"), Verdict::Deny);
        assert_eq!(v("cancela"), Verdict::Deny);
        assert_eq!(v("não roda isso"), Verdict::Deny);
    }

    #[test]
    fn negation_anywhere_blocks_confirm() {
        // A "sim" glued to a negation is not consent.
        assert_eq!(v("sim mas não agora"), Verdict::Deny);
        assert_eq!(v("pode não"), Verdict::Deny);
    }

    #[test]
    fn ordinals_pick_options_pt_br() {
        let labels = ["Migração Assinaturas uat", "Migração Assinaturas prod", "Webhook v2"];
        assert_eq!(interpret("a primeira", &labels, &[]), Verdict::Pick(0));
        assert_eq!(interpret("o segundo", &labels, &[]), Verdict::Pick(1));
        assert_eq!(interpret("número dois", &labels, &[]), Verdict::Pick(1));
        assert_eq!(interpret("opção 3", &labels, &[]), Verdict::Pick(2));
        // Out of range never picks.
        assert_eq!(interpret("a quarta", &labels, &[]), Verdict::Unknown);
        // No options offered = ordinals mean nothing.
        assert_eq!(interpret("a primeira", &[], &[]), Verdict::Unknown);
    }

    #[test]
    fn option_picked_by_spoken_name() {
        let labels = ["Migração Assinaturas uat", "Migração Assinaturas prod", "Webhook v2"];
        assert_eq!(interpret("a do webhook", &labels, &[]), Verdict::Pick(2));
        assert_eq!(interpret("assinaturas prod", &labels, &[]), Verdict::Pick(1));
        // Matching more than one option is not a pick.
        assert_eq!(interpret("a de assinaturas", &labels, &[]), Verdict::Unknown);
    }

    #[test]
    fn registered_action_phrases_route() {
        let actions: &[ActionPhrases] = &[
            ("compact_first", &["compacta antes", "compactar antes"]),
            ("proceed", &["segue assim mesmo", "segue mesmo assim"]),
        ];
        assert_eq!(
            interpret("compacta antes de executar", &[], actions),
            Verdict::Action("compact_first".into())
        );
        assert_eq!(
            interpret("pode seguir mesmo assim", &[], actions),
            Verdict::Confirm { always: false }
        );
        assert_eq!(
            interpret("segue mesmo assim", &[], actions),
            Verdict::Action("proceed".into())
        );
    }

    #[test]
    fn long_sentence_becomes_replacement_instruction() {
        let text = "verifica se o domínio público ainda recebe tráfego no cluster antigo";
        assert_eq!(v(text), Verdict::Instruction(text.into()));
    }

    #[test]
    fn noise_is_unknown_never_confirm() {
        assert_eq!(v(""), Verdict::Unknown);
        assert_eq!(v("hm"), Verdict::Unknown);
        assert_eq!(v("ok"), Verdict::Unknown); // STT noise word, deliberately out
        assert_eq!(v("bom dia"), Verdict::Unknown);
    }
}

/// English is a first-class spoken language, not a translation layer: the
/// same grammar accepts both, and the words that mean different things in
/// each ("no" is Portuguese for "in the") are disambiguated, never dropped.
#[cfg(test)]
mod bilingual {
    use super::*;

    fn v(utterance: &str) -> Verdict {
        interpret(utterance, &[], &[])
    }

    #[test]
    fn english_confirms() {
        assert_eq!(v("yes"), Verdict::Confirm { always: false });
        assert_eq!(v("yeah, go for it"), Verdict::Confirm { always: false });
        assert_eq!(v("sure"), Verdict::Confirm { always: false });
        assert_eq!(v("approve"), Verdict::Confirm { always: false });
        assert_eq!(v("go ahead"), Verdict::Confirm { always: false });
        assert_eq!(v("do it"), Verdict::Confirm { always: false });
        assert_eq!(v("sounds good"), Verdict::Confirm { always: false });
    }

    #[test]
    fn always_records_a_standing_rule_in_english() {
        assert_eq!(v("always allow"), Verdict::Confirm { always: true });
        assert_eq!(interpret_permission("always allow"), Some((true, true)));
    }

    #[test]
    fn english_refusals_deny() {
        assert_eq!(v("no"), Verdict::Deny);
        assert_eq!(v("nope"), Verdict::Deny);
        assert_eq!(v("cancel"), Verdict::Deny);
        assert_eq!(v("stop"), Verdict::Deny);
        assert_eq!(v("no way"), Verdict::Deny);
        assert_eq!(interpret_permission("deny"), Some((false, false)));
    }

    #[test]
    fn portuguese_no_addressing_a_target_is_not_a_refusal() {
        // "no" is English for refusal AND Portuguese for "in the". A bare
        // "no" refuses; "no projeto X" is where the work goes.
        assert_ne!(v("no projeto webhook"), Verdict::Deny);
        assert_ne!(v("no chat de billing"), Verdict::Deny);
    }

    #[test]
    fn english_negation_anywhere_blocks_confirm() {
        assert_eq!(v("yes but not now"), Verdict::Deny);
        assert_eq!(v("never do that"), Verdict::Deny);
    }

    #[test]
    fn english_ordinals_and_numbers_pick_options() {
        let opts = ["billing endpoint", "webhook retry", "dns cleanup"];
        assert_eq!(interpret("the first one", &opts, &[]), Verdict::Pick(0));
        assert_eq!(interpret("second", &opts, &[]), Verdict::Pick(1));
        assert_eq!(interpret("number three", &opts, &[]), Verdict::Pick(2));
    }

    #[test]
    fn an_english_sentence_replaces_the_instruction() {
        assert_eq!(
            v("open the pull request"),
            Verdict::Instruction("open the pull request".into())
        );
        assert_eq!(
            v("run the integration tests instead"),
            Verdict::Instruction("run the integration tests instead".into())
        );
    }

    #[test]
    fn english_pleasantries_are_never_a_verdict() {
        assert_eq!(v("good morning"), Verdict::Unknown);
        assert_eq!(interpret_permission("good morning"), None);
    }
}
