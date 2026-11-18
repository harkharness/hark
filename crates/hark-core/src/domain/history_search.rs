//! Full-text search over what was SAID in the sessions (fase 16).
//!
//! The index kept the prompts a person typed and each session's last
//! reply, clipped: enough to list sessions, not enough to answer "o que
//! ficou pendente" — pending was inferred from what was asked, never from
//! what was concluded. This module is the pure half of a real search:
//! which lines of a session log are prose worth indexing, how a person's
//! words become a query, and how hits read back on a terminal and in a
//! prompt. The store (`SessionStore::search_messages`) and the file
//! reader (`adapters::message_index`) are the other half.

use crate::domain::transcript::{parse_entry, Role};
use serde::{Deserialize, Serialize};

/// One message's prose is clipped to this before it is indexed: a pasted
/// log is not conversation, and the index stays small.
pub const MAX_MESSAGE_CHARS: usize = 4000;

/// A message as the index keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedMessage {
    pub ts: String,
    /// "user" or "assistant".
    pub role: &'static str,
    pub text: String,
}

/// A place in a session where the words were found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageHit {
    pub session_id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub ts: String,
    pub role: String,
    /// The words in their sentence, the rest cut with an ellipsis.
    pub snippet: String,
}

/// A line that cannot be conversation, told WITHOUT parsing it: a tool
/// result is megabytes of JSON, and parsing every one of them is what
/// would make indexing a gigabyte of logs take minutes. A person's own
/// words never carry an unescaped `"tool_result"`: inside prose the
/// quotes are escaped.
pub fn skippable(line: &str) -> bool {
    line.len() < 2 || line.contains("\"tool_result\"")
}

/// The prose of one log line, if it is someone talking: the person's own
/// message or the agent's text. Tool calls, tool results, injected
/// payloads, sidechains and compaction summaries are not conversation.
pub fn prose_of(line: &str) -> Option<IndexedMessage> {
    if skippable(line) {
        return None;
    }
    let entry = parse_entry(line)?;
    let role = match entry.role {
        Role::User => "user",
        Role::Assistant => "assistant",
        _ => return None,
    };
    let text = entry.text.trim();
    if text.is_empty() {
        return None;
    }
    let text = if text.chars().count() > MAX_MESSAGE_CHARS {
        text.chars().take(MAX_MESSAGE_CHARS).collect()
    } else {
        text.to_string()
    };
    Some(IndexedMessage { ts: entry.ts, role, text })
}

/// A person's words as an FTS5 query. Every term is quoted, so nothing
/// typed is read as query syntax (a `-`, a `:`, an `OR`); `all` asks for
/// every term in the same message, else any of them.
pub fn fts_query(terms: &[String], all: bool) -> String {
    let quoted: Vec<String> = terms
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    quoted.join(if all { " " } else { " OR " })
}

