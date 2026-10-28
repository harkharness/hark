//! Artifacts: the documentation a person and their agents write, as a tree.
//!
//! One tree per SPACE (a project, or the personal space). Folders on disk
//! mirror the tree; the manual order and the metadata (kind, origin,
//! links) live in the space's `index.json`, which this module reads and
//! writes as a value. Nothing here touches the disk: the adapter hands in
//! the JSON and the list of files it found, and takes the JSON back.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Doc,
    Slides,
    Design,
}

/// Where a page came from: the chat (session) and the task that wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Origin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

/// A relation the page has: a chat that cites it, a task it belongs to,
/// another page it points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub title: String,
    /// Relative to the space root, forward slashes, `.md` or `.html`.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    pub updated_at: String,
    /// The index names it but the file is gone: shown, never read.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub orphan: bool,
}

/// The tree of one space: the nodes, and the manual order of each
/// parent's children (`""` is the root).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Tree {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub order: BTreeMap<String, Vec<String>>,
}

/// What a pass over the disk changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reconciled {
    /// Files the index did not know, now pages at the root.
    pub added: Vec<String>,
    /// Pages whose file vanished since the last pass.
    pub orphaned: Vec<String>,
}

const PAGE_EXTENSIONS: [&str; 2] = ["md", "html"];

/// A page path is relative to its space, never climbs out of it, and
/// names a page format. The adapter still refuses symlinks; this is the
/// part that needs no disk.
pub fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("empty path".into());
    }
    if path.starts_with('/') || path.contains('\\') {
        return Err(format!("{path}: must be relative to the space"));
    }
    if path.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
        return Err(format!("{path}: no empty, `.` or `..` segments"));
    }
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext {
        Some(e) if PAGE_EXTENSIONS.contains(&e.as_str()) => Ok(()),
        _ => Err(format!("{path}: a page is .md or .html")),
    }
}

/// Lowercase ASCII, hyphens between words: what a title looks like as a
/// file name or an id. Accented Latin letters fold to their base.
pub fn slug(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut dash = true;
    for c in title.chars() {
        match fold(c) {
            Some(f) if f.is_ascii_alphanumeric() => {
                out.push(f.to_ascii_lowercase());
                dash = false;
            }
            _ if !dash => {
                out.push('-');
                dash = true;
            }
            _ => {}
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "pagina".into() } else { trimmed }
}

/// The base letter of an accented Latin one; ASCII as is; the rest is a
/// word break.
fn fold(c: char) -> Option<char> {
    Some(match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        c if c.is_ascii() => c,
        _ => return None,
    })
}

fn stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file)
}

