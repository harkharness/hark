//! Per-turn numbers out of a stream that reports running totals.
//!
//! The CLI's `result` line carries `total_cost_usd` and `modelUsage` as
//! totals for the whole PROCESS, not for the turn that just ended. The
//! ledger showed it (08/2026): a second turn a minute after the first
//! "read from cache" exactly the tokens the first one had written, and the
//! booked cost of a long chat grew with every message. Since CLI 2.1.277 a
//! resumed session no longer even starts those totals at zero: they open
//! at the session's lifetime figures. Taken at face value, one message
//! into a 176-turn terminal session booked the whole session as that turn
//! ($1286) and its lifetime tokens as the context in use (30491%), which
//! set off a compaction nobody asked for (25/09).
//!
//! The meter keeps the previous totals and hands out the difference. The
//! first result of a process has nothing to subtract, so it is checked
//! against what the turn's own API calls reported (every `assistant` line
//! carries its call's usage): totals far beyond the calls mean the process
//! opened on inherited figures, and that turn keeps the calls' tokens with
//! its price left unknown, rather than billed the session's history.

use hark_agent::{ContextReading, ModelUsage, TokenUsage, TurnResult};
use serde_json::Value;

/// One API call of the current turn, as its `assistant` line reported it.
#[derive(Debug, Clone)]
struct Call {
    id: String,
    model: String,
    usage: TokenUsage,
    /// False for a subagent's call: its context is not the chat's.
    main: bool,
}

/// The running totals as of the last result of this process.
#[derive(Debug, Clone)]
struct Totals {
    cost: Option<f64>,
    models: Vec<ModelUsage>,
}

/// One per process: fed every stdout line, asked to settle every result.
#[derive(Debug, Default)]
pub struct Meter {
    last: Option<Totals>,
    calls: Vec<Call>,
}

/// Room for the calls a turn makes that never surface as an `assistant`
/// line (a title, a web page summarised by a small model): a fresh
/// process's first totals may exceed its visible calls by this much and
/// still be taken as its own.
const UNSEEN_CALLS_SLACK: u64 = 200_000;

fn tokens(u: &TokenUsage) -> u64 {
    u.input + u.output + u.cache_read + u.cache_created
}

impl Meter {
    /// One stdout line through the meter: every line informs it, and a
    /// result comes out as the turn's own numbers.
    pub fn read(&mut self, line: &str) -> hark_agent::AgentEvent {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return hark_agent::AgentEvent::Ignored;
        };
        self.observe(&v);
        match crate::stream::parse_value(&v) {
            hark_agent::AgentEvent::Result(r) => hark_agent::AgentEvent::Result(self.settle(r)),
            other => other,
        }
    }

    /// Records the usage of an `assistant` line; every other line is
    /// ignored.
    pub fn observe(&mut self, line: &Value) {
        if line.get("type").and_then(Value::as_str) != Some("assistant") {
            return;
        }
        let Some(message) = line.get("message") else { return };
        let Some(usage) = message.get("usage") else { return };
        let model = message.get("model").and_then(Value::as_str).unwrap_or_default();
        // "<synthetic>" is the CLI speaking for itself (an API error it
        // turned into a message): no call behind it.
        if model.is_empty() || model == "<synthetic>" {
            return;
        }
        let num = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        let usage = TokenUsage {
            input: num("input_tokens"),
            output: num("output_tokens"),
            cache_read: num("cache_read_input_tokens"),
            cache_created: num("cache_creation_input_tokens"),
        };
        let id = message.get("id").and_then(Value::as_str).unwrap_or_default();
        // One message arrives block by block, each line repeating its
        // usage: the latest line of a message is the one that counts.
        if let Some(seen) = self.calls.iter_mut().find(|c| !id.is_empty() && c.id == id) {
            seen.usage = usage;
            return;
        }
        self.calls.push(Call {
            id: id.to_string(),
            model: model.to_string(),
            usage,
            main: line.get("parent_tool_use_id").is_none_or(Value::is_null),
        });
    }

    /// The result line's running totals in, the turn's own numbers out.
    pub fn settle(&mut self, reported: TurnResult) -> TurnResult {
        let calls = std::mem::take(&mut self.calls);
        let context = calls.iter().rev().find(|c| c.main).map(|c| ContextReading {
            model: c.model.clone(),
            tokens: c.usage.input + c.usage.cache_read + c.usage.cache_created,
        });
        // No totals at all (an auth failure reports {} and 0) says nothing
        // about the process: the last totals stand.
        if reported.usage.is_empty() {
            return TurnResult { context, ..reported };
        }
        let now = Totals { cost: reported.cost_usd, models: reported.usage.clone() };
        let turn = match self.last.replace(now) {
            Some(prev) if !went_back(&prev, &reported) => since(&prev, reported),
            Some(_) => reported,
            None if inherited(&reported.usage, &calls) => from_calls(reported, &calls),
            None => reported,
        };
        TurnResult { context, ..turn }
    }
}

