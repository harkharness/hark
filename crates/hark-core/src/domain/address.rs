//! Spoken addressing: "na task X, faz Y" / "no projeto Z, roda W".
//! Voice is GLOBAL — the address says where the work lands; the rest of
//! the sentence is the work itself.

/// Where a spoken instruction points, plus the instruction with the
/// address stripped out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// Task named in the sentence ("na task webhook…").
    pub task: Option<String>,
    /// Project named in the sentence ("no projeto hark…").
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

/// Extract the addressed span: from `start` until a comma, the first
/// action verb, or the end — spoken sentences rarely carry commas, so
/// "na task assinaturas core roda os testes" must not swallow the work.
/// Returns (name, rest_of_sentence_without_the_span).
fn split_span(text: &str, marker: (usize, usize)) -> (String, String) {
    let (mstart, nstart) = marker;
    let tail = &text[nstart..];
    let (name, after) = match tail.find(',') {
        Some(i) => (tail[..i].to_string(), tail[i + 1..].to_string()),
        None => {
            let mut name_words: Vec<&str> = Vec::new();
            let mut rest_words: Vec<&str> = Vec::new();
            for word in tail.split_whitespace() {
                let clean = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                let is_verb = crate::domain::intent::ACTION_VERBS.contains(&clean.as_str());
                // A verb can still OPEN the name ("task roda de conversa"
                // is rare but possible); it only splits once a name exists.
                if rest_words.is_empty() && (!is_verb || name_words.is_empty()) {
                    name_words.push(word);
                } else {
                    rest_words.push(word);
                }
            }
            (name_words.join(" "), rest_words.join(" "))
        }
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
        // The verb-stop in split_span keeps multi-word project names
        // whole ("workspace codigo") and hands the work back untouched.
        let (name, rest) = split_span(&text, marker);
        if !name.is_empty() {
            project = Some(name);
            text = rest;
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
        let a = parse("no projeto hark roda os testes");
        assert_eq!(a.project.as_deref(), Some("hark"));
        assert_eq!(a.instruction, "roda os testes");
    }

    #[test]
    fn addresses_task_and_project_together() {
        let a = parse("no projeto hark, na task terminal, adiciona scrollback");
        assert_eq!(a.task.as_deref(), Some("terminal"));
        assert_eq!(a.project.as_deref(), Some("hark"));
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

    #[test]
    fn task_name_stops_before_the_verb() {
        // Spoken sentences rarely carry commas: the first action verb
        // ends the name ("na task assinaturas core roda os testes").
        let a = parse("na task assinaturas core roda os testes");
        assert_eq!(a.task.as_deref(), Some("assinaturas core"));
        assert_eq!(a.instruction, "roda os testes");
    }

    #[test]
    fn comma_still_wins_over_verb() {
        // With a comma the whole span before it is the name, verbs included.
        let a = parse("na task roda de conversa, adiciona a pauta");
        assert_eq!(a.task.as_deref(), Some("roda de conversa"));
        assert_eq!(a.instruction, "adiciona a pauta");
    }

    #[test]
    fn trailing_task_address_unchanged() {
        let a = parse("corrige o teste na task assinaturas core");
        assert_eq!(a.task.as_deref(), Some("assinaturas core"));
        assert_eq!(a.instruction, "corrige o teste");
    }

    #[test]
    fn project_span_breaks_on_verb_too() {
        // Multi-word project names survive when a verb follows.
        let a = parse("no projeto workspace codigo roda os testes");
        assert_eq!(a.project.as_deref(), Some("workspace codigo"));
        assert_eq!(a.instruction, "roda os testes");
    }
}
