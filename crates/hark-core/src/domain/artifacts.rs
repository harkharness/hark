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
    /// A spreadsheet the agent writes as json; Hark computes it.
    Sheet,
    /// A whiteboard: notes, groups, a timeline.
    Board,
    /// A dashboard over local sources, with saved queries.
    Data,
}

impl Kind {
    /// The file extension of each kind. The three json kinds carry a
    /// compound one, so the file says what it is and a plain `.json`
    /// (the index!) is never a page.
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Doc => "md",
            Kind::Slides | Kind::Design => "html",
            Kind::Sheet => "sheet.json",
            Kind::Board => "board.json",
            Kind::Data => "dash.json",
        }
    }

    /// The kind a json extension names, if it is one of ours.
    pub fn of_json_extension(ext: &str) -> Option<Kind> {
        match ext {
            "sheet.json" => Some(Kind::Sheet),
            "board.json" => Some(Kind::Board),
            "dash.json" => Some(Kind::Data),
            _ => None,
        }
    }

    /// Whether the file's extension fixes the kind (json kinds) or the
    /// person may re-label it (an html page is a deck or a design by its
    /// shape; a document when it says so).
    pub fn fixed_by_extension(self) -> bool {
        matches!(self, Kind::Sheet | Kind::Board | Kind::Data)
    }
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
    /// Pages whose file is gone: they left the tree (their children, whose
    /// files are still there, moved up to where their folder puts them).
    pub dropped: Vec<String>,
    /// Pages whose file was moved to a new name in one pass: same page,
    /// same id, new path (and heading).
    pub renamed: Vec<String>,
}

/// Every extension a page may have; the json ones are compound.
pub const PAGE_EXTENSIONS: [&str; 5] = ["md", "html", "sheet.json", "board.json", "dash.json"];

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
    if PAGE_EXTENSIONS.contains(&ext_of(path)) {
        Ok(())
    } else {
        Err(format!("{path}: a page is .md, .html, .sheet.json, .board.json or .dash.json"))
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

/// The file name without its page extension (compound ones included).
fn stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    let ext = ext_of(file);
    if ext.is_empty() { file } else { &file[..file.len() - ext.len() - 1] }
}

