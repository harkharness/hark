//! The spend ledger's pure heart: every Claude turn becomes rows (one per
//! model) that survive the window closing. Measuring is the precondition
//! for saving: the whole efficiency story of Hark sits on this table.

use hark_agent::{TokenUsage, TurnResult};

/// What kind of Hark action spent these tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SpendKind {
    /// Voice/text question answered by the lean runner.
    Ask,
    /// The pre-execution evaluator.
    Gate,
    /// A conversational worker turn.
    Worker,
    /// One-shot dispatch.
    Dispatch,
    /// Historic row rebuilt from a session log file (tokens, no USD).
    Session,
}

impl SpendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpendKind::Ask => "ask",
            SpendKind::Gate => "gate",
            SpendKind::Worker => "worker",
            SpendKind::Dispatch => "dispatch",
            SpendKind::Session => "session",
        }
    }

    /// Inverse of `as_str` — reading rows back out of the ledger.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ask" => Some(SpendKind::Ask),
            "gate" => Some(SpendKind::Gate),
            "worker" => Some(SpendKind::Worker),
            "dispatch" => Some(SpendKind::Dispatch),
            "session" => Some(SpendKind::Session),
            _ => None,
        }
    }
}

/// Where the row came from. NEVER aggregate across sources: USD exists
/// only in `Live` rows; complete token history only in `Jsonl` rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SpendSource {
    Live,
    Jsonl,
}

impl SpendSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpendSource::Live => "live",
            SpendSource::Jsonl => "jsonl",
        }
    }

    /// Inverse of `as_str` — reading rows back out of the ledger.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "live" => Some(SpendSource::Live),
            "jsonl" => Some(SpendSource::Jsonl),
            _ => None,
        }
    }
}

/// One persisted ledger line: one model of one turn.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SpendRow {
    pub ts: String,
    pub kind: SpendKind,
    pub source: SpendSource,
    pub task_id: Option<String>,
    pub label: Option<String>,
    pub session_id: Option<String>,
    pub workspace: Option<String>,
    pub model: String,
    pub usage: TokenUsage,
    pub cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub is_error: bool,
    pub is_sidechain: bool,
    pub context_window: Option<u64>,
    /// Dedup key (jsonl rebuilds); None on live rows.
    pub request_id: Option<String>,
    /// Free-form marker ("gate:meta_hark", eco fingerprint...).
    pub outcome: Option<String>,
}

/// Context shared by every row of one turn.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpendMeta<'a> {
    pub task_id: Option<&'a str>,
    pub label: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub workspace: Option<&'a str>,
    pub outcome: Option<&'a str>,
}

/// Explode a live turn into ledger rows: one per model reported by the
/// CLI. Falls back to a costless "unknown" row only when the CLI gave
/// nothing at all but the turn still cost money.
pub fn rows_from_turn(
    ts: &str,
    kind: SpendKind,
    meta: &SpendMeta,
    turn: &TurnResult,
) -> Vec<SpendRow> {
    let base = |model: String, usage: TokenUsage, cost: Option<f64>, window: Option<u64>| SpendRow {
        ts: ts.to_string(),
        kind,
        source: SpendSource::Live,
        task_id: meta.task_id.map(String::from),
        label: meta.label.map(String::from),
        session_id: meta.session_id.map(String::from),
        workspace: meta.workspace.map(String::from),
        model,
        usage,
        cost_usd: cost,
        duration_ms: turn.duration_ms,
        is_error: turn.is_error,
        is_sidechain: false,
        context_window: window,
        request_id: None,
        outcome: meta.outcome.map(String::from),
    };
    if turn.usage.is_empty() {
        if turn.cost_usd.is_none() {
            return Vec::new();
        }
        return vec![base("unknown".into(), TokenUsage::default(), turn.cost_usd, None)];
    }
    turn.usage
        .iter()
        .map(|m| base(m.model.clone(), m.usage, m.cost_usd, m.context_window))
        .collect()
}

/// The context window actually in force for a model.
///
/// A model id can carry the window it was asked to run with — the 1M
/// beta shows up as `claude-opus-5[1m]` — while the turn keeps reporting
/// the base window. Believing the report turned a 464k prompt into "232%
/// of the window", which then tripped every heavy-session warning on a
/// session that was under half full. The id wins, but only upwards: a
/// reported window larger than the marker is the better number.
pub fn effective_window(model: &str, reported: Option<u64>) -> Option<u64> {
    let declared = model.to_ascii_lowercase().contains("[1m]").then_some(1_000_000);
    match (declared, reported) {
        (Some(d), Some(r)) => Some(d.max(r)),
        (Some(d), None) => Some(d),
        (None, r) => r,
    }
}

