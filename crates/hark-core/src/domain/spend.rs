//! The spend ledger's pure heart: every Claude turn becomes rows (one per
//! model) that survive the window closing. Measuring is the precondition
//! for saving: the whole efficiency story of Hark sits on this table.

use crate::domain::claude_event::{TokenUsage, TurnResult};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::claude_event::ModelUsage;

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