/// The path without its page extension: what names the page's folder.
fn without_ext(path: &str) -> &str {
    let ext = ext_of(path);
    if ext.is_empty() { path } else { &path[..path.len() - ext.len() - 1] }
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

    /// What a page is (document, deck, design), set by hand: the tree's
    /// guess from the file is a guess.
    pub fn set_kind(&mut self, id: &str, kind: Kind) -> Result<(), String> {
        let n = self.nodes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("{id}: not found"))?;
        n.kind = kind;
        Ok(())
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
        let ext = kind.extension();
        let Some(p) = parent.and_then(|p| self.get(p)) else {
            return format!("{slug}.{ext}");
        };
        let dir = if stem(&p.path) == "index" {
            p.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()
        } else {
            without_ext(&p.path).to_string()
        };
        if dir.is_empty() { format!("{slug}.{ext}") } else { format!("{dir}/{slug}.{ext}") }
    }

    /// The node whose file names a folder: `a/b.md`, `a/b.html` or
    /// `a/b/index.*` all name `a/b`. None when no page does.
    fn page_naming_folder(&self, folder: &str) -> Option<&str> {
        self.nodes
            .iter()
            .find(|n| {
                let p = n.path.as_str();
                without_ext(p) == folder || (stem(p) == "index" && p.rsplit_once('/').map(|(d, _)| d) == Some(folder))
            })
            .map(|n| n.id.as_str())
    }

    /// Bring the index in line with the files on disk: a file the index
    /// does not know becomes a page — titled by its head when it has one,
    /// under the page whose file names its folder when one does, else at
    /// the root for the person to file; a page whose file is gone leaves
    /// the tree, and its children (files still there) move up to where
    /// their folder puts them. Shallow files go first, so a parent written
    /// in the same pass as its children is there before them. Idempotent.
    pub fn reconcile(&mut self, files: &[(&str, Option<&str>)], now: &str) -> Reconciled {
        let mut out = Reconciled::default();
        let on_disk = |path: &str| files.iter().any(|(f, _)| *f == path);
        // Exactly one page vanished and exactly one new file sits beside
        // it, same folder: that is a `mv`, or the page rewritten in the
        // other format ("make the home an html page"). The page keeps its
        // id (so its links hold), its origin and its place; the path, the
        // heading and the kind follow the file. Anything less clear-cut
        // is a page gone plus a page adopted, for the person to sort out.
        let gone: Vec<usize> = (0..self.nodes.len()).filter(|&i| !on_disk(&self.nodes[i].path)).collect();
        let fresh: Vec<&(&str, Option<&str>)> =
            files.iter().filter(|(f, _)| validate_path(f).is_ok() && !self.nodes.iter().any(|n| n.path == *f)).collect();
        if let ([i], [(f, title)]) = (gone.as_slice(), fresh.as_slice()) {
            let n = &mut self.nodes[*i];
            if folder_of(&n.path) == folder_of(f) {
                n.kind = kind_for(f, n.kind);
                n.path = (*f).to_string();
                if let Some(t) = title {
                    n.title = (*t).to_string();
                }
                n.updated_at = now.to_string();
                out.renamed.push(n.id.clone());
            }
        }
        // What is still gone leaves the tree. Its children keep their
        // files, so they keep their pages: moved up, never dropped.
        let dropped: Vec<String> = self.nodes.iter().filter(|n| !on_disk(&n.path)).map(|n| n.id.clone()).collect();
        for id in dropped {
            let kids: Vec<String> = self.children(Some(&id)).iter().map(|n| n.id.clone()).collect();
            let up = self.get(&id).and_then(|n| n.parent.clone());
            for kid in kids {
                let at = self.children(up.as_deref()).len();
                let _ = self.move_to(&kid, up.as_deref(), at);
            }
            self.remove(&id);
            out.dropped.push(id);
        }
        let mut fresh: Vec<&(&str, Option<&str>)> =
            files.iter().filter(|(f, _)| validate_path(f).is_ok() && !self.nodes.iter().any(|n| n.path == *f)).collect();
        fresh.sort_by_key(|(f, _)| (f.matches('/').count(), *f));
        for (f, title) in fresh {
            let title = title.map(str::to_string).unwrap_or_else(|| stem(f).to_string());
            let id = self.fresh_id(&title);
            let parent = f.rsplit_once('/').and_then(|(folder, _)| self.page_naming_folder(folder)).map(str::to_string);
            let node = Node {
                id: id.clone(),
                title,
                path: (*f).to_string(),
                parent: parent.clone(),
                kind: kind_for(f, Kind::Doc),
                origin: None,
                links: vec![],
                updated_at: now.to_string(),
            };
            if self.insert(node, None).is_ok() {
                out.added.push(id);
            }
        }
        out
    }
}

/// The kind a file's format allows: markdown is a document; a json kind
/// is its extension; html is a deck unless the page was already one of
/// the html kinds (a design stays a design through a rewrite).
fn kind_for(path: &str, was: Kind) -> Kind {
    let ext = ext_of(path);
    if ext == "md" {
        Kind::Doc
    } else if let Some(k) = Kind::of_json_extension(ext) {
        k
    } else if matches!(was, Kind::Slides | Kind::Design) {
        was
    } else {
        Kind::Slides
    }
}

fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

/// The page extension of a path, lowercase: a compound json one when the
/// file carries it, else whatever follows the last dot ("" when none).
pub fn ext_of(path: &str) -> &'static str {
    let file = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    for ext in ["sheet.json", "board.json", "dash.json"] {
        if file.ends_with(&format!(".{ext}")) && file.len() > ext.len() + 1 {
            return ext;
        }
    }
    match file.rsplit_once('.').map(|(_, e)| e) {
        Some("md") => "md",
        Some("html") => "html",
        Some(_) | None => "",
    }
}

