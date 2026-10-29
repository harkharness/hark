import { useEffect, useMemo, useRef, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Code2,
  ExternalLink,
  FileText,
  Link2,
  MoreVertical,
  Palette,
  Plus,
  Presentation,
  Search,
  Share2,
  Copy,
  Download,
  FolderOpen,
  ShieldCheck,
  ShieldOff,
  User,
} from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import { childrenOf, crumbOf, recents, rows } from "../lib/artifacts";
import FileViewer from "./FileViewer";
import SharePopover, { type ShareRow } from "./SharePopover";
import Markdown from "./Markdown";
import { renderToStaticMarkup } from "react-dom/server";
import { pageHtml } from "../lib/export";
import type { ArtifactKind, ArtifactNode, ArtifactPage, ArtifactSpace, ArtifactTree } from "../types";

/** What the artifacts view points at: a space, and a page in it or its home. */
export type ArtSel = { space: string; id: string | null };

const OPEN_KEY = "hark.artifacts.open";
const KIND_ICON: Record<ArtifactKind, typeof FileText> = { doc: FileText, slides: Presentation, design: Palette };
const kindLabel = (k: ArtifactKind) => t(k === "doc" ? "art_kind_doc" : k === "slides" ? "art_kind_slides" : "art_kind_design");

/** "há 5 min", "ontem": the touch as the home card prints it. */
export function ago(iso: string, now = Date.now()): string {
  const ms = now - Date.parse(iso);
  if (!Number.isFinite(ms) || ms < 0) return "";
  const mins = Math.round(ms / 60000);
  if (mins < 2) return t("art_ago_now");
  if (mins < 60) return t("art_ago_min", { m: mins });
  const h = Math.round(mins / 60);
  if (h < 24) return t("art_ago_h", { h });
  const d = Math.round(h / 24);
  return d === 1 ? t("art_ago_yesterday") : t("art_ago_d", { d });
}

function readOpen(): Set<string> {
  try {
    const raw = window.localStorage.getItem(OPEN_KEY);
    return new Set(raw ? (JSON.parse(raw) as string[]) : []);
  } catch {
    return new Set();
  }
}

/**
 * The documentation of this machine: one tree column (every space, every
 * page, in the order the index keeps), the home of a space (what moved
 * last) and the open page. Everything local, zero tokens — the agent
 * writes pages through `HARK_ARTIFACTS_DIR`, this screen only reads the
 * index back (docs/ARTIFACTS.md).
 */
