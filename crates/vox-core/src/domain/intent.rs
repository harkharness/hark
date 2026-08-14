//! Pure intent hints extracted from the spoken question.

/// (keywords, hours) pairs; the widest matching window wins.
const WINDOW_HINTS: &[(&[&str], i64)] = &[
    (&["hoje", "today"], 24),
    (&["ontem", "yesterday"], 48),
    (&["semana", "week"], 168),
    (&["mês", "mes", "month"], 720),
];

/// Time window (hours) implied by the question, or `default_hours`.
pub fn window_hours(question: &str, default_hours: i64) -> i64 {
    let q = question.to_lowercase();
    WINDOW_HINTS
        .iter()
        .filter(|(words, _)| words.iter().any(|w| q.contains(w)))
        .map(|(_, hours)| *hours)
        .max()
        .unwrap_or(default_hours)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_time_window_from_question() {
        assert_eq!(window_hours("o que ficou pendente hoje?", 36), 24);
        assert_eq!(window_hours("pendências de ontem", 36), 48);
        assert_eq!(window_hours("o que ficou pendente nessa semana?", 36), 168);
        assert_eq!(window_hours("resumo da semana", 36), 168);
        assert_eq!(window_hours("o que rolou no mês?", 36), 720);
        assert_eq!(window_hours("what was left pending this week?", 36), 168);
    }

    #[test]
    fn falls_back_to_default_without_hints() {
        assert_eq!(window_hours("como está a migração do webhook?", 36), 36);
    }

    #[test]
    fn widest_hint_wins() {
        assert_eq!(window_hours("compara hoje com o resto da semana", 36), 168);
    }
}