/// What a page is, from its file. Markdown is a document. An html page
/// says so with `<meta name="hark-kind" content="doc|slides|design">`
/// (the skill asks agents for it); without the tag, a run of two or more
/// slides (`class="slide"`, `data-slide`) is a deck, anything else a
/// document. The old default (every html a deck) made the project's home
/// page and a real deck wear the same icon (03/11).
pub fn page_kind(path: &str, text: &str) -> Kind {
    if let Some(k) = Kind::of_json_extension(ext_of(path)) {
        return k;
    }
    if !path.to_ascii_lowercase().ends_with(".html") {
        return Kind::Doc;
    }
    let lower = text.to_ascii_lowercase();
    if let Some(at) = lower.find("name=\"hark-kind\"").or_else(|| lower.find("name='hark-kind'")) {
        let tag = &lower[at..lower[at..].find('>').map(|e| at + e).unwrap_or(lower.len())];
        for (needle, kind) in [("slides", Kind::Slides), ("design", Kind::Design), ("doc", Kind::Doc)] {
            if tag.contains(&format!("content=\"{needle}\"")) || tag.contains(&format!("content='{needle}'")) {
                return kind;
            }
        }
    }
    let slides = lower.matches("class=\"slide").count()
        + lower.matches("class='slide").count()
        + lower.matches("data-slide").count();
    if slides >= 2 {
        Kind::Slides
    } else {
        Kind::Doc
    }
}

/// A deck that brought its own pager (a script, a bar of arrows and a
/// counter, a progress strip) with the pager taken out: the slides and
/// the styles stay. None when there was nothing to take. Hark turns the
/// pages itself, so the page's own chrome only gets in the way.
pub fn strip_own_pager(html: &str) -> Option<String> {
    let no_scripts = remove_tag_blocks(html, "script");
    let out = remove_body_chrome(&no_scripts);
    (out != html).then_some(out)
}

/// Every `<tag …>…</tag>` block, gone (case-insensitive).
fn remove_tag_blocks(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(html.len());
    let mut at = 0;
    while let Some(rel) = lower[at..].find(&open) {
        let start = at + rel;
        let Some(end_rel) = lower[start..].find(&close) else { break };
        let end = start + end_rel + close.len();
        out.push_str(&html[at..start]);
        at = end;
    }
    out.push_str(&html[at..]);
    out
}

/// The body's top-level elements that hold no slide and look like a
/// pager — by class or id (nav, pager, controls, progress, counter, dots,
/// arrows) or by a short "n / m" counter as their text — removed. A
/// viewport around the slides stays, whatever it is called.
fn remove_body_chrome(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(b0) = lower.find("<body") else { return html.to_string() };
    let Some(body_open_end) = lower[b0..].find('>') else { return html.to_string() };
    let body_start = b0 + body_open_end + 1;
    let body_end = lower.rfind("</body>").unwrap_or(html.len());
    let body = &html[body_start..body_end];
    let mut keep = String::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        let Some(rel) = body[i..].find('<') else {
            keep.push_str(&body[i..]);
            break;
        };
        keep.push_str(&body[i..i + rel]);
        let tag_at = i + rel;
        match element_at(body, tag_at) {
            Some((name, end)) => {
                let element = &body[tag_at..end];
                if name == "section" || holds_slide(element) || !looks_like_pager(element) {
                    keep.push_str(element);
                }
                i = end;
            }
            None => {
                keep.push('<');
                i = tag_at + 1;
            }
        }
    }
    format!("{}{}{}", &html[..body_start], keep, &html[body_end..])
}

const VOID_TAGS: [&str; 8] = ["br", "hr", "img", "input", "meta", "link", "source", "wbr"];

