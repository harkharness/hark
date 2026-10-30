//! How a page gets its ties: the tool call that wrote it, and the texts
//! that mention it. Pure; the shell brings the artifacts root and the
//! tree, and gets back ids and paths.

use super::artifacts::{validate_path, Node, Tree};

/// A page an agent just wrote: which space, which file inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub space: String,
    pub path: String,
}

const WRITING_TOOLS: [&str; 3] = ["Write", "Edit", "MultiEdit"];

/// The page a writing tool call touched, when its path sits inside a
/// space of the artifacts root: `<root>/<space>/<page>`. Anything else
/// (a read, a file elsewhere, a non-page, a path that climbs) is None.
pub fn written_page(tool: &str, input: &str, root: &str) -> Option<Written> {
    if !WRITING_TOOLS.contains(&tool) {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(input).ok()?;
    let file = parsed.get("file_path")?.as_str()?;
    let root = root.trim_end_matches('/');
    let rel = file.strip_prefix(root)?.strip_prefix('/')?;
    let (space, path) = rel.split_once('/')?;
    if space.is_empty() || space == "." || space == ".." {
        return None;
    }
    validate_path(path).ok()?;
    Some(Written { space: space.to_string(), path: path.to_string() })
}

fn words(text: &str) -> Vec<String> {
    super::artifacts::slug(text).split('-').filter(|w| !w.is_empty() && *w != "pagina").map(str::to_string).collect()
}

/// The pages whose title the text names, whole words, accents and case
/// aside, each once, in the tree's order.
pub fn cites(text: &str, tree: &Tree) -> Vec<String> {
    let hay = words(text);
    tree.nodes
        .iter()
        .filter(|n| {
            let needle = words(&n.title);
            !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle.as_slice())
        })
        .map(|n| n.id.clone())
        .collect()
}

/// The pages a task owns: the ones it wrote (origin) or is tied to (a
/// link naming the task), in the tree's order.
pub fn linked_to_task<'a>(tree: &'a Tree, task: &str) -> Vec<&'a Node> {
    tree.nodes
        .iter()
        .filter(|n| {
            n.origin.as_ref().and_then(|o| o.task.as_deref()) == Some(task)
                || n.links.iter().any(|l| l.task.as_deref() == Some(task))
        })
        .collect()
}

/// The lines a brief carries about a task's pages: title, place, path.
/// Titles and breadcrumbs only — the body rides only when the page is the
/// scope (`scope::page_brief`). Empty when there is nothing to say.
pub fn pages_block(pages: &[(&Node, Vec<&str>)], space: &str) -> String {
    if pages.is_empty() {
        return String::new();
    }
    let mut out = format!("Páginas desta task no espaço «{space}» (HARK_ARTIFACTS_DIR/{space}):\n");
    for (node, crumbs) in pages {
        out.push_str(&format!("- «{}» ({}) — {}\n", node.title, crumbs.join(" › "), node.path));
    }
    out
}

