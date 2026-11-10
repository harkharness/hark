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

/// A sheet page as the viewer shows it: computed, with the cells that
/// differ from the baseline marked, and the file's error when it is not
/// a sheet (the raw text rides along, so nothing is hidden).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SheetView {
    pub computed: crate::domain::sheet::Computed,
    pub changed: Vec<crate::domain::sheet::CellRef>,
    pub versions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub raw: String,
}

pub fn sheet_view(store: &dyn ArtifactStore, space: &str, id: &str, now: &str, today: &str) -> anyhow::Result<SheetView> {
    use crate::domain::sheet;
    let path = path_of(store, space, id)?;
    let raw = store.read_page(space, &path)?;
    let base_text = baseline(store, space, id, now)?;
    let versions = store.list_versions(space, &path)?.len();
    match sheet::parse(&raw) {
        Ok(sheet) => {
            let changed = sheet::parse(&base_text).map(|base| sheet::changed(&base, &sheet)).unwrap_or_default();
            Ok(SheetView { computed: sheet::compute(&sheet, today), changed, versions, error: None, raw })
        }
        Err(e) => Ok(SheetView {
            computed: sheet::Computed { title: None, tabs: vec![] },
            changed: vec![],
            versions,
            error: Some(e),
            raw,
        }),
    }
}

/// The person typed into a cell: the file is rewritten and becomes the
/// baseline at once — one's own edit is nothing to review.
pub fn sheet_set_cell(
    store: &dyn ArtifactStore,
    space: &str,
    id: &str,
    at: crate::domain::sheet::CellRef,
    typed: &str,
    now: &str,
) -> anyhow::Result<()> {
    use crate::domain::sheet;
    let path = path_of(store, space, id)?;
    let mut sheet = sheet::parse(&store.read_page(space, &path)?).map_err(anyhow::Error::msg)?;
    sheet::set_cell(&mut sheet, at, typed).map_err(anyhow::Error::msg)?;
    store.write_page(space, &path, &sheet::to_json(&sheet))?;
    let mut tree = load(store, space, now)?;
    if let Some(n) = tree.nodes.iter_mut().find(|n| n.id == id) {
        n.updated_at = now.to_string();
    }
    save(store, space, &tree)?;
    accept(store, space, id, now)
}

/// The person's edit on a sheet (fase 14.1): one op, one accepted version.
pub fn sheet_edit(store: &dyn ArtifactStore, space: &str, id: &str, op: crate::domain::sheet::Op, now: &str) -> anyhow::Result<()> {
    use crate::domain::sheet;
    let path = path_of(store, space, id)?;
    let mut sheet = sheet::parse(&store.read_page(space, &path)?).map_err(anyhow::Error::msg)?;
    sheet::apply(&mut sheet, op).map_err(anyhow::Error::msg)?;
    write(store, space, id, &sheet::to_json(&sheet), now)?;
    accept(store, space, id, now)
}

/// One query as the dashboard's "fontes e consultas" screen lists it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct QueryView {
    pub id: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    pub columns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    pub certified: bool,
    /// The agent changed or added it since the baseline.
    pub changed: bool,
    /// The first rows of its cached result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<crate::domain::dash::Table>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A dashboard page as the viewer shows it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DashView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub refresh: crate::domain::dash::Refresh,
    pub sources: Vec<crate::domain::dash::Source>,
    pub queries: Vec<QueryView>,
    pub panels: Vec<crate::domain::dash::PanelView>,
    /// When the numbers were taken; None before the first run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_at: Option<String>,
    pub stale: bool,
    /// The ids of the queries the agent changed since the baseline.
    pub changed: Vec<String>,
    pub versions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub raw: String,
}

const PREVIEW_ROWS: usize = 20;

