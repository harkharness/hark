//! Parser for the payload Claude Code feeds to a `statusLine` command.
//!
//! This is the ONLY place that knows the real percentage of the
//! subscription windows (five-hour, weekly, per-model weekly) and the live
//! context-window occupancy — the CLI never puts them in `-p` output. A
//! bridge script (installed only when the user clicks) tees that stdin into
//! a file; this parser turns the file into numbers the UI can draw.

use serde::Serialize;

/// One usage window of the subscription.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LimitWindow {
    /// Raw key as the CLI names it: "five_hour", "seven_day", …
    pub key: String,
    /// 0.0-1.0 (the payload speaks percent).
    pub used: f64,
    /// ISO-8601 or epoch-seconds as a string, exactly as received.
    pub resets_at: Option<String>,
}

/// A snapshot of what the status line knew at its last render.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct StatusLine {
    /// Context window occupancy of the session that rendered it (0.0-1.0).
    pub context_used: Option<f64>,
    pub context_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub limits: Vec<LimitWindow>,
}

/// Percentages arrive as 0-100 in the payload; a few fields use 0-1. Treat
/// anything above 1 as percent, and clamp so a bad value can't paint a bar
/// past its track.
fn ratio(value: f64) -> f64 {
    let r = if value > 1.0 { value / 100.0 } else { value };
    r.clamp(0.0, 1.0)
}

fn as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Parse one statusLine payload. Unknown shapes yield None; unknown WINDOW
/// names are kept (the CLI adds new ones — "seven_day_opus" appeared after
/// the others, and the next one must show up without a code change).
pub fn parse(text: &str) -> Option<StatusLine> {
    let root: serde_json::Value = serde_json::from_str(text).ok()?;
    let object = root.as_object()?;

    let context = root.get("context_window").and_then(|c| c.as_object());
    let context_used = context
        .and_then(|c| c.get("used_percentage"))
        .and_then(|v| v.as_f64())
        .map(ratio);
    // Real keys first — "total_input_tokens" and "context_window_size" are
    // what a live 2.1.236 sends. The others were guesses written from
    // memory, and because the fixture guessed the same way the test agreed
    // with the bug: the window came back None and Hark inferred one from
    // billing tokens instead. They stay as fallbacks, not as the plan.
    let pick = |c: &serde_json::Map<String, serde_json::Value>, keys: &[&str]| {
        keys.iter().find_map(|k| c.get(*k)).and_then(|v| v.as_u64())
    };
    let context_tokens =
        context.and_then(|c| pick(c, &["total_input_tokens", "used_tokens", "input_tokens"]));
    let context_window =
        context.and_then(|c| pick(c, &["context_window_size", "context_window", "size"]));

    let mut limits: Vec<LimitWindow> = root
        .get("rate_limits")
        .and_then(|l| l.as_object())
        .map(|windows| {
            windows
                .iter()
                .filter_map(|(key, value)| {
                    let window = value.as_object()?;
                    let used = window
                        .get("used_percentage")
                        .or_else(|| window.get("used"))
                        .and_then(|v| v.as_f64())
                        .map(ratio)?;
                    Some(LimitWindow {
                        key: key.clone(),
                        used,
                        resets_at: window
                            .get("resets_at")
                            .or_else(|| window.get("reset_at"))
                            .and_then(as_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    // Heaviest window first: that is the one about to bite.
    limits.sort_by(|a, b| b.used.total_cmp(&a.used));

    Some(StatusLine {
        context_used,
        context_tokens,
        context_window,
        model: object
            .get("model")
            .and_then(|m| m.get("display_name").or_else(|| m.get("id")))
            .and_then(as_string),
        session_id: object.get("session_id").and_then(as_string),
        limits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = r#"{
      "session_id": "abc-123",
      "model": { "id": "claude-fable-5", "display_name": "Fable 5" },
      "workspace": { "current_dir": "/p/hark" },
      "context_window": { "used_tokens": 207200, "context_window": 1000000, "used_percentage": 21 },
      "rate_limits": {
        "five_hour": { "used_percentage": 64, "resets_at": "2026-08-17T21:00:00Z" },
        "seven_day": { "used_percentage": 59, "resets_at": "2026-08-18T17:00:00Z" },
        "seven_day_fable": { "used_percentage": 76 }
      }
    }"#;

    /// RECORDED from a live statusLine on claude 2.1.236 — the shape the
    /// CLI actually sends. The hand-written PAYLOAD above guessed
    /// "used_tokens" and "context_window"; the real keys are
    /// "total_input_tokens" and "context_window_size", so the parser found
    /// no window and Hark went off and inferred one from billing tokens
    /// instead. It read 232% for a session the CLI itself called 10%.
    const OBSERVED: &str = r#"{
      "session_id": "e3469264",
      "model": { "id": "claude-opus-5[1m]", "display_name": "Opus 5 (1M context)" },
      "version": "2.1.236",
      "context_window": {
        "total_input_tokens": 95471,
        "total_output_tokens": 1088,
        "context_window_size": 1000000,
        "current_usage": {
          "input_tokens": 2,
          "output_tokens": 1088,
          "cache_creation_input_tokens": 827,
          "cache_read_input_tokens": 94642
        },
        "used_percentage": 10,
        "remaining_percentage": 90
      }
    }"#;

    #[test]
    fn reads_the_window_the_cli_actually_sends() {
        let line = parse(OBSERVED).unwrap();
        assert_eq!(line.context_window, Some(1_000_000));
        assert_eq!(line.context_tokens, Some(95_471));
        assert!((line.context_used.unwrap() - 0.10).abs() < 1e-9);
        assert_eq!(line.model.as_deref(), Some("Opus 5 (1M context)"));
    }

    #[test]
    fn reads_context_and_every_limit_window() {
        let line = parse(PAYLOAD).unwrap();
        assert_eq!(line.session_id.as_deref(), Some("abc-123"));
        assert_eq!(line.model.as_deref(), Some("Fable 5"));
        assert_eq!(line.context_tokens, Some(207_200));
        assert_eq!(line.context_window, Some(1_000_000));
        assert!((line.context_used.unwrap() - 0.21).abs() < 1e-9, "percent → ratio");

        // Heaviest first, and an unknown window name survives.
        let keys: Vec<&str> = line.limits.iter().map(|l| l.key.as_str()).collect();
        assert_eq!(keys, vec!["seven_day_fable", "five_hour", "seven_day"]);
        assert!((line.limits[1].used - 0.64).abs() < 1e-9);
        assert_eq!(line.limits[1].resets_at.as_deref(), Some("2026-08-17T21:00:00Z"));
        assert_eq!(line.limits[0].resets_at, None, "missing reset is not an error");
    }

    #[test]
    fn survives_payloads_without_the_interesting_parts() {
        let bare = parse(r#"{"session_id":"s"}"#).unwrap();
        assert!(bare.limits.is_empty());
        assert_eq!(bare.context_used, None);
        // Already-normalized ratios must not be divided again.
        let ratios = parse(r#"{"rate_limits":{"five_hour":{"used":0.5}}}"#).unwrap();
        assert!((ratios.limits[0].used - 0.5).abs() < 1e-9);
        // Garbage in, nothing out (never a panic, never a fake number).
        assert!(parse("not json").is_none());
        assert!(parse("[1,2,3]").is_none());
        // A window with no usage at all is dropped, not shown as zero.
        let empty = parse(r#"{"rate_limits":{"five_hour":{"resets_at":"x"}}}"#).unwrap();
        assert!(empty.limits.is_empty());
    }
}