/// The pages a text names by link (`hark://artifacts/<space>/<id>`), in
/// order, each once. A chat link is not a page.
pub fn cited_links(text: &str) -> Vec<(String, String)> {
    use super::deep_link::{parse, Target};
    let mut out: Vec<(String, String)> = vec![];
    for token in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ')' || c == '(') {
        if let Some(Target::Page { space, id }) = parse(token.trim_end_matches(['.', '!', '?'])) {
            if !out.iter().any(|(s, i)| *s == space && *i == id) {
                out.push((space, id));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::artifacts::{Kind, Link, Node, Origin};

    const ROOT: &str = "/Users/x/.hark/artifacts";

    #[test]
    fn a_write_inside_the_artifacts_root_names_its_space_and_page() {
        let input = r##"{"file_path":"/Users/x/.hark/artifacts/hark/decisoes/adr-004.md","content":"# ADR"}"##;
        assert_eq!(
            written_page("Write", input, ROOT),
            Some(Written { space: "hark".into(), path: "decisoes/adr-004.md".into() })
        );
        let edit = r#"{"file_path":"/Users/x/.hark/artifacts/pessoal/notas.md","old_string":"a","new_string":"b"}"#;
        assert_eq!(written_page("Edit", edit, ROOT).map(|w| w.space), Some("pessoal".into()));
        assert_eq!(written_page("MultiEdit", edit, ROOT).map(|w| w.path), Some("notas.md".into()));
    }

    #[test]
    fn anything_else_is_not_a_page() {
        let elsewhere = r#"{"file_path":"/Users/x/Projects/hark/src/main.rs","content":""}"#;
        assert_eq!(written_page("Write", elsewhere, ROOT), None);
        let read = r#"{"file_path":"/Users/x/.hark/artifacts/hark/decisoes/adr-004.md"}"#;
        assert_eq!(written_page("Read", read, ROOT), None);
        let not_a_page = r#"{"file_path":"/Users/x/.hark/artifacts/hark/img/a.png","content":""}"#;
        assert_eq!(written_page("Write", not_a_page, ROOT), None);
        let at_root = r#"{"file_path":"/Users/x/.hark/artifacts/solto.md","content":""}"#;
        assert_eq!(written_page("Write", at_root, ROOT), None, "a page lives in a space");
        let climbing = r#"{"file_path":"/Users/x/.hark/artifacts/hark/../../../etc/x.md","content":""}"#;
        assert_eq!(written_page("Write", climbing, ROOT), None);
        assert_eq!(written_page("Write", "{not json", ROOT), None);
        let sibling = r#"{"file_path":"/Users/x/.hark/artifacts-old/hark/a.md","content":""}"#;
        assert_eq!(written_page("Write", sibling, ROOT), None, "a sibling with the same prefix is outside");
    }

    #[test]
    fn a_root_with_a_trailing_slash_or_a_tilde_is_the_same_root() {
        let input = r#"{"file_path":"/Users/x/.hark/artifacts/hark/a.md","content":""}"#;
        assert!(written_page("Write", input, "/Users/x/.hark/artifacts/").is_some());
    }

    fn tree() -> Tree {
        let mut t = Tree::default();
        for (id, title) in [("adr-004", "ADR 004: janela única"), ("retry", "Retry"), ("visao", "Visão geral")] {
            t.insert(
                Node { id: id.into(), title: title.into(), path: format!("{id}.md"), parent: None, kind: Kind::Doc, origin: None, links: vec![], updated_at: String::new(), orphan: false },
                None,
            )
            .unwrap();
        }
        t
    }

    #[test]
    fn a_text_cites_the_pages_whose_title_it_names() {
        let t = tree();
        assert_eq!(cites("aplica o que ficou no ADR 004: janela única e segue", &t), vec!["adr-004"]);
        assert_eq!(cites("olha a visão geral antes", &t), vec!["visao"], "accents and case do not matter");
        assert_eq!(cites("sem nada aqui", &t), Vec::<String>::new());
    }

    #[test]
    fn a_short_title_needs_its_whole_word_and_a_title_is_cited_once() {
        let t = tree();
        assert_eq!(cites("o retry queue fix falhou", &t), vec!["retry"]);
        assert_eq!(cites("retrying is not it", &t), Vec::<String>::new());
        assert_eq!(cites("Retry, e de novo o retry", &t), vec!["retry"]);
    }

    #[test]
    fn the_pages_of_a_task_are_the_ones_it_wrote_or_tied() {
        let mut t = Tree::default();
        let page = |id: &str, origin: Option<Origin>, links: Vec<Link>| Node { id: id.into(), title: id.into(), path: format!("{id}.md"), parent: None, kind: Kind::Doc, origin, links, updated_at: String::new(), orphan: false };
        t.insert(page("adr-004", Some(Origin { session_id: Some("s1".into()), task: Some("Billing".into()) }), vec![]), None).unwrap();
        t.insert(page("retry", None, vec![]), None).unwrap();
        t.insert(page("visao", None, vec![Link { kind: "cita".into(), session_id: None, task: Some("Billing".into()), node: None }]), None).unwrap();
        let ids: Vec<&str> = linked_to_task(&t, "Billing").iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["adr-004", "visao"], "tree order, origin or link");
        assert!(linked_to_task(&t, "Retry queue").is_empty());
    }

    #[test]
    fn a_pages_block_names_each_page_with_its_place_and_path() {
        let t = tree();
        let adr = t.get("adr-004").unwrap();
        let block = pages_block(&[(adr, vec!["hark", "Decisões", "ADR 004: janela única"])], "hark");
        assert!(block.starts_with("Páginas desta task no espaço «hark»"), "{block}");
        assert!(block.contains("- «ADR 004: janela única» (hark › Decisões › ADR 004: janela única) — adr-004.md"), "{block}");
        assert_eq!(pages_block(&[], "hark"), "");
    }

    #[test]
    fn a_hark_link_in_a_text_cites_that_page_by_space_and_id() {
        let text = "atualiza hark://artifacts/hark/adr-004 e também hark://artifacts/meu%20projeto/home, por favor";
        assert_eq!(cited_links(text), vec![("hark".to_string(), "adr-004".to_string()), ("meu projeto".to_string(), "home".to_string())]);
        assert!(cited_links("hark://chat/abc e nada mais").is_empty());
    }
}