/// The dashboard computed from its cache. `run` is the person's
/// "atualizar agora": every source runs, commands included. Without it,
/// a stale cache re-runs the files and sqlite only — a command runs
/// when the person asks, never on its own.
pub fn dash_view(
    store: &dyn ArtifactStore,
    runner: &dyn crate::ports::DataRunner,
    project: Option<&std::path::Path>,
    space: &str,
    id: &str,
    now: &str,
    run: bool,
) -> anyhow::Result<DashView> {
    use crate::domain::dash::{self, Cache, Result_, SourceKind};
    let path = path_of(store, space, id)?;
    let raw = store.read_page(space, &path)?;
    let base_text = baseline(store, space, id, now)?;
    let versions = store.list_versions(space, &path)?.len();
    let dash = match dash::parse(&raw) {
        Ok(d) => d,
        Err(e) => {
            return Ok(DashView {
                title: None,
                refresh: dash::Refresh::Manual,
                sources: vec![],
                queries: vec![],
                panels: vec![],
                cache_at: None,
                stale: true,
                changed: vec![],
                versions,
                error: Some(e),
                raw,
            })
        }
    };
    let cache_file = dash::cache_path(&path);
    let mut cache: Cache = store
        .read_aside(space, &cache_file)?
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let stale = dash::stale(dash.refresh, Some(cache.at.as_str()), now);
    if run || stale {
        for q in &dash.queries {
            let Some(src) = dash.sources.iter().find(|s| s.id == q.source) else { continue };
            // A command runs on the person's word only; a stale cache
            // keeps its last answer.
            if src.kind == SourceKind::Command && !run {
                continue;
            }
            let result = match project {
                None => Err(anyhow::anyhow!("o espaço não tem projeto: fontes só em projetos")),
                Some(root) => match src.kind {
                    SourceKind::Sqlite => runner.sql(root, src.path.as_deref().unwrap_or(""), q.sql.as_deref().unwrap_or("")),
                    SourceKind::Csv => runner.file(root, src.path.as_deref().unwrap_or("")).and_then(|t| dash::pick(&dash::parse_csv(&t), q).map_err(anyhow::Error::msg)),
                    SourceKind::Json => runner
                        .file(root, src.path.as_deref().unwrap_or(""))
                        .and_then(|t| dash::parse_json_table(&t).and_then(|tb| dash::pick(&tb, q)).map_err(anyhow::Error::msg)),
                    SourceKind::Command => runner.command(root, src.command.as_deref().unwrap_or("")).and_then(|out| {
                        let trimmed = out.trim_start();
                        let table = if trimmed.starts_with('[') || trimmed.starts_with('{') {
                            dash::parse_json_table(&out).map_err(anyhow::Error::msg)?
                        } else {
                            dash::parse_csv(&out)
                        };
                        dash::pick(&table, q).map_err(anyhow::Error::msg)
                    }),
                },
            };
            cache.results.insert(
                q.id.clone(),
                match result {
                    Ok(table) => Result_ { table: Some(table), error: None },
                    Err(e) => Result_ { table: None, error: Some(e.to_string()) },
                },
            );
        }
        cache.at = now.to_string();
        store.write_aside(space, &cache_file, &serde_json::to_string(&cache).unwrap_or_default())?;
    }
    let changed = dash::parse(&base_text).map(|b| dash::changed_queries(&b, &dash)).unwrap_or_default();
    let queries = dash
        .queries
        .iter()
        .map(|q| {
            let r = cache.results.get(&q.id);
            QueryView {
                id: q.id.clone(),
                source: q.source.clone(),
                sql: q.sql.clone(),
                columns: q.columns.clone(),
                limit: q.limit,
                certified: q.certified,
                changed: changed.contains(&q.id),
                preview: r.and_then(|r| r.table.as_ref()).map(|t| dash::Table { columns: t.columns.clone(), rows: t.rows.iter().take(PREVIEW_ROWS).cloned().collect() }),
                error: r.and_then(|r| r.error.clone()),
            }
        })
        .collect();
    let panels = dash.panels.iter().map(|p| dash::panel_view(&dash, p, cache.results.get(&p.query))).collect();
    Ok(DashView {
        title: dash.title.clone(),
        refresh: dash.refresh,
        sources: dash.sources.clone(),
        queries,
        panels,
        cache_at: (!cache.at.is_empty()).then(|| cache.at.clone()),
        stale: dash::stale(dash.refresh, Some(cache.at.as_str()), now),
        changed,
        versions,
        error: None,
        raw,
    })
}

