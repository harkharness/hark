//! Pure pieces of Hark's file-based memory: the Q&A journal and the
//! machine-wide worker registry entries.

use serde::{Deserialize, Serialize};

/// One dispatched worker, as recorded in the global state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub task_id: String,
    /// Which backend ran this work. Registry id — "claude" for everything
    /// recorded before Hark knew there could be another one.
    #[serde(default = "claude_id")]
    pub agent: String,
    pub context: String,
    pub workspace: String,
    pub session_id: String,
    pub status: WorkerStatus,
    pub started_at: String,
    pub summary: String,
}

fn claude_id() -> String {
    "claude".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Running,
    Done,
    Failed,
}

/// The journal remembers the ANSWER, not just the spoken headline — but it
/// is memory, not archive: a runaway body gets clipped.
const BODY_MAX_CHARS: usize = 700;

/// Render one journal entry block (append-only markdown). `body` is the
/// on-screen content (detalhes + itens); empty keeps the classic Q/A shape.
pub fn journal_entry(ts: &str, question: &str, fala: &str, body: &str) -> String {
    let question = flatten(question);
    let fala = flatten(fala);
    let body = clip(&sanitize_body(body), BODY_MAX_CHARS);
    if body.is_empty() {
        format!("## {ts}\nQ: {question}\nA: {fala}\n\n")
    } else {
        format!("## {ts}\nQ: {question}\nA: {fala}\n{body}\n\n")
    }
}

