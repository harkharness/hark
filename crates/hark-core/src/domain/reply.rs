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
}
