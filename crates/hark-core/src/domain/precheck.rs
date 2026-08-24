//! Local, deterministic pre-dispatch warnings. These replace the gate's
//! LLM-authored cost warnings: the numbers come from THIS machine (session
//! file size, context weight), so they are always true and always carry
//! actions — never "MCP caiu" hallucinations.

use crate::domain::lang::Lang;
use serde::Serialize;

/// Everything the shell knows about a session before dispatching to it.
#[derive(Debug, Clone, Default)]
pub struct SessionFacts {
    /// Size of the session's jsonl on disk, in MB.
    pub size_mb: f64,
    /// Last known context-window usage (0..1), when available.
    pub context_pct: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarnKind {
    BigHistory,
    FullContext,
}

/// What the user can do about a warning, right on the confirm surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarnAction {
    /// Send "/compact" as its own turn, then the message.
    CompactFirst,
    /// Dispatch as-is.
    Proceed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    pub kind: WarnKind,
    /// pt-BR, carries the real number ("histórico de 3.8 MB…").
    pub text: String,
    pub actions: Vec<WarnAction>,
}

/// Derive the warnings a dispatch to this session deserves. Zero tokens.
pub fn prechecks(facts: &SessionFacts, lang: Lang) -> Vec<Warning> {
    let mut out = Vec::new();
    if facts.size_mb >= 2.0 {
        out.push(Warning {
            kind: WarnKind::BigHistory,
            text: format!(
                "{} {:.1} MB — {}",
                lang.pick("histórico da sessão tem", "session history is"),
                facts.size_mb,
                lang.pick("turnos podem ser caros", "turns here can get expensive"),
            ),
            actions: vec![WarnAction::CompactFirst, WarnAction::Proceed],
        });
    }
    if let Some(pct) = facts.context_pct {
        if pct >= 0.8 {
            out.push(Warning {
                kind: WarnKind::FullContext,
                text: format!(
                    "{} {}% {}",
                    lang.pick("contexto em", "context at"),
                    (pct * 100.0).round() as u32,
                    lang.pick("da janela — perto do limite", "of the window — close to the limit"),
                ),
                actions: vec![WarnAction::CompactFirst, WarnAction::Proceed],
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_session_yields_no_warnings() {
        let out = prechecks(&SessionFacts { size_mb: 0.4, context_pct: Some(0.3) }, Lang::Pt);
        assert!(out.is_empty());
    }

    #[test]
    fn heavy_history_offers_compact_first() {
        let out = prechecks(&SessionFacts { size_mb: 3.8, context_pct: None }, Lang::Pt);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WarnKind::BigHistory);
        assert_eq!(out[0].actions, vec![WarnAction::CompactFirst, WarnAction::Proceed]);
    }

    #[test]
    fn near_full_context_offers_compact_first() {
        let out = prechecks(&SessionFacts { size_mb: 0.2, context_pct: Some(0.86) }, Lang::Pt);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, WarnKind::FullContext);
        assert_eq!(out[0].actions, vec![WarnAction::CompactFirst, WarnAction::Proceed]);
    }

    #[test]
    fn warning_texts_are_pt_br_and_carry_numbers() {
        let out = prechecks(&SessionFacts { size_mb: 3.8, context_pct: Some(0.86) }, Lang::Pt);
        assert_eq!(out.len(), 2);
        assert!(out[0].text.contains("3.8"), "size in text: {}", out[0].text);
        assert!(out[0].text.contains("histórico"), "pt-BR: {}", out[0].text);
        assert!(out[1].text.contains("86%"), "pct in text: {}", out[1].text);
    }
}

#[cfg(test)]
mod bilingual {
    use super::*;
    use crate::domain::lang::Lang;

    #[test]
    fn warnings_speak_the_interface_language() {
        let facts = SessionFacts { size_mb: 3.8, context_pct: Some(0.86) };
        let en = prechecks(&facts, Lang::En);
        assert!(en[0].text.contains("3.8"), "number survives: {}", en[0].text);
        assert!(en[0].text.to_lowercase().contains("history"), "en: {}", en[0].text);
        assert!(en[1].text.contains("86%"), "pct survives: {}", en[1].text);
        let pt = prechecks(&facts, Lang::Pt);
        assert!(pt[0].text.contains("histórico"), "pt: {}", pt[0].text);
    }
}