export default function Artifacts({
  sel,
  onSel,
  colourOf,
  tick,
  onPage,
  onOpenTask,
  linksOn,
  onToggleLinks,
}: {
  sel: ArtSel | null;
  onSel: (sel: ArtSel | null) => void;
  /** The project colour of a space (by its project path). */
  colourOf: (path?: string) => string | undefined;
  /** Bumped when an agent wrote a page: the tree re-reads. */
  tick: number;
  /** The page on screen, for the composer's scope and the rail. */
  onPage: (page: ArtifactPage | null, tree: ArtifactTree | null) => void;
  onOpenTask: (title: string, sessionId?: string) => void;
  linksOn: boolean;
  onToggleLinks: () => void;
}) {
  const [spaces, setSpaces] = useState<ArtifactSpace[]>([]);
  const [trees, setTrees] = useState<Record<string, ArtifactTree>>({});
  const [open, setOpen] = useState<Set<string>>(readOpen);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState<ArtifactPage | null>(null);
  const [source, setSource] = useState(false);
  const [scripts, setScripts] = useState(false);
  const [menu, setMenu] = useState<string | null>(null);
  const [naming, setNaming] = useState<{ parent: string | null; kind: ArtifactKind } | null>(null);
  const [renaming, setRenaming] = useState<{ id: string; title: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const dragId = useRef<string | null>(null);

  async function load() {
    try {
      const list = await ipc.artifactsSpaces();
      setSpaces(list);
      const pairs = await Promise.all(list.map(async (s) => [s.name, await ipc.artifactsTree(s.name)] as const));
      setTrees(Object.fromEntries(pairs));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }
  useEffect(() => {
    void load();
  }, [tick]);

  // Land on the first space when nothing is selected yet.
  useEffect(() => {
    if (!sel && spaces.length > 0) onSel({ space: spaces[0].name, id: null });
  }, [sel, spaces, onSel]);

  useEffect(() => {
    window.localStorage.setItem(OPEN_KEY, JSON.stringify([...open]));
  }, [open]);

  const tree = sel ? (trees[sel.space] ?? null) : null;

  // The page follows the selection; a write by the agent re-reads it.
  useEffect(() => {
    if (!sel?.id) {
      setPage(null);
      onPage(null, tree);
      return;
    }
    let live = true;
    ipc
      .artifactRead(sel.space, sel.id)
      .then((p) => {
        if (!live) return;
        setPage(p);
        onPage(p, tree);
      })
      .catch((err) => {
        if (!live) return;
        setError(String(err));
        onSel({ space: sel.space, id: null });
      });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel?.space, sel?.id, tick, tree]);

  useEffect(() => {
    setSource(false);
    setScripts(false);
  }, [sel?.id]);

  const toggleOpen = (id: string) =>
    setOpen((o) => {
      const next = new Set(o);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  async function create(space: string, parent: string | null, kind: ArtifactKind, title: string) {
    const clean = title.trim();
    if (!clean) return;
    try {
      const node = await ipc.artifactCreate(space, parent, clean, kind);
      if (parent) setOpen((o) => new Set(o).add(parent));
      await load();
      onSel({ space, id: node.id });
    } catch (err) {
      setError(String(err));
    } finally {
      setNaming(null);
    }
  }
  async function rename(space: string, id: string, title: string) {
    const clean = title.trim();
    setRenaming(null);
    if (!clean) return;
    try {
      await ipc.artifactRename(space, id, clean);
      await load();
    } catch (err) {
      setError(String(err));
    }
  }
  async function remove(space: string, id: string) {
    setMenu(null);
    try {
      await ipc.artifactRemove(space, id);
      if (sel?.id === id) onSel({ space, id: null });
      await load();
    } catch (err) {
      setError(String(err));
    }
  }
  async function moveTo(space: string, id: string, parent: string | null) {
    const at = childrenOf(trees[space], parent).length;
    try {
      await ipc.artifactMove(space, id, parent, at);
      if (parent) setOpen((o) => new Set(o).add(parent));
      await load();
    } catch (err) {
      setError(String(err));
    }
  }

  const spaceOf = (name: string) => spaces.find((s) => s.name === name);
  const spaceColour = (name: string) => colourOf(spaceOf(name)?.project ?? undefined);

  return (
    <div className="artifacts" onClick={() => menu && setMenu(null)}>
      <aside className="art-tree">
        <div className="art-tree-top">
          <label className="art-search">
            <Search size={12} />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("art_search")} spellCheck={false} />
          </label>
          <button
            className="art-ibtn"
            title={t("art_new_page")}
            disabled={!sel}
            onClick={() => sel && setNaming({ parent: null, kind: "doc" })}
          >
            <Plus size={13} />
          </button>
        </div>
        {spaces.map((sp) => {
          const tr = trees[sp.name];
          const colour = spaceColour(sp.name);
          const onHome = sel?.space === sp.name && !sel.id;
          return (
            <section key={sp.name} className="art-space">
              <button
                className={`art-space-head ${onHome ? "on" : ""}`}
                onClick={() => onSel({ space: sp.name, id: null })}
                onDragOver={(e) => dragId.current && e.preventDefault()}
                onDrop={(e) => {
                  e.preventDefault();
                  const id = dragId.current;
                  dragId.current = null;
                  if (id && sel?.space === sp.name) void moveTo(sp.name, id, null);
                }}
                title={sp.project ?? t("art_space_personal_hint")}
              >
                {sp.project ? (
                  <span className="art-dot" style={colour ? { background: colour } : undefined} />
                ) : (
                  <User size={12} />
                )}
                <span className="art-space-name">{sp.project ? sp.name : t("art_space_personal")}</span>
                <span className="dim">{tr?.nodes.length ?? sp.pages}</span>
              </button>
              {tr &&
                rows(tr, open, query).map(({ node, depth, hasChildren }) => {
                  const Icon = KIND_ICON[node.kind];
                  const on = sel?.space === sp.name && sel.id === node.id;
                  return (
                    <div
                      key={node.id}
                      className={`art-node ${on ? "on" : ""} ${node.orphan ? "orphan" : ""}`}
                      style={{ paddingLeft: 6 + depth * 14 }}
                      draggable
                      onDragStart={() => {
                        dragId.current = node.id;
                      }}
                      onDragOver={(e) => dragId.current && dragId.current !== node.id && e.preventDefault()}
                      onDrop={(e) => {
                        e.preventDefault();
                        const id = dragId.current;
                        dragId.current = null;
                        if (id && id !== node.id) void moveTo(sp.name, id, node.id);
                      }}
                      onClick={() => onSel({ space: sp.name, id: node.id })}
                      title={node.orphan ? t("art_orphan") : node.path}
                    >
                      <button
                        className="art-caret"
                        style={{ visibility: hasChildren && !query ? "visible" : "hidden" }}
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleOpen(node.id);
                        }}
                      >
                        {open.has(node.id) ? <ChevronDown size={11} /> : <ChevronRight size={11} />}
                      </button>
                      <Icon size={13} className="art-kind" />
                      {renaming?.id === node.id ? (
                        <input
                          className="art-inline"
                          autoFocus
                          value={renaming.title}
                          onClick={(e) => e.stopPropagation()}
                          onChange={(e) => setRenaming({ id: node.id, title: e.target.value })}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") void rename(sp.name, node.id, renaming.title);
                            if (e.key === "Escape") setRenaming(null);
                          }}
                          onBlur={() => void rename(sp.name, node.id, renaming.title)}
                        />
                      ) : (
                        <span className="art-title">{node.title}</span>
                      )}
                      <button
                        className="art-node-btn"
                        title={t("art_new_child")}
                        onClick={(e) => {
                          e.stopPropagation();
                          setNaming({ parent: node.id, kind: "doc" });
                          if (sel?.space !== sp.name) onSel({ space: sp.name, id: node.id });
                        }}
                      >
                        <Plus size={11} />
                      </button>
                      <button
                        className="art-node-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          setMenu(menu === node.id ? null : node.id);
                        }}
                      >
                        <MoreVertical size={11} />
                      </button>
                      {menu === node.id && (
                        <div className="side-menu art-menu" onClick={(e) => e.stopPropagation()}>
                          <button
                            onClick={() => {
                              setMenu(null);
                              setRenaming({ id: node.id, title: node.title });
                            }}
                          >
                            {t("side_rename")}
                          </button>
                          <button
                            onClick={() => {
                              setMenu(null);
                              setNaming({ parent: node.id, kind: "doc" });
                            }}
                          >
                            {t("art_new_child")}
                          </button>
                          <button className="danger" onClick={() => void remove(sp.name, node.id)}>
                            {t("art_remove")}
                          </button>
                        </div>
                      )}
                    </div>
                  );
                })}
            </section>
          );
        })}
        <div className="art-tree-foot">
          <span className="dim">{t("art_drag_hint")}</span>
        </div>
      </aside>

      <main className="art-main">
        {error && <div className="art-error">{error}</div>}
        {naming && sel && (
          <NameBox
            kind={naming.kind}
            parentTitle={naming.parent && tree ? crumbOf(tree, naming.parent).at(-1) : undefined}
            onKind={(kind) => setNaming({ ...naming, kind })}
            onCancel={() => setNaming(null)}
            onCreate={(title) => void create(sel.space, naming.parent, naming.kind, title)}
          />
        )}
        {!sel || !tree ? (
          <div className="art-empty">{t("art_none_space")}</div>
        ) : !sel.id ? (
          <Home
            space={spaceOf(sel.space)}
            tree={tree}
            colour={spaceColour(sel.space)}
            onOpen={(id) => onSel({ space: sel.space, id })}
            onNew={(kind) => setNaming({ parent: null, kind })}
          />
        ) : page ? (
          <Page
            page={page}
            tree={tree}
            space={sel.space}
            colour={spaceColour(sel.space)}
            source={source}
            scripts={scripts}
            linksOn={linksOn}
            onSource={() => setSource((s) => !s)}
            onScripts={() => setScripts((s) => !s)}
            onToggleLinks={onToggleLinks}
            onOpen={(id) => onSel({ space: sel.space, id })}
            onOpenTask={onOpenTask}
            onNewChild={() => setNaming({ parent: page.node.id, kind: "doc" })}
            onSaved={() => void load()}
          />
        ) : (
          <div className="art-empty dim">…</div>
        )}
      </main>
    </div>
  );
}