/// Totals only grow within one count. Lower numbers mean a new count
/// started, and the new numbers are taken as they come.
fn went_back(prev: &Totals, now: &TurnResult) -> bool {
    let cost_fell = matches!((prev.cost, now.cost_usd), (Some(p), Some(n)) if n + 1e-9 < p);
    cost_fell
        || prev.models.iter().any(|p| {
            now.usage.iter().find(|n| n.model == p.model).is_none_or(|n| {
                n.usage.input < p.usage.input
                    || n.usage.output < p.usage.output
                    || n.usage.cache_read < p.usage.cache_read
                    || n.usage.cache_created < p.usage.cache_created
            })
        })
}

fn minus(now: Option<f64>, before: Option<f64>) -> Option<f64> {
    match (now, before) {
        (Some(n), Some(b)) => Some((n - b).max(0.0)),
        (n, None) => n,
        (None, Some(_)) => None,
    }
}

/// This turn = these totals minus the previous ones. A model that did not
/// move is left out: it spent nothing this turn.
fn since(prev: &Totals, now: TurnResult) -> TurnResult {
    let usage: Vec<ModelUsage> = now
        .usage
        .iter()
        .filter_map(|m| {
            let before = prev.models.iter().find(|p| p.model == m.model);
            let b = before.map(|p| p.usage).unwrap_or_default();
            let usage = TokenUsage {
                input: m.usage.input - b.input,
                output: m.usage.output - b.output,
                cache_read: m.usage.cache_read - b.cache_read,
                cache_created: m.usage.cache_created - b.cache_created,
            };
            (tokens(&usage) > 0).then(|| ModelUsage {
                model: m.model.clone(),
                usage,
                cost_usd: minus(m.cost_usd, before.and_then(|p| p.cost_usd)),
                context_window: m.context_window,
            })
        })
        .collect();
    TurnResult {
        cost_usd: minus(now.cost_usd, prev.cost),
        model: busiest(&usage).or(now.model),
        usage,
        ..now
    }
}

/// Did this process open on figures it did not earn? Its first totals
/// can only be its own calls plus the few that never show as a line.
fn inherited(reported: &[ModelUsage], calls: &[Call]) -> bool {
    let reported: u64 = reported.iter().map(|m| tokens(&m.usage)).sum();
    let seen: u64 = calls.iter().map(|c| tokens(&c.usage)).sum();
    reported > seen.saturating_mul(2).saturating_add(UNSEEN_CALLS_SLACK)
}

/// The totals' own name for a call's model: `claude-opus-5` is reported
/// as `claude-opus-5[1m]` when it ran with the long window.
fn totals_name<'a>(reported: &'a [ModelUsage], model: &str) -> Option<&'a ModelUsage> {
    reported.iter().find(|m| m.model == model).or_else(|| {
        reported
            .iter()
            .find(|m| m.model.strip_prefix(model).is_some_and(|rest| rest.starts_with('[')))
    })
}

/// A turn priced from its calls alone: tokens exact, dollars unknown.
fn from_calls(reported: TurnResult, calls: &[Call]) -> TurnResult {
    let mut usage: Vec<ModelUsage> = Vec::new();
    for c in calls {
        let known = totals_name(&reported.usage, &c.model);
        let name = known.map_or(c.model.as_str(), |m| m.model.as_str());
        match usage.iter_mut().find(|m| m.model == name) {
            Some(m) => {
                m.usage.input += c.usage.input;
                m.usage.output += c.usage.output;
                m.usage.cache_read += c.usage.cache_read;
                m.usage.cache_created += c.usage.cache_created;
            }
            None => usage.push(ModelUsage {
                model: name.to_string(),
                usage: c.usage,
                cost_usd: None,
                context_window: known.and_then(|m| m.context_window),
            }),
        }
    }
    let main = calls
        .iter()
        .rev()
        .find(|c| c.main)
        .map(|c| totals_name(&reported.usage, &c.model).map_or(c.model.clone(), |m| m.model.clone()));
    TurnResult {
        cost_usd: None,
        model: main.or(reported.model),
        usage,
        ..reported
    }
}

