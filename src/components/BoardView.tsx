import { useCallback, useEffect, useRef, useState } from "react";
import { Check, FileText, History, Link2, ListChecks, MessageSquare, Plus, Presentation, Sparkles, Trash2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { BoardLink, BoardMilestone, BoardNote, BoardOp, BoardView as View, CommentAnchor } from "../types";

const NOTE_W = 150;
const NOTE_H = 96;
const COLORS = ["yellow", "blue", "green", "pink", "gray"];

/** The notes whose frame lies inside a rectangle (mirror of whiteboard::within). */
export function within(notes: BoardNote[], x1: number, y1: number, x2: number, y2: number): string[] {
  const [l, r] = [Math.min(x1, x2), Math.max(x1, x2)];
  const [tp, b] = [Math.min(y1, y2), Math.max(y1, y2)];
  return notes.filter((n) => (n.x ?? 0) >= l && (n.x ?? 0) + NOTE_W <= r && (n.y ?? 0) >= tp && (n.y ?? 0) + NOTE_H <= b).map((n) => n.id);
}

/** Where an arrow between two note frames starts and ends: centre to
 *  centre, clipped to each frame's edge so the head is visible. */
export function arrow(a: { x: number; y: number }, b: { x: number; y: number }): { x1: number; y1: number; x2: number; y2: number } {
  const c1 = { x: a.x + NOTE_W / 2, y: a.y + NOTE_H / 2 };
  const c2 = { x: b.x + NOTE_W / 2, y: b.y + NOTE_H / 2 };
  const clip = (from: { x: number; y: number }, to: { x: number; y: number }) => {
    const dx = to.x - from.x;
    const dy = to.y - from.y;
    if (dx === 0 && dy === 0) return from;
    const sx = Math.abs(dx) > 0 ? NOTE_W / 2 / Math.abs(dx) : Infinity;
    const sy = Math.abs(dy) > 0 ? NOTE_H / 2 / Math.abs(dy) : Infinity;
    const s = Math.min(sx, sy);
    return { x: from.x + dx * s, y: from.y + dy * s };
  };
  const p1 = clip(c1, c2);
  const p2 = clip(c2, c1);
  return { x1: p1.x, y1: p1.y, x2: p2.x, y2: p2.y };
}

type Drag =
  | { kind: "note"; id: string; dx: number; dy: number; x: number; y: number; moved: boolean }
  | { kind: "group"; id: string; dx: number; dy: number; x: number; y: number; moved: boolean }
  | { kind: "resize"; id: string; x: number; y: number; w: number; h: number; moved: boolean }
  | { kind: "lasso"; x1: number; y1: number; x2: number; y2: number };

type MilestoneForm = { index: number | null; date: string; title: string; owner: string };

/**
 * The whiteboard page: notes on a canvas inside their groups, dragged
 * into place (dropping inside a group joins it), edited on a double
 * click, coloured, added and removed; arrows between notes, made by
 * "link" and a click on the target, labelled on a click; groups
 * retitled on a double click and resized by their corner; the timeline
 * edited through a small form. A lasso on the background selects notes
 * and "ask about the selection" takes them to a chat as text. The notes
 * the agent changed since you last accepted wear a mark.
 */
export default function BoardView({
  space,
  id,
  onChanged,
  onAsk,
  onAnchor,
  marks = [],
}: {
  space: string;
  id: string;
  onChanged?: () => void;
  onAsk?: (text: string) => void;
  /** The one selected note, as a comment's anchor (fase 14.3). */
  onAnchor?: (a: CommentAnchor | null) => void;
  /** Notes that carry an open comment: marked. */
  marks?: CommentAnchor[];
}) {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sel, setSel] = useState<string[]>([]);
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [linking, setLinking] = useState<string | null>(null);
  const [linkSel, setLinkSel] = useState<{ from: string; to: string; label: string } | null>(null);
  const [groupTitle, setGroupTitle] = useState<{ id: string; value: string } | null>(null);
  const [milestone, setMilestone] = useState<MilestoneForm | null>(null);
  const canvas = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    try {
      setView(await ipc.boardView(space, id));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [space, id]);
  useEffect(() => {
    void load();
  }, [load]);
  useEffect(() => {
    onAnchor?.(sel.length === 1 ? { kind: "note", id: sel[0] } : null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel.join(",")]);
  useEffect(() => {
    if (!linking) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setLinking(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [linking]);

  async function edit(op: BoardOp) {
    try {
      await ipc.boardEdit(space, id, op);
      await load();
      onChanged?.();
    } catch (err) {
      setError(String(err));
    }
  }
  async function accept() {
    await ipc.artifactAccept(space, id);
    await load();
    onChanged?.();
  }
  async function ask() {
    if (!sel.length) return;
    const text = await ipc.boardSelection(space, id, sel);
    onAsk?.(text.trimEnd());
  }

  /** Canvas coordinates of a pointer event. */
  const at = (e: { clientX: number; clientY: number }) => {
    const el = canvas.current;
    if (!el) return { x: 0, y: 0 };
    const r = el.getBoundingClientRect();
    return { x: e.clientX - r.left + el.scrollLeft, y: e.clientY - r.top + el.scrollTop };
  };

  function onMove(e: React.MouseEvent) {
    if (!drag) return;
    const p = at(e);
    if (drag.kind === "lasso") setDrag({ ...drag, x2: p.x, y2: p.y });
    else if (drag.kind === "resize") setDrag({ ...drag, w: Math.max(NOTE_W + 28, p.x - drag.x), h: Math.max(NOTE_H + 28, p.y - drag.y), moved: true });
    else setDrag({ ...drag, x: p.x - drag.dx, y: p.y - drag.dy, moved: true });
  }
  function onUp() {
    if (!drag) return;
    setDrag(null);
    if (drag.kind === "lasso") {
      setSel(within(view?.board.notes ?? [], drag.x1, drag.y1, drag.x2, drag.y2));
    } else if (drag.kind === "resize") {
      if (drag.moved) void edit({ op: "resize_group", id: drag.id, w: Math.round(drag.w), h: Math.round(drag.h) });
    } else if (drag.moved) {
      const x = Math.max(0, Math.round(drag.x));
      const y = Math.max(0, Math.round(drag.y));
      void edit(drag.kind === "note" ? { op: "move", id: drag.id, x, y } : { op: "move_group", id: drag.id, x, y });
    }
  }

  /** A click on a note: the link target while linking, else the selection. */
  function pick(n: BoardNote, e: React.MouseEvent) {
    if (linking) {
      const from = linking;
      setLinking(null);
      if (from !== n.id) void edit({ op: "link", from, to: n.id });
      return;
    }
    if (drag && drag.kind !== "lasso" && drag.moved) return;
    setSel(e.shiftKey ? (sel.includes(n.id) ? sel.filter((s) => s !== n.id) : [...sel, n.id]) : [n.id]);
  }

  function saveMilestone(m: MilestoneForm) {
    const owner = m.owner.trim() || null;
    setMilestone(null);
    void edit(m.index == null ? { op: "add_milestone", date: m.date, title: m.title, owner } : { op: "edit_milestone", index: m.index, date: m.date, title: m.title, owner });
  }

  if (error) return <div className="board-error">{t("board_open_err", { e: error })}</div>;
  if (!view) return <div className="board-loading dim">…</div>;
  if (view.error) {
    return (
      <div className="board-broken">
        <div className="board-error">{t("board_file_err", { e: view.error })}</div>
        <pre className="mono">{view.raw}</pre>
      </div>
    );
  }
  const { board } = view;
  const pos = (n: { id: string; x?: number | null; y?: number | null }, kind: "note" | "group") =>
    drag && drag.kind === kind && drag.id === n.id ? { x: drag.x, y: drag.y } : { x: n.x ?? 0, y: n.y ?? 0 };
  const size = (g: { id: string; w?: number | null; h?: number | null }) =>
    drag?.kind === "resize" && drag.id === g.id ? { w: drag.w, h: drag.h } : { w: g.w ?? 0, h: g.h ?? 0 };
  const extent = board.notes.reduce((m, n) => Math.max(m, (n.x ?? 0) + NOTE_W + 60, (n.y ?? 0) + NOTE_H + 60), 600);
  const groupExtent = board.groups.reduce((m, g) => Math.max(m, (g.x ?? 0) + size(g).w + 60, (g.y ?? 0) + size(g).h + 60), extent);
  const lasso = drag?.kind === "lasso" ? drag : null;
  const selNotes = board.notes.filter((n) => sel.includes(n.id));
  const floatAt = selNotes.length
    ? { x: Math.min(...selNotes.map((n) => n.x ?? 0)), y: Math.max(...selNotes.map((n) => (n.y ?? 0) + NOTE_H)) + 10 }
    : null;
  const dates: BoardMilestone[] = board.timeline ?? [];
  const links: BoardLink[] = board.links ?? [];
  const noteAt = (nid: string) => {
    const n = board.notes.find((x) => x.id === nid);
    return n ? pos(n, "note") : null;
  };
  const linkEdit = (() => {
    if (!linkSel || drag) return null;
    const a = noteAt(linkSel.from);
    const b = noteAt(linkSel.to);
    if (!a || !b) return null;
    const r = arrow(a, b);
    return { x: (r.x1 + r.x2) / 2 - 90, y: (r.y1 + r.y2) / 2 + 8 };
  })();

  return (
    <div className="board-view">
      {view.changed.length > 0 && (
        <div className="board-banner">
          <Sparkles size={13} />
          <b>{t("board_changed_n", { n: view.changed.length })}</b>
          <span className="dim">{t("board_changed_hint")}</span>
          <span className="board-banner-acts">
            <button className="pill-btn on" onClick={() => void accept()} title={t("sheet_accept_hint")}>
              <Check size={11} /> {t("sheet_accept")}
            </button>
          </span>
        </div>
      )}
      <div className="board-tools">
        <button className="pill-btn" onClick={() => void edit({ op: "add", text: "", x: 24, y: Math.round(groupExtent - 40), group: null })} title={t("board_note_ph")}>
          <Plus size={11} /> {t("board_add_note")}
        </button>
        <button className="pill-btn" onClick={() => void edit({ op: "add_group", title: t("board_group_ph"), x: 24, y: Math.round(groupExtent) })}>
          <Plus size={11} /> {t("board_add_group")}
        </button>
        <button className="pill-btn" onClick={() => setMilestone({ index: null, date: "", title: "", owner: "" })}>
          <Plus size={11} /> {t("board_add_milestone")}
        </button>
        {linking ? <span className="board-linking">{t("board_linking")}</span> : <span className="dim">{t("board_lasso_hint")}</span>}
        {onAsk && (
          <>
            <button className="pill-btn" onClick={() => onAsk(t("board_to_doc_text"))} title={t("board_to_doc_text")}>
              <FileText size={11} /> {t("board_to_doc")}
            </button>
            <button className="pill-btn" onClick={() => onAsk(t("board_to_deck_text"))} title={t("board_to_deck_text")}>
              <Presentation size={11} /> {t("board_to_deck")}
            </button>
          </>
        )}
        <span className="spacer" />
        {sel.length > 0 && (
          <>
            <span className="dim">{t("board_selected_n", { n: sel.length })}</span>
            <button className="pill-btn on" onClick={() => void ask()} title={t("board_ask_hint")}>
              <MessageSquare size={11} /> {t("board_ask")}
            </button>
          </>
        )}
        <span className="dim" title={t("sheet_versions_hint")}>
          <History size={11} /> {t("sheet_versions", { n: view.versions })}
        </span>
      </div>
      <div
        className={`board-canvas-wrap ${linking ? "linking" : ""}`}
        ref={canvas}
        onMouseMove={onMove}
        onMouseUp={onUp}
        onMouseLeave={onUp}
        onMouseDown={(e) => {
          if (e.target !== e.currentTarget && !(e.target as HTMLElement).classList.contains("board-canvas")) return;
          const p = at(e);
          setSel([]);
          setLinkSel(null);
          setDrag({ kind: "lasso", x1: p.x, y1: p.y, x2: p.x, y2: p.y });
        }}
      >
        <div className="board-canvas" style={{ width: groupExtent, height: groupExtent }}>
          {board.notes.length === 0 && board.groups.length === 0 && <div className="board-empty dim">{t("board_empty")}</div>}
          {board.groups.map((g) => {
            const p = pos(g, "group");
            const s = size(g);
            return (
              <div key={g.id} className="board-group" style={{ left: p.x, top: p.y, width: s.w, height: s.h }}>
                {groupTitle?.id === g.id ? (
                  <input
                    autoFocus
                    className="board-group-input"
                    value={groupTitle.value}
                    onMouseDown={(e) => e.stopPropagation()}
                    onChange={(e) => setGroupTitle({ id: g.id, value: e.target.value })}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        const title = groupTitle.value.trim();
                        setGroupTitle(null);
                        if (title && title !== g.title) void edit({ op: "group_title", id: g.id, title });
                      }
                      if (e.key === "Escape") setGroupTitle(null);
                    }}
                    onBlur={() => setGroupTitle(null)}
                  />
                ) : (
                  <span
                    className="board-group-title"
                    title={t("board_group_title_hint")}
                    onMouseDown={(e) => {
                      e.stopPropagation();
                      const q = at(e);
                      setDrag({ kind: "group", id: g.id, dx: q.x - p.x, dy: q.y - p.y, x: p.x, y: p.y, moved: false });
                    }}
                    onDoubleClick={() => setGroupTitle({ id: g.id, value: g.title })}
                  >
                    {g.title}
                    <button title={t("board_remove")} onMouseDown={(e) => e.stopPropagation()} onClick={() => void edit({ op: "remove_group", id: g.id })}>
                      <Trash2 size={9} />
                    </button>
                  </span>
                )}
                <span
                  className="board-group-corner"
                  title={t("board_group_title_hint")}
                  onMouseDown={(e) => {
                    e.stopPropagation();
                    setDrag({ kind: "resize", id: g.id, x: p.x, y: p.y, w: s.w, h: s.h, moved: false });
                  }}
                />
              </div>
            );
          })}
          <svg className="board-links" width={groupExtent} height={groupExtent}>
            <defs>
              <marker id="board-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse">
                <path d="M 0 0 L 10 5 L 0 10 z" />
              </marker>
            </defs>
            {links.map((l) => {
              const a = noteAt(l.from);
              const b = noteAt(l.to);
              if (!a || !b) return null;
              const r = arrow(a, b);
              const on = linkSel?.from === l.from && linkSel?.to === l.to;
              return (
                <g key={`${l.from}-${l.to}`} className={`board-link ${on ? "on" : ""}`} onClick={(e) => { e.stopPropagation(); setLinkSel({ from: l.from, to: l.to, label: l.label ?? "" }); }}>
                  <line x1={r.x1} y1={r.y1} x2={r.x2} y2={r.y2} className="hit" />
                  <line x1={r.x1} y1={r.y1} x2={r.x2} y2={r.y2} markerEnd="url(#board-arrow)" />
                  {l.label && (
                    <text x={(r.x1 + r.x2) / 2} y={(r.y1 + r.y2) / 2 - 4} textAnchor="middle">
                      {l.label}
                    </text>
                  )}
                </g>
              );
            })}
          </svg>
          {board.notes.map((n) => {
            const p = pos(n, "note");
            const isEdit = editing?.id === n.id;
            return (
              <div
                key={n.id}
                className={`board-note ${n.color ?? "yellow"} ${view.changed.includes(n.id) ? "changed" : ""} ${sel.includes(n.id) ? "sel" : ""} ${linking === n.id ? "linking-from" : ""} ${marks.some((m) => m.kind === "note" && m.id === n.id) ? "cm" : ""}`}
                style={{ left: p.x, top: p.y }}
                title={n.id}
                onMouseDown={(e) => {
                  if (isEdit || linking) return;
                  e.stopPropagation();
                  const q = at(e);
                  setDrag({ kind: "note", id: n.id, dx: q.x - p.x, dy: q.y - p.y, x: p.x, y: p.y, moved: false });
                }}
                onClick={(e) => pick(n, e)}
                onDoubleClick={() => !linking && setEditing({ id: n.id, text: n.text })}
              >
                {isEdit ? (
                  <textarea
                    autoFocus
                    value={editing.text}
                    placeholder={t("board_note_ph")}
                    onChange={(e) => setEditing({ id: n.id, text: e.target.value })}
                    onBlur={() => {
                      const text = editing.text;
                      setEditing(null);
                      if (text !== n.text) void edit({ op: "text", id: n.id, text });
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Escape") setEditing(null);
                      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) (e.target as HTMLTextAreaElement).blur();
                    }}
                  />
                ) : (
                  <div className="board-note-text">{n.text || <span className="dim">{t("board_note_ph")}</span>}</div>
                )}
                <div className="board-note-foot">
                  <span>{n.owner ?? ""}</span>
                  <span>{n.date ?? ""}</span>
                </div>
                <span className="board-note-acts" onMouseDown={(e) => e.stopPropagation()}>
                  {COLORS.filter((c) => c !== (n.color ?? "yellow")).map((c) => (
                    <button key={c} title={c} onClick={(e) => { e.stopPropagation(); void edit({ op: "color", id: n.id, color: c }); }}>
                      <span className={`board-swatch ${c}`} />
                    </button>
                  ))}
                  <button title={t("board_remove")} onClick={(e) => { e.stopPropagation(); void edit({ op: "remove", id: n.id }); }}>
                    <Trash2 size={9} />
                  </button>
                </span>
              </div>
            );
          })}
          {lasso && (
            <div
              className="board-lasso"
              style={{ left: Math.min(lasso.x1, lasso.x2), top: Math.min(lasso.y1, lasso.y2), width: Math.abs(lasso.x2 - lasso.x1), height: Math.abs(lasso.y2 - lasso.y1) }}
            />
          )}
          {floatAt && !drag && !linking && (
            <div className="board-floating" style={{ left: floatAt.x, top: floatAt.y }} onMouseDown={(e) => e.stopPropagation()}>
              <button className="pill-btn on" onClick={() => void ask()} title={t("board_ask_hint")}>
                <MessageSquare size={11} /> {t("board_ask")}
              </button>
              {sel.length === 1 && (
                <button className="pill-btn" title={t("board_link_hint")} onClick={() => { setLinking(sel[0]); setSel([]); }}>
                  <Link2 size={11} /> {t("board_link")}
                </button>
              )}
              <button
                className="pill-btn"
                title={t("board_to_tasks_text")}
                onClick={() => void ipc.boardSelection(space, id, sel).then((text) => onAsk?.(`${t("board_to_tasks_text")}\n${text.trimEnd()}`))}
              >
                <ListChecks size={11} /> {t("board_to_tasks")}
              </button>
              <button className="pill-btn" onClick={() => { for (const s of sel) void edit({ op: "remove", id: s }); setSel([]); }}>
                <Trash2 size={11} /> {t("board_remove")}
              </button>
            </div>
          )}
          {linkSel && linkEdit && (
            <div className="board-floating board-link-edit" style={{ left: linkEdit.x, top: linkEdit.y }} onMouseDown={(e) => e.stopPropagation()}>
              <input
                autoFocus
                placeholder={t("board_link_label")}
                value={linkSel.label}
                onChange={(e) => setLinkSel({ ...linkSel, label: e.target.value })}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    const { from, to, label } = linkSel;
                    setLinkSel(null);
                    void edit({ op: "link_label", from, to, label: label.trim() || null });
                  }
                  if (e.key === "Escape") setLinkSel(null);
                }}
              />
              <button className="pill-btn" onClick={() => { const { from, to } = linkSel; setLinkSel(null); void edit({ op: "unlink", from, to }); }}>
                <Trash2 size={11} /> {t("board_unlink")}
              </button>
            </div>
          )}
        </div>
      </div>
      {(dates.length > 0 || milestone) && (
        <div className="board-timeline">
          <div className="board-timeline-title">{t("board_timeline")}</div>
          {dates.length > 0 && (
            <div className="board-timeline-line">
              {dates.map((m, i) => (
                <div
                  key={i}
                  className={`board-milestone ${i === dates.length - 1 && dates.length > 1 ? "last" : ""} ${milestone?.index === i ? "on" : ""}`}
                  style={{ left: `${dates.length > 1 ? (i / (dates.length - 1)) * 100 : 0}%` }}
                  onClick={() => setMilestone({ index: i, date: m.date, title: m.title, owner: m.owner ?? "" })}
                >
                  <span className="dot" />
                  <b>{m.title}</b>
                  <span className="dim">
                    {m.date}
                    {m.owner ? ` · ${m.owner}` : ""}
                  </span>
                </div>
              ))}
            </div>
          )}
          {milestone && (
            <div className="board-milestone-form">
              <input className="mono" placeholder={t("board_milestone_date")} value={milestone.date} onChange={(e) => setMilestone({ ...milestone, date: e.target.value })} />
              <input placeholder={t("board_milestone_title")} value={milestone.title} onChange={(e) => setMilestone({ ...milestone, title: e.target.value })} />
              <input placeholder={t("board_milestone_owner")} value={milestone.owner} onChange={(e) => setMilestone({ ...milestone, owner: e.target.value })} />
              <button className="pill-btn on" onClick={() => saveMilestone(milestone)} disabled={!milestone.date || !milestone.title.trim()}>
                <Check size={11} /> {t("board_save")}
              </button>
              {milestone.index != null && (
                <button className="pill-btn" onClick={() => { const i = milestone.index as number; setMilestone(null); void edit({ op: "remove_milestone", index: i }); }}>
                  <Trash2 size={11} /> {t("board_remove")}
                </button>
              )}
              <button className="pill-btn" onClick={() => setMilestone(null)}>
                {t("board_cancel")}
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
