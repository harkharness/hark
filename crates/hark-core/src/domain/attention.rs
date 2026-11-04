//! Attention: the words that move Hark's focus from Início — "foca no
//! billing", "calma, vamos focar nessa", "volta", "pausa". Pure: the
//! shell resolves the target against the board, the funnel reuses this
//! for the spoken word.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Attention {
    /// Pin Início to one chat. An empty target means "the one we were
    /// talking about": the shell picks the demand on screen.
    Focus { target: String },
    /// Out of the focus, back to the whole of Início.
    Leave,
}

/// The whole sentence, nothing else: a verb inside a longer order ("pausa
/// o worker") is not an order to Hark's attention.
const LEAVE: [&str; 14] = [
    "volta", "voltar", "pausa", "pausar", "pausa o foco", "pausar o foco", "sai do foco", "sair do foco",
    "tira o foco", "back", "pause", "leave the focus", "leave focus", "unfocus",
];
/// A marker and what follows it is the target.
const FOCUS: [&str; 10] = [
    "foca no ", "foca na ", "foca em ", "focar no ", "focar na ", "focar em ", "foco no ", "foco na ",
    "focus on ", "focusing on ",
];
/// "Foca nessa": the target is the thing on screen.
const FOCUS_THIS: [&str; 8] = ["foca nessa", "foca nesse", "foca nisso", "focar nessa", "focar nesse", "focar nisso", "focus on this", "focus on that"];
const TRAILING: [&str; 4] = [" agora", " now", " por favor", " please"];
const LEADING: [&str; 4] = ["o ", "a ", "the ", "na "];

pub fn parse(text: &str) -> Option<Attention> {
    let t = normalize(text);
    if LEAVE.contains(&t.as_str()) {
        return Some(Attention::Leave);
    }
    if FOCUS_THIS.iter().any(|m| t.contains(m)) {
        return Some(Attention::Focus { target: String::new() });
    }
    let (_, rest) = FOCUS.iter().find_map(|m| t.find(m).map(|i| (i, &t[i + m.len()..])))?;
    let mut target = rest.trim().to_string();
    for tail in TRAILING {
        if let Some(cut) = target.strip_suffix(tail) {
            target = cut.trim().to_string();
        }
    }
    for lead in LEADING {
        if let Some(cut) = target.strip_prefix(lead) {
            target = cut.trim().to_string();
        }
    }
    Some(Attention::Focus { target })
}

/// Lowercase, no accents, no trailing punctuation.
fn normalize(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            _ => c,
        })
        .collect::<String>()
        .trim_end_matches(['.', '!', '?', ','])
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foca_no_x_names_the_target() {
        assert_eq!(parse("foca no billing"), Some(Attention::Focus { target: "billing".into() }));
        assert_eq!(parse("Calma. Vamos focar no billing agora"), Some(Attention::Focus { target: "billing".into() }));
        assert_eq!(parse("focus on the retry queue"), Some(Attention::Focus { target: "retry queue".into() }));
        assert_eq!(parse("let's focus on billing"), Some(Attention::Focus { target: "billing".into() }));
        assert_eq!(parse("foca nessa"), Some(Attention::Focus { target: String::new() }));
    }

    #[test]
    fn volta_and_pausa_leave_the_focus() {
        for s in ["volta", "Volta.", "pausa", "pausar o foco", "sai do foco", "back", "pause", "leave the focus"] {
            assert!(matches!(parse(s), Some(Attention::Leave)), "{s}");
        }
    }

    #[test]
    fn ordinary_sentences_are_not_attention() {
        for s in ["volta o deploy para staging", "o foco do produto é mobile", "pausa o worker do retry", "continua a migração", "focus the tests on billing"] {
            assert_eq!(parse(s), None, "{s}");
        }
    }
}
