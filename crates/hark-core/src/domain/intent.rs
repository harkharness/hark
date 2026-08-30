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

/// Verbs that mean "do work" — shared with address spans and verdicts.
pub const ACTION_VERBS: &[&str] = &[
    "abre", "abra", "ajusta", "aplica", "atualiza", "commita", "conserta", "continua",
    "corrige", "cria", "crie", "deleta", "deploya", "edita", "executa", "faz", "faça",
    "gera", "implementa", "implemente", "instala", "merge", "mergeia", "migra", "prepara",
    "organiza", "refatora", "remove", "renomeia", "resolve", "roda", "rode", "sobe",
    "trabalha", "vamos",
    // English. Read-only verbs ("show", "check") are deliberately absent:
    // they read as questions far more often than as work.
    "add", "apply", "build", "bump", "commit", "continue", "create", "delete",
    "deploy", "edit", "extract", "finish", "fix", "generate", "implement",
    "install", "lets", "make", "migrate", "open", "prepare", "publish", "push",
    "rebase", "refactor", "release", "rename", "resolve", "revert", "rewrite",
    "run", "ship", "split", "start", "test", "update", "upgrade", "work", "write",
];

/// Is this word an order? Two forms of the same order fall outside a list
/// of imperatives, and both are everywhere in ordinary speech:
///
/// - the sibling imperative ("atualiza" / "atualize", "resolve" / "resolva"),
///   since a hand-written list holds one of each pair by accident;
/// - the infinitive after a modal ("pode commitar", "preciso atualizar"),
///   which is the same order with a soft edge.
///
/// Dropping a final "r" turns the infinitive back into the imperative. It
/// also turns a handful of English nouns into verbs ("resolver", "updater"),
/// which at worst opens a confirmation the user can read and refuse.
pub fn is_action_verb(word: &str) -> bool {
    let word = word.trim_matches(|c: char| !c.is_alphabetic());
    let stem = word.strip_suffix('r').unwrap_or(word);
    listed_verb(word) || (stem.len() != word.len() && listed_verb(stem))
}

/// A word form in the list, or its sibling imperative.
fn listed_verb(word: &str) -> bool {
    if ACTION_VERBS.contains(&word) {
        return true;
    }
    let mut chars: Vec<char> = word.chars().collect();
    let swapped = match chars.pop() {
        Some('a') => 'e',
        Some('e') => 'a',
        _ => return false,
    };
    chars.push(swapped);
    let other: String = chars.into_iter().collect();
    ACTION_VERBS.contains(&other.as_str())
}

/// Openers that ask for an explanation, in both languages. These veto the
/// verb scan: "o que falta pra abrir o PR?" asks ABOUT work, it does not
/// order it.
const WH_STARTS: &[&str] = &[
    "quais", "qual", "quanto", "quando", "onde", "quem", "o que", "oq ", "que que",
    "como", "por que", "porque", "pq ",
    "what", "which", "how", "when", "where", "who", "why",
];

/// Classify an utterance. An opening interrogative wins over verbs; a
/// question MARK does not.
///
/// A question mark used to end the discussion, and it was wrong twice over:
/// "vc pode verificar e organizar os commits?" is an order asked politely
/// (Portuguese does this constantly, English too — "can you run the
/// tests?"), and the yes/no openers that carried it ("pode", "dá pra",
/// "can", "could") are exactly the modals people put in FRONT of an order.
/// Read as questions, they reached the global ask, which cannot touch a
/// repository and answered with a menu of options instead of working.
pub fn route(utterance: &str) -> Route {
    let lower = utterance.to_lowercase();
    if WH_STARTS.iter().any(|q| lower.starts_with(q)) {
        return Route::Ask;
    }
    // The WHOLE utterance, not the first four words: "eu quero que você
    // faça essa alteração" buries the verb in fifth place, and speech is
    // full of that. Anything opening with an interrogative already
    // returned, so this scan only sees sentences that state or ask for
    // something — and an order is the likelier of the two.
    let acts = lower.split_whitespace().any(is_action_verb);
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
        "investigate", "analyze", "analyse", "deep dive", "compare", "decision",
        "architecture", "why is", "why did", "debug",
    ];
    if HEAVY.iter().any(|w| lower.contains(w)) {
        return models.heavy.clone();
    }

    // Layer 3: cheap lookups (simple starts + short utterances).
    const LIGHT_STARTS: &[&str] = &[
        "quais", "qual", "lista", "resumo", "status", "quantos", "quantas",
        "list", "which", "how many", "how much", "summary", "summarize",
        "what is", "what's", "whats", "show me",
    ];
    let word_count = lower.split_whitespace().count();
    if word_count <= 12 && LIGHT_STARTS.iter().any(|w| lower.starts_with(w)) {
        return models.light.clone();
    }

    models.standard.clone()
}

