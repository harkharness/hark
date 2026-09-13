//! Hark's structured voice answer — the product-level shape that
//! `prompt::RESPONSE_SCHEMA` constrains. Backends return it as raw JSON in
//! `TurnResult.reply`; this module gives it a type.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceReply {
    pub fala: String,
    pub detalhes: String,
    #[serde(default)]
    pub itens: Vec<String>,
    /// Board updates proposed by the model (the invisible kanban feed).
    #[serde(default)]
    pub board: Vec<crate::domain::board::BoardUpdate>,
}

impl VoiceReply {
    /// The on-screen/remembered content: detalhes + itens as markdown.
    /// `fala` is the spoken headline and lives apart; detalhes that merely
    /// repeat it add nothing and are dropped.
    pub fn body(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let detalhes = self.detalhes.trim();
        if !detalhes.is_empty() && detalhes != self.fala.trim() {
            parts.push(detalhes.to_string());
        }
        for item in &self.itens {
            let item = item.trim();
            if !item.is_empty() {
                parts.push(format!("- {item}"));
            }
        }
        parts.join("\n")
    }

    /// Parse a backend's structured reply (None when the turn had no schema
    /// or the JSON isn't a VoiceReply).
    pub fn from_turn(result: &hark_agent::TurnResult) -> Option<Self> {
        result
            .reply
            .as_ref()
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }
}

/// The JSON object a model wrote somewhere in its prose.
///
/// Backends without a schema-constrained mode (every ACP agent) are ASKED
/// for JSON in the prompt, and answer the way models do: the object, a
/// fenced block, a sentence and then the object. This finds the first
/// balanced `{…}` that parses as an object and hands it back; anything
/// else is `None`, never a guess — the caller decides what "no structured
/// answer" means (the gate confirms, the ask reads the prose).
pub fn extract_lenient(text: &str) -> Option<serde_json::Value> {
    let mut from = 0;
    while let Some(open) = text[from..].find('{').map(|i| i + from) {
        if let Some(close) = balanced_close(text, open) {
            if let Ok(value @ serde_json::Value::Object(_)) =
                serde_json::from_str::<serde_json::Value>(&text[open..=close])
            {
                return Some(value);
            }
        }
        // Not JSON after all (prose with braces): keep looking past it.
        from = open + 1;
    }
    None
}

/// Byte index of the `}` that closes the `{` at `open`, honouring strings
/// (a brace inside quotes is text). JSON's structural characters are
/// ASCII, so scanning bytes is safe on UTF-8 — a multi-byte sequence never
/// contains one.
fn balanced_close(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, byte) in text.bytes().enumerate().skip(open) {
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_carries_detalhes_and_itens_for_screen_and_memory() {
        let reply = VoiceReply {
            fala: "Três pendências.".into(),
            detalhes: "O Assinaturas espera revisão.".into(),
            itens: vec!["revisar PR".into(), "subir migração".into()],
            board: vec![],
        };
        assert_eq!(reply.body(), "O Assinaturas espera revisão.\n- revisar PR\n- subir migração");
    }

    #[test]
    fn body_skips_detalhes_that_just_repeat_the_headline() {
        let reply = VoiceReply {
            fala: "Tudo verde.".into(),
            detalhes: "Tudo verde.".into(),
            itens: vec![],
            board: vec![],
        };
        assert_eq!(reply.body(), "");
    }

    #[test]
    fn parses_a_turns_structured_reply() {
        let turn = hark_agent::TurnResult {
            is_error: false,
            reply: Some(serde_json::json!({
                "fala": "Duas pendências hoje.",
                "detalhes": "PR aberto e teste falhando",
                "itens": ["revisar PR"],
            })),
            raw: String::new(),
            cost_usd: None,
            duration_ms: None,
            model: None,
            usage: vec![],
        };
        let reply = VoiceReply::from_turn(&turn).expect("typed reply");
        assert_eq!(reply.fala, "Duas pendências hoje.");
        assert_eq!(reply.itens, vec!["revisar PR"]);

        let untyped = hark_agent::TurnResult { reply: Some(serde_json::json!(42)), ..turn };
        assert_eq!(VoiceReply::from_turn(&untyped), None);
    }

    mod lenient {
        use super::super::extract_lenient;
        use serde_json::json;

        #[test]
        fn a_bare_object_is_taken_as_is() {
            let got = extract_lenient(r#"{"fala":"oi","detalhes":"","itens":[]}"#);
            assert_eq!(got, Some(json!({"fala": "oi", "detalhes": "", "itens": []})));
        }

        #[test]
        fn a_fenced_block_is_unwrapped() {
            let text = "Claro, aqui está:\n```json\n{\"fala\": \"duas pendências\", \"detalhes\": \"x\"}\n```\nQualquer coisa me avisa.";
            assert_eq!(
                extract_lenient(text),
                Some(json!({"fala": "duas pendências", "detalhes": "x"}))
            );
        }

        #[test]
        fn prose_around_the_object_is_ignored_and_nesting_survives() {
            let text = "Resposta: {\"acao\":\"despachar\",\"meta\":{\"n\":1,\"tags\":[\"a\",\"b\"]},\"motivo\":\"tem {chaves} no texto\"} — pronto.";
            let got = extract_lenient(text).expect("the object in the middle");
            assert_eq!(got["acao"], "despachar");
            assert_eq!(got["meta"]["tags"], json!(["a", "b"]));
            assert_eq!(got["motivo"], "tem {chaves} no texto");
        }

        #[test]
        fn the_first_object_that_parses_wins_over_an_earlier_brace_that_does_not() {
            let text = "{isto não é json} mas {\"ok\": true} é";
            assert_eq!(extract_lenient(text), Some(json!({"ok": true})));
        }

        #[test]
        fn no_object_means_none_never_a_guess() {
            assert_eq!(extract_lenient("não sei responder isso"), None);
            assert_eq!(extract_lenient(""), None);
            assert_eq!(extract_lenient("{ aberto sem fim"), None);
            // An array is not the object the schema asked for.
            assert_eq!(extract_lenient("[1, 2, 3]"), None);
        }
    }
}