/// The person certifies a query (or takes it back): the file is
/// rewritten and becomes the baseline, as any edit of theirs.
pub fn dash_certify(store: &dyn ArtifactStore, space: &str, id: &str, query: &str, on: bool, now: &str) -> anyhow::Result<()> {
    use crate::domain::dash;
    let path = path_of(store, space, id)?;
    let mut dash = dash::parse(&store.read_page(space, &path)?).map_err(anyhow::Error::msg)?;
    dash::set_certified(&mut dash, query, on).map_err(anyhow::Error::msg)?;
    write(store, space, id, &dash::to_json(&dash), now)?;
    accept(store, space, id, now)
}

/// The person's edit on a dashboard (fase 14.1): one op, one accepted
/// version; a query she writes is certified by that.
pub fn dash_edit(store: &dyn ArtifactStore, space: &str, id: &str, op: crate::domain::dash::Op, now: &str) -> anyhow::Result<()> {
    use crate::domain::dash;
    let path = path_of(store, space, id)?;
    let mut dash = dash::parse(&store.read_page(space, &path)?).map_err(anyhow::Error::msg)?;
    dash::apply(&mut dash, op).map_err(anyhow::Error::msg)?;
    write(store, space, id, &dash::to_json(&dash), now)?;
    accept(store, space, id, now)
}

// ---------- comments (fase 14.3) ----------

use crate::domain::comments::{self, Anchor, Comment, Thread};

fn thread_of(store: &dyn ArtifactStore, space: &str, path: &str) -> anyhow::Result<(String, Thread)> {
    let file = comments::comments_path(path);
    let text = store.read_aside(space, &file)?.unwrap_or_default();
    Ok((file, comments::parse(&text)))
}

/// A page's comments, oldest first.
pub fn comments_of(store: &dyn ArtifactStore, space: &str, id: &str) -> anyhow::Result<Vec<Comment>> {
    let path = path_of(store, space, id)?;
    Ok(thread_of(store, space, &path)?.1.comments)
}

fn with_thread(store: &dyn ArtifactStore, space: &str, id: &str, f: impl FnOnce(&mut Thread) -> Result<(), String>) -> anyhow::Result<()> {
    let path = path_of(store, space, id)?;
    let (file, mut thread) = thread_of(store, space, &path)?;
    f(&mut thread).map_err(anyhow::Error::msg)?;
    store.write_aside(space, &file, &comments::to_json(&thread))
}

pub fn comment_add(store: &dyn ArtifactStore, space: &str, id: &str, text: &str, anchor: Anchor, now: &str) -> anyhow::Result<Comment> {
    let path = path_of(store, space, id)?;
    let (file, mut thread) = thread_of(store, space, &path)?;
    let c = comments::add(&mut thread, text, anchor, now).map_err(anyhow::Error::msg)?;
    store.write_aside(space, &file, &comments::to_json(&thread))?;
    Ok(c)
}

pub fn comment_resolve(store: &dyn ArtifactStore, space: &str, id: &str, cid: &str, on: bool) -> anyhow::Result<()> {
    with_thread(store, space, id, |th| comments::resolve(th, cid, on))
}

pub fn comment_remove(store: &dyn ArtifactStore, space: &str, id: &str, cid: &str) -> anyhow::Result<()> {
    with_thread(store, space, id, |th| comments::remove(th, cid))
}