/// The element opening at `at` (a `<tag`): its name and where it ends,
/// matching nested tags of the same name. Comments and void tags are
/// one token. None when the text is not an element start.
fn element_at(s: &str, at: usize) -> Option<(String, usize)> {
    if s[at..].starts_with("<!--") {
        return s[at..].find("-->").map(|e| ("!--".to_string(), at + e + 3));
    }
    let name: String = s[at + 1..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>().to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    let gt = at + s[at..].find('>')?;
    if VOID_TAGS.contains(&name.as_str()) || s[..gt].ends_with('/') {
        return Some((name, gt + 1));
    }
    let lower = s.to_ascii_lowercase();
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut depth = 1;
    let mut pos = gt + 1;
    while depth > 0 {
        let next_open = lower[pos..].find(&open).map(|r| pos + r);
        let next_close = lower[pos..].find(&close).map(|r| pos + r)?;
        match next_open {
            Some(o) if o < next_close && !lower[o + open.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-') => {
                depth += 1;
                pos = o + open.len();
            }
            _ => {
                depth -= 1;
                pos = next_close + close.len();
            }
        }
    }
    Some((name, pos))
}

/// An element with a slide somewhere inside: a deck's viewport or stage.
/// Whatever it is called and whatever its text says, it stays.
fn holds_slide(element: &str) -> bool {
    let lower = element.to_ascii_lowercase();
    lower.contains("<section") || lower.contains("data-slide") || lower.contains("class=\"slide") || lower.contains("class='slide")
}

/// A short "n / m" counter in the element's text — not a date in a
/// sentence.
const COUNTER_MAX_CHARS: usize = 16;

fn looks_like_pager(element: &str) -> bool {
    let lower = element.to_ascii_lowercase();
    let head_end = lower.find('>').unwrap_or(lower.len());
    let head = &lower[..head_end];
    const WORDS: [&str; 8] = ["nav", "pager", "control", "progress", "counter", "dots", "arrow", "toolbar"];
    if (head.contains("class=") || head.contains("id=")) && WORDS.iter().any(|w| head.contains(w)) {
        return true;
    }
    // "1 / 10" in its text, tags stripped.
    let mut text = String::new();
    let mut in_tag = false;
    for c in lower.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let t: Vec<char> = text.trim().chars().collect();
    if t.len() > COUNTER_MAX_CHARS {
        return false;
    }
    for (i, c) in t.iter().enumerate() {
        if *c == '/' {
            let left = t[..i].iter().rev().find(|c| !c.is_whitespace()).is_some_and(|c| c.is_ascii_digit());
            let right = t[i + 1..].iter().find(|c| !c.is_whitespace()).is_some_and(|c| c.is_ascii_digit());
            if left && right {
                return true;
            }
        }
    }
    false
}

/// What the head of a file says a page is called: the first `# heading`
/// of markdown (front matter skipped), the `<title>` or first `<h1>` of
/// html, else the file name.
pub fn page_title(path: &str, head: &str) -> String {
    let found = if Kind::of_json_extension(ext_of(path)).is_some() {
        json_title(head)
    } else if path.ends_with(".md") {
        let mut lines = head.lines().map(str::trim);
        let mut body: Vec<&str> = vec![];
        if lines.next() == Some("---") {
            for l in lines.by_ref() {
                if l == "---" {
                    break;
                }
            }
        } else {
            body = head.lines().map(str::trim).collect();
        }
        if body.is_empty() {
            body = lines.collect();
        }
        body.iter().find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
    } else {
        tag_text(head, "title").or_else(|| tag_text(head, "h1"))
    };
    found.filter(|t| !t.is_empty()).unwrap_or_else(|| stem(path).to_string())
}

/// The `"title"` of a json page's head, when the head holds one. The
/// head may be cut mid-document, so this is a scan, not a parse.
fn json_title(head: &str) -> Option<String> {
    let at = head.find("\"title\"")?;
    let rest = head[at + 7..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(match n {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                }
            }
            '"' => return Some(out.trim().to_string()),
            _ => out.push(c),
        }
    }
    None
}

