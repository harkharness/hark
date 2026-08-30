//! Cross-session references (FASE 8.2).
//!
//! The declared pain, in the user's own words: "quando eu tento falar pro
//! Chat algum tema e talz de outro chat não rola". A work sentence often
//! CITES another conversation — "faz igual a gente decidiu no chat do
//! webhook" — and no dictation tool can resolve that, because none of them
//! knows which sessions exist. Hark's local index does.
//!
//! Two pure pieces:
//! - `detect`: does this sentence cite another conversation, and by what
//!   name? Grammar is deliberately tight — citing markers only, never the
//!   TARGET address ("na task X faz Y" is where work GOES, address.rs owns
//!   it) and never the open-a-chat command (task_command owns that).
//! - `excerpt_about`: the cited conversation's lines that talk about the
//!   sentence's terms, clipped to a budget. Zero tokens: the log is local.

use super::matching;
use super::transcript::{Entry, Role};

/// A detected citation: which conversation, said how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The spoken name of the cited conversation ("webhook", "migração do
    /// pagamentos") — a search query, not an id.
    pub query: String,
}

/// Markers that cite another conversation as a SOURCE of context. Each is
/// followed by the conversation's spoken name.
const CITE_MARKERS: &[&str] = &[
    // "o que a gente decidiu/combinou no chat do X"
    "decidimos no chat", "decidiu no chat", "combinamos no chat", "combinou no chat",
    "decidimos na sessão", "combinamos na sessão", "decidimos na sessao", "combinamos na sessao",
    "discutimos no chat", "falamos no chat", "conversamos no chat",
    // "como fizemos no chat do X"
    "fizemos no chat", "fez no chat", "fizemos na sessão", "fizemos na sessao",
    // "igual/baseado/conforme o chat do X"
    "igual no chat", "igual ao chat", "baseado no chat", "conforme o chat",
    // "pega/usa/puxa (o que tem) do chat do X"
    "pega do chat", "usa do chat", "puxa do chat", "pega o contexto do chat",
    // English
    "we decided in the", "we agreed in the", "like we did in the", "from the chat",
];

/// Glue between the marker and the conversation's name.
const LEAD: &[&str] = &["do ", "da ", "de ", "dos ", "das ", "sobre ", "of ", "about "];

/// The name ends where the citing clause ends: a comma, or a connective
/// that resumes the instruction.
const ENDS: &[&str] = &[",", " e ", " aí ", " ai ", " então ", " entao ", " and ", " then "];

/// Does the sentence cite another conversation? Returns the spoken name.
pub fn detect(utterance: &str) -> Option<Reference> {
    let lower = matching::fold(utterance);
    let (marker, at) = CITE_MARKERS
        .iter()
        .filter_map(|m| lower.find(&matching::fold(m)).map(|i| (*m, i)))
        .min_by_key(|(_, i)| *i)?;
    let mut rest = &lower[at + matching::fold(marker).len()..];
    rest = rest.trim_start();
    for lead in LEAD {
        if let Some(stripped) = rest.strip_prefix(lead) {
            rest = stripped;
            break;
        }
    }
    let mut name = rest;
    for end in ENDS {
        if let Some(i) = name.find(end) {
            name = &name[..i];
        }
    }
    let name = name.trim().trim_end_matches(['.', '?', '!']);
    (!name.is_empty()).then(|| Reference { query: name.to_string() })
}

/// The cited conversation's lines about the sentence's subject, newest
/// kept when the budget cuts, tool noise dropped. `terms` come from the
/// WHOLE instruction so the excerpt is about what the user is doing, not
/// about the citation wording.
pub fn excerpt_about(entries: &[Entry], terms: &[String], max_chars: usize) -> String {
    let relevant: Vec<String> = entries
        .iter()
        .filter(|e| matches!(e.role, Role::User | Role::Assistant))
        .filter(|e| {
            let hay = matching::fold(&e.text);
            terms.iter().any(|t| hay.contains(t.as_str()))
        })
        .map(|e| {
            let who = if e.role == Role::User { "você" } else { "assistente" };
            let text: String = e.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let clipped: String = text.chars().take(240).collect();
            format!("{who}: {clipped}")
        })
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut used = 0usize;
    // Newest lines carry the decisions; walk backwards and keep order.
    for line in relevant.iter().rev() {
        let cost = line.chars().count() + 1;
        if used + cost > max_chars {
            break;
        }
        used += cost;
        out.push(line.clone());
    }
    out.reverse();
    out.join("\n")
}

