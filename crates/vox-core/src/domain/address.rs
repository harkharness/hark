//! Spoken addressing: "na task X, faz Y" / "no projeto Z, roda W".
//! Voice is GLOBAL — the address says where the work lands; the rest of
//! the sentence is the work itself.

/// Where a spoken instruction points, plus the instruction with the
/// address stripped out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// Task named in the sentence ("na task webhook…").
    pub task: Option<String>,
    /// Project named in the sentence ("no projeto vox…").
    pub project: Option<String>,
    /// The sentence minus the address words.
    pub instruction: String,
}

/// Markers that OPEN a task address. The task name runs until a comma,
/// a verb-ish continuation, or the end of the sentence.
const TASK_MARKERS: &[&str] = &[
    "na task ", "pra task ", "para a task ", "na tarefa ", "na sessão de ",
    "na sessao de ", "no chat de ", "no chat da ", "no chat do ",
];
const PROJECT_MARKERS: &[&str] = &["no projeto ", "do projeto ", "no repositório ", "no repositorio "];

fn find_marker(lower: &str, markers: &[&str]) -> Option<(usize, usize)> {
    markers
        .iter()
        .filter_map(|m| lower.find(m).map(|i| (i, i + m.len())))
        .min_by_key(|(i, _)| *i)
}

/// Extract the addressed span: from `start` until a comma or the end.
/// Returns (name, rest_of_sentence_without_the_span).
fn split_span(text: &str, marker: (usize, usize)) -> (String, String) {
    let (mstart, nstart) = marker;
    let tail = &text[nstart..];
    let (name, after) = match tail.find(',') {
        Some(i) => (&tail[..i], &tail[i + 1..]),
        None => (tail, ""),
    };
    let before = &text[..mstart];
    let rest = format!("{} {}", before.trim(), after.trim());
    (name.trim().to_string(), rest.trim().to_string())
}

/// Parse the spoken address of an utterance. No address = both None and
/// the sentence untouched (the caller falls back to the active context).
pub fn parse(utterance: &str) -> Address {
    let lower = utterance.to_lowercase();
    let mut task = None;
    let mut project = None;
    let mut text = utterance.to_string();

    if let Some(marker) = find_marker(&lower, TASK_MARKERS) {
        let (name, rest) = split_span(&text, marker);
        if !name.is_empty() {
            task = Some(name);
            text = rest;
        }
    }
    let lower = text.to_lowercase();
    if let Some(marker) = find_marker(&lower, PROJECT_MARKERS) {
        let (name, rest) = split_span(&text, marker);
        // A project name is one or two words; a long span means the
        // marker swallowed the instruction ("no projeto vox roda os
        // testes" has no comma) — keep only the first word as the name.
        let mut words = name.split_whitespace();
        let head: Vec<&str> = words.by_ref().take(1).collect();
        let spill: Vec<&str> = words.collect();
        if !head.is_empty() {
            project = Some(head.join(" "));
            let mut rest_full = rest;
            if !spill.is_empty() {
                rest_full = format!("{} {}", spill.join(" "), rest_full)
                    .trim()
                    .to_string();
            }
            text = rest_full;
        }
    }

    Address {
        task,
        project,
        instruction: text.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_a_task_with_comma() {
        let a = parse("na task webhook, corrige o teste e roda de novo");
        assert_eq!(a.task.as_deref(), Some("webhook"));
        assert_eq!(a.project, None);
        assert_eq!(a.instruction, "corrige o teste e roda de novo");
    }

    #[test]
    fn addresses_a_task_at_the_end() {
        let a = parse("corrige o teste na task webhook");
        assert_eq!(a.task.as_deref(), Some("webhook"));
        assert_eq!(a.instruction, "corrige o teste");
    }

    #[test]
    fn addresses_a_project_keeping_the_instruction() {
        let a = parse("no projeto vox roda os testes");
        assert_eq!(a.project.as_deref(), Some("vox"));
        assert_eq!(a.instruction, "roda os testes");
    }

    #[test]
    fn addresses_task_and_project_together() {
        let a = parse("no projeto vox, na task terminal, adiciona scrollback");
        assert_eq!(a.task.as_deref(), Some("terminal"));
        assert_eq!(a.project.as_deref(), Some("vox"));
        assert_eq!(a.instruction, "adiciona scrollback");
    }

    #[test]
    fn no_address_leaves_the_sentence_alone() {
        let a = parse("corrige o teste do webhook e roda de novo");
        assert_eq!(a.task, None);
        assert_eq!(a.project, None);
        assert_eq!(a.instruction, "corrige o teste do webhook e roda de novo");
    }

    #[test]
    fn multiword_task_names_survive() {
        let a = parse("na task migração dos alertas, valida a query no new relic");
        assert_eq!(a.task.as_deref(), Some("migração dos alertas"));
        assert_eq!(a.instruction, "valida a query no new relic");
    }
}
