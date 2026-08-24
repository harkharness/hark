//! The ONE honest matcher behind spoken targeting (board tasks, sessions,
//! dispatch). Whole-word matching over accent-folded tokens, rare-term
//! weighting, and an explicit Ambiguous outcome — a wrong silent guess is
//! how the app opens the wrong chat four times in a row.

use std::collections::HashSet;

/// Outcome of ranking candidates against a spoken query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match<T> {
    /// One clear winner.
    Hit(T),
    /// Too close to call: candidates best-first for the user to pick.
    Ambiguous(Vec<T>),
    /// Nothing qualified.
    None,
}

/// Lowercase and strip pt-BR accents: "Migração" → "migracao".
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Word tokens of folded text, split on anything non-alphanumeric
/// ("hark-core" → ["hark", "core"]).
pub fn tokens(text: &str) -> Vec<String> {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

/// Sentence glue and structural words that never identify a topic.
/// Folded forms only (tokens() folds before comparing).
const STOPWORDS: &[&str] = &[
    "abre", "abrir", "chat", "com", "como", "continua", "das", "dos", "ele", "ela",
    "essa", "esse", "esta", "para", "pra", "por", "que", "sessao", "sobre", "task",
    "tarefa", "the", "uma", "vamos", "hark",
    // English structural filler. Same trade as "abre"/"continua": a title
    // that leans on one of these loses a term, and gains precision on the
    // ones that actually name a topic.
    "about", "and", "are", "but", "continue", "for", "from", "has", "have",
    "into", "its", "not", "open", "our", "out", "please", "session", "sessions",
    "tasks", "that", "this", "was", "were", "what", "when", "which", "with",
    "you", "your",
];

/// Query words that actually identify a topic (folded, length ≥ 3,
/// minus structural filler like "chat"/"task"/"sessao").
pub fn significant(text: &str) -> Vec<String> {
    tokens(text)
        .into_iter()
        .filter(|w| w.chars().count() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Rank candidates against a spoken query.
///
/// - hits are whole WORDS on folded tokens, never substrings;
/// - a term hitting exactly one candidate weighs double (the distinctive
///   word: "assinaturas" outvotes junk hits on "migração");
/// - a candidate qualifies at weighted ratio ≥ 0.5 of the query terms;
/// - the winner needs a ≥ 25% relative margin over the runner-up —
///   anything closer (exact ties included) returns Ambiguous, newest
///   first. Recency orders; it never overturns a term-hit lead.
pub fn rank<T, H, R>(query: &str, candidates: Vec<T>, hay: H, recency: R) -> Match<T>
where
    H: Fn(&T) -> String,
    R: Fn(&T) -> String,
{
    let terms = significant(query);
    if terms.is_empty() || candidates.is_empty() {
        return Match::None;
    }
    let words: Vec<HashSet<String>> = candidates
        .iter()
        .map(|c| tokens(&hay(c)).into_iter().collect())
        .collect();
    let weights: Vec<f64> = terms
        .iter()
        .map(|t| {
            let hits_across = words.iter().filter(|w| w.contains(t)).count();
            if hits_across == 1 { 2.0 } else { 1.0 }
        })
        .collect();
    let total: f64 = weights.iter().sum();

    let mut scored: Vec<(f64, String, T)> = candidates
        .into_iter()
        .zip(words)
        .filter_map(|(cand, wset)| {
            let hit: f64 = terms
                .iter()
                .zip(&weights)
                .filter(|(t, _)| wset.contains(*t))
                .map(|(_, w)| w)
                .sum();
            let ratio = hit / total;
            (ratio >= 0.5).then(|| (ratio, recency(&cand), cand))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
    });

    match scored.len() {
        0 => Match::None,
        1 => Match::Hit(scored.remove(0).2),
        _ if scored[0].0 > scored[1].0 * 1.25 => Match::Hit(scored.remove(0).2),
        _ => Match::Ambiguous(scored.into_iter().take(3).map(|(_, _, c)| c).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Cand = (String, String, String); // (id, haystack, recency ts)

    /// "hit:<id>" | "ambiguous:<id,id>" | "none" — readable assertions.
    fn rank_titles(query: &str, cands: Vec<(&str, &str, &str)>) -> String {
        let owned: Vec<Cand> = cands
            .into_iter()
            .map(|(id, hay, ts)| (id.to_string(), hay.to_string(), ts.to_string()))
            .collect();
        match rank(query, owned, |c: &Cand| c.1.clone(), |c: &Cand| c.2.clone()) {
            Match::Hit((id, _, _)) => format!("hit:{id}"),
            Match::Ambiguous(list) => format!(
                "ambiguous:{}",
                list.iter().map(|(id, _, _)| id.as_str()).collect::<Vec<_>>().join(",")
            ),
            Match::None => "none".into(),
        }
    }

    #[test]
    fn folds_accents_and_case() {
        assert_eq!(fold("Migração"), "migracao");
        assert_eq!(fold("SERVIÇOS Úteis"), "servicos uteis");
        assert_eq!(tokens("hark-core targeting"), vec!["hark", "core", "targeting"]);
    }

    #[test]
    fn matches_whole_words_never_substrings() {
        // "core" must NOT hit "score keeper" (substring), but MUST hit
        // "hark-core" (hyphen splits into whole words).
        let out = rank_titles(
            "core parser",
            vec![("junk", "score keeper encore", "2026-08-19")],
        );
        assert_eq!(out, "none");
        let out = rank_titles("core parser", vec![("hit", "hark-core parser fixes", "2026-08-19")]);
        assert_eq!(out, "hit:hit");
    }

    #[test]
    fn requires_half_the_terms_to_match() {
        // The 19/08 incident: a fresh hark card with one junk "core" hit
        // must NOT win "migração dos serviços Assinaturas Core".
        let out = rank_titles(
            "migração dos serviços Assinaturas Core",
            vec![("hark", "hark-core targeting refactor", "2026-08-19T12:00:00Z")],
        );
        assert_eq!(out, "none");
    }

    #[test]
    fn rare_terms_outweigh_common_ones() {
        // "migração" hits two candidates (common); "assinaturas" hits one
        // (distinctive). The distinctive hit must win alone.
        let out = rank_titles(
            "migração de assinaturas",
            vec![
                ("alerts", "migração de alertas", "2026-08-19"),
                ("webhooks", "migração de webhooks", "2026-08-19"),
                ("assinaturas", "assinaturas deploy", "2026-08-01"),
            ],
        );
        assert_eq!(out, "hit:assinaturas");
    }

    #[test]
    fn close_scores_return_ambiguous_candidates() {
        let out = rank_titles(
            "migração assinaturas",
            vec![
                ("uat", "Migração Assinaturas uat", "2026-08-10"),
                ("prod", "Migração Assinaturas prod", "2026-08-18"),
            ],
        );
        assert_eq!(out, "ambiguous:prod,uat");
    }

    #[test]
    fn recency_breaks_only_exact_ties() {
        // Exact tie: the newer candidate leads the Ambiguous list…
        let out = rank_titles(
            "migração assinaturas",
            vec![
                ("old", "Migração Assinaturas uat", "2026-08-01"),
                ("new", "Migração Assinaturas prod", "2026-08-18"),
            ],
        );
        assert_eq!(out, "ambiguous:new,old");
        // …but recency NEVER overturns a term-hit lead: the older
        // candidate matching every term beats the fresh partial match.
        let out = rank_titles(
            "migração dos serviços assinaturas",
            vec![
                ("fresh", "migração de alertas", "2026-08-19"),
                ("exact", "migração dos serviços assinaturas", "2026-08-01"),
            ],
        );
        assert_eq!(out, "hit:exact");
    }
}

#[cfg(test)]
mod bilingual {
    use super::*;

    #[test]
    fn english_filler_never_counts_as_a_topic_term() {
        assert_eq!(significant("continue the webhook migration"), ["webhook", "migration"]);
        assert_eq!(significant("open the chat about billing"), ["billing"]);
    }
}