/// The context block as it travels inside the dispatched message —
/// VISIBLE, attributed, and closed, so the receiving agent knows exactly
/// where it came from and the user can read what was injected.
pub fn context_block(title: &str, excerpt: &str) -> String {
    format!("[contexto de \"{title}\", puxado do histórico local]\n{excerpt}\n[fim do contexto]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(role: Role, text: &str) -> Entry {
        Entry {
            ts: "2026-08-25T10:00:00Z".into(),
            role,
            text: text.into(),
            tool: None,
            is_error: false,
        }
    }

    #[test]
    fn citing_another_chat_is_detected_with_its_name() {
        let got = detect("manda pro alertas o que a gente decidiu no chat de assinaturas sobre threshold");
        assert_eq!(got, Some(Reference { query: "assinaturas sobre threshold".into() }));
        assert_eq!(
            detect("faz igual no chat da migração do webhook"),
            Some(Reference { query: "migracao do webhook".into() })
        );
        assert_eq!(
            detect("aplica aqui o que combinamos no chat de pagamentos, com os mesmos nomes"),
            Some(Reference { query: "pagamentos".into() })
        );
    }

    /// The TARGET address is not a citation: "na task X faz Y" says where
    /// work GOES (address.rs), and "abre o chat de X" is a command
    /// (task_command). Neither may pull context.
    #[test]
    fn addresses_and_open_commands_are_not_citations() {
        assert_eq!(detect("na task assinaturas core roda os testes"), None);
        assert_eq!(detect("abre o chat de migração dos serviços"), None);
        assert_eq!(detect("roda os testes do webhook"), None);
        assert_eq!(detect("quais as pendências de hoje?"), None);
    }

    #[test]
    fn excerpt_keeps_lines_about_the_terms_and_fits_the_budget() {
        let entries = vec![
            e(Role::User, "define o threshold do certmanager em 80%"),
            e(Role::ToolUse, r#"{"command":"kubectl get pods"}"#),
            e(Role::Assistant, "Threshold combinado: 80% com janela de 10 minutos."),
            e(Role::Assistant, "Também renomeei o dashboard, sem relação."),
            e(Role::User, "fecha assim então"),
        ];
        let terms = vec!["threshold".to_string(), "certmanager".to_string()];
        let got = excerpt_about(&entries, &terms, 400);
        assert!(got.contains("80% com janela"), "decision line survives");
        assert!(got.contains("você: define o threshold"), "asker attribution survives");
        assert!(!got.contains("kubectl"), "tool noise stays out");
        assert!(!got.contains("dashboard"), "off-topic prose stays out");
    }

    #[test]
    fn excerpt_prefers_the_newest_lines_when_the_budget_cuts() {
        let entries: Vec<Entry> = (0..40)
            .map(|i| e(Role::Assistant, &format!("threshold rodada {i} definido")))
            .collect();
        let got = excerpt_about(&entries, &["threshold".to_string()], 200);
        assert!(got.contains("rodada 39"), "newest decision is the one that matters");
        assert!(!got.contains("rodada 0"), "oldest goes first when clipping");
    }

    #[test]
    fn the_block_is_attributed_and_closed() {
        let block = context_block("Migração Assinaturas", "assistente: threshold 80%");
        assert!(block.starts_with("[contexto de \"Migração Assinaturas\""));
        assert!(block.ends_with("[fim do contexto]"));
    }
}