/// The model that did the most of a turn: the dearest, else the one that
/// moved the most tokens.
fn busiest(usage: &[ModelUsage]) -> Option<String> {
    usage
        .iter()
        .max_by(|a, b| {
            let cost = a.cost_usd.unwrap_or(0.0).total_cmp(&b.cost_usd.unwrap_or(0.0));
            cost.then(tokens(&a.usage).cmp(&tokens(&b.usage)))
        })
        .map(|m| m.model.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(id: &str, model: &str, input: u64, read: u64, written: u64, output: u64) -> Value {
        json!({"type": "assistant", "parent_tool_use_id": null, "message": {
            "id": id, "model": model,
            "usage": {"input_tokens": input, "cache_read_input_tokens": read,
                      "cache_creation_input_tokens": written, "output_tokens": output},
            "content": [{"type": "text", "text": "…"}]}})
    }

    fn subagent_call(id: &str, model: &str, read: u64) -> Value {
        let mut v = call(id, model, 0, read, 0, 10);
        v["parent_tool_use_id"] = json!("toolu_1");
        v
    }

    fn totals(model: &str, cost: f64, read: u64, written: u64, output: u64) -> TurnResult {
        TurnResult {
            is_error: false,
            reply: None,
            raw: "ok".into(),
            cost_usd: Some(cost),
            duration_ms: Some(1000),
            model: Some(model.into()),
            usage: vec![ModelUsage {
                model: model.into(),
                usage: TokenUsage { input: 10, output, cache_read: read, cache_created: written },
                cost_usd: Some(cost),
                context_window: Some(1_000_000),
            }],
            context: None,
        }
    }

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn a_second_turn_books_only_what_it_added() {
        // Recorded (ledger, 21/08): two turns of one worker, seven seconds
        // apart. The second "read" 74802 more tokens: exactly what the
        // first wrote. Totals, not turns.
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-opus-5", 10, 290_925, 74_802, 5008));
        let first = meter.settle(totals("claude-opus-5", 1.0188, 290_925, 74_802, 5008));
        assert!(close(first.cost_usd, 1.0188));

        meter.observe(&call("m2", "claude-opus-5", 0, 74_802, 2_156, 402));
        let second = meter.settle(totals("claude-opus-5", 1.0878, 365_727, 76_958, 5410));
        assert!(close(second.cost_usd, 1.0878 - 1.0188), "{:?}", second.cost_usd);
        assert_eq!(second.usage.len(), 1);
        assert_eq!(second.usage[0].usage.cache_read, 74_802);
        assert_eq!(second.usage[0].usage.cache_created, 2_156);
        assert_eq!(second.usage[0].usage.output, 402);
        assert!(close(second.usage[0].cost_usd, 1.0878 - 1.0188));
    }

    #[test]
    fn a_fresh_process_takes_its_first_turn_as_reported() {
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-opus-5", 10, 40_000, 9_000, 800));
        meter.observe(&call("m2", "claude-opus-5", 5, 49_000, 1_200, 300));
        let turn = meter.settle(totals("claude-opus-5", 0.61, 89_000, 10_200, 1100));
        assert!(close(turn.cost_usd, 0.61));
        assert_eq!(turn.usage[0].usage.cache_read, 89_000);
    }

    #[test]
    fn a_resume_that_opens_on_the_sessions_totals_leaves_that_turn_unpriced() {
        // 25/09: one call into a long terminal session; the result carried
        // the session's lifetime (178.8M read, $1286). The call itself read
        // 900k. Billing the history as this turn is the bug.
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-fable-5-1", 3, 900_000, 2_000, 1500));
        let turn = meter.settle(totals("claude-fable-5-1[1m]", 1286.36, 178_800_000, 2_000_000, 400_000));
        assert_eq!(turn.cost_usd, None, "the history is not this turn's price");
        assert_eq!(turn.usage.len(), 1);
        let only = &turn.usage[0];
        assert_eq!(only.usage.cache_read, 900_000, "tokens come from the call itself");
        assert_eq!(only.usage.output, 1500);
        assert_eq!(only.cost_usd, None);
        // Named like the totals name it, window included, so the ledger
        // row and the context gauge agree with every other row.
        assert_eq!(only.model, "claude-fable-5-1[1m]");
        assert_eq!(only.context_window, Some(1_000_000));

        // From here on the totals are known, and turns are exact again.
        meter.observe(&call("m2", "claude-fable-5-1", 2, 902_000, 1_000, 700));
        let next = meter.settle(totals("claude-fable-5-1[1m]", 1290.36, 179_702_000, 2_001_000, 400_700));
        assert!(close(next.cost_usd, 4.0), "{:?}", next.cost_usd);
        assert_eq!(next.usage[0].usage.cache_read, 902_000);
    }

    #[test]
    fn the_context_is_the_last_main_call_not_the_sum_of_the_turn() {
        // A turn that ran tools reads the context once per call: its usage
        // adds those reads up, and dividing that by the window is how a
        // 300k chat reads as 30491%.
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-opus-5", 3, 300_000, 1_000, 100));
        meter.observe(&subagent_call("s1", "claude-haiku-4-5", 700_000));
        meter.observe(&call("m2", "claude-opus-5", 2, 301_000, 900, 100));
        let turn = meter.settle(totals("claude-opus-5[1m]", 1.2, 1_301_000, 1_900, 200));
        assert_eq!(
            turn.context,
            Some(ContextReading { model: "claude-opus-5".into(), tokens: 2 + 301_000 + 900 })
        );
    }

    #[test]
    fn a_message_seen_block_by_block_counts_once() {
        // One API message arrives as one line per content block, each
        // repeating the message's usage.
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-opus-5", 3, 400_000, 1_000, 50));
        meter.observe(&call("m1", "claude-opus-5", 3, 400_000, 1_000, 420));
        let turn = meter.settle(totals("claude-opus-5", 0.5, 400_000, 1_000, 420));
        assert!(close(turn.cost_usd, 0.5), "one call, not two: {:?}", turn.cost_usd);
        assert_eq!(turn.context.map(|c| c.tokens), Some(3 + 400_000 + 1_000));
    }

    #[test]
    fn a_result_that_reports_nothing_leaves_the_totals_standing() {
        // An auth failure reports cost 0 and an empty modelUsage. Taken as
        // the new totals, the next turn would be billed from zero again.
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-opus-5", 10, 50_000, 5_000, 100));
        meter.settle(totals("claude-opus-5", 1.0, 50_000, 5_000, 100));
        let silent = TurnResult {
            is_error: true,
            cost_usd: Some(0.0),
            usage: vec![],
            ..totals("claude-opus-5", 0.0, 0, 0, 0)
        };
        let failed = meter.settle(silent);
        assert!(failed.usage.is_empty());

        meter.observe(&call("m2", "claude-opus-5", 1, 55_000, 500, 60));
        let next = meter.settle(totals("claude-opus-5", 1.25, 105_000, 5_500, 160));
        assert!(close(next.cost_usd, 0.25), "{:?}", next.cost_usd);
    }

    #[test]
    fn totals_that_go_backwards_are_a_new_count_taken_as_reported() {
        let mut meter = Meter::default();
        meter.settle(totals("claude-opus-5", 2.0, 500_000, 50_000, 900));
        let turn = meter.settle(totals("claude-opus-5", 0.3, 40_000, 8_000, 100));
        assert!(close(turn.cost_usd, 0.3));
        assert_eq!(turn.usage[0].usage.cache_read, 40_000);
    }

    #[test]
    fn reading_stdout_lines_meters_the_results_they_end_in() {
        // The worker's stdout as the bridges see it: text lines in, events
        // out, the result already reduced to its own turn.
        let mut meter = Meter::default();
        let lines = [
            call("m1", "claude-opus-5", 10, 290_925, 74_802, 5008).to_string(),
            r#"{"type":"result","subtype":"success","is_error":false,"result":"um","total_cost_usd":1.0188,
                "modelUsage":{"claude-opus-5":{"inputTokens":10,"outputTokens":5008,"cacheReadInputTokens":290925,"cacheCreationInputTokens":74802,"costUSD":1.0188,"contextWindow":1000000}}}"#
                .replace('\n', ""),
            call("m2", "claude-opus-5", 0, 74_802, 2_156, 402).to_string(),
            r#"{"type":"result","subtype":"success","is_error":false,"result":"dois","total_cost_usd":1.0878,
                "modelUsage":{"claude-opus-5":{"inputTokens":10,"outputTokens":5410,"cacheReadInputTokens":365727,"cacheCreationInputTokens":76958,"costUSD":1.0878,"contextWindow":1000000}}}"#
                .replace('\n', ""),
        ];
        let results: Vec<TurnResult> = lines
            .iter()
            .filter_map(|l| match meter.read(l) {
                hark_agent::AgentEvent::Result(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(results.len(), 2);
        assert!(close(results[1].cost_usd, 1.0878 - 1.0188), "{:?}", results[1].cost_usd);
        assert_eq!(results[1].context.as_ref().map(|c| c.tokens), Some(74_802 + 2_156));
        assert!(matches!(meter.read("not json"), hark_agent::AgentEvent::Ignored));
    }

    #[test]
    fn a_turn_without_calls_books_nothing_new() {
        // A local command (/cost, /context) answers without the API: the
        // totals repeat, and the repeat is not a second bill.
        let mut meter = Meter::default();
        meter.observe(&call("m1", "claude-opus-5", 10, 50_000, 5_000, 100));
        meter.settle(totals("claude-opus-5", 1.0, 50_000, 5_000, 100));
        let repeat = meter.settle(totals("claude-opus-5", 1.0, 50_000, 5_000, 100));
        assert!(close(repeat.cost_usd, 0.0), "{:?}", repeat.cost_usd);
        assert!(repeat.usage.is_empty(), "{:?}", repeat.usage);
        assert_eq!(repeat.context, None, "no call, no reading");
    }
}