/// Model hint for a WORKER's opening instruction (FASE 8.6): route UP,
/// never down. "investiga a causa raiz do webhook" deserves the heavy
/// tier without the user naming a model; nothing here ever downgrades
/// work to the light tier — a cheap-looking instruction still edits code.
/// Explicit overrides ("usa o haiku") are parsed by directives.rs first
/// and win; this only fills the gap when no model was named.
pub fn worker_model_hint(instruction: &str, models: &Models) -> Option<String> {
    let chosen = model_for(instruction, models);
    (chosen == models.heavy || chosen == models.max).then_some(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_hint_routes_up_and_never_down() {
        let m = Models::default();
        assert_eq!(
            worker_model_hint("investiga a causa raiz do webhook cair", &m),
            Some(m.heavy.clone())
        );
        assert_eq!(
            worker_model_hint("usa o melhor modelo e analisa a arquitetura", &m),
            Some(m.max.clone())
        );
        // A cheap-LOOKING instruction still edits code: never the light tier.
        assert_eq!(worker_model_hint("lista os arquivos e apaga os órfãos", &m), None);
        assert_eq!(worker_model_hint("roda os testes do webhook", &m), None);
    }

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
        assert_eq!(route("vamos trabalhar agora no hark"), Route::Dispatch);
        assert_eq!(route("implementa o worker conversacional"), Route::Dispatch);
        // Parallel dispatch while another worker runs.
        assert_eq!(route("enquanto isso faz a task dos alertas"), Route::Dispatch);
        assert_eq!(route("em paralelo roda a migração de pagamentos"), Route::Dispatch);
    }

    /// Incident 24/08: typed into a project chat, this exact sentence was
    /// read as a question (it ends in "?") and answered by the global ask
    /// with two options instead of being executed. Politeness is not a
    /// question: "pode fazer X?" is an order with a soft edge.
    #[test]
    fn polite_requests_are_work_even_with_a_question_mark() {
        assert_eq!(
            route(
                "temos muitas coisas para commitar no workspace-fabrica, vc pode \
                 verificar oq temos e organizar em commits semânticos?"
            ),
            Route::Dispatch
        );
        assert_eq!(route("você pode rodar os testes?"), Route::Dispatch);
        assert_eq!(route("can you run the tests?"), Route::Dispatch);
        assert_eq!(route("dá pra atualizar o chart do vigia?"), Route::Dispatch);
    }

    /// Every order in the list is an imperative ("commita", "roda"), but
    /// speech puts the verb in the infinitive right after a modal —
    /// "preciso commitar", "quero organizar". Same order, no match.
    #[test]
    fn infinitives_are_orders_too() {
        assert!(is_action_verb("commitar"));
        assert!(is_action_verb("organizar"));
        assert!(is_action_verb("resolver"));
        assert!(is_action_verb("atualizar"));
        assert_eq!(route("preciso commitar isso tudo"), Route::Dispatch);
        assert_eq!(route("quero organizar os commits por tema"), Route::Dispatch);
    }

    /// The veto that survives: a sentence that OPENS with an interrogative
    /// wants an answer, whatever verbs it carries.
    #[test]
    fn wh_questions_still_win_over_verbs() {
        assert_eq!(route("quais commits faltam?"), Route::Ask);
        assert_eq!(route("como eu organizo os commits?"), Route::Ask);
        assert_eq!(route("por que o deploy quebrou?"), Route::Ask);
        assert_eq!(route("tem alguma coisa pendente?"), Route::Ask);
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

/// The spoken grammar accepts Portuguese and English side by side. Words
/// that exist in both with different meanings are handled where they are
/// read, never by picking one language and losing the other.
#[cfg(test)]
mod bilingual {
    use super::*;

    fn models() -> Models {
        Models {
            light: "haiku".into(),
            standard: "sonnet".into(),
            heavy: "sonnet-heavy".into(),
            max: "opus".into(),
        }
    }

    #[test]
    fn english_action_verbs_dispatch() {
        assert_eq!(route("run the migration tests"), Route::Dispatch);
        assert_eq!(route("open the pull request"), Route::Dispatch);
        assert_eq!(route("fix the failing build"), Route::Dispatch);
        assert_eq!(route("deploy the webhook service"), Route::Dispatch);
    }

    #[test]
    fn english_questions_win_over_verbs() {
        assert_eq!(route("what is running right now"), Route::Ask);
        assert_eq!(route("which sessions are still open"), Route::Ask);
        assert_eq!(route("how do I open the PR?"), Route::Ask);
        assert_eq!(route("where did I leave the migration"), Route::Ask);
    }

    #[test]
    fn english_time_windows() {
        assert_eq!(window_hours("what did I ship yesterday", 36), 48);
        assert_eq!(window_hours("everything from last week", 36), 168);
    }

    #[test]
    fn english_depth_markers_pick_the_tier() {
        let m = models();
        assert_eq!(model_for("investigate the root cause of the timeout", &m), m.heavy);
        assert_eq!(model_for("compare the two approaches", &m), m.heavy);
        assert_eq!(model_for("list the open sessions", &m), m.light);
        assert_eq!(model_for("how many workers are running", &m), m.light);
    }
}

/// The 24/08 voice session: three steps, none of them executed. Every
/// utterance below was a command the router read as small talk, and the
/// mother answered each one with prose at four cents a turn.
#[cfg(test)]
mod incident_2408 {
    use super::*;

    #[test]
    fn the_other_imperative_of_a_listed_verb_still_acts() {
        // Portuguese imperatives come in pairs: atualiza/atualize,
        // resolve/resolva. The list had one of each pair, chosen by
        // accident, so half of normal speech fell through to Ask.
        assert_eq!(route("atualize o último artefato da conversa"), Route::Dispatch);
        assert_eq!(route("atualiza o último artefato da conversa"), Route::Dispatch);
        assert_eq!(route("resolva o conflito do rebase"), Route::Dispatch);
        assert_eq!(route("implemente o cache no endpoint"), Route::Dispatch);
    }

    #[test]
    fn a_buried_verb_is_still_a_command() {
        // Only the first four words were scanned. Nobody speaks like that:
        // "eu quero que você faça X" puts the verb fifth.
        assert_eq!(
            route("eu quero que você faça essa alteração dentro da sessão"),
            Route::Dispatch
        );
        assert_eq!(
            route("então o que eu preciso é que você atualize o chart"),
            Route::Dispatch
        );
    }

    #[test]
    fn questions_still_win_over_a_buried_verb() {
        // The whole-utterance scan must not turn questions into work.
        assert_eq!(route("o que falta pra abrir o PR?"), Route::Ask);
        assert_eq!(route("quais tarefas estão rodando agora"), Route::Ask);
        assert_eq!(route("quanto gastei hoje"), Route::Ask);
        assert_eq!(route("como está o board"), Route::Ask);
    }
}