/// The comment became a message to a chat: the request text comes back
/// and the comment remembers where it went.
pub fn comment_send(store: &dyn ArtifactStore, space: &str, id: &str, cid: &str, session_id: Option<&str>, task: Option<&str>, now: &str) -> anyhow::Result<String> {
    let path = path_of(store, space, id)?;
    let tree = load(store, space, now)?;
    let title = tree.nodes.iter().find(|n| n.id == id).map(|n| n.title.clone()).unwrap_or_else(|| id.to_string());
    let (file, mut thread) = thread_of(store, space, &path)?;
    comments::sent(&mut thread, cid, session_id, task, now).map_err(anyhow::Error::msg)?;
    let c = thread.comments.iter().find(|c| c.id == cid).cloned().ok_or_else(|| anyhow::anyhow!("comentário \"{cid}\" não existe"))?;
    store.write_aside(space, &file, &comments::to_json(&thread))?;
    Ok(comments::request_text(&crate::domain::deep_link::page_link(space, id), &title, &c))
}

/// A chat wrote the page: the requests sent to it are answered. Returns
/// the ids answered; a page with no comments file costs one read.
pub fn comments_answered(store: &dyn ArtifactStore, space: &str, path: &str, session_id: Option<&str>, task: Option<&str>, now: &str) -> anyhow::Result<Vec<String>> {
    let (file, mut thread) = thread_of(store, space, path)?;
    let ids = comments::answered(&mut thread, session_id, task, now);
    if !ids.is_empty() {
        store.write_aside(space, &file, &comments::to_json(&thread))?;
    }
    Ok(ids)
}

/// A whiteboard page as the viewer shows it: laid out, with the notes
/// the agent changed since the baseline named.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BoardView {
    pub board: crate::domain::whiteboard::Board,
    pub changed: Vec<String>,
    pub versions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub raw: String,
}

pub fn board_view(store: &dyn ArtifactStore, space: &str, id: &str, now: &str) -> anyhow::Result<BoardView> {
    use crate::domain::whiteboard as wb;
    let path = path_of(store, space, id)?;
    let raw = store.read_page(space, &path)?;
    let base_text = baseline(store, space, id, now)?;
    let versions = store.list_versions(space, &path)?.len();
    match wb::parse(&raw) {
        Ok(board) => {
            let changed = wb::parse(&base_text).map(|b| wb::changed(&b, &board)).unwrap_or_default();
            Ok(BoardView { board: wb::lay_out(&board), changed, versions, error: None, raw })
        }
        Err(e) => Ok(BoardView { board: wb::Board::default(), changed: vec![], versions, error: Some(e), raw }),
    }
}

/// The person edited the board on the canvas: the file is rewritten,
/// laid out, and becomes the baseline — one's own edit is not reviewed.
pub fn board_edit(store: &dyn ArtifactStore, space: &str, id: &str, op: crate::domain::whiteboard::Op, now: &str) -> anyhow::Result<()> {
    use crate::domain::whiteboard as wb;
    let path = path_of(store, space, id)?;
    let mut board = wb::lay_out(&wb::parse(&store.read_page(space, &path)?).map_err(anyhow::Error::msg)?);
    wb::apply(&mut board, op).map_err(anyhow::Error::msg)?;
    store.write_page(space, &path, &wb::to_json(&board))?;
    let mut tree = load(store, space, now)?;
    if let Some(n) = tree.nodes.iter_mut().find(|n| n.id == id) {
        n.updated_at = now.to_string();
    }
    save(store, space, &tree)?;
    accept(store, space, id, now)
}

/// The selected notes as the text that goes to a chat.
pub fn board_selection(store: &dyn ArtifactStore, space: &str, id: &str, ids: &[String]) -> anyhow::Result<String> {
    use crate::domain::whiteboard as wb;
    let path = path_of(store, space, id)?;
    let board = wb::parse(&store.read_page(space, &path)?).map_err(anyhow::Error::msg)?;
    Ok(wb::selection_text(&board, ids))
}

