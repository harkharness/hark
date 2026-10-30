//! The artifact use cases: what the window and the reader loop ask for.
//! Each one reads the space's index, changes the tree, and writes the
//! index back; the disk is behind the `ArtifactStore` port.

use crate::domain::artifacts::{Kind, Node, Origin, Tree};
use crate::ports::ArtifactStore;

fn save(store: &dyn ArtifactStore, space: &str, tree: &Tree) -> anyhow::Result<()> {
    store.write_index(space, &tree.to_json())
}

/// The space's tree, in line with its files: a page the agent wrote
/// straight to the disk shows up at the root, a page whose file is gone
/// is flagged. Saved back only when the pass changed something.
pub fn load(store: &dyn ArtifactStore, space: &str, now: &str) -> anyhow::Result<Tree> {
    let mut tree = Tree::parse(&store.read_index(space)?).map_err(anyhow::Error::msg)?;
    let files = store.list_pages(space)?;
    // Only the files the index does not know get their head read: the
    // title of an adopted page comes from it. Two kilobytes is plenty.
    let heads: Vec<(String, Option<String>)> = files
        .iter()
        .map(|f| {
            let known = tree.nodes.iter().any(|n| &n.path == f);
            let title = (!known)
                .then(|| store.read_page(space, f).ok())
                .flatten()
                .map(|text| {
                    let cut = text.char_indices().map(|(i, _)| i).find(|&i| i >= 2048).unwrap_or(text.len());
                    crate::domain::artifacts::page_title(f, &text[..cut])
                });
            (f.clone(), title)
        })
        .collect();
    let refs: Vec<(&str, Option<&str>)> = heads.iter().map(|(f, t)| (f.as_str(), t.as_deref())).collect();
    let pass = tree.reconcile(&refs, now);
    if !pass.added.is_empty() || !pass.orphaned.is_empty() || !pass.renamed.is_empty() {
        save(store, space, &tree)?;
    }
    Ok(tree)
}

/// What a fresh page holds: a heading for a document, a shell for the
/// HTML kinds, so the viewer has something to show before anyone types.
fn seed(title: &str, kind: Kind) -> String {
    match kind {
        Kind::Doc => format!("# {title}\n\n"),
        Kind::Slides | Kind::Design => format!(
            "<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n</head>\n<body>\n<h1>{title}</h1>\n</body>\n</html>\n"
        ),
    }
}

/// A new page under a parent (None = the root), its file seeded, its
/// node in the index with the origin that asked for it.
pub fn create(
    store: &dyn ArtifactStore,
    space: &str,
    parent: Option<&str>,
    title: &str,
    kind: Kind,
    origin: Option<Origin>,
    now: &str,
) -> anyhow::Result<Node> {
    let mut tree = load(store, space, now)?;
    let id = tree.fresh_id(title);
    let path = tree.path_under(parent, &id, kind);
    let node = Node {
        id,
        title: title.trim().to_string(),
        path: path.clone(),
        parent: parent.map(str::to_string),
        kind,
        origin,
        links: vec![],
        updated_at: now.to_string(),
        orphan: false,
    };
    tree.insert(node.clone(), None).map_err(anyhow::Error::msg)?;
    store.write_page(space, &path, &seed(&node.title, kind))?;
    save(store, space, &tree)?;
    Ok(node)
}

/// The page's text replaced, its touch recorded.
pub fn write(store: &dyn ArtifactStore, space: &str, id: &str, text: &str, now: &str) -> anyhow::Result<()> {
    let mut tree = load(store, space, now)?;
    let path = tree.path_for(id).ok_or_else(|| anyhow::anyhow!("{id}: not found"))?.to_string();
    store.write_page(space, &path, text)?;
    if let Some(n) = tree.nodes.iter_mut().find(|n| n.id == id) {
        n.updated_at = now.to_string();
        n.orphan = false;
    }
    save(store, space, &tree)
}

pub fn move_to(store: &dyn ArtifactStore, space: &str, id: &str, parent: Option<&str>, at: usize) -> anyhow::Result<()> {
    let mut tree = load(store, space, "")?;
    tree.move_to(id, parent, at).map_err(anyhow::Error::msg)?;
    save(store, space, &tree)
}

pub fn rename(store: &dyn ArtifactStore, space: &str, id: &str, title: &str) -> anyhow::Result<()> {
    let mut tree = load(store, space, "")?;
    tree.rename(id, title).map_err(anyhow::Error::msg)?;
    save(store, space, &tree)
}

/// The page and everything under it, out of the index AND off the disk.
/// Returns the ids that went.
pub fn remove(store: &dyn ArtifactStore, space: &str, id: &str) -> anyhow::Result<Vec<String>> {
    let mut tree = load(store, space, "")?;
    let paths: Vec<(String, String)> = tree.nodes.iter().map(|n| (n.id.clone(), n.path.clone())).collect();
    let gone = tree.remove(id);
    for (_, path) in paths.iter().filter(|(gid, _)| gone.contains(gid)) {
        // A page already missing from the disk is not a failure to remove.
        let _ = store.remove_page(space, path);
    }
    save(store, space, &tree)?;
    Ok(gone)
}

