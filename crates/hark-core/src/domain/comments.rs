//! Comments on a page (fase 14.3): the person's notes, anchored to a
//! cell, a note, a panel, a slide, a quoted passage or the page itself,
//! kept in `x.comments.json` beside the page. A comment that names
//! `@hark` is a request: it becomes a message to a chat, and the version
//! the agent writes after it is its answer. Pure: who sends and who
//! writes is the shell's.

use serde::{Deserialize, Serialize};

/// What a comment points at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    Page,
    Cell { tab: usize, row: usize, col: usize, #[serde(default, skip_serializing_if = "Option::is_none")] tab_name: Option<String> },
    Note { id: String },
    Panel { index: usize, #[serde(default, skip_serializing_if = "Option::is_none")] title: Option<String> },
    Slide { n: usize },
    Quote { text: String },
}

/// Where a request went: the chat, and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sent {
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub at: String,
    pub text: String,
    pub anchor: Anchor,
    #[serde(default)]
    pub resolved: bool,
    /// The request this comment became, when it named @hark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent: Option<Sent>,
    /// When the chat it went to wrote the page back: the answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Thread {
    #[serde(default)]
    pub comments: Vec<Comment>,
}

/// The file beside the page that holds its comments.
pub fn comments_path(page_path: &str) -> String {
    format!("{}.comments.json", crate::domain::artifacts::without_ext(page_path))
}

pub fn parse(json: &str) -> Thread {
    if json.trim().is_empty() {
        return Thread::default();
    }
    serde_json::from_str(json).unwrap_or_default()
}

pub fn to_json(thread: &Thread) -> String {
    let mut s = serde_json::to_string_pretty(thread).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

/// `@hark` in the text, as a word, any case: the comment is a request.
pub fn mentions_hark(text: &str) -> bool {
    let lower = text.to_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("@hark") {
        let end = from + i + 5;
        let next = lower[end..].chars().next();
        if next.is_none_or(|c| !c.is_alphanumeric() && c != '_') {
            return true;
        }
        from = end;
    }
    false
}

/// A new comment, newest last; blank text is refused.
pub fn add(thread: &mut Thread, text: &str, anchor: Anchor, now: &str) -> Result<Comment, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("um comentário tem texto".into());
    }
    let id = (1..).map(|i| format!("c{i}")).find(|id| !thread.comments.iter().any(|c| &c.id == id)).unwrap_or_else(|| "c0".into());
    let c = Comment { id, at: now.to_string(), text: text.to_string(), anchor, resolved: false, sent: None, answered_at: None };
    thread.comments.push(c.clone());
    Ok(c)
}

fn find<'a>(thread: &'a mut Thread, id: &str) -> Result<&'a mut Comment, String> {
    thread.comments.iter_mut().find(|c| c.id == id).ok_or_else(|| format!("comentário \"{id}\" não existe"))
}

pub fn resolve(thread: &mut Thread, id: &str, on: bool) -> Result<(), String> {
    find(thread, id)?.resolved = on;
    Ok(())
}

pub fn remove(thread: &mut Thread, id: &str) -> Result<(), String> {
    let before = thread.comments.len();
    thread.comments.retain(|c| c.id != id);
    if thread.comments.len() == before {
        return Err(format!("comentário \"{id}\" não existe"));
    }
    Ok(())
}

/// The comment went to a chat: remember where, so the chat's next write
/// of the page reads as the answer.
pub fn sent(thread: &mut Thread, id: &str, session_id: Option<&str>, task: Option<&str>, now: &str) -> Result<(), String> {
    let c = find(thread, id)?;
    c.sent = Some(Sent { at: now.to_string(), session_id: session_id.map(String::from), task: task.map(String::from) });
    c.answered_at = None;
    Ok(())
}

/// A chat wrote the page: every open request sent to that chat (by
/// session, or by task when the session is unknown) is answered. The ids
/// answered come back, so the window can say so.
pub fn answered(thread: &mut Thread, session_id: Option<&str>, task: Option<&str>, now: &str) -> Vec<String> {
    let mut out = Vec::new();
    for c in thread.comments.iter_mut().filter(|c| !c.resolved && c.answered_at.is_none()) {
        let Some(s) = &c.sent else { continue };
        let same_session = session_id.is_some() && s.session_id.as_deref() == session_id;
        let same_task = task.is_some() && s.task.as_deref() == task;
        if same_session || same_task {
            c.answered_at = Some(now.to_string());
            out.push(c.id.clone());
        }
    }
    out
}

/// The anchor as a person reads it.
pub fn anchor_label(anchor: &Anchor) -> String {
    match anchor {
        Anchor::Page => "a página inteira".into(),
        Anchor::Cell { tab, row, col, tab_name } => {
            let cell = format!("{}{}", crate::domain::sheet::column_name(*col), row + 2);
            match tab_name {
                Some(n) => format!("célula {cell} da aba \"{n}\""),
                None => format!("célula {cell} da aba {}", tab + 1),
            }
        }
        Anchor::Note { id } => format!("nota {id}"),
        Anchor::Panel { index, title } => match title {
            Some(t) => format!("painel \"{t}\""),
            None => format!("painel {}", index + 1),
        },
        Anchor::Slide { n } => format!("slide {}", n + 1),
        Anchor::Quote { text } => {
            let short: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
            format!("o trecho «{short}»")
        }
    }
}