/** The one-line form a new page is named in: title, kind, enter. */
function NameBox({
  kind,
  parentTitle,
  onKind,
  onCancel,
  onCreate,
}: {
  kind: ArtifactKind;
  parentTitle?: string;
  onKind: (k: ArtifactKind) => void;
  onCancel: () => void;
  onCreate: (title: string) => void;
}) {
  const [title, setTitle] = useState("");
  return (
    <div className="art-namebox">
      <span className="dim">{parentTitle ? t("art_new_under", { p: parentTitle }) : t("art_new_root")}</span>
      <input
        autoFocus
        value={title}
        placeholder={t("art_title_ph")}
        onChange={(e) => setTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onCreate(title);
          if (e.key === "Escape") onCancel();
        }}
      />
      <span className="art-kinds">
        {(["doc", "slides", "design"] as ArtifactKind[]).map((k) => {
          const Icon = KIND_ICON[k];
          return (
            <button key={k} className={`pill-btn ${kind === k ? "on" : ""}`} onClick={() => onKind(k)} title={kindLabel(k)}>
              <Icon size={11} /> {kindLabel(k)}
            </button>
          );
        })}
      </span>
      <button className="pill-btn" onClick={() => onCreate(title)} disabled={!title.trim()}>
        {t("art_create")}
      </button>
      <button className="pill-btn" onClick={onCancel}>
        {t("m_cancel")}
      </button>
    </div>
  );
}