/// An agent wrote a page: the page learns where it came from. The first
/// origin sticks (a page is born once); every write records the touch.
pub fn attach_origin(store: &dyn ArtifactStore, space: &str, path: &str, origin: Origin, now: &str) -> anyhow::Result<Node> {
    let mut tree = load(store, space, now)?;
    let node = tree
        .nodes
        .iter_mut()
        .find(|n| n.path == path)
        .ok_or_else(|| anyhow::anyhow!("{path}: not a page of {space}"))?;
    if node.origin.is_none() {
        node.origin = Some(origin);
    }
    node.updated_at = now.to_string();
    node.orphan = false;
    let out = node.clone();
    save(store, space, &tree)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::artifacts_fs::FsArtifacts;

    const NOW: &str = "2026-10-28T10:00:00Z";

    fn store() -> (tempfile::TempDir, FsArtifacts) {
        let dir = tempfile::tempdir().unwrap();
        let store = FsArtifacts::new(dir.path().join("artifacts"));
        (dir, store)
    }

    #[test]
    fn loading_a_space_adopts_files_the_index_does_not_know_and_saves_that() {
        let (_d, s) = store();
        s.write_page("hark", "notas/reuniao.md", "# Reunião").unwrap();
        let tree = load(&s, "hark", NOW).unwrap();
        assert_eq!(tree.children(None).len(), 1);
        assert_eq!(tree.get("reuniao").unwrap().path, "notas/reuniao.md");
        assert!(s.read_index("hark").unwrap().contains("\"reuniao\""), "the adoption is written back");
        let again = load(&s, "hark", NOW).unwrap();
        assert_eq!(again, tree);
    }

    #[test]
    fn creating_a_page_writes_a_seed_file_under_its_parent_and_records_its_origin() {
        let (_d, s) = store();
        let parent = create(&s, "hark", None, "Decisões", Kind::Doc, None, NOW).unwrap();
        assert_eq!(parent.path, "decisoes.md");
        let origin = Origin { session_id: Some("s-1".into()), task: Some("Billing".into()) };
        let child = create(&s, "hark", Some(&parent.id), "ADR 004: janela única", Kind::Doc, Some(origin.clone()), NOW).unwrap();
        assert_eq!(child.path, "decisoes/adr-004-janela-unica.md");
        assert_eq!(child.parent.as_deref(), Some("decisoes"));
        assert_eq!(child.origin, Some(origin));
        assert_eq!(s.read_page("hark", &child.path).unwrap(), "# ADR 004: janela única\n\n");
        let slides = create(&s, "hark", None, "Infra", Kind::Slides, None, NOW).unwrap();
        assert_eq!(slides.path, "infra.html");
        assert!(s.read_page("hark", "infra.html").unwrap().starts_with("<!doctype html>"));
        let tree = load(&s, "hark", NOW).unwrap();
        assert_eq!(tree.breadcrumb(&child.id), vec!["Decisões", "ADR 004: janela única"]);
    }

    #[test]
    fn writing_a_page_updates_its_text_and_its_timestamp() {
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Nota", Kind::Doc, None, NOW).unwrap();
        write(&s, "hark", &page.id, "# Nota\n\ncorpo\n", "2026-10-28T11:00:00Z").unwrap();
        assert_eq!(s.read_page("hark", "nota.md").unwrap(), "# Nota\n\ncorpo\n");
        let tree = load(&s, "hark", NOW).unwrap();
        assert_eq!(tree.get("nota").unwrap().updated_at, "2026-10-28T11:00:00Z");
        assert!(write(&s, "hark", "nope", "x", NOW).is_err());
    }

    #[test]
    fn moving_renaming_and_removing_go_through_the_index_and_the_disk() {
        let (_d, s) = store();
        let a = create(&s, "hark", None, "A", Kind::Doc, None, NOW).unwrap();
        let b = create(&s, "hark", None, "B", Kind::Doc, None, NOW).unwrap();
        let c = create(&s, "hark", Some(&b.id), "C", Kind::Doc, None, NOW).unwrap();
        move_to(&s, "hark", &a.id, Some(&b.id), 0).unwrap();
        rename(&s, "hark", &c.id, "C renamed").unwrap();
        let tree = load(&s, "hark", NOW).unwrap();
        let kids: Vec<&str> = tree.children(Some("b")).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(kids, vec!["a", "c"]);
        assert_eq!(tree.get("c").unwrap().title, "C renamed");
        let gone = remove(&s, "hark", &b.id).unwrap();
        assert_eq!(gone.len(), 3);
        assert!(s.list_pages("hark").unwrap().is_empty(), "the files went with the pages");
        assert!(load(&s, "hark", NOW).unwrap().children(None).is_empty());
    }

    #[test]
    fn an_agent_s_write_attaches_the_origin_to_the_page_it_touched() {
        let (_d, s) = store();
        // The agent wrote straight to the disk: the index has never seen it.
        s.write_page("hark", "decisoes/adr-005.md", "# ADR 005").unwrap();
        let origin = Origin { session_id: Some("s-9".into()), task: Some("Retry".into()) };
        let node = attach_origin(&s, "hark", "decisoes/adr-005.md", origin.clone(), NOW).unwrap();
        assert_eq!(node.origin, Some(origin.clone()));
        assert_eq!(node.title, "ADR 005", "the heading the agent wrote is the title");
        // A second write by another chat keeps the first origin: the page
        // was born once.
        let other = Origin { session_id: Some("s-2".into()), task: None };
        let again = attach_origin(&s, "hark", "decisoes/adr-005.md", other, "2026-10-28T12:00:00Z").unwrap();
        assert_eq!(again.origin, Some(origin));
        assert_eq!(again.updated_at, "2026-10-28T12:00:00Z", "but the touch is recorded");
    }
}