/// The text inside the first `<tag …>…</tag>` of an html head, trimmed.
fn tag_text(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find(&format!("<{tag}"))?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find(&format!("</{tag}>"))?;
    Some(html[start..end].trim().to_string())
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
    fn reconcile_adopts_files_the_index_does_not_know_and_drops_the_pages_whose_file_is_gone() {
        let mut t = sample();
        let on_disk = found(&["decisoes/adr-003.md", "decisoes/adr-004.md", "visao-geral.md", "notas/reuniao.md", "fase-10.html"]);
        let out = t.reconcile(&on_disk, "2026-10-28T11:00:00Z");
        assert_eq!(out.added, vec!["fase-10", "reuniao"], "shallow first");
        assert_eq!(out.dropped, vec!["decisoes"], "its file is gone: the page leaves the tree");
        assert!(t.get("decisoes").is_none());
        let reuniao = t.get("reuniao").unwrap();
        assert_eq!(reuniao.title, "reuniao");
        assert_eq!(reuniao.parent, None, "no page named the folder: the root, and the person files it");
        assert_eq!(reuniao.kind, Kind::Doc);
        assert_eq!(t.get("fase-10").unwrap().kind, Kind::Slides, "html adopted is a deck until the index says otherwise");
        // The children whose files are still there stay pages, at the root
        // now that nothing names their folder.
        assert_eq!(t.get("adr-003").unwrap().parent, None);
        assert_eq!(t.get("adr-004").unwrap().parent, None);
        let roots: Vec<&str> = t.children(None).iter().map(|n| n.id.as_str()).collect();
        assert!(roots.contains(&"adr-003") && roots.contains(&"adr-004"), "{roots:?}");
        // Running it again changes nothing.
        let again = t.reconcile(&on_disk, "2026-10-28T12:00:00Z");
        assert!(again.added.is_empty() && again.dropped.is_empty() && again.renamed.is_empty(), "{again:?}");
        // The file is back: a page again, adopted fresh.
        let back = found(&["decisoes/index.md", "decisoes/adr-003.md", "decisoes/adr-004.md", "visao-geral.md", "notas/reuniao.md", "fase-10.html"]);
        let out = t.reconcile(&back, "2026-10-28T13:00:00Z");
        assert_eq!(out.added, vec!["index"]);
        assert_eq!(t.get("index").unwrap().path, "decisoes/index.md");
    }

    #[test]
    fn a_file_gone_and_one_new_beside_it_in_the_same_pass_is_a_rename_and_the_page_keeps_its_identity() {
        let mut t = sample();
        let origin = Origin { session_id: Some("s1".into()), task: Some("escrever a home".into()) };
        t.nodes.iter_mut().find(|n| n.id == "visao").unwrap().origin = Some(origin.clone());
        // The agent did `mv visao-geral.md home.md` and changed the heading.
        let disk = vec![
            ("decisoes/index.md", None),
            ("decisoes/adr-003.md", None),
            ("decisoes/adr-004.md", None),
            ("home.md", Some("Home do workspace")),
        ];
        let out = t.reconcile(&disk, "2026-10-30T17:00:00Z");
        assert_eq!(out.renamed, vec!["visao"]);
        assert!(out.added.is_empty() && out.dropped.is_empty(), "{out:?}");
        let n = t.get("visao").unwrap();
        assert_eq!(n.path, "home.md");
        assert_eq!(n.title, "Home do workspace", "the new heading wins");
        assert_eq!(n.origin, Some(origin), "the chat it was born in stays");
        assert_eq!(n.updated_at, "2026-10-30T17:00:00Z");
        assert!(t.get("home").is_none(), "no second node for the same page");
        // Without a heading the old title stays.
        let mut t2 = sample();
        t2.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("decisoes/adr-004.md", None), ("geral.md", None)], "x");
        assert_eq!(t2.get("visao").unwrap().title, "Visão geral");
        assert_eq!(t2.get("visao").unwrap().path, "geral.md");
    }

    #[test]
    fn a_page_rewritten_in_the_other_format_is_still_that_page_and_changes_kind() {
        // "make the home an html page": the agent wrote home.html and
        // deleted home.md. Same page, now a deck; the link still holds.
        let mut t = sample();
        let out = t.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("decisoes/adr-004.md", None), ("home.html", Some("Home"))], "x");
        assert_eq!(out.renamed, vec!["visao"]);
        let n = t.get("visao").unwrap();
        assert_eq!((n.path.as_str(), n.kind), ("home.html", Kind::Slides));
        // And back to markdown: a document again.
        let out = t.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("decisoes/adr-004.md", None), ("home.md", None)], "y");
        assert_eq!(out.renamed, vec!["visao"]);
        assert_eq!(t.get("visao").unwrap().kind, Kind::Doc);
    }

    #[test]
    fn two_files_gone_or_a_new_file_in_another_folder_is_not_a_rename() {
        let mut t = sample();
        // Two vanished, one new: ambiguous, no guess.
        let out = t.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("novo.md", None)], "x");
        assert!(out.renamed.is_empty());
        assert_eq!(out.dropped, vec!["adr-004", "visao"]);
        assert_eq!(out.added, vec!["novo"]);
        // One vanished at the root, one new under decisoes: different folders.
        let mut t = sample();
        let out = t.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("decisoes/adr-004.md", None), ("decisoes/visao.md", None)], "x");
        assert!(out.renamed.is_empty());
        assert_eq!(out.dropped, vec!["visao"]);
        // The id is free again, so the adopted file takes it: a new page
        // under "decisoes", not the old one moved.
        assert_eq!(out.added, vec!["visao"]);
        let n = t.get("visao").unwrap();
        assert_eq!((n.path.as_str(), n.parent.as_deref()), ("decisoes/visao.md", Some("decisoes")));
    }

    #[test]
    fn an_html_page_says_what_it_is_or_is_read_from_its_shape() {
        // The meta tag wins: the agent (or the person) said so.
        assert_eq!(page_kind("x.html", "<html><head><meta name=\"hark-kind\" content=\"design\"></head>"), Kind::Design);
        assert_eq!(page_kind("x.html", "<meta name='hark-kind' content='slides'>"), Kind::Slides);
        assert_eq!(page_kind("x.html", "<meta name=\"hark-kind\" content=\"doc\"><section class=\"slide\"></section><section class=\"slide\">"), Kind::Doc);
        // No tag: a deck is a run of slides; anything else is a document.
        assert_eq!(page_kind("deck.html", "<body><section class=\"slide active\">1</section><section class=\"slide\">2</section></body>"), Kind::Slides);
        assert_eq!(page_kind("deck.html", "<div data-slide>1</div><div data-slide>2</div>"), Kind::Slides);
        assert_eq!(page_kind("home.html", "<body><h1>Home</h1><section class=\"slide\">one</section></body>"), Kind::Doc);
        assert_eq!(page_kind("home.html", "<body><h1>Home</h1><p>prose</p></body>"), Kind::Doc);
        // Markdown is always a document.
        assert_eq!(page_kind("notes.md", "<section class=\"slide\"></section><section class=\"slide\">"), Kind::Doc);
    }

    #[test]
    fn a_page_s_kind_can_be_set_by_hand_and_a_missing_page_is_refused() {
        let mut t = sample();
        t.set_kind("visao", Kind::Design).unwrap();
        assert_eq!(t.get("visao").unwrap().kind, Kind::Design);
        assert!(t.set_kind("nope", Kind::Doc).unwrap_err().contains("not found"));
    }

    #[test]
    fn a_deck_s_own_pager_comes_out_of_the_file_and_the_slides_stay() {
        let deck = r#"<!doctype html><html><head><title>D</title><style>.slide{opacity:0}.slide.active{opacity:1}</style></head>
<body>
<section class="slide active"><h1>One</h1></section>
<section class="slide"><h2>Two</h2><p>2 of 3 things</p></section>
<div class="nav"><button id="prev">←</button><span id="counter">1 / 2</span><button id="next">→</button></div>
<div class="progress"><div class="bar"></div></div>
<script>let i = 0; function go(n) { }</script>
</body></html>"#;
        let out = strip_own_pager(deck).expect("there was a pager to strip");
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("id=\"counter\""), "{out}");
        assert!(!out.contains("class=\"progress\""), "{out}");
        assert!(out.contains("<h1>One</h1>") && out.contains("<p>2 of 3 things</p>"), "slides untouched: {out}");
        assert!(out.contains("<style>"), "its styles stay: {out}");
        // Nothing to strip: the file is left alone.
        assert!(strip_own_pager("<html><body><section>a</section><section>b</section></body></html>").is_none());
    }

    #[test]
    fn a_wrapper_that_holds_the_slides_stays_whatever_its_text_says() {
        // The slides live inside a viewport; a date in one of them is not a
        // "n / m" counter, and a long footer with a date is not a pager.
        let deck = r#"<html><head><style>.slide{}</style></head>
<body>
<div class="progress" id="progress"></div>
<div class="viewport"><div class="stage">
<section class="slide"><h1>One</h1><p>Entrega 28/10</p></section>
<section class="slide"><h2>Two</h2></section>
</div></div>
<div class="arrow left">‹</div>
<div class="foot">Revisão de 28/10 com o time, antes da fase um</div>
<div id="counter">1 / 10</div>
<script>go()</script>
</body></html>"#;
        let out = strip_own_pager(deck).expect("there was a pager to strip");
        assert_eq!(out.matches("<section").count(), 2, "{out}");
        assert!(out.contains("class=\"viewport\"") && out.contains("Entrega 28/10"), "{out}");
        assert!(out.contains("Revisão de 28/10"), "a long footer is not a counter: {out}");
        assert!(!out.contains("id=\"progress\"") && !out.contains("class=\"arrow left\""), "{out}");
        assert!(!out.contains("id=\"counter\"") && !out.contains("<script"), "{out}");
    }

    /// Files as the store lists them, with no title hint.
    fn found<'a>(paths: &[&'a str]) -> Vec<(&'a str, Option<&'a str>)> {
        paths.iter().map(|p| (*p, None)).collect()
    }

    #[test]
    fn an_adopted_file_takes_its_heading_as_title_and_its_folder_s_page_as_parent() {
        let mut t = sample();
        let on_disk = vec![
            ("decisoes/index.md", None),
            ("decisoes/adr-003.md", None),
            ("decisoes/adr-004.md", None),
            ("visao-geral.md", None),
            ("decisoes/adr-005.md", Some("ADR 005: teto por turno")),
            ("guia/passo-1.md", Some("Passo 1")),
            ("guia.md", Some("Guia")),
        ];
        let out = t.reconcile(&on_disk, "2026-10-28T11:00:00Z");
        assert_eq!(out.added, vec!["guia", "adr-005-teto-por-turno", "passo-1"], "shallow first, so a parent exists before its child");
        let adr = t.get("adr-005-teto-por-turno").unwrap();
        assert_eq!(adr.title, "ADR 005: teto por turno");
        assert_eq!(adr.parent.as_deref(), Some("decisoes"), "decisoes/index.md names the folder");
        assert_eq!(t.get("passo-1").unwrap().parent.as_deref(), Some("guia"), "guia.md names the folder, adopted in the same pass");
        assert_eq!(t.children(Some("decisoes")).last().map(|n| n.id.as_str()), Some("adr-005-teto-por-turno"));
    }

    #[test]
    fn a_page_s_title_comes_from_its_head_or_its_file_name() {
        assert_eq!(page_title("decisoes/adr-004.md", "# ADR 004: janela única\n\ntexto"), "ADR 004: janela única");
        assert_eq!(page_title("x.md", "---\ntitle: y\n---\n\n# Título real\n"), "Título real", "front matter is skipped");
        assert_eq!(page_title("x.md", "sem heading"), "x");
        assert_eq!(page_title("deck.html", "<html><head><title>Infra como plataforma</title></head>"), "Infra como plataforma");
        assert_eq!(page_title("deck.html", "<h1>Só um h1</h1>"), "Só um h1");
        assert_eq!(page_title("deck.html", "<p>nada</p>"), "deck");
    }

    #[test]
    fn a_sheet_a_board_and_a_dashboard_are_pages_by_their_compound_extension() {
        // Three kinds the agent writes as json (10/11): the extension says
        // what the page is, so the tree needs no tag inside the file. A
        // plain .json (index.json!) is never a page.
        assert!(validate_path("custos.sheet.json").is_ok());
        assert!(validate_path("planos/release.board.json").is_ok());
        assert!(validate_path("uso.dash.json").is_ok());
        assert!(validate_path("index.json").is_err());
        assert!(validate_path("dados.json").is_err());
        assert_eq!(page_kind("custos.sheet.json", "{}"), Kind::Sheet);
        assert_eq!(page_kind("planos/release.board.json", "{}"), Kind::Board);
        assert_eq!(page_kind("uso.dash.json", "{}"), Kind::Data);
        // The title is the file's own, else the stem without the kind.
        assert_eq!(page_title("custos.sheet.json", r#"{"title": "Custos de infra", "tabs": []}"#), "Custos de infra");
        assert_eq!(page_title("custos-de-infra.sheet.json", r#"{"tabs": []}"#), "custos-de-infra");
        // A new page of each kind gets its compound extension.
        let t = sample();
        assert_eq!(t.path_under(None, "custos", Kind::Sheet), "custos.sheet.json");
        assert_eq!(t.path_under(Some("decisoes"), "plano", Kind::Board), "decisoes/plano.board.json");
        assert_eq!(t.path_under(None, "uso", Kind::Data), "uso.dash.json");
        // And its folder, when it gets children, is named without it.
        let mut t = sample();
        t.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("decisoes/adr-004.md", None), ("home.md", None), ("custos.sheet.json", Some("Custos"))], "x");
        assert_eq!(t.path_under(Some("custos"), "detalhe", Kind::Doc), "custos/detalhe.md");
        // Rewritten in another format in one pass: still the same page, now that kind.
        let out = t.reconcile(&[("decisoes/index.md", None), ("decisoes/adr-003.md", None), ("decisoes/adr-004.md", None), ("home.md", None), ("custos.dash.json", Some("Custos"))], "y");
        assert_eq!(out.renamed, vec!["custos"]);
        assert_eq!(t.get("custos").unwrap().kind, Kind::Data);
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
