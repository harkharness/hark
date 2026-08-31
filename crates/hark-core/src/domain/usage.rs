//! The /usage view, pure: ledger rows in, a structured session/day report
//! out. The shell only fetches rows and serializes — every number the card
//! shows is computed (and tested) here.

use serde::Serialize;

/// One model's line in the breakdown table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelLine {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_created: u64,
    pub cost_usd: f64,
    pub turns: usize,
}

/// Rows aggregated per model, most expensive first (ties: most tokens).
pub fn breakdown(rows: &[super::spend::SpendRow]) -> Vec<ModelLine> {
    let mut lines: Vec<ModelLine> = Vec::new();
    for row in rows {
        let line = match lines.iter_mut().find(|l| l.model == row.model) {
            Some(line) => line,
            None => {
                lines.push(ModelLine {
                    model: row.model.clone(),
                    input: 0,
                    output: 0,
                    cache_read: 0,
                    cache_created: 0,
                    cost_usd: 0.0,
                    turns: 0,
                });
                lines.last_mut().expect("just pushed")
            }
        };
        line.input += row.usage.input;
        line.output += row.usage.output;
        line.cache_read += row.usage.cache_read;
        line.cache_created += row.usage.cache_created;
        line.cost_usd += row.cost_usd.unwrap_or(0.0);
        line.turns += 1;
    }
    lines.sort_by(|a, b| {
        b.cost_usd
            .total_cmp(&a.cost_usd)
            .then_with(|| (b.input + b.output + b.cache_read).cmp(&(a.input + a.output + a.cache_read)))
    });
    lines
}

/// Share of the prompt served from cache: read / (input + read + created).
/// None when nothing was consumed at all.
pub fn cache_hit(lines: &[ModelLine]) -> Option<f64> {
    let read: u64 = lines.iter().map(|l| l.cache_read).sum();
    let input: u64 = lines.iter().map(|l| l.input).sum();
    let created: u64 = lines.iter().map(|l| l.cache_created).sum();
    let denom = read + input + created;
    (denom > 0).then(|| read as f64 / denom as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::spend::{SpendKind, SpendRow, SpendSource};
    use hark_agent::TokenUsage;

    fn row(model: &str, input: u64, output: u64, read: u64, created: u64, cost: f64) -> SpendRow {
        SpendRow {
            ts: "2026-08-31T10:00:00Z".into(),
            kind: SpendKind::Worker,
            source: SpendSource::Live,
            task_id: None,
            label: None,
            session_id: Some("s1".into()),
            workspace: None,
            model: model.into(),
            usage: TokenUsage { input, output, cache_read: read, cache_created: created },
            cost_usd: Some(cost),
            duration_ms: Some(1000),
            is_error: false,
            is_sidechain: false,
            context_window: None,
            request_id: None,
            outcome: None,
        }
    }

    #[test]
    fn breakdown_aggregates_per_model_most_expensive_first() {
        let rows = vec![
            row("haiku", 100, 50, 1000, 0, 0.01),
            row("fable-5", 500, 2000, 90_000, 3000, 0.42),
            row("fable-5", 300, 1000, 60_000, 1000, 0.30),
        ];
        let lines = breakdown(&rows);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].model, "fable-5");
        assert_eq!(lines[0].input, 800);
        assert_eq!(lines[0].output, 3000);
        assert_eq!(lines[0].cache_read, 150_000);
        assert_eq!(lines[0].cache_created, 4000);
        assert_eq!(lines[0].turns, 2);
        assert!((lines[0].cost_usd - 0.72).abs() < 1e-9);
        assert_eq!(lines[1].model, "haiku");
    }

    #[test]
    fn rows_without_cost_still_count_their_tokens() {
        let lines = breakdown(&[SpendRow { cost_usd: None, ..row("gemini", 10, 20, 0, 0, 0.0) }]);
        assert_eq!(lines[0].output, 20);
        assert_eq!(lines[0].cost_usd, 0.0);
    }

    #[test]
    fn cache_hit_is_read_over_everything_entering_the_window() {
        let lines = breakdown(&[row("fable-5", 1000, 500, 98_000, 1000, 0.1)]);
        let hit = cache_hit(&lines).expect("consumed something");
        assert!((hit - 0.98).abs() < 0.001);
    }

    #[test]
    fn cache_hit_of_an_empty_session_is_none() {
        assert_eq!(cache_hit(&[]), None);
        assert_eq!(cache_hit(&breakdown(&[row("m", 0, 5, 0, 0, 0.0)])), None);
    }
}
