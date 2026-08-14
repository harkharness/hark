//! Pure pieces of Vox's file-based memory: the Q&A journal and the
//! machine-wide worker registry entries.

use serde::{Deserialize, Serialize};

/// One dispatched worker, as recorded in the global state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub task_id: String,
    pub context: String,
    pub workspace: String,
    pub session_id: String,
    pub status: WorkerStatus,
    pub started_at: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Running,
    Done,
    Failed,
}

/// Render one journal entry block (append-only markdown).
pub fn journal_entry(ts: &str, question: &str, fala: &str) -> String {
    format!("## {ts}\nQ: {question}\nA: {fala}\n\n")
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

    #[test]
    fn journal_roundtrip_keeps_last_n_entries() {
        let content: String = (0..7)
            .map(|i| journal_entry(&format!("2026-08-14T10:0{i}:00Z"), &format!("q{i}"), "ok"))
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
    fn worker_record_serializes_with_snake_case_status() {
        let record = WorkerRecord {
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
