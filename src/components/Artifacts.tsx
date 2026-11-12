import { useEffect, useMemo, useRef, useState } from "react";
import { BarChart3, ChevronDown, ChevronLeft, ChevronRight, Code2, Copy, Download, ExternalLink, FileText, FolderOpen, Link2, Maximize2, MessageSquare, Minimize2, MoreVertical, Palette, Plus, Presentation, Search, Share2, ShieldCheck, ShieldOff, Sparkles, StickyNote, Table2, User } from "lucide-react";
import * as ipc from "../lib/ipc";
import { STAGE_H, STAGE_W, ZOOMS, applyTextEdit, canvasSize, deckDocument, editableDocument, fitZoom, hasOwnScript, slideCount } from "../lib/artifactView";
import { t } from "../lib/i18n";
import { childrenOf, crumbOf, kindCounts, recents, rows } from "../lib/artifacts";
import FileViewer from "./FileViewer";
import SharePopover, { type ShareRow } from "./SharePopover";
import SheetView from "./SheetView";
import DashView from "./DashView";
import BoardView from "./BoardView";
import Markdown from "./Markdown";
import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { pageHtml } from "../lib/export";
import { ARTIFACT_KINDS, type ArtifactKind, type ArtifactNode, type ArtifactPage, type ArtifactSpace, type ArtifactTree, type CommentAnchor, type PageComment } from "../types";
import { destinations, type Destination, type RecentChat } from "../lib/takeTo";
import Comments from "./Comments";
import { ThumbBoard, ThumbDash, ThumbDoc, ThumbSheet } from "./Thumbs";
import MarkLayer, { MarkBar, type Mark, type MarkTool, type RegionAnchor } from "./MarkLayer";
import { sameAnchor } from "../lib/comments";
import { KIND_ICON } from "./kindIcon";

/** An html page that brought no style of its own gets the reading base:
 *  system font, a column, dark and light. A page with its own `<style>`
 *  or stylesheet is left exactly as the author made it. */
const BASE_STYLE = `<style>
  :root { color-scheme: light dark; }
  body { max-width: 820px; margin: 40px auto; padding: 0 24px; font: 15px/1.6 -apple-system, "Segoe UI", sans-serif; color: #1f2430; background: #fff; }
  h1 { font-size: 28px; letter-spacing: -.01em; margin: 0 0 8px; } h2 { font-size: 20px; margin-top: 28px; } h3 { font-size: 16px; }
  p, li { max-width: 72ch; } table { border-collapse: collapse; } td, th { border: 1px solid #ddd; padding: 6px 10px; text-align: left; }
  pre { background: #f4f5f8; padding: 12px; border-radius: 8px; overflow: auto; font-size: 13px; }
  code { font-family: ui-monospace, Menlo, monospace; font-size: .92em; }
  a { color: #2f5fd1; }
  @media (prefers-color-scheme: dark) { body { background: #12151c; color: #d6dae3; } pre { background: #1b1f29; } td, th { border-color: #333; } a { color: #7aa2f7; } }
</style>`;
function withBaseStyle(html: string): string {
  return /<style[\s>]|<link[^>]+stylesheet/i.test(html) ? html : BASE_STYLE + html;
}

/** What the artifacts view points at: a space, and a page in it or its home. */
export type ArtSel = { space: string; id: string | null };

