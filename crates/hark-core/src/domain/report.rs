//! A bug report, as the app assembles it: what the machine knows about
//! itself, a scrubbed tail of its own log, and the GitHub issue that
//! carries it. Nothing here talks to the network: the person opens the
//! issue in their own browser, under their own account, and can edit
//! every line before pressing "Submit".

use super::redact::redact;

/// The public repository issues go to.
pub const REPO: &str = "harkharness/hark";

/// What the machine knows about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bundle {
    pub app_version: String,
    /// "macOS 15.6", or whatever `sw_vers` said.
    pub os: String,
    /// `std::env::consts::ARCH`: "aarch64" | "x86_64".
    pub arch: String,
    /// Agent ids with the version their binary reports, when detected.
    pub agents: Vec<(String, Option<String>)>,
    /// Config as `key = value`, secrets already masked by the caller.
    pub config: Vec<(String, String)>,
    /// The newest lines of the app log, oldest first.
    pub log: Vec<String>,
}

/// Which "Mac" the issue form's dropdown means.
pub fn mac_label(arch: &str) -> &'static str {
    if arch == "aarch64" { "Apple Silicon" } else { "Intel" }
}

/// The agents line of the issue form ("claude 2.1.281, gemini 0.60.0").
pub fn agents_line(agents: &[(String, Option<String>)]) -> String {
    agents
        .iter()
        .filter_map(|(id, v)| v.as_ref().map(|v| format!("{id} {v}")))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The diagnostic block as it goes into the issue: everything scrubbed,
/// the log folded so the description stays readable.
pub fn render(bundle: &Bundle) -> String {
    let mut out = String::new();
    out.push_str(&format!("Hark {} · {} · {}\n", bundle.app_version, bundle.os, mac_label(&bundle.arch)));
    let agents = agents_line(&bundle.agents);
    if !agents.is_empty() {
        out.push_str(&format!("Agents: {agents}\n"));
    }
    if !bundle.config.is_empty() {
        out.push_str("\n<details><summary>config</summary>\n\n```toml\n");
        for (k, v) in &bundle.config {
            out.push_str(&format!("{k} = {v}\n"));
        }
        out.push_str("```\n</details>\n");
    }
    if !bundle.log.is_empty() {
        out.push_str(&format!("\n<details><summary>log · {} lines</summary>\n\n```\n", bundle.log.len()));
        for line in &bundle.log {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("```\n</details>\n");
    }
    redact(&out)
}

/// What the person typed, plus what the machine filled in.
#[derive(Debug, Clone, Default)]
pub struct IssueFields {
    pub title: String,
    pub what: String,
    pub steps: String,
    pub version: String,
    pub arch: String,
    pub macos: String,
    pub agent: String,
    /// The rendered bundle, when the person chose to include it.
    pub logs: Option<String>,
}

/// A URL longer than this makes GitHub answer 414; the logs then travel
/// on the clipboard instead, and the person pastes them into the form.
pub const URL_LIMIT: usize = 6_000;

/// The "new issue" URL with the bug form prefilled. `logs_in_url` says
/// whether the diagnostic block fit; when it did not, the caller offers
/// it another way.
pub fn issue_url(fields: &IssueFields) -> (String, bool) {
    let base = format!("https://github.com/{REPO}/issues/new?template=bug_report.yml");
    let mut params: Vec<(&str, String)> = vec![
        ("title", fields.title.clone()),
        ("what", fields.what.clone()),
        ("steps", fields.steps.clone()),
        ("version", fields.version.clone()),
        ("arch", mac_label(&fields.arch).to_string()),
        ("macos", fields.macos.clone()),
        ("agent", fields.agent.clone()),
    ];
    let query = |params: &[(&str, String)]| {
        params
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| format!("&{k}={}", encode(&redact(v))))
            .collect::<String>()
    };
    if let Some(logs) = &fields.logs {
        params.push(("logs", logs.clone()));
        let url = format!("{base}{}", query(&params));
        if url.len() <= URL_LIMIT {
            return (url, true);
        }
        params.pop();
    }
    (format!("{base}{}", query(&params)), false)
}

/// Percent-encoding of a query value (RFC 3986 unreserved kept).
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// One log line for a UI event: its kind and the facts that help a bug
/// report, never the bodies (prose, tool inputs, tool outputs). None for
/// events that say nothing a report needs.
pub fn summarize_event(payload: &serde_json::Value) -> Option<String> {
    let kind = payload.get("kind")?.as_str()?;
    let s = |key: &str| payload.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let task = s("task_id").map(|t| format!(" task={t}")).unwrap_or_default();
    let line = match kind {
        "worker_turn" => {
            let mut facts = Vec::new();
            if payload.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
                facts.push("error".to_string());
            }
            if let Some(code) = s("error_code") {
                facts.push(format!("code={code}"));
            }
            if let Some(model) = s("model") {
                facts.push(format!("model={model}"));
            }
            if let Some(cost) = payload.get("cost_usd").and_then(|v| v.as_f64()) {
                facts.push(format!("cost={cost:.4}"));
            }
            if let Some(pct) = payload.get("context_pct").and_then(|v| v.as_f64()) {
                facts.push(format!("context={:.0}%", pct * 100.0));
            }
            if let Some(text) = s("text").filter(|_| payload.get("is_error").and_then(|v| v.as_bool()) == Some(true)) {
                facts.push(format!("text={:?}", text.chars().take(160).collect::<String>()));
            }
            format!("{kind}{task} {}", facts.join(" "))
        }
        "worker_exit" => format!(
            "{kind}{task} reason={:?} exit={}",
            s("reason").unwrap_or_default(),
            payload.get("exit_code").map(|v| v.to_string()).unwrap_or_else(|| "null".into())
        ),
        "worker" => format!("{kind}{task} tool={}", s("name").unwrap_or_default()),
        "status" | "queued_failed" => format!("{kind}{task} {}", s("text").or_else(|| s("why")).unwrap_or_default()),
        "rate_limit" => format!("{kind} status={}", s("status").unwrap_or_default()),
        "worker_started" | "session_started" => format!("{kind}{task}"),
        _ => return None,
    };
    Some(redact(&line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> Bundle {
        Bundle {
            app_version: "0.7.2".into(),
            os: "macOS 15.6".into(),
            arch: "aarch64".into(),
            agents: vec![("claude".into(), Some("2.1.281".into())), ("gemini".into(), None)],
            config: vec![("model".into(), "\"sonnet\"".into()), ("worker_budget_usd".into(), "5".into())],
            log: vec!["12:00 worker_turn task=t-1 error code=budget".into(), "12:01 cwd /Users/ana/proj".into()],
        }
    }

    #[test]
    fn the_rendered_bundle_is_scrubbed_and_folds_the_log() {
        let md = render(&bundle());
        assert!(md.starts_with("Hark 0.7.2 · macOS 15.6 · Apple Silicon\n"), "{md}");
        assert!(md.contains("Agents: claude 2.1.281\n"), "an undetected agent has no version to show");
        assert!(md.contains("<details><summary>log · 2 lines</summary>"));
        assert!(md.contains("cwd ~/proj") && !md.contains("/Users/ana"), "{md}");
        assert!(md.contains("worker_budget_usd = 5"));
    }

    #[test]
    fn the_issue_url_prefills_the_bug_form() {
        let fields = IssueFields {
            title: "Composer doubles text".into(),
            what: "Typed a long draft\nsaw two copies".into(),
            steps: "1. type".into(),
            version: "0.7.2".into(),
            arch: "x86_64".into(),
            macos: "14.5".into(),
            agent: "claude 2.1.281".into(),
            logs: None,
        };
        let (url, logs_in_url) = issue_url(&fields);
        assert!(url.starts_with("https://github.com/harkharness/hark/issues/new?template=bug_report.yml&"));
        assert!(url.contains("&title=Composer%20doubles%20text"));
        assert!(url.contains("&what=Typed%20a%20long%20draft%0Asaw%20two%20copies"));
        assert!(url.contains("&arch=Intel"));
        assert!(url.contains("&macos=14.5") && url.contains("&agent=claude%202.1.281"));
        assert!(!url.contains("logs="), "no logs were offered");
        assert!(!logs_in_url);
    }

    #[test]
    fn logs_ride_the_url_only_while_it_stays_short() {
        let mut fields = IssueFields { version: "0.7.2".into(), logs: Some("short log".into()), ..Default::default() };
        let (url, in_url) = issue_url(&fields);
        assert!(in_url && url.contains("&logs=short%20log"));
        fields.logs = Some("x".repeat(URL_LIMIT));
        let (url, in_url) = issue_url(&fields);
        assert!(!in_url && !url.contains("logs="), "too long: the clipboard carries it");
        assert!(url.len() < URL_LIMIT);
    }

    #[test]
    fn what_the_person_typed_is_scrubbed_too() {
        let fields = IssueFields { what: "it failed with sk-ant-api03-abcdefghijklmnopqrstuvwxyz".into(), ..Default::default() };
        let (url, _) = issue_url(&fields);
        assert!(url.contains("%3Credacted%3E") && !url.contains("sk-ant"), "{url}");
    }

    #[test]
    fn an_event_summary_keeps_the_facts_and_drops_the_bodies() {
        let turn = serde_json::json!({ "kind": "worker_turn", "task_id": "t-1", "is_error": true,
            "error_code": "budget", "model": "claude-opus-5", "cost_usd": 0.4211, "context_pct": 0.31,
            "text": "the whole answer /Users/ana/secret" });
        let line = summarize_event(&turn).unwrap();
        assert!(line.starts_with("worker_turn task=t-1 error code=budget model=claude-opus-5 cost=0.4211 context=31%"), "{line}");
        assert!(line.contains("~/secret") && !line.contains("/Users/ana"));
        let ok = serde_json::json!({ "kind": "worker_turn", "task_id": "t-1", "is_error": false, "text": "prose" });
        assert!(!summarize_event(&ok).unwrap().contains("prose"), "a good turn's prose is not the log's business");
        let tool = serde_json::json!({ "kind": "worker", "task_id": "t-1", "name": "Bash", "input": "{\"command\":\"rm -rf x\"}" });
        assert_eq!(summarize_event(&tool).unwrap(), "worker task=t-1 tool=Bash");
        assert_eq!(summarize_event(&serde_json::json!({ "kind": "assistant_text", "text": "hi" })), None);
    }
}
