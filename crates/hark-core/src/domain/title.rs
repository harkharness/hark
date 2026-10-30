//! The name a new chat gets: a short line the light model writes from the
//! first message, instead of the first sixty characters of it. Pure; the
//! shell runs the model and keeps the old name when this says so.

/// Below this many words the message IS a fine name already.
const SHORT_WORDS: usize = 4;
/// The longest name a row carries without truncation.
const MAX_CHARS: usize = 48;

/// Whether the first message deserves a model-written name: anything
/// longer than a few words reads as a sentence, not a title.
pub fn needs_title(instruction: &str) -> bool {
    instruction.split_whitespace().count() > SHORT_WORDS
}

/// What the light model is asked: one short name, same language.
pub fn prompt(instruction: &str) -> String {
    format!(
        "Dê um título curto para um chat de trabalho que começa com esta mensagem. \
         Regras: 3 a 6 palavras, na língua da mensagem, sem aspas, sem ponto final, \
         sem links nem caminhos; nomeie o assunto, não a ação pedida.\n\nMensagem:\n{}",
        instruction.chars().take(1200).collect::<String>()
    )
}

pub const SCHEMA: &str = r#"{"type":"object","properties":{"title":{"type":"string"}},"required":["title"]}"#;

/// The model's answer as a row would show it, or None when it is not a
/// usable name: empty, not the JSON asked for. Quotes and a trailing stop
/// are stripped; anything past 48 characters is cut on a word.
pub fn clean(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let title = value.get("title")?.as_str()?;
    let title = title.trim_matches(|c: char| matches!(c, '"' | '“' | '”' | '\'' | '.' | ' ' | '\n'));
    if title.is_empty() {
        return None;
    }
    let mut out = String::new();
    for word in title.split_whitespace() {
        let next = if out.is_empty() { word.to_string() } else { format!("{out} {word}") };
        if next.chars().count() > MAX_CHARS {
            break;
        }
        out = next;
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_message_is_already_a_name() {
        assert!(!needs_title("Oncall SRE"));
        assert!(!needs_title("revisa o PR"));
        assert!(needs_title("gerei um artefato hark://artifacts/x/test-documento preciso que vc atualize transformando esse artefato em uma pagina home"));
        assert!(needs_title("continua a migração do webhook de onde paramos ontem e roda os testes"));
    }

    #[test]
    fn the_prompt_carries_the_message_and_asks_for_a_short_name() {
        let p = prompt("continua a migração do webhook de onde paramos");
        assert!(p.contains("continua a migração do webhook"), "{p}");
        assert!(p.contains("3 a 6 palavras"), "{p}");
    }

    #[test]
    fn the_answer_is_cleaned_or_refused() {
        assert_eq!(clean(r#"{"title":"Migração do webhook"}"#), Some("Migração do webhook".into()));
        assert_eq!(clean(r#"{"title":"  \"Home do workspace\". "}"#), Some("Home do workspace".into()));
        assert_eq!(clean(r#"{"title":""}"#), None);
        assert_eq!(clean("not json"), None);
        let long = format!(r#"{{"title":"{}"}}"#, "palavra ".repeat(20));
        assert!(clean(&long).map(|t| t.chars().count() <= 48).unwrap_or(false), "cut at 48");
    }
}