/** The home of a space: what moved last. The tree is the documentation;
 *  this only shows the freshest touches. */
function Home({
  space,
  tree,
  colour,
  onOpen,
  onNew,
}: {
  space?: ArtifactSpace;
  tree: ArtifactTree;
  colour?: string;
  onOpen: (id: string) => void;
  onNew: (kind: ArtifactKind) => void;
}) {
  const recent = useMemo(() => recents(tree, 12), [tree]);
  const name = space?.project ? space.name : t("art_space_personal");
  return (
    <div className="art-home">
      <div className="art-crumb">
        {space?.project ? <span className="art-dot" style={colour ? { background: colour } : undefined} /> : <User size={12} />}
        <b>{name}</b>
        <span className="dim">
          · {space?.project ? t("art_space_project") : t("art_space_personal_hint")} · {t("art_pages_n", { n: tree.nodes.length })} ·{" "}
          <span className="mono">~/.hark/artifacts/{space?.name}</span>
        </span>
      </div>
      <div className="art-home-bar">
        <span className="art-home-title">{t("art_recent")}</span>
        <span className="art-kinds">
          {(["doc", "slides", "design"] as ArtifactKind[]).map((k) => {
            const Icon = KIND_ICON[k];
            return (
              <button key={k} className="pill-btn" onClick={() => onNew(k)}>
                <Plus size={11} /> {kindLabel(k)}
              </button>
            );
          })}
        </span>
      </div>
      {recent.length === 0 ? (
        <div className="art-empty">
          <FileText size={22} strokeWidth={1.6} />
          <div>{t("art_none_pages")}</div>
          <div className="dim">{t("art_none_pages_hint")}</div>
        </div>
      ) : (
        <div className="art-grid">
          {recent.map((n) => {
            const Icon = KIND_ICON[n.kind];
            const crumb = crumbOf(tree, n.id).slice(0, -1);
            return (
              <button key={n.id} className="art-card" onClick={() => onOpen(n.id)} title={n.path}>
                <div className={`art-thumb ${n.kind}`}>
                  <div className="art-thumb-sheet">
                    <b>{n.title}</b>
                  </div>
                </div>
                <div className="art-card-foot">
                  <Icon size={14} className="art-kind" />
                  <div className="art-card-text">
                    <span className="art-card-title">{n.title}</span>
                    <span className="dim">
                      {crumb.length ? crumb.join(" › ") : t("art_root")} · {ago(n.updated_at)}
                    </span>
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** The open page: breadcrumb, meta with its origin, body, children. A
 *  markdown page reads and edits through the files viewer; an html page
 *  renders in a sandboxed frame (no scripts unless asked) and edits as
 *  source. */
function Page({
  page,
  tree,
  space,
  colour,
  source,
  scripts,
  linksOn,
  onSource,
  onScripts,
  onToggleLinks,
  onOpen,
  onOpenTask,
  onNewChild,
  onSaved,
}: {
  page: ArtifactPage;
  tree: ArtifactTree;
  space: string;
  colour?: string;
  source: boolean;
  scripts: boolean;
  linksOn: boolean;
  onSource: () => void;
  onScripts: () => void;
  onToggleLinks: () => void;
  onOpen: (id: string) => void;
  onOpenTask: (title: string, sessionId?: string) => void;
  onNewChild: () => void;
  onSaved: () => void;
}) {
  const { node } = page;
  const html = /\.html?$/i.test(node.path);
  const Icon = KIND_ICON[node.kind];
  const kids = childrenOf(tree, node.id);
  const [shareOpen, setShareOpen] = useState(false);
  const link = `hark://artifacts/${encodeURIComponent(space)}/${encodeURIComponent(node.id)}`;
  const shareRows: ShareRow[] = [
    {
      icon: <Copy size={13} />,
      label: t("share_copy_link"),
      hint: link,
      run: () => navigator.clipboard.writeText(link).then(() => t("share_link_copied")),
    },
    {
      icon: <FileText size={13} />,
      label: html ? t("share_copy_source") : t("share_copy_md"),
      hint: t("share_copy_md_hint"),
      run: () => navigator.clipboard.writeText(page.text).then(() => t("share_copied")),
    },
    {
      icon: <Download size={13} />,
      label: t("share_export_html"),
      hint: t("share_export_html_hint"),
      run: async () => {
        const body = html ? page.text : pageHtml(node.title, renderToStaticMarkup(<div className="md"><Markdown>{page.text}</Markdown></div>));
        const abs = await ipc.exportFile(`${node.title}.html`, body);
        await ipc.revealPath(abs).catch(() => {});
        return t("share_exported");
      },
    },
    {
      icon: <FolderOpen size={13} />,
      label: t("share_reveal"),
      hint: page.abs.replace(/^\/Users\/[^/]+/, "~"),
      run: () => ipc.revealPath(page.abs).then(() => undefined),
    },
  ];
  const crumbs = page.breadcrumb.slice(0, -1);
  const io = useMemo(
    () => ({
      read: () => ipc.artifactRead(space, node.id).then((p) => ({ content: p.text, truncated: false })),
      save: (text: string) => ipc.artifactWrite(space, node.id, text).then(onSaved),
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [space, node.id],
  );
  return (
    <div className="art-page">
      <div className="art-page-head">
        <div className="art-crumb">
          <span className="art-dot" style={colour ? { background: colour } : undefined} />
          <b>{space}</b>
          {crumbs.map((c, i) => (
            <span key={i} className="art-crumb-seg">
              <ChevronRight size={10} /> {c}
            </span>
          ))}
          <span className="art-crumb-seg">
            <ChevronRight size={10} /> <b>{node.title}</b>
          </span>
        </div>
        <span className="art-actions">
          {html && (
            <>
              <button className={`pill-btn ${source ? "on" : ""}`} onClick={onSource} title={t("art_source_hint")}>
                <Code2 size={11} /> {t("art_source")}
              </button>
              {!source && (
                <button className={`pill-btn ${scripts ? "warn" : ""}`} onClick={onScripts} title={t("art_scripts_hint")}>
                  {scripts ? <ShieldOff size={11} /> : <ShieldCheck size={11} />} {scripts ? t("art_scripts_on") : t("art_scripts_off")}
                </button>
              )}
            </>
          )}
          <button className={`pill-btn ${linksOn ? "on" : ""}`} onClick={onToggleLinks} title={t("art_links_hint")}>
            <Link2 size={11} /> {t("art_links")}
          </button>
          <span className="share-anchor">
            <button className={`pill-btn ${shareOpen ? "on" : ""}`} onClick={() => setShareOpen((o) => !o)} title={t("share_title")}>
              <Share2 size={11} /> {t("share_btn")}
            </button>
            {shareOpen && <SharePopover title={node.title} rows={shareRows} onClose={() => setShareOpen(false)} />}
          </span>
          <button className="pill-btn" onClick={() => void ipc.openExternal(page.abs).catch(() => {})} title={page.abs}>
            <ExternalLink size={11} />
          </button>
        </span>
      </div>
      <div className="art-meta">
        <Icon size={12} className="art-kind" /> {kindLabel(node.kind)} · {t("art_edited", { when: ago(node.updated_at) })}
        {node.origin?.task && (
          <>
            {" · "}
            <button className="art-origin" onClick={() => onOpenTask(node.origin!.task!, node.origin?.session_id ?? undefined)} title={t("art_origin_hint")}>
              {t("art_born_in")} <span className="art-dot" style={colour ? { background: colour } : undefined} /> {node.origin.task}
            </button>
          </>
        )}
        {" · "}
        <span className="mono">{node.path}</span>
        {node.orphan && <span className="pill-kind edited"> {t("art_orphan")}</span>}
      </div>
      <div className="art-body">
        {html && !source ? (
          <iframe
            className="art-frame"
            title={node.title}
            sandbox={scripts ? "allow-scripts" : ""}
            srcDoc={page.text}
          />
        ) : (
          <FileViewer file={{ abs: page.abs, rel: node.path, project: { name: space, path: "" } }} io={io} />
        )}
      </div>
      <div className="art-children">
        <span className="dim">{t("art_children")}</span>
        {kids.map((k) => {
          const KIcon = KIND_ICON[k.kind];
          return (
            <button key={k.id} className="pill-btn" onClick={() => onOpen(k.id)}>
              <KIcon size={11} className="art-kind" /> {k.title}
            </button>
          );
        })}
        <button className="pill-btn" onClick={onNewChild}>
          <Plus size={11} /> {t("art_new_child")}
        </button>
      </div>
    </div>
  );
}

/** The rail panel beside a page: what it is, what it is tied to, what it
 *  costs to carry. Zero tokens: all of it is the index. */
export function ArtifactLinks({
  page,
  tree,
  onOpen,
  onOpenTask,
}: {
  page: ArtifactPage | null;
  tree: ArtifactTree | null;
  onOpen: (id: string) => void;
  onOpenTask: (title: string, sessionId?: string) => void;
}) {
  if (!page || !tree) return <div className="session-panel"><p className="hint">{t("art_links_none")}</p></div>;
  const { node } = page;
  const Icon = KIND_ICON[node.kind];
  const kids = childrenOf(tree, node.id);
  const links = node.links ?? [];
  const nodeById = (id: string): ArtifactNode | undefined => tree.nodes.find((n) => n.id === id);
  return (
    <div className="session-panel">
      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("art_this_page")}</span>
        </div>
        <div className="sp-row">
          <Icon size={13} />
          <span>
            {kindLabel(node.kind)} · {/\.html?$/i.test(node.path) ? "html" : "markdown"}
          </span>
        </div>
        <div className="sp-row">
          <span className="sp-ico" />
          <span>{page.breadcrumb.join(" › ")}</span>
        </div>
        <div className="sp-row">
          <span className="sp-ico" />
          <span className="mono dim">{page.abs.replace(/^\/Users\/[^/]+/, "~")}</span>
        </div>
      </section>
      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("art_links")}</span>
          <b>{(node.origin?.task ? 1 : 0) + links.length + kids.length}</b>
        </div>
        {node.origin?.task && (
          <button className="sp-out" onClick={() => onOpenTask(node.origin!.task!, node.origin?.session_id ?? undefined)}>
            <span className="art-dot ok" />
            <span className="sp-out-name">{node.origin.task}</span>
            <span className="dim">{t("art_link_origin")}</span>
          </button>
        )}
        {links.map((l, i) => {
          const target = l.node ? nodeById(l.node) : undefined;
          return target ? (
            <button key={i} className="sp-out" onClick={() => onOpen(target.id)}>
              <FileText size={13} />
              <span className="sp-out-name">{target.title}</span>
              <span className="dim">{l.kind}</span>
            </button>
          ) : l.task ? (
            <button key={i} className="sp-out" onClick={() => onOpenTask(l.task!, l.session_id ?? undefined)}>
              <span className="art-dot warn" />
              <span className="sp-out-name">{l.task}</span>
              <span className="dim">{l.kind}</span>
            </button>
          ) : null;
        })}
        {kids.map((k) => (
          <button key={k.id} className="sp-out" onClick={() => onOpen(k.id)}>
            <FileText size={13} />
            <span className="sp-out-name">{k.title}</span>
            <span className="dim">{t("art_link_child")}</span>
          </button>
        ))}
        {!node.origin?.task && links.length === 0 && kids.length === 0 && <p className="hint">{t("art_links_empty")}</p>}
      </section>
      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("art_in_context")}</span>
        </div>
        <p className="hint">{t("art_in_context_hint", { n: Math.round(page.text.length / 4 / 100) / 10 })}</p>
      </section>
    </div>
  );
}