/// How full the context window is, from a turn's per-model usage.
///
/// The prompt of a turn IS the context, so the measure is prompt over
/// window. The trap is that one turn touches SEVERAL models — the main
/// one plus whatever answered a cheap side question — and each carries
/// its own prompt and its own window. Adding those prompts together and
/// dividing by the biggest window adds two unrelated conversations and
/// reports the total as one model's occupancy. Each model is measured
/// against its own window; the fullest one is the answer.
///
/// Not clamped: a ratio above 1.0 means the window we were told about is
/// not the window in force, and hiding that behind a confident 100% is
/// how a wrong number passes for a right one.
pub fn context_fill(usage: &[hark_agent::ModelUsage]) -> Option<f64> {
    usage
        .iter()
        .filter_map(|m| {
            let window = effective_window(&m.model, m.context_window).filter(|w| *w > 0)?;
            let prompt = m.usage.input + m.usage.cache_read + m.usage.cache_created;
            Some(prompt as f64 / window as f64)
        })
        .fold(None, |best: Option<f64>, r| Some(best.map_or(r, |b| b.max(r))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hark_agent::ModelUsage;

    fn turn(usage: Vec<ModelUsage>, cost: Option<f64>, is_error: bool) -> TurnResult {
        TurnResult {
            is_error,
            reply: None,
            raw: "r".into(),
            cost_usd: cost,
            duration_ms: Some(1200),
            model: None,
            usage,
        }
    }

    fn used(name: &str, prompt: u64, window: Option<u64>) -> ModelUsage {
        ModelUsage {
            model: name.into(),
            usage: hark_agent::TokenUsage { input: prompt, output: 0, cache_read: 0, cache_created: 0 },
            cost_usd: None,
            context_window: window,
        }
    }

    #[test]
    fn a_model_that_declares_a_million_is_not_measured_against_two_hundred_thousand() {
        // Observed: the pill reads "opus-5[1m]" and the turn still reports
        // contextWindow 200000, so a 464k prompt came out as 232% of the
        // window. The id is the honest source — it is what the CLI was
        // asked to run with.
        assert_eq!(effective_window("claude-opus-5[1m]", Some(200_000)), Some(1_000_000));
        assert_eq!(effective_window("claude-opus-5[1M]", None), Some(1_000_000));
    }

    #[test]
    fn a_model_that_declares_nothing_keeps_what_it_reported() {
        assert_eq!(effective_window("claude-opus-5", Some(200_000)), Some(200_000));
        assert_eq!(effective_window("claude-haiku-4-5", Some(200_000)), Some(200_000));
        assert_eq!(effective_window("claude-opus-5", None), None);
        // A bigger reported window is never talked down by the marker.
        assert_eq!(effective_window("claude-opus-5[1m]", Some(2_000_000)), Some(2_000_000));
    }

    #[test]
    fn two_models_in_one_turn_are_never_added_together() {
        // The big model is 80% full; a cheap side question filled half of
        // its own small window. Adding the prompts and dividing by the
        // larger window claimed 90% — two conversations counted as one.
        let fill = context_fill(&[
            used("opus", 800_000, Some(1_000_000)),
            used("haiku", 100_000, Some(200_000)),
        ]);
        assert_eq!(fill, Some(0.8));
    }

    #[test]
    fn the_fullest_model_is_the_answer() {
        let fill = context_fill(&[
            used("opus", 100_000, Some(1_000_000)),
            used("haiku", 180_000, Some(200_000)),
        ]);
        assert_eq!(fill, Some(0.9));
    }

    #[test]
    fn a_window_nobody_reported_is_not_a_measurement() {
        assert_eq!(context_fill(&[used("x", 50_000, None)]), None);
        assert_eq!(context_fill(&[]), None);
        // One model without a window must not sink one that has it.
        assert_eq!(
            context_fill(&[used("x", 9_000, None), used("opus", 500_000, Some(1_000_000))]),
            Some(0.5)
        );
    }

    #[test]
    fn overflowing_the_window_is_reported_not_hidden() {
        // Exactly the case that showed a confident 100%: the window we were
        // told about is not the one in force. Saying 1.25 is what makes
        // that visible instead of plausible.
        assert_eq!(context_fill(&[used("opus", 250_000, Some(200_000))]), Some(1.25));
    }

    fn model(name: &str, cost: f64) -> ModelUsage {
        ModelUsage {
            model: name.into(),
            usage: TokenUsage { input: 10, output: 20, cache_read: 30, cache_created: 40 },
            cost_usd: Some(cost),
            context_window: Some(200_000),
        }
    }

    #[test]
    fn one_row_per_model_and_costs_sum_to_total() {
        let t = turn(vec![model("haiku", 0.001), model("sonnet", 0.049)], Some(0.05), false);
        let meta = SpendMeta { task_id: Some("t-1"), label: Some("migração"), ..Default::default() };
        let rows = rows_from_turn("2026-08-17T10:00:00Z", SpendKind::Worker, &meta, &t);
        assert_eq!(rows.len(), 2);
        let total: f64 = rows.iter().filter_map(|r| r.cost_usd).sum();
        assert!((total - 0.05).abs() < 1e-9);
        assert!(rows.iter().all(|r| r.kind == SpendKind::Worker));
        assert!(rows.iter().all(|r| r.source == SpendSource::Live));
        assert!(rows.iter().all(|r| r.task_id.as_deref() == Some("t-1")));
        assert_eq!(rows[0].usage.cache_created, 40);
    }

    #[test]
    fn error_turns_still_become_rows() {
        let t = turn(vec![model("sonnet", 0.02)], Some(0.02), true);
        let rows = rows_from_turn("ts", SpendKind::Dispatch, &SpendMeta::default(), &t);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_error, "failed turns burn tokens too");
    }

    #[test]
    fn costless_empty_turns_produce_nothing_but_costed_ones_fall_back() {
        let silent = turn(vec![], None, false);
        assert!(rows_from_turn("ts", SpendKind::Ask, &SpendMeta::default(), &silent).is_empty());

        let costed = turn(vec![], Some(0.01), false);
        let rows = rows_from_turn("ts", SpendKind::Ask, &SpendMeta::default(), &costed);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].model, "unknown");
        assert_eq!(rows[0].cost_usd, Some(0.01));
    }
}
