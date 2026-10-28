//! How a page gets its ties: the tool call that wrote it, and the texts
//! that mention it. Pure; the shell brings the artifacts root and the
//! tree, and gets back ids and paths.

use super::artifacts::{validate_path, Tree};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::artifacts::{Kind, Node};

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
}
