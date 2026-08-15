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

/// Where a spoken utterance should go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Read-only question about state/history (cheap fast mode).
    Ask,
    /// Instruction that changes things: needs confirmation + a worker.
    Dispatch,
}

const ACTION_VERBS: &[&str] = &[
    "abre", "abra", "ajusta", "aplica", "atualiza", "commita", "conserta", "continua",
    "corrige", "cria", "crie", "deleta", "deploya", "edita", "executa", "faz", "faça",
    "gera", "implementa", "implemente", "instala", "merge", "mergeia", "migra", "prepara",
    "refatora", "remove", "renomeia", "resolve", "roda", "rode", "sobe", "trabalha", "vamos",
];

/// Classify an utterance. Questions win over verbs: "o que falta pra abrir o
/// PR?" is an Ask even though it mentions an action.
pub fn route(utterance: &str) -> Route {
    let lower = utterance.to_lowercase();
    let question = lower.ends_with('?')
        || ["quais", "qual", "quanto", "quando", "onde", "quem", "o que", "como", "tem "]
            .iter()
            .any(|q| lower.starts_with(q));
    if question {
        return Route::Ask;
    }
    let first_words: Vec<&str> = lower.split_whitespace().take(4).collect();
    let acts = first_words
        .iter()
        .any(|w| ACTION_VERBS.contains(&w.trim_matches(|c: char| !c.is_alphabetic())));
    if acts {
        Route::Dispatch
    } else {
        Route::Ask
    }
}

/// Model tiers, all overridable via config.
#[derive(Debug, Clone)]
pub struct Models {
    /// Cheap lookups: lists, status, short summaries.
    pub light: String,
    /// Default tier.
    pub standard: String,
    /// Deep analysis and decisions.
    pub heavy: String,
    /// Only on explicit request ("melhor modelo").
    pub max: String,
}

impl Default for Models {
    fn default() -> Self {
        Self {
            light: "haiku".into(),
            standard: "sonnet".into(),
            heavy: "opus".into(),
            max: "fable".into(),
        }
    }
}

/// Pick the model for an utterance. An explicit request always wins;
/// otherwise a zero-cost heuristic routes by task weight.
pub fn model_for(utterance: &str, models: &Models) -> String {
    let lower = utterance.to_lowercase();

    // Layer 1: explicit override.
    if lower.contains("melhor modelo") || lower.contains("best model") {
        return models.max.clone();
    }
    for name in ["fable", "opus", "sonnet", "haiku"] {
        for verb in ["usa o ", "usa ", "use o ", "use ", "com o ", "with "] {
            if lower.contains(&format!("{verb}{name}")) {
                return name.to_string();
            }
        }
    }

    // Layer 2: heavy reasoning markers.
    const HEAVY: &[&str] = &[
        "investiga", "analisa", "a fundo", "profundo", "compara", "decide", "decisão",
        "trade-off", "arquitetura", "root cause", "causa raiz",
    ];
    if HEAVY.iter().any(|w| lower.contains(w)) {
        return models.heavy.clone();
    }

    // Layer 3: cheap lookups (simple starts + short utterances).
    const LIGHT_STARTS: &[&str] = &["quais", "qual", "lista", "resumo", "status", "quantos", "quantas"];
    let word_count = lower.split_whitespace().count();
    if word_count <= 12 && LIGHT_STARTS.iter().any(|w| lower.starts_with(w)) {
        return models.light.clone();
    }

    models.standard.clone()
}

/// Interpret a spoken yes/no confirmation.
pub fn is_affirmative(utterance: &str) -> bool {
    let lower = utterance.to_lowercase();
    ["sim", "pode", "confirmo", "confirma", "vai", "manda", "bora", "yes", "aprova"]
        .iter()
        .any(|w| lower.contains(w))
        && !lower.contains("não")
        && !lower.contains("nao")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_questions_to_ask() {
        assert_eq!(route("quais são as pendências de hoje?"), Route::Ask);
        assert_eq!(route("como está a migração do webhook"), Route::Ask);
        assert_eq!(route("o que falta pra abrir o PR?"), Route::Ask);
        assert_eq!(route("me dá um resumo da semana"), Route::Ask);
    }

    #[test]
    fn routes_actions_to_dispatch() {
        assert_eq!(route("continua a migração de assinaturas"), Route::Dispatch);
        assert_eq!(route("abre o PR do DNS antigo"), Route::Dispatch);
        assert_eq!(route("agora roda os testes do webhook"), Route::Dispatch);
        assert_eq!(route("vamos trabalhar agora no vox"), Route::Dispatch);
        assert_eq!(route("implementa o worker conversacional"), Route::Dispatch);
    }

    #[test]
    fn confirmations_parse_pt_br() {
        assert!(is_affirmative("sim, pode mandar"));
        assert!(is_affirmative("confirmo"));
        assert!(!is_affirmative("não"));
        assert!(!is_affirmative("não pode"));
        assert!(!is_affirmative("espera"));
    }

    #[test]
    fn explicit_model_request_always_wins() {
        let m = |t: &str| model_for(t, &Models::default());
        assert_eq!(m("usa o opus: quais as pendências?"), "opus");
        assert_eq!(m("com o haiku, resume a semana"), "haiku");
        assert_eq!(m("usa o melhor modelo e analisa a arquitetura"), "fable");
        assert_eq!(m("use the best model to review this"), "fable");
        assert_eq!(m("usa o sonnet aqui"), "sonnet");
    }

    #[test]
    fn cheap_lookups_go_to_light_model() {
        let m = |t: &str| model_for(t, &Models::default());
        assert_eq!(m("quais são as pendências de hoje?"), "haiku");
        assert_eq!(m("lista as sessões abertas"), "haiku");
        assert_eq!(m("qual o status do PR?"), "haiku");
        assert_eq!(m("resumo rápido da semana"), "haiku");
    }

    #[test]
    fn deep_reasoning_goes_to_heavy_model() {
        let m = |t: &str| model_for(t, &Models::default());
        assert_eq!(m("investiga por que o webhook caiu ontem"), "opus");
        assert_eq!(m("analisa a fundo os trade-offs dessa migração"), "opus");
        assert_eq!(m("compara as duas abordagens e decide a melhor"), "opus");
    }

    #[test]
    fn everything_else_uses_standard_model() {
        let m = |t: &str| model_for(t, &Models::default());
        assert_eq!(m("como está a migração de pagamentos?"), "sonnet");
        assert_eq!(m("me explica esse erro do terraform"), "sonnet");
    }

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