/// A page's text as a chat should read it: a json page becomes its
/// summary (rows, notes, queries), any other page is its own text.
pub fn page_excerpt(kind: Kind, text: &str, today: &str) -> String {
    match kind {
        Kind::Sheet => crate::domain::sheet::parse(text).map(|s| crate::domain::sheet::summary(&s, today, 40)).unwrap_or_else(|e| format!("(planilha inválida: {e})\n{text}")),
        Kind::Board => crate::domain::whiteboard::parse(text).map(|b| crate::domain::whiteboard::summary(&b)).unwrap_or_else(|e| format!("(quadro inválido: {e})\n{text}")),
        Kind::Data => crate::domain::dash::parse(text).map(|d| crate::domain::dash::summary(&d)).unwrap_or_else(|e| format!("(painel inválido: {e})\n{text}")),
        Kind::Doc | Kind::Slides | Kind::Design => text.to_string(),
    }
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
    fn a_sheet_is_shown_computed_with_the_cells_the_agent_changed_marked() {
        use crate::domain::sheet::CellRef;
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Custos", Kind::Sheet, None, NOW).unwrap();
        s.write_page("hark", &page.path, r#"{"title":"Custos","tabs":[{"name":"Mensal","columns":["mês","infra"],"rows":[["set", 412],["out", "=B2*1.03"]]}]}"#).unwrap();
        // First sight: computed, nothing marked, the baseline taken.
        let view = sheet_view(&s, "hark", &page.id, NOW, "2026-10-28").unwrap();
        assert!(view.error.is_none());
        assert_eq!(view.computed.tabs[0].cells[1][1].text, "424.36");
        assert!(view.changed.is_empty());
        assert_eq!(view.versions, 1);
        // The agent changed a value and added a row: marked until accepted.
        s.write_page("hark", &page.path, r#"{"title":"Custos","tabs":[{"name":"Mensal","columns":["mês","infra"],"rows":[["set", 412],["out", 438],["nov", "=B3*1.03"]]}]}"#).unwrap();
        let view = sheet_view(&s, "hark", &page.id, "2026-10-28T10:05:00Z", "2026-10-28").unwrap();
        assert_eq!(view.changed, vec![CellRef { tab: 0, row: 1, col: 1 }, CellRef { tab: 0, row: 2, col: 0 }, CellRef { tab: 0, row: 2, col: 1 }]);
        accept(&s, "hark", &page.id, "2026-10-28T10:06:00Z").unwrap();
        assert!(sheet_view(&s, "hark", &page.id, "2026-10-28T10:07:00Z", "2026-10-28").unwrap().changed.is_empty());
        // A broken file is shown as an error with the raw text, never refused.
        s.write_page("hark", &page.path, "{ not json").unwrap();
        let view = sheet_view(&s, "hark", &page.id, "2026-10-28T10:08:00Z", "2026-10-28").unwrap();
        assert!(view.error.as_deref().unwrap_or("").contains("json"));
        assert_eq!(view.raw, "{ not json");
    }

    #[test]
    fn typing_into_a_sheet_cell_writes_the_file_and_needs_no_review() {
        use crate::domain::sheet::CellRef;
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Custos", Kind::Sheet, None, NOW).unwrap();
        sheet_view(&s, "hark", &page.id, NOW, "2026-10-28").unwrap();
        sheet_set_cell(&s, "hark", &page.id, CellRef { tab: 0, row: 0, col: 0 }, "=1+1", "2026-10-28T10:05:00Z").unwrap();
        let view = sheet_view(&s, "hark", &page.id, "2026-10-28T10:06:00Z", "2026-10-28").unwrap();
        assert_eq!(view.computed.tabs[0].cells[0][0].text, "2");
        assert!(view.changed.is_empty(), "your own edit is not something to review");
        assert!(sheet_set_cell(&s, "hark", &page.id, CellRef { tab: 9, row: 0, col: 0 }, "x", NOW).is_err());
    }

    #[test]
    fn a_structural_edit_on_a_sheet_is_one_accepted_version() {
        use crate::domain::sheet::Op;
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Custos", Kind::Sheet, None, NOW).unwrap();
        sheet_view(&s, "hark", &page.id, NOW, "2026-10-28").unwrap();
        sheet_edit(&s, "hark", &page.id, Op::InsertCol { tab: 0, at: 0 }, "2026-10-28T10:05:00Z").unwrap();
        sheet_edit(&s, "hark", &page.id, Op::SetFormat { tab: 0, col: 1, format: Some("money".into()) }, "2026-10-28T10:06:00Z").unwrap();
        let view = sheet_view(&s, "hark", &page.id, "2026-10-28T10:07:00Z", "2026-10-28").unwrap();
        assert_eq!(view.computed.tabs[0].columns[0], "A");
        assert_eq!(view.computed.tabs[0].formats[1].as_deref(), Some("money"));
        assert!(view.changed.is_empty(), "your own edit is not something to review");
        assert_eq!(view.versions, 3, "the baseline and one per edit");
        assert!(sheet_edit(&s, "hark", &page.id, Op::RemoveTab { tab: 0 }, NOW).is_err(), "the last tab stays");
    }

    #[test]
    fn a_comment_sits_beside_its_page_and_a_hark_request_is_answered_by_the_chat_that_wrote_it() {
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Custos", Kind::Sheet, None, NOW).unwrap();
        let c = comment_add(&s, "hark", &page.id, "@hark tira a linha do legado", Anchor::Cell { tab: 0, row: 1, col: 0, tab_name: None }, NOW).unwrap();
        assert!(s.read_aside("hark", "custos.comments.json").unwrap().is_some(), "the comments file sits beside the page");
        let text = comment_send(&s, "hark", &page.id, &c.id, Some("s-1"), Some("Billing"), "2026-10-28T10:05:00Z").unwrap();
        assert!(text.contains("hark://artifacts/hark/custos"), "{text}");
        assert!(text.contains("\"Custos\""));
        assert!(!text.contains("@hark"));
        // The page is written by that chat: the request is answered; another chat's write is not.
        assert!(comments_answered(&s, "hark", &page.path, Some("s-2"), Some("Outra"), "2026-10-28T10:06:00Z").unwrap().is_empty());
        assert_eq!(comments_answered(&s, "hark", &page.path, Some("s-1"), None, "2026-10-28T10:07:00Z").unwrap(), vec![c.id.clone()]);
        let list = comments_of(&s, "hark", &page.id).unwrap();
        assert_eq!(list[0].answered_at.as_deref(), Some("2026-10-28T10:07:00Z"));
        comment_resolve(&s, "hark", &page.id, &c.id, true).unwrap();
        comment_remove(&s, "hark", &page.id, &c.id).unwrap();
        assert!(comments_of(&s, "hark", &page.id).unwrap().is_empty());
        assert!(comment_remove(&s, "hark", &page.id, "c9").is_err());
    }

    /// A runner that answers from memory: sqlite and files by name, a
    /// command by its text. Counts the commands it ran.
    struct FakeRunner(std::cell::Cell<usize>);
    impl crate::ports::DataRunner for FakeRunner {
        fn sql(&self, _p: &std::path::Path, path: &str, sql: &str) -> anyhow::Result<crate::domain::dash::Table> {
            if path == "data/app.db" && sql.contains("lancamentos") {
                Ok(crate::domain::dash::parse_csv("dia,n\n01,3\n02,5\n"))
            } else {
                anyhow::bail!("no such table")
            }
        }
        fn file(&self, _p: &std::path::Path, path: &str) -> anyhow::Result<String> {
            if path == "data/ev.csv" { Ok("mes,d7\nset,38\nout,41\n".into()) } else { anyhow::bail!("{path}: not found") }
        }
        fn command(&self, _p: &std::path::Path, _c: &str) -> anyhow::Result<String> {
            self.0.set(self.0.get() + 1);
            Ok("n,antes\n1284,1209\n".into())
        }
    }

    const DASH: &str = r#"{ "title": "Uso", "refresh": "hourly",
      "sources": [ {"id": "db", "kind": "sqlite", "path": "data/app.db"}, {"id": "ev", "kind": "csv", "path": "data/ev.csv"}, {"id": "st", "kind": "command", "command": "./stats.sh"} ],
      "queries": [ {"id": "por_dia", "source": "db", "sql": "SELECT dia, n FROM lancamentos", "certified": true}, {"id": "ret", "source": "ev"}, {"id": "ativos", "source": "st"} ],
      "panels": [ {"title": "ativos", "query": "ativos", "chart": "number", "value": "n", "previous": "antes"}, {"title": "por dia", "query": "por_dia", "chart": "bar"}, {"title": "ret", "query": "ret", "chart": "line"} ] }"#;

    #[test]
    fn a_dashboard_runs_its_sources_caches_the_numbers_and_only_runs_a_command_on_request() {
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Uso", Kind::Data, None, NOW).unwrap();
        s.write_page("hark", &page.path, DASH).unwrap();
        let runner = FakeRunner(std::cell::Cell::new(0));
        let project = std::path::Path::new("/p/x");
        // First view: no cache, so files and sqlite run; the command waits for the person.
        let v = dash_view(&s, &runner, Some(project), "hark", &page.id, NOW, false).unwrap();
        assert!(v.error.is_none());
        assert_eq!(v.panels[1].values, vec![3.0, 5.0]);
        assert_eq!(v.panels[2].labels, vec!["set", "out"]);
        assert!(v.panels[0].error.as_deref().unwrap().contains("atualize"));
        assert_eq!(runner.0.get(), 0, "a command never runs on its own");
        assert_eq!(v.cache_at.as_deref(), Some(NOW));
        assert!(!v.stale);
        assert!(v.panels[1].certified && !v.panels[2].certified);
        assert!(s.read_aside("hark", "uso.dash.cache.json").unwrap().is_some(), "the cache sits beside the page");
        // "atualizar agora": everything runs, the command included.
        let v = dash_view(&s, &runner, Some(project), "hark", &page.id, "2026-10-28T10:10:00Z", true).unwrap();
        assert_eq!(runner.0.get(), 1);
        assert_eq!(v.panels[0].number, Some(1284.0));
        assert_eq!(v.queries[2].preview.as_ref().unwrap().rows.len(), 1);
        // Within the hour the cache answers; nothing runs again.
        dash_view(&s, &runner, Some(project), "hark", &page.id, "2026-10-28T10:20:00Z", false).unwrap();
        assert_eq!(runner.0.get(), 1);
        // A space without a project cannot read sources, and says so per query.
        let v = dash_view(&s, &runner, None, "hark", &page.id, "2026-10-28T12:00:00Z", true).unwrap();
        assert!(v.queries[0].error.as_deref().unwrap().contains("projeto"));
    }

    #[test]
    fn the_queries_the_agent_changed_are_marked_until_certified_or_accepted() {
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Uso", Kind::Data, None, NOW).unwrap();
        s.write_page("hark", &page.path, DASH).unwrap();
        let runner = FakeRunner(std::cell::Cell::new(0));
        let project = std::path::Path::new("/p/x");
        let v = dash_view(&s, &runner, Some(project), "hark", &page.id, NOW, false).unwrap();
        assert!(v.changed.is_empty(), "the first sight is the baseline");
        let edited = DASH.replace("SELECT dia, n FROM lancamentos", "SELECT dia, n FROM lancamentos WHERE n > 1");
        s.write_page("hark", &page.path, &edited).unwrap();
        let v = dash_view(&s, &runner, Some(project), "hark", &page.id, "2026-10-28T10:05:00Z", false).unwrap();
        assert_eq!(v.changed, vec!["por_dia"]);
        assert!(v.queries[0].changed);
        dash_certify(&s, "hark", &page.id, "ret", true, "2026-10-28T10:06:00Z").unwrap();
        dash_edit(&s, "hark", &page.id, crate::domain::dash::Op::Refresh { refresh: crate::domain::dash::Refresh::Daily }, "2026-10-28T10:06:30Z").unwrap();
        assert!(dash_edit(&s, "hark", &page.id, crate::domain::dash::Op::RemoveSource { id: "zzz".into() }, NOW).is_err());
        let v = dash_view(&s, &runner, Some(project), "hark", &page.id, "2026-10-28T10:07:00Z", false).unwrap();
        assert!(v.queries[1].certified);
        assert!(v.changed.is_empty(), "certifying is the person's edit: the baseline moves");
        assert!(dash_certify(&s, "hark", &page.id, "zzz", true, NOW).is_err());
        // A broken file: the error and the raw text, never a blank.
        s.write_page("hark", &page.path, "{ nope").unwrap();
        let v = dash_view(&s, &runner, Some(project), "hark", &page.id, "2026-10-28T10:08:00Z", false).unwrap();
        assert!(v.error.as_deref().unwrap().contains("json"));
    }

    #[test]
    fn a_whiteboard_is_laid_out_edited_on_the_canvas_and_read_as_text() {
        use crate::domain::whiteboard::Op;
        let (_d, s) = store();
        let page = create(&s, "hark", None, "Plano", Kind::Board, None, NOW).unwrap();
        s.write_page("hark", &page.path, r#"{"title":"Plano","groups":[{"id":"g1","title":"Quick-add"}],"notes":[{"id":"n1","text":"Atalho","group":"g1","owner":"Ana"},{"id":"n2","text":"Solta"}]}"#).unwrap();
        let v = board_view(&s, "hark", &page.id, NOW).unwrap();
        assert!(v.error.is_none());
        assert!(v.board.notes[0].x.is_some(), "laid out for the canvas");
        assert!(v.changed.is_empty());
        // The agent added a note: marked until accepted.
        s.write_page("hark", &page.path, r#"{"title":"Plano","groups":[{"id":"g1","title":"Quick-add"}],"notes":[{"id":"n1","text":"Atalho","group":"g1","owner":"Ana"},{"id":"n2","text":"Solta"},{"id":"n3","text":"Nova"}]}"#).unwrap();
        let v = board_view(&s, "hark", &page.id, "2026-10-28T10:05:00Z").unwrap();
        assert_eq!(v.changed, vec!["n3"]);
        // The person drags a note: the file is rewritten and it is no longer "changed".
        board_edit(&s, "hark", &page.id, Op::Move { id: "n3".into(), x: 500.0, y: 600.0 }, "2026-10-28T10:06:00Z").unwrap();
        let v = board_view(&s, "hark", &page.id, "2026-10-28T10:07:00Z").unwrap();
        assert!(v.changed.is_empty());
        assert_eq!(v.board.notes[2].x, Some(500.0));
        assert!(board_edit(&s, "hark", &page.id, Op::Remove { id: "zz".into() }, NOW).is_err());
        assert_eq!(board_selection(&s, "hark", &page.id, &["n1".into()]).unwrap(), "- [Quick-add] Atalho (Ana)\n");
        assert!(page_excerpt(Kind::Board, &s.read_page("hark", &page.path).unwrap(), "2026-10-28").starts_with("# Plano\n\n## Quick-add\n- Atalho (Ana)"));
        assert_eq!(page_excerpt(Kind::Doc, "# x", "2026-10-28"), "# x");
        assert!(page_excerpt(Kind::Sheet, "{ nope", "2026-10-28").contains("inválida"));
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