/// A QUESTION's words as a query: any of them, the longer ones by their
/// root (the word less its last letters, as a prefix), so "pendente"
/// finds "pendências" and "semana" finds "semanal". No stemmer, two
/// languages: a prefix is the honest approximation.
pub fn fts_query_loose(terms: &[String]) -> String {
    terms
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let chars: Vec<char> = t.chars().collect();
            if chars.len() >= 6 {
                let root: String = chars[..chars.len() - 2].iter().collect();
                format!("\"{}\"*", root.replace('"', "\"\""))
            } else {
                format!("\"{}\"", t.replace('"', "\"\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Split what was typed on the command line into terms: words, with a
/// "quoted phrase" kept whole.
pub fn terms_of(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => {
                if quoted && !current.trim().is_empty() {
                    terms.push(current.trim().to_string());
                    current.clear();
                }
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    terms.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        terms.push(current.trim().to_string());
    }
    terms
}

/// The folder a session ran in, as a person names it.
fn project_of(cwd: Option<&str>) -> &str {
    cwd.and_then(|c| c.trim_end_matches('/').rsplit('/').next()).unwrap_or("-")
}

/// Hits as a terminal reads them: one block per session (newest hit
/// first as they came), its lines under it. The id is cut to what
/// `hark show` needs.
pub fn render_hits(hits: &[MessageHit]) -> String {
    let mut out = String::new();
    let mut seen: Vec<&str> = Vec::new();
    for hit in hits {
        if seen.contains(&hit.session_id.as_str()) {
            continue;
        }
        seen.push(&hit.session_id);
        let short: String = hit.session_id.chars().take(8).collect();
        out.push_str(&format!(
            "{}  {} · {}\n",
            short,
            hit.title.as_deref().unwrap_or("(sem título)"),
            project_of(hit.cwd.as_deref()),
        ));
        // Within a session the newest line first: the last word on a
        // subject is the one that counts.
        let mut lines: Vec<&MessageHit> = hits.iter().filter(|h| h.session_id == hit.session_id).collect();
        lines.sort_by(|a, b| b.ts.cmp(&a.ts));
        for line in lines {
            let day: String = line.ts.chars().take(10).collect();
            let who = if line.role == "user" { "você" } else { "agente" };
            out.push_str(&format!("  {day} {who}: {}\n", flatten(&line.snippet)));
        }
        out.push('\n');
    }
    out
}

/// Hits as a prompt section: what was actually said, with where and when,
/// inside a character budget. Empty when nothing was found.
pub fn excerpts_section(hits: &[MessageHit], max_chars: usize) -> String {
    let mut out = String::new();
    for hit in hits {
        let day: String = hit.ts.chars().take(10).collect();
        let line = format!(
            "- [{} · {} · {}] {}: {}\n",
            hit.title.as_deref().unwrap_or("sem título"),
            project_of(hit.cwd.as_deref()),
            day,
            if hit.role == "user" { "usuário" } else { "agente" },
            flatten(&hit.snippet),
        );
        if out.chars().count() + line.chars().count() > max_chars {
            break;
        }
        out.push_str(&line);
    }
    out
}

/// A session read back on a terminal: who said what, in order, the
/// newest `tail` messages. Tool calls and their output are left out, as
/// they are from the index: this is the conversation, not the work log.
pub fn render_transcript<'a>(lines: impl Iterator<Item = &'a str>, tail: usize) -> String {
    let said: Vec<IndexedMessage> = lines.filter_map(prose_of).collect();
    let skip = said.len().saturating_sub(tail);
    let mut out = String::new();
    for m in said.into_iter().skip(skip) {
        let when: String = m.ts.chars().take(16).collect::<String>().replace('T', " ");
        let who = if m.role == "user" { "você" } else { "agente" };
        out.push_str(&format!("── {who} · {when}\n{}\n\n", m.text.trim()));
    }
    out
}

fn flatten(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = r#"{"type":"user","timestamp":"2026-11-17T10:00:00Z","message":{"role":"user","content":"migra o logstash pro cluster novo"}}"#;
    const ASSISTANT: &str = r#"{"type":"assistant","timestamp":"2026-11-17T10:01:00Z","message":{"role":"assistant","content":[{"type":"text","text":"Migração concluída: o LogStash roda no cluster novo."}]}}"#;
    const TOOL_USE: &str = r#"{"type":"assistant","timestamp":"2026-11-17T10:00:30Z","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{"command":"kubectl get pods"}}]}}"#;
    const TOOL_RESULT: &str = r#"{"type":"user","timestamp":"2026-11-17T10:00:40Z","message":{"role":"user","content":[{"type":"tool_result","content":"pod/logstash-0 Running"}]}}"#;
    const INJECTED: &str = r#"{"type":"user","timestamp":"2026-11-17T10:00:50Z","message":{"role":"user","content":"<system-reminder>não é conversa</system-reminder>"}}"#;

    #[test]
    fn what_was_said_is_prose_and_the_machinery_is_not() {
        let said = prose_of(USER).unwrap();
        assert_eq!((said.role, said.text.as_str()), ("user", "migra o logstash pro cluster novo"));
        assert_eq!(prose_of(ASSISTANT).unwrap().role, "assistant");
        assert!(prose_of(TOOL_USE).is_none());
        assert!(prose_of(TOOL_RESULT).is_none());
        assert!(prose_of(INJECTED).is_none());
        assert!(prose_of("not json").is_none());
    }

    #[test]
    fn a_tool_result_is_skipped_before_it_is_parsed() {
        assert!(skippable(TOOL_RESULT));
        // A person QUOTING the word is still talking: the quotes are escaped.
        let quoting = r#"{"type":"user","timestamp":"t","message":{"content":"o que é \"tool_result\"?"}}"#;
        assert!(!skippable(quoting));
        assert!(prose_of(quoting).is_some());
    }

    #[test]
    fn a_wall_of_text_is_clipped_before_it_is_indexed() {
        let wall = "x".repeat(MAX_MESSAGE_CHARS * 2);
        let line = format!(r#"{{"type":"user","timestamp":"t","message":{{"content":"{wall}"}}}}"#);
        assert_eq!(prose_of(&line).unwrap().text.chars().count(), MAX_MESSAGE_CHARS);
    }

    #[test]
    fn typed_words_become_a_query_that_cannot_be_read_as_syntax() {
        let terms = vec!["fila".to_string(), "antiga".to_string()];
        assert_eq!(fts_query(&terms, true), "\"fila\" \"antiga\"");
        assert_eq!(fts_query(&terms, false), "\"fila\" OR \"antiga\"");
        assert_eq!(fts_query(&["a\"b".to_string(), " ".to_string()], true), "\"a\"\"b\"");
        assert_eq!(fts_query(&[], true), "");
    }

    /// "pendente" must find "pendências": a question is asked in one
    /// form and answered in another, and there is no stemmer here.
    #[test]
    fn a_question_searches_by_the_root_of_its_longer_words() {
        let terms = vec!["pendente".to_string(), "log".to_string(), "semana".to_string()];
        assert_eq!(fts_query_loose(&terms), "\"penden\"* OR \"log\" OR \"sema\"*");
        assert_eq!(fts_query_loose(&[]), "");
    }

    #[test]
    fn the_command_line_splits_on_spaces_and_keeps_a_quoted_phrase() {
        assert_eq!(terms_of("fila antiga"), vec!["fila", "antiga"]);
        assert_eq!(terms_of("\"fila antiga\" retry"), vec!["fila antiga", "retry"]);
        assert!(terms_of("   ").is_empty());
    }

    fn hit(session: &str, title: &str, role: &str, snippet: &str) -> MessageHit {
        MessageHit {
            session_id: session.into(),
            title: Some(title.into()),
            cwd: Some("/Users/x/Projects/infra".into()),
            ts: "2026-11-17T10:01:00Z".into(),
            role: role.into(),
            snippet: snippet.into(),
        }
    }

    #[test]
    fn hits_read_on_a_terminal_one_block_per_session() {
        let out = render_hits(&[
            hit("11111111-aaaa", "Migração LogStash", "user", "migra o logstash"),
            hit("22222222-bbbb", "OnCall SRE", "assistant", "alerta do logstash"),
            hit("11111111-aaaa", "Migração LogStash", "assistant", "LogStash no cluster novo"),
        ]);
        assert_eq!(out.matches("Migração LogStash · infra").count(), 1);
        assert!(out.starts_with("11111111  Migração LogStash · infra\n  2026-11-17 você: migra o logstash\n  2026-11-17 agente: LogStash no cluster novo\n"));
        assert!(out.contains("22222222  OnCall SRE · infra"));
    }

    #[test]
    fn a_session_reads_back_as_who_said_what_newest_last() {
        let log = [USER, TOOL_USE, TOOL_RESULT, ASSISTANT];
        let all = render_transcript(log.iter().copied(), 40);
        assert_eq!(
            all,
            "── você · 2026-11-17 10:00\nmigra o logstash pro cluster novo\n\n── agente · 2026-11-17 10:01\nMigração concluída: o LogStash roda no cluster novo.\n\n"
        );
        let last = render_transcript(log.iter().copied(), 1);
        assert!(last.starts_with("── agente") && !last.contains("você"));
    }

    #[test]
    fn hits_fit_a_prompt_within_its_budget() {
        let hits = vec![
            hit("a", "Um", "assistant", "primeiro trecho"),
            hit("b", "Dois", "user", "segundo trecho"),
        ];
        let all = excerpts_section(&hits, 1000);
        assert!(all.contains("[Um · infra · 2026-11-17] agente: primeiro trecho"));
        assert!(all.contains("usuário: segundo trecho"));
        let one = excerpts_section(&hits, 60);
        assert!(one.contains("primeiro") && !one.contains("segundo"));
        assert_eq!(excerpts_section(&[], 1000), "");
    }
}