/// The message a `@hark` comment becomes: the page by its link, the
/// anchor, the words without the mention, and the one instruction that
/// matters: change that page, in its file, and nothing else.
pub fn request_text(link: &str, title: &str, comment: &Comment) -> String {
    let words = comment.text.replace("@Hark", "").replace("@hark", "").replace("@HARK", "");
    let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "Na página {link} (\"{title}\"), em {}: {words}\n\nAjuste a página no arquivo dela, só o que esse comentário pede; o resto fica como está.",
        anchor_label(&comment.anchor)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-10T20:00:00Z";

    #[test]
    fn comments_live_beside_the_page_and_survive_the_file() {
        assert_eq!(comments_path("custos.sheet.json"), "custos.comments.json");
        assert_eq!(comments_path("decisions/adr-004.md"), "decisions/adr-004.comments.json");
        assert_eq!(comments_path("deck.html"), "deck.comments.json");
        let mut th = parse("");
        let c = add(&mut th, "  o total está errado  ", Anchor::Cell { tab: 0, row: 2, col: 3, tab_name: Some("Mensal".into()) }, NOW).unwrap();
        assert_eq!(c.id, "c1");
        assert_eq!(c.text, "o total está errado");
        add(&mut th, "segunda", Anchor::Page, NOW).unwrap();
        assert!(add(&mut th, "   ", Anchor::Page, NOW).is_err());
        assert_eq!(parse(&to_json(&th)), th);
        assert_eq!(parse("not json").comments.len(), 0, "a broken file is an empty thread, never a crash");
        resolve(&mut th, "c1", true).unwrap();
        assert!(th.comments[0].resolved);
        remove(&mut th, "c2").unwrap();
        assert!(remove(&mut th, "c2").is_err());
        assert_eq!(th.comments.len(), 1);
    }

    #[test]
    fn a_mention_of_hark_is_a_word_in_any_case() {
        assert!(mentions_hark("@hark troca o título"));
        assert!(mentions_hark("troca o título, @Hark"));
        assert!(mentions_hark("@HARK!"));
        assert!(!mentions_hark("mande para @harkness"));
        assert!(!mentions_hark("sem menção"));
        assert!(!mentions_hark("email hark@example.com"));
    }

    #[test]
    fn a_request_names_the_page_the_anchor_and_the_words_without_the_mention() {
        let mut th = Thread::default();
        let c = add(&mut th, "@hark some com a linha do legado, ela já migrou", Anchor::Cell { tab: 0, row: 5, col: 0, tab_name: Some("Custo".into()) }, NOW).unwrap();
        let text = request_text("hark://artifacts/tulipa/custos", "Custos", &c);
        assert!(text.starts_with("Na página hark://artifacts/tulipa/custos (\"Custos\"), em célula A7 da aba \"Custo\": some com a linha do legado, ela já migrou"), "{text}");
        assert!(!text.contains("@hark"));
        assert!(text.contains("só o que esse comentário pede"));
        assert_eq!(anchor_label(&Anchor::Note { id: "n3".into() }), "nota n3");
        assert_eq!(anchor_label(&Anchor::Panel { index: 1, title: Some("por dia".into()) }), "painel \"por dia\"");
        assert_eq!(anchor_label(&Anchor::Slide { n: 2 }), "slide 3");
        assert_eq!(anchor_label(&Anchor::Quote { text: "  um   trecho\ncitado ".into() }), "o trecho «um trecho citado»");
        assert_eq!(anchor_label(&Anchor::Page), "a página inteira");
    }

    #[test]
    fn the_chat_the_request_went_to_answers_it_by_writing_the_page() {
        let mut th = Thread::default();
        let a = add(&mut th, "@hark x", Anchor::Page, NOW).unwrap();
        let b = add(&mut th, "@hark y", Anchor::Page, NOW).unwrap();
        let _note = add(&mut th, "só uma anotação", Anchor::Page, NOW).unwrap();
        sent(&mut th, &a.id, Some("s-1"), Some("Billing"), NOW).unwrap();
        sent(&mut th, &b.id, None, Some("Outra"), NOW).unwrap();
        // Another chat writing the page answers nothing.
        assert!(answered(&mut th, Some("s-9"), Some("Nada"), "2026-11-10T20:05:00Z").is_empty());
        // The chat by session; the other by task when its session was unknown.
        assert_eq!(answered(&mut th, Some("s-1"), Some("Billing"), "2026-11-10T20:06:00Z"), vec!["c1"]);
        assert_eq!(th.comments[0].answered_at.as_deref(), Some("2026-11-10T20:06:00Z"));
        assert_eq!(answered(&mut th, None, Some("Outra"), "2026-11-10T20:07:00Z"), vec!["c2"]);
        // Answered once; a later write of the same chat is not a second answer.
        assert!(answered(&mut th, Some("s-1"), None, "2026-11-10T20:08:00Z").is_empty());
        // Sent again: the slate is clean for the next answer.
        sent(&mut th, &a.id, Some("s-1"), None, "2026-11-10T20:09:00Z").unwrap();
        assert!(th.comments[0].answered_at.is_none());
        assert!(sent(&mut th, "zz", None, None, NOW).is_err());
    }
}
