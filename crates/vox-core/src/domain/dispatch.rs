//! Pure dispatch logic: which existing session should receive an instruction.

use crate::domain::snapshot::SessionSummary;

/// Result of matching an instruction against the indexed sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// One clear winner.
    Chosen(String),
    /// Close call: candidates (best first) for the user to pick from.
    Ambiguous(Vec<String>),
    /// Nothing matched at all.
    None,
}

/// Rank sessions by term overlap with the instruction (title, recent prompts,
/// cwd) plus a recency boost. `sessions` must arrive newest first.
pub fn resolve_target(instruction: &str, sessions: &[SessionSummary]) -> Target {
    let terms = significant_terms(instruction);
    if terms.is_empty() || sessions.is_empty() {
        return Target::None;
    }
    let mut scored: Vec<(f64, &SessionSummary)> = sessions
        .iter()
        .enumerate()
        .map(|(rank, s)| (score(&terms, s) + recency_boost(rank), s))
        .filter(|(score, _)| *score > 0.5)
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    match scored.as_slice() {
        [] => Target::None,
        [(_, only)] => Target::Chosen(only.session_id.clone()),
        // A full-term lead (1.0) survives any recency differential (<= 0.4).
        [(s1, first), (s2, _), ..] if s1 - s2 >= 0.6 => Target::Chosen(first.session_id.clone()),
        _ => Target::Ambiguous(
            scored
                .iter()
                .take(3)
                .map(|(_, s)| s.session_id.clone())
                .collect(),
        ),
    }
}

/// Words that actually identify a topic (length >= 3, minus filler).
pub fn significant_terms(text: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "abre", "abrir", "com", "como", "continua", "das", "dos", "ele", "ela", "essa", "esse",
        "está", "esta", "para", "pra", "por", "que", "sobre", "the", "uma", "vamos", "vox",
    ];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.chars().count() >= 3 && !STOPWORDS.contains(w))
        .map(String::from)
        .collect()
}

fn score(terms: &[String], session: &SessionSummary) -> f64 {
    // Whole words on folded tokens: "core" hits "vox-core", never "score".
    let words: std::collections::HashSet<String> = crate::domain::matching::tokens(&format!(
        "{} {} {}",
        session.title.as_deref().unwrap_or_default(),
        session.cwd.as_deref().unwrap_or_default(),
        session
            .recent_prompts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    ))
    .into_iter()
    .collect();
    terms
        .iter()
        .filter(|t| words.contains(&crate::domain::matching::fold(t)))
        .count() as f64
}

/// Newest first: small bonus that only breaks ties between equal matches.
fn recency_boost(rank: usize) -> f64 {
    0.4 / (1.0 + rank as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::snapshot::{RecentPrompt, SessionSummary};

    fn session(id: &str, title: &str, prompt: &str) -> SessionSummary {
        SessionSummary {
            session_id: id.into(),
            title: Some(title.into()),
            recent_prompts: vec![RecentPrompt {
                ts: "2026-08-14T10:00:00Z".into(),
                text: prompt.into(),
            }],
            ..SessionSummary::new(id)
        }
    }

    #[test]
    fn picks_the_session_matching_the_topic() {
        let sessions = vec![
            session("s-alerts", "Migração de alertas", "migra os alertas do terraform"),
            session("s-assinaturas", "Migração Assinaturas", "prepara a migração de assinaturas"),
            session("s-webhook", "Webhook v2", "valida o webhook em uat"),
        ];
        assert_eq!(
            resolve_target("continua a migração de assinaturas, abre o PR do DNS antigo", &sessions),
            Target::Chosen("s-assinaturas".into())
        );
        assert_eq!(
            resolve_target("valida o webhook", &sessions),
            Target::Chosen("s-webhook".into())
        );
    }

    #[test]
    fn ambiguous_when_two_sessions_tie() {
        let sessions = vec![
            session("s1", "Migração Assinaturas uat", "migração de assinaturas uat"),
            session("s2", "Migração Assinaturas prod", "migração de assinaturas prod"),
        ];
        let Target::Ambiguous(candidates) =
            resolve_target("continua a migração de assinaturas", &sessions)
        else {
            panic!("expected ambiguous");
        };
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn none_when_nothing_matches() {
        let sessions = vec![session("s1", "Webhook v2", "valida o webhook")];
        assert_eq!(resolve_target("cria o cluster kafka", &sessions), Target::None);
        assert_eq!(resolve_target("", &sessions), Target::None);
    }

    #[test]
    fn substring_no_longer_matches_inside_words() {
        // "core" used to hit "score"/"encore" via contains(); words only.
        let sessions = vec![session("s1", "score keeper encore", "melhora o score do jogo")];
        assert_eq!(resolve_target("ajusta o core do parser", &sessions), Target::None);
        // Hyphenated names still split into whole words.
        let sessions = vec![session("s2", "vox-core refactor", "refatora o vox-core")];
        assert_eq!(
            resolve_target("ajusta o core do parser e refactor", &sessions),
            Target::Chosen("s2".into())
        );
    }
}
