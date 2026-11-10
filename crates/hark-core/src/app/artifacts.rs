//! The artifact use cases: what the window and the reader loop ask for.
//! Each one reads the space's index, changes the tree, and writes the
//! index back; the disk is behind the `ArtifactStore` port.

use crate::domain::artifacts::{Kind, Node, Origin, Tree};
use crate::domain::page_history;
use crate::ports::{ArtifactStore, PageVersion};

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
    // An html page just adopted or rewritten says what it is in its own
    // text (meta tag or shape); the reconcile only saw the file name.
    for id in pass.added.iter().chain(pass.renamed.iter()) {
        let Some(path) = tree.get(id).map(|n| n.path.clone()) else { continue };
        if !path.to_ascii_lowercase().ends_with(".html") {
            continue;
        }
        if let Ok(text) = store.read_page(space, &path) {
            let kind = crate::domain::artifacts::page_kind(&path, &text);
            let _ = tree.set_kind(id, kind);
        }
    }
    if !pass.added.is_empty() || !pass.dropped.is_empty() || !pass.renamed.is_empty() {
        save(store, space, &tree)?;
    }
    Ok(tree)
}

/// A deck's own pager out of its file; true when the file changed.
pub fn strip_pager(store: &dyn ArtifactStore, space: &str, id: &str) -> anyhow::Result<bool> {
    let tree = load(store, space, "")?;
    let path = tree.get(id).map(|n| n.path.clone()).ok_or_else(|| anyhow::anyhow!("{id}: not found"))?;
    let text = store.read_page(space, &path)?;
    match crate::domain::artifacts::strip_own_pager(&text) {
        Some(clean) => {
            store.write_page(space, &path, &clean)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// The person says what an html page is: document, deck or design. A
/// json kind is its file's and cannot be re-labelled.
pub fn set_kind(store: &dyn ArtifactStore, space: &str, id: &str, kind: Kind) -> anyhow::Result<()> {
    let mut tree = load(store, space, "")?;
    let was = tree.get(id).map(|n| n.kind).ok_or_else(|| anyhow::anyhow!("{id}: not found"))?;
    anyhow::ensure!(!was.fixed_by_extension() && !kind.fixed_by_extension(), "{id}: the file says what this page is");
    tree.set_kind(id, kind).map_err(anyhow::Error::msg)?;
    save(store, space, &tree)
}

/// What a fresh page holds: a heading for a document, a shell for the
/// HTML kinds, so the viewer has something to show before anyone types.
fn seed(title: &str, kind: Kind) -> String {
    let t = serde_json::Value::from(title);
    match kind {
        Kind::Doc => format!("# {title}\n\n"),
        Kind::Slides | Kind::Design => format!(
            "<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n</head>\n<body>\n<h1>{title}</h1>\n</body>\n</html>\n"
        ),
        Kind::Sheet => format!("{{\n  \"title\": {t},\n  \"tabs\": [\n    {{ \"name\": \"Aba 1\", \"columns\": [\"A\", \"B\"], \"rows\": [[\"\", \"\"]] }}\n  ]\n}}\n"),
        Kind::Board => format!("{{\n  \"title\": {t},\n  \"notes\": [],\n  \"groups\": [],\n  \"timeline\": []\n}}\n"),
        Kind::Data => format!("{{\n  \"title\": {t},\n  \"sources\": [],\n  \"queries\": [],\n  \"panels\": [],\n  \"refresh\": \"manual\"\n}}\n"),
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
    };
    tree.insert(node.clone(), None).map_err(anyhow::Error::msg)?;
    store.write_page(space, &path, &seed(&node.title, kind))?;
    save(store, space, &tree)?;
    Ok(node)
}

/// The page's text replaced by the person, its touch recorded, and the
/// text before it kept as a version.
pub fn write(store: &dyn ArtifactStore, space: &str, id: &str, text: &str, now: &str) -> anyhow::Result<()> {
    let mut tree = load(store, space, now)?;
    let path = tree.path_for(id).ok_or_else(|| anyhow::anyhow!("{id}: not found"))?.to_string();
    if let Ok(before) = store.read_page(space, &path) {
        if before != text {
            keep(store, space, &path, &before, now)?;
        }
    }
    store.write_page(space, &path, text)?;
    if let Some(n) = tree.nodes.iter_mut().find(|n| n.id == id) {
        n.updated_at = now.to_string();
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

fn path_of(store: &dyn ArtifactStore, space: &str, id: &str) -> anyhow::Result<String> {
    let tree = load(store, space, "")?;
    tree.path_for(id).map(str::to_string).ok_or_else(|| anyhow::anyhow!("{id}: not found"))
}

/// A text kept as a version of the page, the oldest beyond the limit
/// dropped (domain::page_history).
fn keep(store: &dyn ArtifactStore, space: &str, path: &str, text: &str, now: &str) -> anyhow::Result<()> {
    store.write_version(space, path, now, text)?;
    let instants: Vec<String> = store.list_versions(space, path)?.into_iter().map(|v| v.at).collect();
    for at in page_history::to_prune(&instants) {
        let _ = store.remove_version(space, path, &at);
    }
    Ok(())
}

/// The versions a page keeps, oldest first.
pub fn versions(store: &dyn ArtifactStore, space: &str, id: &str) -> anyhow::Result<Vec<PageVersion>> {
    let path = path_of(store, space, id)?;
    store.list_versions(space, &path)
}

/// The text the person last accepted: the newest version. A page seen
/// for the first time takes its current text as that baseline, so the
/// next agent write has something to be compared with.
pub fn baseline(store: &dyn ArtifactStore, space: &str, id: &str, now: &str) -> anyhow::Result<String> {
    let path = path_of(store, space, id)?;
    match store.list_versions(space, &path)?.last() {
        Some(v) => store.read_version(space, &path, &v.at),
        None => {
            let text = store.read_page(space, &path)?;
            keep(store, space, &path, &text, now)?;
            Ok(text)
        }
    }
}

/// The current text becomes the baseline: what the agent changed is
/// taken, and the marks clear.
pub fn accept(store: &dyn ArtifactStore, space: &str, id: &str, now: &str) -> anyhow::Result<()> {
    let path = path_of(store, space, id)?;
    let text = store.read_page(space, &path)?;
    keep(store, space, &path, &text, now)
}

/// The page goes back to a version. What is there now is kept first,
/// so going back is never a loss.
pub fn restore(store: &dyn ArtifactStore, space: &str, id: &str, at: &str, now: &str) -> anyhow::Result<()> {
    let path = path_of(store, space, id)?;
    let old = store.read_version(space, &path, at)?;
    let current = store.read_page(space, &path)?;
    if current != old {
        keep(store, space, &path, &current, now)?;
    }
    store.write_page(space, &path, &old)?;
    let mut tree = load(store, space, now)?;
    if let Some(n) = tree.nodes.iter_mut().find(|n| n.id == id) {
        n.updated_at = now.to_string();
    }
    save(store, space, &tree)
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
    fn a_new_sheet_board_or_dashboard_is_seeded_as_minimal_json_the_viewer_can_open() {
        let (_d, s) = store();
        let sheet = create(&s, "hark", None, "Custos de infra", Kind::Sheet, None, NOW).unwrap();
        assert_eq!(sheet.path, "custos-de-infra.sheet.json");
        let text = s.read_page("hark", &sheet.path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(v["title"], "Custos de infra");
        assert!(v["tabs"].is_array());
        let board = create(&s, "hark", None, "Plano", Kind::Board, None, NOW).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s.read_page("hark", &board.path).unwrap()).unwrap();
        assert!(v["notes"].is_array());
        let dash = create(&s, "hark", None, "Uso", Kind::Data, None, NOW).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s.read_page("hark", &dash.path).unwrap()).unwrap();
        assert!(v["sources"].is_array() && v["panels"].is_array());
        // Reloaded, the kinds come from the extension.
        let tree = load(&s, "hark", NOW).unwrap();
        assert_eq!(tree.get("plano").unwrap().kind, Kind::Board);
        // A json kind cannot be re-labelled by hand: the file says what it is.
        assert!(set_kind(&s, "hark", "plano", Kind::Doc).is_err());
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
    fn a_page_keeps_the_version_before_each_change_and_can_go_back() {
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Custos", Kind::Sheet, None, NOW).unwrap();
        // First sight: the page's content becomes its baseline.
        assert_eq!(versions(&s, "hark", &page.id).unwrap().len(), 0);
        let base = baseline(&s, "hark", &page.id, NOW).unwrap();
        assert_eq!(base, s.read_page("hark", &page.path).unwrap());
        assert_eq!(versions(&s, "hark", &page.id).unwrap().len(), 1);
        // The agent rewrote the file (straight to disk). The baseline is
        // still what you last accepted; accepting makes the new text the
        // baseline.
        s.write_page("hark", &page.path, "{\"tabs\":[]}\n").unwrap();
        assert_eq!(baseline(&s, "hark", &page.id, "2026-10-28T10:05:00Z").unwrap(), base, "a baseline is not retaken on its own");
        accept(&s, "hark", &page.id, "2026-10-28T10:10:00Z").unwrap();
        assert_eq!(baseline(&s, "hark", &page.id, "2026-10-28T10:11:00Z").unwrap(), "{\"tabs\":[]}\n");
        let list = versions(&s, "hark", &page.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].at, NOW, "oldest first, the ISO instant it was taken");
        // Going back keeps what is there as a version too, so nothing is lost.
        restore(&s, "hark", &page.id, &list[0].at, "2026-10-28T10:20:00Z").unwrap();
        assert_eq!(s.read_page("hark", &page.path).unwrap(), base);
        assert_eq!(versions(&s, "hark", &page.id).unwrap().len(), 3);
        // Your own edit through Hark keeps the text before it.
        write(&s, "hark", &page.id, "{\"tabs\":[{\"name\":\"x\"}]}\n", "2026-10-28T10:30:00Z").unwrap();
        let list = versions(&s, "hark", &page.id).unwrap();
        assert_eq!(list.len(), 4);
        assert_eq!(s.read_version("hark", &page.path, &list[3].at).unwrap(), base, "the text before the edit");
        assert!(restore(&s, "hark", &page.id, "2000-01-01T00:00:00Z", NOW).is_err(), "a version that is not there");
    }

    #[test]
    fn old_versions_are_pruned_to_the_last_thirty() {
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Nota", Kind::Doc, None, NOW).unwrap();
        for i in 0..40 {
            accept(&s, "hark", &page.id, &format!("2026-10-28T11:{i:02}:00Z")).unwrap();
        }
        let list = versions(&s, "hark", &page.id).unwrap();
        assert_eq!(list.len(), crate::domain::page_history::KEEP);
        assert_eq!(list[0].at, "2026-10-28T11:10:00Z", "the oldest went first");
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
