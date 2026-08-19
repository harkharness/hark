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
];
/// First tokens that refuse on their own.
const DENY_STARTS: &[&str] = &[
    "nao", "nega", "negar", "cancela", "cancelar", "bloqueia", "bloquear",
    "esquece", "aborta", "abortar", "para", "pare",
];
/// Words that poison a confirm anywhere in the sentence.
const NEGATION: &[&str] = &["nao", "nunca", "jamais", "nem"];

/// Ordinal/number the utterance points at (zero-based), if any.
fn spoken_index(words: &[&str]) -> Option<usize> {
    const ORDINALS: &[(&str, usize)] = &[
        ("primeiro", 0), ("primeira", 0), ("segundo", 1), ("segunda", 1),
        ("terceiro", 2), ("terceira", 2), ("quarto", 3), ("quarta", 3),
        ("quinto", 4), ("quinta", 4),
    ];
    // Cardinals only right after "número"/"opção" — "dois" alone is prose.
    const CARDINALS: &[(&str, usize)] = &[
        ("dois", 1), ("duas", 1), ("tres", 2), ("quatro", 3), ("cinco", 4),
    ];
    for (i, w) in words.iter().enumerate() {
        if let Some((_, n)) = ORDINALS.iter().find(|(o, _)| o == w) {
            return Some(*n);
        }
        if let Ok(d) = w.parse::<usize>() {
            return d.checked_sub(1);
        }
        let after_marker = i > 0 && matches!(words[i - 1], "numero" | "opcao");
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
    if words.len() <= 4 {
        if negated || DENY_STARTS.contains(&words[0]) {
            return Verdict::Deny;
        }
        let (always, rest) = match words[0] {
            "sempre" => (true, &words[1..]),
            _ => (false, &words[..]),
        };
        if rest.first().is_some_and(|w| ALLOW.contains(w)) {
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