const OPEN_KEY = "hark.artifacts.open";
const KIND_LABEL = { doc: "art_kind_doc", slides: "art_kind_slides", design: "art_kind_design", sheet: "art_kind_sheet", board: "art_kind_board", data: "art_kind_data" } as const;
const kindLabel = (k: ArtifactKind) => t(KIND_LABEL[k]);
/** The html kinds a person may re-label by hand; a json kind is its file's. */
const RELABEL_KINDS: ArtifactKind[] = ["doc", "slides", "design"];

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
  onTakeTo,
  onSendTo,
  recentChats = [],
  onBack,
}: {
  sel: ArtSel | null;
  onSel: (sel: ArtSel | null) => void;
  /** Present when the view was expanded from a chat's pane: a way back. */
  onBack?: () => void;
  /** The project colour of a space (by its project path). */
  colourOf: (path?: string) => string | undefined;
  /** Bumped when an agent wrote a page: the tree re-reads. */
  tick: number;
  /** The page on screen, for the composer's scope and the rail. */
  onPage: (page: ArtifactPage | null, tree: ArtifactTree | null) => void;
  onOpenTask: (title: string, sessionId?: string) => void;
  linksOn: boolean;
  onToggleLinks: () => void;
  /** "Conversar": open the page's chat with the page attached. */
  /** "Levar para…": the page goes to Hark, its origin chat, another chat
   *  or a new one — the person picks, never a silent default. */
  onTakeTo: (page: ArtifactPage, space: string, dest: Destination, extra?: string) => void;
  /** A @hark comment's request goes to a chat as a message (fase 14.3). */
  onSendTo: (dest: Destination, text: string) => Promise<void> | void;
  /** The chats the menu offers besides the origin, newest first. */
  recentChats?: RecentChat[];
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
        // A deck or a design is built with its own script more often than
        // not (a pager, a tab strip): it runs, still inside the sandbox
        // with no origin. A document opens with scripts off.
        setScripts(p.node.kind !== "doc" && p.node.path.endsWith(".html"));
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
  async function setKind(space: string, id: string, kind: ArtifactKind) {
    setMenu(null);
    try {
      await ipc.artifactSetKind(space, id, kind);
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
        {onBack && (
          <button className="art-back" onClick={onBack} title={t("art_back_chat_hint")}>
            <ChevronLeft size={12} /> {t("art_back_chat")}
          </button>
        )}
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
                      className={`art-node ${on ? "on" : ""}`}
                      style={{ paddingLeft: 6 + depth * 14 }}
                      draggable
                      onDragStart={(e) => {
                        dragId.current = node.id;
                        // Dropped on a chat in the sidebar, the page's link
                        // lands in that chat's composer.
                        e.dataTransfer.setData("text/hark-artifact", `hark://artifacts/${encodeURIComponent(sp.name)}/${encodeURIComponent(node.id)}`);
                      }}
                      onDragOver={(e) => dragId.current && dragId.current !== node.id && e.preventDefault()}
                      onDrop={(e) => {
                        e.preventDefault();
                        const id = dragId.current;
                        dragId.current = null;
                        if (id && id !== node.id) void moveTo(sp.name, id, node.id);
                      }}
                      onClick={() => onSel({ space: sp.name, id: node.id })}
                      title={node.path}
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
                      <Icon size={13} className={`art-kind ${node.kind}`} />
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
                          {node.path.endsWith(".html") &&
                            RELABEL_KINDS
                              .filter((k) => k !== node.kind)
                              .map((k) => (
                                <button key={k} onClick={() => void setKind(sp.name, node.id, k)}>
                                  {t("art_mark_as", { k: kindLabel(k) })}
                                </button>
                              ))}
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
            onTakeTo={(dest, extra) => onTakeTo(page, sel.space, dest, extra)}
            onSendTo={onSendTo}
            recentChats={recentChats}
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
        {ARTIFACT_KINDS.map((k) => {
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
  const [filter, setFilter] = useState<ArtifactKind | null>(null);
  const counts = useMemo(() => kindCounts(tree), [tree]);
  const recent = useMemo(() => (filter ? recents(tree, 200, filter) : recents(tree, 12)), [tree, filter]);
  const name = space?.project ? space.name : t("art_space_personal");
  const path = `~/.hark/artifacts/${space?.name ?? ""}`;
  const kindsLabel = (k: ArtifactKind) => t(`art_kinds_${k}`);
  return (
    <div className="art-home">
      <div className="art-head">
        <div>
          <h1 className="art-h1">{t("art_title")}</h1>
          <div className="art-sub dim" title={path}>
            {space?.project ? <span className="art-dot" style={colour ? { background: colour } : undefined} /> : <User size={11} />} {name} · {t("art_pages_n", { n: tree.nodes.length })}
          </div>
        </div>
      </div>
      <div className="art-tabs">
        <button className={`art-tab ${filter === null ? "on" : ""}`} onClick={() => setFilter(null)}>
          {t("art_tab_all")}
          {filter === null && <span className="art-tab-n">{tree.nodes.length}</span>}
        </button>
        {ARTIFACT_KINDS.filter((k) => counts[k]).map((k) => (
          <button key={k} className={`art-tab ${filter === k ? "on" : ""}`} onClick={() => setFilter(filter === k ? null : k)}>
            {kindsLabel(k)}
            {filter === k && <span className="art-tab-n">{counts[k]}</span>}
          </button>
        ))}
      </div>
      {!filter && (
        <div className="art-create">
          <div className="dim">{t("art_create_new")}</div>
          <div className="art-tiles">
            {ARTIFACT_KINDS.map((k) => {
              const Icon = KIND_ICON[k];
              return (
                <button key={k} className="art-tile" onClick={() => onNew(k)} title={t("art_new_page")}>
                  <span className={`art-tile-pic ${k}`}>
                    <Icon size={24} className={`art-kind ${k}`} strokeWidth={1.7} />
                  </span>
                  <span>{kindsLabel(k)}</span>
                </button>
              );
            })}
          </div>
        </div>
      )}
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
              <button key={n.id} className="art-card" onClick={() => onOpen(n.id)} title={crumb.length ? `${crumb.join(" › ")} · ${n.path}` : n.path}>
                <Thumb space={space?.name ?? ""} node={n} />
                <div className="art-card-foot">
                  <div className="art-card-text">
                    <span className="art-card-title">{n.title}</span>
                    <span className="dim art-card-meta">
                      <Icon size={11} className={`art-kind ${n.kind}`} /> {crumb.length ? `${crumb[crumb.length - 1]} · ` : ""}{ago(n.updated_at)}
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

/**
 * Hark's deck viewer: the 1280×720 stage scaled to the room, Hark's
 * pager under it (arrows, n/N, a filmstrip, present), the arrow keys.
 * The page only brings the slides; a deck that brought its own pager
 * keeps it and gets the stage alone (lib/artifactView).
 */
function DeckViewer({
  title,
  html,
  scripts,
  onStripPager,
  frameRef,
  overlay,
  onSlide,
}: {
  title: string;
  html: string;
  scripts: boolean;
  /** Take the page's own pager out of its file (Hark's does the job). */
  onStripPager: () => Promise<unknown>;
  /** The page's frame, for whoever needs to talk to it (marks probe it). */
  frameRef?: React.RefObject<HTMLIFrameElement>;
  /** Drawn over the stage, in its pixels (the markup layer). */
  overlay?: ReactNode;
  /** The slide on screen changed. */
  onSlide?: (n: number) => void;
}) {
  const own = hasOwnScript(html);
  const doc = useMemo(() => (scripts ? deckDocument(html) : html), [html, scripts]);
  const count = useMemo(() => slideCount(html), [html]);
  const [at, setAt] = useState(0);
  const [strip, setStrip] = useState(false);
  const [present, setPresent] = useState(false);
  const ownFrame = useRef<HTMLIFrameElement>(null!);
  const frame = frameRef ?? ownFrame;
  const go = (n: number) => frame.current?.contentWindow?.postMessage({ hark: "slide", n }, "*");
  // The frame says where it is (and how many it has); Esc leaves present.
  useEffect(() => {
    const onMsg = (e: MessageEvent) => {
      const m = e.data as { hark?: string; at?: number } | null;
      if (!m || e.source !== frame.current?.contentWindow) return;
      if (m.hark === "slide" && typeof m.at === "number") {
        setAt(m.at);
        onSlide?.(m.at);
      }
      if (m.hark === "escape") setPresent(false);
    };
    window.addEventListener("message", onMsg);
    return () => window.removeEventListener("message", onMsg);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  useEffect(() => {
    if (!present) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPresent(false);
      if (e.key === "ArrowRight") go(at + 1);
      if (e.key === "ArrowLeft") go(at - 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [present, at]);
  const navigable = scripts && count > 0;
  return (
    <div className={`art-deck ${present ? "present" : ""}`}>
      <Stage title={title} html={doc} scripts={scripts} frameRef={frame} overlay={overlay} />
      <div className="art-deck-bar">
        {navigable ? (
          <>
            <button className="frame-ctl" title={t("deck_prev")} onClick={() => go(at - 1)} disabled={at <= 0}>
              <ChevronLeft size={13} />
            </button>
            <span className="mono">
              {at + 1} / {count}
            </span>
            <button className="frame-ctl" title={t("deck_next")} onClick={() => go(at + 1)} disabled={at >= count - 1}>
              <ChevronRight size={13} />
            </button>
            <button className={`pill-btn ${strip ? "on" : ""}`} onClick={() => setStrip((s) => !s)}>
              {t("deck_strip")}
            </button>
          </>
        ) : (
          <span className="dim">{scripts ? t("deck_no_slides") : t("deck_scripts_off")}</span>
        )}
        {own && (
          <button className="pill-btn" title={t("deck_strip_file_hint")} onClick={() => void onStripPager().catch(() => {})}>
            {t("deck_strip_file")}
          </button>
        )}
        <span className="grow" />
        <button className="pill-btn" onClick={() => setPresent((p) => !p)} title={t("deck_present_hint")}>
          {present ? <Minimize2 size={11} /> : <Maximize2 size={11} />} {present ? t("deck_leave") : t("deck_present")}
        </button>
      </div>
      {strip && navigable && (
        <div className="art-strip">
          {Array.from({ length: count }, (_, i) => (
            <button key={i} className={`art-strip-slide ${i === at ? "on" : ""}`} onClick={() => go(i)} title={`${i + 1}`}>
              <iframe
                className="art-strip-frame"
                title={`${title} ${i + 1}`}
                sandbox="allow-scripts"
                srcDoc={doc}
                tabIndex={-1}
                onLoad={(e) => e.currentTarget.contentWindow?.postMessage({ hark: "slide", n: i }, "*")}
              />
              <span>{i + 1}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** The 1280×720 stage scaled to the room it has, centred. */
function Stage({
  title,
  html,
  scripts,
  frameRef,
  overlay,
}: {
  title: string;
  html: string;
  scripts: boolean;
  frameRef?: React.RefObject<HTMLIFrameElement>;
  overlay?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const fit = () => {
      const r = el.getBoundingClientRect();
      setScale(fitZoom(r.width, r.height, STAGE_W, STAGE_H));
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <div className="art-stage" ref={ref}>
      <div className="art-stage-box" style={{ width: STAGE_W, height: STAGE_H, transform: `translate(-50%, -50%) scale(${scale})` }}>
        <iframe ref={frameRef} className="art-stage-frame" title={title} sandbox={scripts ? "allow-scripts" : ""} srcDoc={html} style={{ width: STAGE_W, height: STAGE_H }} />
        {overlay}
      </div>
    </div>
  );
}

/**
 * Hark's canvas for a design: the page at its own size (meta hark-canvas,
 * 1440×900 by default) on a dotted board, zoomed from a pill (fit, 25%…
 * 200%) or Cmd+wheel, panned by dragging the board. The page is only
 * the artwork.
 */
function CanvasViewer({ title, html, scripts, frameRef, overlay }: { title: string; html: string; scripts: boolean; frameRef?: React.RefObject<HTMLIFrameElement>; overlay?: (size: { w: number; h: number }) => ReactNode }) {
  const size = useMemo(() => canvasSize(html), [html]);
  const ref = useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = useState<number | null>(null); // null = fit
  const [room, setRoom] = useState({ w: 0, h: 0 });
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      setRoom({ w: r.width, h: r.height });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const z = zoom ?? fitZoom(room.w - 24, room.h - 24, size.w, size.h);
  return (
    <div className="art-canvas">
      <div className="art-canvas-bar">
        <span className="mono dim">
          {size.w}×{size.h}
        </span>
        <span className="grow" />
        <button className={`pill-btn ${zoom == null ? "on" : ""}`} onClick={() => { setZoom(null); setPan({ x: 0, y: 0 }); }}>
          {t("canvas_fit")}
        </button>
        {ZOOMS.map((v) => (
          <button key={v} className={`pill-btn ${zoom === v ? "on" : ""}`} onClick={() => setZoom(v)}>
            {Math.round(v * 100)}%
          </button>
        ))}
      </div>
      <div
        className="art-canvas-board"
        ref={ref}
        onMouseDown={(e) => {
          if ((e.target as HTMLElement).closest?.(".mark-layer.drawing")) return;
          drag.current = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y };
        }}
        onMouseMove={(e) => {
          const d = drag.current;
          if (!d) return;
          setPan({ x: d.px + (e.clientX - d.x), y: d.py + (e.clientY - d.y) });
        }}
        onMouseUp={() => {
          drag.current = null;
        }}
        onMouseLeave={() => {
          drag.current = null;
        }}
        onWheel={(e) => {
          if (!e.metaKey && !e.ctrlKey) return;
          e.preventDefault();
          const next = Math.min(4, Math.max(0.1, z * (e.deltaY < 0 ? 1.1 : 0.9)));
          setZoom(Number(next.toFixed(2)));
        }}
      >
        <div className="art-canvas-sheet" style={{ width: size.w, height: size.h, transform: `translate(calc(-50% + ${pan.x}px), calc(-50% + ${pan.y}px)) scale(${z})` }}>
          <iframe ref={frameRef} className="art-canvas-frame" title={title} sandbox={scripts ? "allow-scripts" : ""} srcDoc={html} style={{ width: size.w, height: size.h }} />
          {overlay?.(size)}
        </div>
      </div>
    </div>
  );
}

/**
 * A card's picture is the page itself, at scale (round "miniaturas"):
 * an html deck through Hark's deck runtime on its first slide, a design
 * in its frame, and the json kinds and documents drawn by Hark from
 * the file the card already reads. Nothing is computed for a card.
 */
function Thumb({ space, node }: { space: string; node: ArtifactNode }) {
  const [text, setText] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    ipc
      .artifactRead(space, node.id)
      .then((p) => {
        if (live) setText(p.text);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [space, node.id, node.updated_at]);
  const html = node.path.endsWith(".html");
  return (
    <div className={`art-thumb ${node.kind}`}>
      {text == null ? null : html && node.kind === "slides" ? (
        <iframe
          className="art-thumb-frame"
          title={node.title}
          sandbox="allow-scripts"
          srcDoc={deckDocument(withBaseStyle(text))}
          tabIndex={-1}
          onLoad={(e) => e.currentTarget.contentWindow?.postMessage({ hark: "slide", n: 0 }, "*")}
        />
      ) : html ? (
        <iframe className="art-thumb-frame" title={node.title} sandbox="allow-scripts" srcDoc={withBaseStyle(text)} tabIndex={-1} />
      ) : node.kind === "sheet" ? (
        <ThumbSheet text={text} />
      ) : node.kind === "board" ? (
        <ThumbBoard text={text} />
      ) : node.kind === "data" ? (
        <ThumbDash text={text} />
      ) : (
        <ThumbDoc text={text} title={node.title} />
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
  onTakeTo,
  onSendTo,
  recentChats,
  compact = false,
}: {
  page: ArtifactPage;
  tree: ArtifactTree;
  space: string;
  /** In the rail beside a chat: the crumb is the title, no links column,
   *  no child-making — the full view is one click away. */
  compact?: boolean;
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
  onTakeTo: (dest: Destination, extra?: string) => void;
  onSendTo: (dest: Destination, text: string) => Promise<void> | void;
  recentChats: RecentChat[];
}) {
  const { node } = page;
  const html = /\.html?$/i.test(node.path);
  const Icon = KIND_ICON[node.kind];
  const kids = childrenOf(tree, node.id);
  const [shareOpen, setShareOpen] = useState(false);
  // Comments (fase 14.3): the column, what is selected as the anchor,
  // and the comments themselves, read from beside the page.
  const [commenting, setCommenting] = useState(false);
  const [comments, setComments] = useState<PageComment[]>([]);
  const [anchor, setAnchor] = useState<CommentAnchor>({ kind: "page" });
  const loadComments = () => ipc.artifactComments(space, node.id).then(setComments).catch(() => setComments([]));
  useEffect(() => {
    void loadComments();
    setAnchor({ kind: "page" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [space, node.id, page.text]);
  const marks = comments.filter((c) => !c.resolved).map((c) => c.anchor);
  const onAnchor = (a: CommentAnchor | null) => setAnchor(a ?? { kind: "page" });
  // Marks over an html page (fase 14.3b): the tool, the slide on screen,
  // the frame to ask what sits under a mark, and a nudge to the bubble.
  const [tool, setTool] = useState<MarkTool>("select");
  const [slide, setSlide] = useState(0);
  const [focusNonce, setFocusNonce] = useState(0);
  const frameRef = useRef<HTMLIFrameElement>(null!);
  const regionMarks: Mark[] = comments
    .filter((c) => !c.resolved && c.anchor.kind === "region")
    .map((c) => ({ id: c.id, anchor: c.anchor as RegionAnchor, n: comments.indexOf(c) + 1 }));
  const probe = (x: number, y: number) =>
    new Promise<string>((resolve) => {
      const win = frameRef.current?.contentWindow;
      if (!win) return resolve("");
      const id = `p${Date.now()}`;
      const done = (text: string) => {
        window.removeEventListener("message", onMsg);
        resolve(text);
      };
      const onMsg = (e: MessageEvent) => {
        const m = e.data as { hark?: string; id?: string; text?: unknown } | null;
        if (m?.hark === "probe" && m.id === id) done(typeof m.text === "string" ? m.text : "");
      };
      window.addEventListener("message", onMsg);
      win.postMessage({ hark: "probe", x, y, id }, "*");
      setTimeout(() => done(""), 400);
    });
  const onMark = async (a: RegionAnchor) => {
    const over = await probe(a.x + a.w / 2, a.y + a.h / 2);
    setAnchor({ ...a, over: over || null });
    setFocusNonce((n) => n + 1);
  };
  const markLayer = (w: number, h: number) =>
    commenting ? (
      <MarkLayer w={w} h={h} tool={tool} marks={regionMarks} slide={node.kind === "slides" ? slide : null} onMark={(a) => void onMark(a)} onPick={(m) => setAnchor(m.anchor)} pending={anchor.kind === "region" ? anchor : null} />
    ) : null;
  useEffect(() => {
    if (!commenting) setTool("select");
  }, [commenting]);
  useEffect(() => {
    if (!commenting || tool === "select") return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setTool("select");
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [commenting, tool]);
  // A passage selected in a rendered document (or quoted by a frame).
  useEffect(() => {
    if (!commenting) return;
    const onUp = () => {
      const s = window.getSelection();
      const text = s?.toString().trim() ?? "";
      if (text && s?.anchorNode && document.querySelector(".art-body")?.contains(s.anchorNode)) setAnchor({ kind: "quote", text: text.slice(0, 500) });
    };
    const onMessage = (e: MessageEvent) => {
      const m = e.data as { hark?: string; text?: unknown } | null;
      if (m?.hark === "quote" && typeof m.text === "string" && m.text.trim()) setAnchor({ kind: "quote", text: m.text.trim() });
    };
    document.addEventListener("mouseup", onUp);
    window.addEventListener("message", onMessage);
    return () => {
      document.removeEventListener("mouseup", onUp);
      window.removeEventListener("message", onMessage);
    };
  }, [commenting]);
  // In-place edits (fase 14.2): a frame says which words changed; the
  // change is placed in the file by the old text, or refused.
  const [inPlace, setInPlace] = useState<string | null>(null);
  const pageRef = useRef(page);
  pageRef.current = page;
  useEffect(() => {
    if (!html || source) return;
    const onMessage = (e: MessageEvent) => {
      const m = e.data as { hark?: string; before?: unknown; after?: unknown; inner?: { before?: unknown; after?: unknown } } | null;
      if (!m || m.hark !== "edit" || typeof m.before !== "string" || typeof m.after !== "string") return;
      const candidates = [{ before: m.before, after: m.after }];
      if (typeof m.inner?.before === "string" && typeof m.inner?.after === "string") candidates.push({ before: m.inner.before, after: m.inner.after });
      const next = applyTextEdit(pageRef.current.text, candidates);
      if (next == null) {
        setInPlace(t("art_inplace_unplaced"));
        return;
      }
      ipc
        .artifactWrite(space, node.id, next)
        .then(() => {
          setInPlace(t("art_inplace_saved"));
          onSaved();
        })
        .catch((err) => setInPlace(String(err)));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [html, source, space, node.id]);
  useEffect(() => {
    if (!inPlace) return;
    const id = setTimeout(() => setInPlace(null), 4000);
    return () => clearTimeout(id);
  }, [inPlace]);
  // The menu may open with an excerpt (a board's selection) to carry along.
  const [take, setTake] = useState<{ open: boolean; extra?: string }>({ open: false });
  const takeOpen = take.open;
  const setTakeOpen = (open: boolean, extra?: string) => setTake({ open, extra });
  const link = `hark://artifacts/${encodeURIComponent(space)}/${encodeURIComponent(node.id)}`;
  const takeRows: ShareRow[] = destinations(node.origin?.task ?? undefined, recentChats, space).map((dest) => {
    const row = (icon: ReactNode, label: string, hint: string): ShareRow => ({
      icon,
      label,
      hint,
      run: () => {
        const extra = take.extra;
        setTakeOpen(false);
        onTakeTo(dest, extra);
      },
    });
    switch (dest.kind) {
      case "hark":
        return row(<Sparkles size={13} />, t("art_take_hark"), t("art_take_hark_hint"));
      case "origin":
        return row(<MessageSquare size={13} />, t("art_take_origin"), dest.title);
      case "chat":
        return row(<MessageSquare size={13} />, dest.title, t("art_take_other_hint"));
      case "new":
        return row(<Plus size={13} />, t("art_take_new", { name: dest.project }), t("art_take_new_hint"));
    }
  });
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
          {!compact && crumbs.map((c, i) => (
            <span key={i} className="art-crumb-seg">
              <ChevronRight size={10} /> {c}
            </span>
          ))}
          <span className="art-crumb-seg">
            <ChevronRight size={10} /> <b>{node.title}</b>
          </span>
        </div>
        <span className="art-actions">
          {(html || node.kind === "sheet" || node.kind === "data" || node.kind === "board") && (
            <>
              <button className={`pill-btn ${source ? "on" : ""}`} onClick={onSource} title={t("art_source_hint")}>
                <Code2 size={11} /> {t("art_source")}
              </button>
              {!source && html && (
                <button className={`pill-btn ${scripts ? "warn" : ""}`} onClick={onScripts} title={t("art_scripts_hint")}>
                  {scripts ? <ShieldOff size={11} /> : <ShieldCheck size={11} />} {scripts ? t("art_scripts_on") : t("art_scripts_off")}
                </button>
              )}
            </>
          )}
          <button className={`pill-btn ${commenting ? "on" : ""}`} onClick={() => setCommenting((v) => !v)} title={t("cm_btn_hint")}>
            <MessageSquare size={11} /> {t("cm_btn")}
            {comments.some((c) => !c.resolved) && <span className="cm-count">{comments.filter((c) => !c.resolved).length}</span>}
          </button>
          <span className="share-anchor">
            <button className={`pill-btn ${takeOpen ? "on" : ""}`} onClick={() => setTakeOpen(!takeOpen)} title={t("art_take_to_hint")}>
              <MessageSquare size={11} /> {t("art_take_to")}
            </button>
            {takeOpen && <SharePopover title={t("art_take_title")} rows={takeRows} onClose={() => setTakeOpen(false)} soon={false} />}
          </span>
          {!compact && (
            <button className={`pill-btn ${linksOn ? "on" : ""}`} onClick={onToggleLinks} title={t("art_links_hint")}>
              <Link2 size={11} /> {t("art_links")}
            </button>
          )}
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
        <Icon size={12} className={`art-kind ${node.kind}`} /> {kindLabel(node.kind)} · {t("art_edited", { when: ago(node.updated_at) })}
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
        {html && !source && (
          <>
            {" · "}
            <span className={inPlace ? "art-inplace on" : "art-inplace dim"}>{inPlace ?? (scripts ? t("art_inplace_hint") : t("art_inplace_needs_scripts"))}</span>
          </>
        )}
      </div>
      <div className={`art-main ${commenting ? "with-comments" : ""}`}>
      <div className="art-body">
        {commenting && html && !source && <MarkBar tool={tool} onTool={setTool} onClose={() => setCommenting(false)} />}
        {commenting && html && !source && tool !== "select" && <div className="mark-on dim">{t("mk_on")}</div>}
        {node.kind === "sheet" && !source ? (
          <SheetView space={space} id={node.id} onChanged={onSaved} onAnchor={onAnchor} marks={marks} />
        ) : node.kind === "data" && !source ? (
          <DashView space={space} id={node.id} onChanged={onSaved} onAsk={(text) => setTakeOpen(true, text)} onDoc={(pid) => { onSaved(); onOpen(pid); }} onAnchor={onAnchor} marks={marks} />
        ) : node.kind === "board" && !source ? (
          <BoardView space={space} id={node.id} onChanged={onSaved} onAsk={(text) => setTakeOpen(true, text)} onAnchor={onAnchor} marks={marks} />
        ) : html && !source && node.kind === "slides" ? (
          <DeckViewer title={node.title} html={page.text} scripts={scripts} onStripPager={() => ipc.artifactStripPager(space, node.id).then(onSaved)} frameRef={frameRef} overlay={markLayer(STAGE_W, STAGE_H)} onSlide={setSlide} />
        ) : html && !source && node.kind === "design" ? (
          <CanvasViewer title={node.title} html={scripts ? editableDocument(withBaseStyle(page.text)) : withBaseStyle(page.text)} scripts={scripts} frameRef={frameRef} overlay={(size) => markLayer(size.w, size.h)} />
        ) : html && !source ? (
          <div className="art-frame-wrap">
            <iframe
              ref={frameRef}
              className="art-frame"
              title={node.title}
              sandbox={scripts ? "allow-scripts" : ""}
              srcDoc={scripts ? editableDocument(withBaseStyle(page.text)) : withBaseStyle(page.text)}
            />
            {commenting && <div className="art-frame-marks">{markLayer(1200, 1600)}</div>}
          </div>
        ) : (
          <FileViewer file={{ abs: page.abs, rel: node.path, project: { name: space, path: "" } }} io={io} />
        )}
      </div>
      {commenting && (
        <Comments
          space={space}
          id={node.id}
          comments={comments}
          anchor={anchor}
          destinations={destinations(node.origin?.task ?? undefined, recentChats, space)}
          onChanged={() => void loadComments()}
          onSend={onSendTo}
          onPick={(c) => setAnchor(comments.some((x) => sameAnchor(x.anchor, c.anchor)) ? c.anchor : anchor)}
          focusNonce={focusNonce}
        />
      )}
      </div>
      {(!compact || kids.length > 0) && (
      <div className="art-children">
        <span className="dim">{t("art_children")}</span>
        {kids.map((k) => {
          const KIcon = KIND_ICON[k.kind];
          return (
            <button key={k.id} className="pill-btn" onClick={() => onOpen(k.id)}>
              <KIcon size={11} className={`art-kind ${k.kind}`} /> {k.title}
            </button>
          );
        })}
        {!compact && (
          <button className="pill-btn" onClick={onNewChild}>
            <Plus size={11} /> {t("art_new_child")}
          </button>
        )}
      </div>
      )}
    </div>
  );
}

/**
 * One page in the rail beside a chat (fase 14.4), the way Claude Code's
 * app shows an artifact next to the terminal and the files: the page
 * alone, no tree. It reads its space's index and the page itself, and
 * re-reads when the agent writes (`tick`). Expanding is the frame's job:
 * it opens the full artifacts view with a way back to this chat.
 */
export function ArtifactPane({
  sel,
  onSel,
  tick,
  colourOf,
  onOpenTask,
  onTakeTo,
  onSendTo,
  recentChats = [],
}: {
  sel: { space: string; id: string };
  /** A page named inside the page (a child, a linked doc) replaces it. */
  onSel: (sel: { space: string; id: string }) => void;
  tick: number;
  colourOf: (path?: string) => string | undefined;
  onOpenTask: (title: string, sessionId?: string) => void;
  onTakeTo: (page: ArtifactPage, space: string, dest: Destination, extra?: string) => void;
  onSendTo: (dest: Destination, text: string) => Promise<void> | void;
  recentChats?: RecentChat[];
}) {
  const [tree, setTree] = useState<ArtifactTree | null>(null);
  const [page, setPage] = useState<ArtifactPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [source, setSource] = useState(false);
  const [scripts, setScripts] = useState(false);
  const [spaces, setSpaces] = useState<ArtifactSpace[]>([]);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    ipc.artifactsSpaces()
      .then(setSpaces)
      .catch(() => {});
  }, []);

  useEffect(() => {
    let live = true;
    Promise.all([ipc.artifactsTree(sel.space), ipc.artifactRead(sel.space, sel.id)])
      .then(([tr, p]) => {
        if (!live) return;
        setTree(tr);
        setPage(p);
        setError(null);
      })
      .catch((err) => {
        if (!live) return;
        setError(String(err));
      });
    return () => {
      live = false;
    };
  }, [sel.space, sel.id, tick, reload]);

  useEffect(() => {
    setSource(false);
    setScripts(page ? page.node.kind !== "doc" && page.node.path.endsWith(".html") : false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel.id, page?.node.id]);

  const colour = colourOf(spaces.find((s) => s.name === sel.space)?.project ?? undefined);
  return (
    <div className="art-pane">
      {error && <div className="art-error">{error}</div>}
      {page && tree ? (
        <Page
          compact
          page={page}
          tree={tree}
          space={sel.space}
          colour={colour}
          source={source}
          scripts={scripts}
          linksOn={false}
          onSource={() => setSource((s) => !s)}
          onScripts={() => setScripts((s) => !s)}
          onToggleLinks={() => {}}
          onOpen={(id) => onSel({ space: sel.space, id })}
          onOpenTask={onOpenTask}
          onNewChild={() => {}}
          onSaved={() => setReload((n) => n + 1)}
          onTakeTo={(dest, extra) => onTakeTo(page, sel.space, dest, extra)}
          onSendTo={onSendTo}
          recentChats={recentChats}
        />
      ) : (
        !error && <div className="art-empty dim">…</div>
      )}
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