impl Tree {
    /// The index file as a tree; nothing (an absent or empty file) is an
    /// empty tree, which is how every space starts.
    pub fn parse(json: &str) -> Result<Tree, String> {
        if json.trim().is_empty() {
            return Ok(Tree::default());
        }
        serde_json::from_str(json).map_err(|e| format!("index.json: {e}"))
    }

    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into());
        s.push('\n');
        s
    }

    pub fn get(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    fn get_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    fn key(parent: Option<&str>) -> String {
        parent.unwrap_or("").to_string()
    }

    /// The children of a parent (None = the root) in their manual order;
    /// a node the order forgot comes last, never lost.
    pub fn children(&self, parent: Option<&str>) -> Vec<&Node> {
        let key = Self::key(parent);
        let ordered = self.order.get(&key).cloned().unwrap_or_default();
        let mut out: Vec<&Node> = ordered.iter().filter_map(|id| self.get(id)).collect();
        for n in &self.nodes {
            if n.parent.as_deref() == parent && !ordered.contains(&n.id) {
                out.push(n);
            }
        }
        out
    }

    pub fn path_for(&self, id: &str) -> Option<&str> {
        self.get(id).map(|n| n.path.as_str())
    }

    /// Titles from the root down to the page.
    pub fn breadcrumb(&self, id: &str) -> Vec<&str> {
        let mut out = vec![];
        let mut cur = self.get(id);
        while let Some(n) = cur {
            out.push(n.title.as_str());
            cur = n.parent.as_deref().and_then(|p| self.get(p));
        }
        out.reverse();
        out
    }

    fn is_descendant(&self, id: &str, ancestor: &str) -> bool {
        let mut cur = self.get(id).and_then(|n| n.parent.as_deref());
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.get(p).and_then(|n| n.parent.as_deref());
        }
        false
    }

    /// A new page under its parent, at a position (None = last).
    pub fn insert(&mut self, node: Node, at: Option<usize>) -> Result<(), String> {
        validate_path(&node.path)?;
        if self.get(&node.id).is_some() {
            return Err(format!("{}: id already exists", node.id));
        }
        if let Some(p) = node.parent.as_deref() {
            if self.get(p).is_none() {
                return Err(format!("{p}: parent not found"));
            }
        }
        let key = Self::key(node.parent.as_deref());
        let list = self.order.entry(key).or_default();
        let i = at.unwrap_or(list.len()).min(list.len());
        list.insert(i, node.id.clone());
        self.nodes.push(node);
        Ok(())
    }

    /// Move a page (with its subtree) under another parent, at a position.
    pub fn move_to(&mut self, id: &str, parent: Option<&str>, at: usize) -> Result<(), String> {
        if self.get(id).is_none() {
            return Err(format!("{id}: not found"));
        }
        if let Some(p) = parent {
            if p == id || self.is_descendant(p, id) {
                return Err(format!("{id}: cannot move under its own subtree"));
            }
            if self.get(p).is_none() {
                return Err(format!("{p}: parent not found"));
            }
        }
        let old_key = Self::key(self.get(id).and_then(|n| n.parent.as_deref()));
        if let Some(list) = self.order.get_mut(&old_key) {
            list.retain(|x| x != id);
        }
        let new_key = Self::key(parent);
        let list = self.order.entry(new_key).or_default();
        list.insert(at.min(list.len()), id.to_string());
        if let Some(n) = self.get_mut(id) {
            n.parent = parent.map(str::to_string);
        }
        Ok(())
    }

    /// Remove a page and everything under it; the ids that went.
    pub fn remove(&mut self, id: &str) -> Vec<String> {
        let mut gone = vec![];
        let mut stack = vec![id.to_string()];
        while let Some(cur) = stack.pop() {
            if self.get(&cur).is_none() {
                continue;
            }
            for child in self.nodes.iter().filter(|n| n.parent.as_deref() == Some(cur.as_str())) {
                stack.push(child.id.clone());
            }
            gone.push(cur);
        }
        for g in &gone {
            self.nodes.retain(|n| &n.id != g);
            self.order.remove(g);
        }
        for list in self.order.values_mut() {
            list.retain(|x| !gone.contains(x));
        }
        gone
    }

    pub fn rename(&mut self, id: &str, title: &str) -> Result<(), String> {
        match self.get_mut(id) {
            Some(n) => {
                n.title = title.to_string();
                Ok(())
            }
            None => Err(format!("{id}: not found")),
        }
    }

    /// A slug of the title that no page in this tree uses yet.
    pub fn fresh_id(&self, title: &str) -> String {
        let base = slug(title);
        if self.get(&base).is_none() {
            return base;
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|cand| self.get(cand).is_none())
            .expect("an unused suffix exists")
    }

    /// Where a new page's file goes: beside its siblings, in the folder
    /// of its parent. A parent that is a leaf page gets a folder named
    /// after its file; an `index` page already names its folder.
    pub fn path_under(&self, parent: Option<&str>, slug: &str, kind: Kind) -> String {
        let ext = match kind {
            Kind::Doc => "md",
            Kind::Slides | Kind::Design => "html",
        };
        let Some(p) = parent.and_then(|p| self.get(p)) else {
            return format!("{slug}.{ext}");
        };
        let dir = if stem(&p.path) == "index" {
            p.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()
        } else {
            p.path.rsplit_once('.').map(|(d, _)| d.to_string()).unwrap_or_else(|| p.path.clone())
        };
        if dir.is_empty() { format!("{slug}.{ext}") } else { format!("{dir}/{slug}.{ext}") }
    }

    /// Bring the index in line with the files on disk: a file the index
    /// does not know becomes a page at the root (the person files it); a
    /// page whose file is gone is flagged, never deleted. Idempotent.
    pub fn reconcile(&mut self, files: &[&str], now: &str) -> Reconciled {
        let mut out = Reconciled::default();
        for n in &mut self.nodes {
            let present = files.contains(&n.path.as_str());
            if !present && !n.orphan {
                out.orphaned.push(n.id.clone());
            }
            n.orphan = !present;
        }
        for f in files {
            if validate_path(f).is_err() || self.nodes.iter().any(|n| n.path == *f) {
                continue;
            }
            let id = self.fresh_id(stem(f));
            let node = Node {
                id: id.clone(),
                title: stem(f).to_string(),
                path: (*f).to_string(),
                parent: None,
                kind: Kind::Doc,
                origin: None,
                links: vec![],
                updated_at: now.to_string(),
                orphan: false,
            };
            if self.insert(node, None).is_ok() {
                out.added.push(id);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, title: &str, path: &str, parent: Option<&str>) -> Node {
        Node {
            id: id.into(),
            title: title.into(),
            path: path.into(),
            parent: parent.map(str::to_string),
            kind: Kind::Doc,
            origin: None,
            links: vec![],
            updated_at: "2026-10-28T10:00:00Z".into(),
            orphan: false,
        }
    }

    fn sample() -> Tree {
        let mut t = Tree::default();
        t.insert(node("decisoes", "Decisões", "decisoes/index.md", None), None).unwrap();
        t.insert(node("adr-003", "ADR 003", "decisoes/adr-003.md", Some("decisoes")), None).unwrap();
        t.insert(node("adr-004", "ADR 004", "decisoes/adr-004.md", Some("decisoes")), None).unwrap();
        t.insert(node("visao", "Visão geral", "visao-geral.md", None), None).unwrap();
        t
    }

    #[test]
    fn an_empty_index_is_an_empty_tree_and_round_trips() {
        let t = Tree::parse("").unwrap();
        assert!(t.children(None).is_empty());
        let s = sample();
        let again = Tree::parse(&s.to_json()).unwrap();
        assert_eq!(again, s);
    }

    #[test]
    fn children_come_in_manual_order_and_a_breadcrumb_walks_to_the_root() {
        let t = sample();
        let roots: Vec<&str> = t.children(None).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(roots, vec!["decisoes", "visao"]);
        let kids: Vec<&str> = t.children(Some("decisoes")).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(kids, vec!["adr-003", "adr-004"]);
        assert_eq!(t.breadcrumb("adr-004"), vec!["Decisões", "ADR 004"]);
        assert_eq!(t.path_for("adr-004"), Some("decisoes/adr-004.md"));
    }

    #[test]
    fn inserting_at_a_position_respects_it_and_a_missing_parent_is_refused() {
        let mut t = sample();
        t.insert(node("adr-002", "ADR 002", "decisoes/adr-002.md", Some("decisoes")), Some(0)).unwrap();
        let kids: Vec<&str> = t.children(Some("decisoes")).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(kids, vec!["adr-002", "adr-003", "adr-004"]);
        let err = t.insert(node("x", "x", "x.md", Some("nope")), None).unwrap_err();
        assert!(err.contains("parent"), "{err}");
        let err = t.insert(node("adr-003", "dup", "dup.md", None), None).unwrap_err();
        assert!(err.contains("exists"), "{err}");
    }

    #[test]
    fn a_page_moves_between_parents_but_never_under_itself() {
        let mut t = sample();
        t.move_to("adr-004", None, 0).unwrap();
        let roots: Vec<&str> = t.children(None).iter().map(|n| n.id.as_str()).collect();
        assert_eq!(roots, vec!["adr-004", "decisoes", "visao"]);
        assert_eq!(t.children(Some("decisoes")).len(), 1);
        let err = t.move_to("decisoes", Some("adr-003"), 0).unwrap_err();
        assert!(err.contains("own"), "{err}");
        assert!(t.move_to("decisoes", Some("decisoes"), 0).is_err());
    }

    #[test]
    fn removing_a_page_takes_its_descendants_and_reports_them() {
        let mut t = sample();
        let mut gone = t.remove("decisoes");
        gone.sort();
        assert_eq!(gone, vec!["adr-003", "adr-004", "decisoes"]);
        assert_eq!(t.children(None).len(), 1);
        assert!(t.path_for("adr-003").is_none());
    }

    #[test]
    fn rename_changes_the_title_and_nothing_else() {
        let mut t = sample();
        t.rename("adr-004", "ADR 004: janela única").unwrap();
        assert_eq!(t.breadcrumb("adr-004"), vec!["Decisões", "ADR 004: janela única"]);
        assert_eq!(t.path_for("adr-004"), Some("decisoes/adr-004.md"));
        assert!(t.rename("nope", "x").is_err());
    }

    #[test]
    fn a_path_is_relative_inside_the_space_and_a_page_format() {
        assert!(validate_path("decisoes/adr-004.md").is_ok());
        assert!(validate_path("slides/infra.html").is_ok());
        assert!(validate_path("/etc/passwd").is_err());
        assert!(validate_path("../outro/x.md").is_err());
        assert!(validate_path("a/../../x.md").is_err());
        assert!(validate_path("notas.txt").is_err());
        assert!(validate_path("a//b.md").is_err());
        assert!(validate_path("").is_err());
    }

    #[test]
    fn reconcile_adopts_files_the_index_does_not_know_and_flags_the_ones_that_vanished() {
        let mut t = sample();
        let on_disk = ["decisoes/adr-003.md", "decisoes/adr-004.md", "visao-geral.md", "notas/reuniao.md", "fase-10.html"];
        let out = t.reconcile(&on_disk, "2026-10-28T11:00:00Z");
        assert_eq!(out.added, vec!["reuniao", "fase-10"]);
        assert_eq!(out.orphaned, vec!["decisoes"]);
        let reuniao = t.get("reuniao").unwrap();
        assert_eq!(reuniao.title, "reuniao");
        assert_eq!(reuniao.parent, None, "adopted files land at the root; the person files them");
        assert_eq!(reuniao.kind, Kind::Doc);
        assert_eq!(t.get("fase-10").unwrap().kind, Kind::Doc, "html is a document unless the index says otherwise");
        assert!(t.get("decisoes").unwrap().orphan);
        // Running it again changes nothing.
        let again = t.reconcile(&on_disk, "2026-10-28T12:00:00Z");
        assert!(again.added.is_empty() && again.orphaned.is_empty());
        // The file is back: no longer an orphan.
        let back = ["decisoes/index.md", "decisoes/adr-003.md", "decisoes/adr-004.md", "visao-geral.md", "notas/reuniao.md", "fase-10.html"];
        t.reconcile(&back, "2026-10-28T13:00:00Z");
        assert!(!t.get("decisoes").unwrap().orphan);
    }

    #[test]
    fn an_id_is_a_slug_of_the_title_made_unique_inside_the_tree() {
        let t = sample();
        assert_eq!(t.fresh_id("ADR 004: Janela Única"), "adr-004-janela-unica");
        assert_eq!(t.fresh_id("Visão geral"), "visao-geral");
        assert_eq!(t.fresh_id("Decisões"), "decisoes-2", "the slug exists: a suffix keeps it unique");
        assert_eq!(t.fresh_id("   "), "pagina");
    }

    #[test]
    fn a_new_page_gets_its_path_from_its_parent_and_its_slug() {
        let t = sample();
        assert_eq!(t.path_under(Some("decisoes"), "adr-005", Kind::Doc), "decisoes/adr-005.md");
        assert_eq!(t.path_under(None, "roadmap", Kind::Slides), "roadmap.html");
        assert_eq!(t.path_under(Some("visao"), "detalhe", Kind::Doc), "visao-geral/detalhe.md", "a leaf page becomes a folder named after its file");
    }
}