/// Q and A live on single lines so the block stays parseable.
fn flatten(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Markdown headers inside a body would masquerade as new entries
/// (the journal splits blocks on `\n## `); soften them into plain lines.
fn sanitize_body(body: &str) -> String {
    body.trim()
        .lines()
        .map(|line| match line.trim_start().starts_with('#') {
            true => format!("— {}", line.trim_start().trim_start_matches('#').trim_start()),
            false => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

/// One remembered Q&A turn, parsed back from the journal (restores the
/// mother thread across app restarts — local, zero tokens).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalTurn {
    pub ts: String,
    pub question: String,
    pub fala: String,
    pub body: String,
}

/// Newest `n` journal entries as structured turns, oldest first.
/// Blocks without the Q/A shape are skipped, never errors.
pub fn parse_journal(content: &str, n: usize) -> Vec<JournalTurn> {
    let turns: Vec<JournalTurn> = content
        .split("\n## ")
        .map(|block| block.trim_start_matches("## ").trim())
        .filter(|block| !block.is_empty())
        .filter_map(parse_block)
        .collect();
    let skip = turns.len().saturating_sub(n);
    turns.into_iter().skip(skip).collect()
}

fn parse_block(block: &str) -> Option<JournalTurn> {
    let mut lines = block.lines();
    let ts = lines.next()?.trim().to_string();
    let question = lines.next()?.strip_prefix("Q: ")?.trim().to_string();
    let fala = lines.next()?.strip_prefix("A: ")?.trim().to_string();
    let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    Some(JournalTurn { ts, question, fala, body })
}

/// Last `n` entries of a journal, oldest first.
pub fn journal_tail(content: &str, n: usize) -> Vec<String> {
    let entries: Vec<String> = content
        .split("\n## ")
        .map(|block| block.trim_start_matches("## ").trim().to_string())
        .filter(|block| !block.is_empty())
        .collect();
    let skip = entries.len().saturating_sub(n);
    entries.into_iter().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every worker on disk today was recorded before backends were a
    /// thing. Reading one must not fail, and must not leave the agent
    /// blank — the whole registry keys on that id.
    #[test]
    fn a_worker_recorded_before_agents_existed_reads_as_claude() {
        let old = r#"{
            "task_id": "t-1",
            "context": "fabrica",
            "workspace": "/w",
            "session_id": "s-1",
            "status": "running",
            "started_at": "2026-08-01T10:00:00Z",
            "summary": ""
        }"#;
        let record: WorkerRecord = serde_json::from_str(old).expect("old records still parse");
        assert_eq!(record.agent, "claude");
    }

    #[test]
    fn journal_roundtrip_keeps_last_n_entries() {
        let content: String = (0..7)
            .map(|i| journal_entry(&format!("2026-08-14T10:0{i}:00Z"), &format!("q{i}"), "ok", ""))
            .collect();
        let tail = journal_tail(&content, 3);
        assert_eq!(tail.len(), 3);
        assert!(tail[0].contains("q4"));
        assert!(tail[2].contains("q6"));
        assert!(tail[2].contains("A: ok"));
    }

    #[test]
    fn journal_tail_of_empty_content_is_empty() {
        assert!(journal_tail("", 5).is_empty());
    }

    #[test]
    fn journal_remembers_the_body_not_just_the_headline() {
        let entry = journal_entry(
            "2026-08-30T10:00:00Z",
            "quais as pendências?",
            "Três pendências na tela.",
            "O Assinaturas espera revisão desde quinta.\n- revisar PR do webhook\n- subir migração",
        );
        assert!(entry.contains("A: Três pendências na tela."));
        assert!(entry.contains("revisar PR do webhook"));
        assert!(entry.contains("subir migração"));
    }

    #[test]
    fn journal_clips_a_runaway_body() {
        let body = "x".repeat(5000);
        let entry = journal_entry("2026-08-30T10:00:00Z", "q", "a", &body);
        assert!(entry.chars().count() < 1000);
        assert!(entry.contains('…'));
    }

    #[test]
    fn journal_survives_markdown_headers_inside_the_body() {
        let one = journal_entry("2026-08-30T10:00:00Z", "q1", "a1", "## Pendências\n- item um");
        let two = journal_entry("2026-08-30T11:00:00Z", "q2", "a2", "");
        let content = format!("{one}{two}");
        // A header in the body must not masquerade as a third entry.
        assert_eq!(journal_tail(&content, 10).len(), 2);
        let turns = parse_journal(&content, 10);
        assert_eq!(turns.len(), 2);
        assert!(turns[0].body.contains("item um"));
    }

    #[test]
    fn journal_turns_parse_back_question_answer_and_body() {
        let one = journal_entry(
            "2026-08-30T10:00:00Z",
            "quais as demandas da semana?",
            "Oito frentes ativas.",
            "Assinaturas migrado.\n- pendência: DNS antigo",
        );
        let two = journal_entry("2026-08-30T11:00:00Z", "e o planejamento?", "Três itens.", "");
        let turns = parse_journal(&format!("{one}{two}"), 10);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].ts, "2026-08-30T10:00:00Z");
        assert_eq!(turns[0].question, "quais as demandas da semana?");
        assert_eq!(turns[0].fala, "Oito frentes ativas.");
        assert!(turns[0].body.contains("DNS antigo"));
        assert_eq!(turns[1].question, "e o planejamento?");
        assert_eq!(turns[1].body, "");
    }

    #[test]
    fn parse_journal_keeps_the_newest_n_and_skips_junk() {
        let mut content = String::from("rabisco solto sem shape\n\n");
        for i in 0..6 {
            content.push_str(&journal_entry(
                &format!("2026-08-30T10:0{i}:00Z"),
                &format!("q{i}"),
                "ok",
                "",
            ));
        }
        let turns = parse_journal(&content, 3);
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[0].question, "q3");
        assert_eq!(turns[2].question, "q5");
    }

    #[test]
    fn multiline_question_and_fala_flatten_to_one_line_each() {
        let entry = journal_entry("2026-08-30T10:00:00Z", "linha um\nlinha dois", "fala\nquebrada", "");
        let turns = parse_journal(&entry, 5);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].question, "linha um linha dois");
        assert_eq!(turns[0].fala, "fala quebrada");
    }

    #[test]
    fn worker_record_serializes_with_snake_case_status() {
        let record = WorkerRecord {
            agent: "claude".into(),
            task_id: "t1".into(),
            context: "alpha".into(),
            workspace: "/home/dev/alpha".into(),
            session_id: "s1".into(),
            status: WorkerStatus::Running,
            started_at: "2026-08-14T10:00:00Z".into(),
            summary: "open PR".into(),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains(r#""status":"running""#));
        let back: WorkerRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }
}
