import { useCallback, useEffect, useRef, useState } from "react";
import { Check, History, MessageSquare, Plus, Sparkles, Trash2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { BoardNote, BoardOp, BoardView as View } from "../types";

const NOTE_W = 150;
const NOTE_H = 96;
const COLORS = ["yellow", "blue", "green", "pink", "gray"];

/** The notes whose frame lies inside a rectangle (mirror of whiteboard::within). */
export function within(notes: BoardNote[], x1: number, y1: number, x2: number, y2: number): string[] {
  const [l, r] = [Math.min(x1, x2), Math.max(x1, x2)];
  const [tp, b] = [Math.min(y1, y2), Math.max(y1, y2)];
  return notes.filter((n) => (n.x ?? 0) >= l && (n.x ?? 0) + NOTE_W <= r && (n.y ?? 0) >= tp && (n.y ?? 0) + NOTE_H <= b).map((n) => n.id);
}

type Drag =
  | { kind: "note"; id: string; dx: number; dy: number; x: number; y: number; moved: boolean }
  | { kind: "group"; id: string; dx: number; dy: number; x: number; y: number; moved: boolean }
  | { kind: "lasso"; x1: number; y1: number; x2: number; y2: number };

/**
 * The whiteboard page: notes on a canvas inside their groups, dragged
 * into place (dropping inside a group joins it), edited on a double
 * click, coloured, added and removed; a lasso on the background selects
 * notes and "ask about the selection" takes them to a chat as text. The
 * notes the agent changed since you last accepted wear a mark.
 */
export default function BoardView({ space, id, onChanged, onAsk }: { space: string; id: string; onChanged?: () => void; onAsk?: (text: string) => void }) {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sel, setSel] = useState<string[]>([]);
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
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
    else setDrag({ ...drag, x: p.x - drag.dx, y: p.y - drag.dy, moved: true });
  }
  function onUp() {
    if (!drag) return;
    setDrag(null);
    if (drag.kind === "lasso") {
      setSel(within(view?.board.notes ?? [], drag.x1, drag.y1, drag.x2, drag.y2));
    } else if (drag.moved) {
      const x = Math.max(0, Math.round(drag.x));
      const y = Math.max(0, Math.round(drag.y));
      void edit(drag.kind === "note" ? { op: "move", id: drag.id, x, y } : { op: "move_group", id: drag.id, x, y });
    }
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
  const extent = board.notes.reduce((m, n) => Math.max(m, (n.x ?? 0) + NOTE_W + 60, (n.y ?? 0) + NOTE_H + 60), 600);
  const groupExtent = board.groups.reduce((m, g) => Math.max(m, (g.x ?? 0) + (g.w ?? 0) + 60, (g.y ?? 0) + (g.h ?? 0) + 60), extent);
  const lasso = drag?.kind === "lasso" ? drag : null;
  const selNotes = board.notes.filter((n) => sel.includes(n.id));
  const floatAt = selNotes.length
    ? { x: Math.min(...selNotes.map((n) => n.x ?? 0)), y: Math.max(...selNotes.map((n) => (n.y ?? 0) + NOTE_H)) + 10 }
    : null;
  const dates = board.timeline ?? [];

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
        <span className="dim">{t("board_lasso_hint")}</span>
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
        className="board-canvas-wrap"
        ref={canvas}
        onMouseMove={onMove}
        onMouseUp={onUp}
        onMouseLeave={onUp}
        onMouseDown={(e) => {
          if (e.target !== e.currentTarget && !(e.target as HTMLElement).classList.contains("board-canvas")) return;
          const p = at(e);
          setSel([]);
          setDrag({ kind: "lasso", x1: p.x, y1: p.y, x2: p.x, y2: p.y });
        }}
      >
        <div className="board-canvas" style={{ width: groupExtent, height: groupExtent }}>
          {board.notes.length === 0 && board.groups.length === 0 && <div className="board-empty dim">{t("board_empty")}</div>}
          {board.groups.map((g) => {
            const p = pos(g, "group");
            return (
              <div key={g.id} className="board-group" style={{ left: p.x, top: p.y, width: g.w ?? 0, height: g.h ?? 0 }}>
                <span
                  className="board-group-title"
                  onMouseDown={(e) => {
                    e.stopPropagation();
                    const q = at(e);
                    setDrag({ kind: "group", id: g.id, dx: q.x - p.x, dy: q.y - p.y, x: p.x, y: p.y, moved: false });
                  }}
                >
                  {g.title}
                  <button title={t("board_remove")} onMouseDown={(e) => e.stopPropagation()} onClick={() => void edit({ op: "remove_group", id: g.id })}>
                    <Trash2 size={9} />
                  </button>
                </span>
              </div>
            );
          })}
          {board.notes.map((n) => {
            const p = pos(n, "note");
            const isEdit = editing?.id === n.id;
            return (
              <div
                key={n.id}
                className={`board-note ${n.color ?? "yellow"} ${view.changed.includes(n.id) ? "changed" : ""} ${sel.includes(n.id) ? "sel" : ""}`}
                style={{ left: p.x, top: p.y }}
                title={n.id}
                onMouseDown={(e) => {
                  if (isEdit) return;
                  e.stopPropagation();
                  const q = at(e);
                  setDrag({ kind: "note", id: n.id, dx: q.x - p.x, dy: q.y - p.y, x: p.x, y: p.y, moved: false });
                }}
                onClick={(e) => {
                  if (drag && drag.kind !== "lasso" && drag.moved) return;
                  setSel(e.shiftKey ? (sel.includes(n.id) ? sel.filter((s) => s !== n.id) : [...sel, n.id]) : [n.id]);
                }}
                onDoubleClick={() => setEditing({ id: n.id, text: n.text })}
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
          {floatAt && !drag && (
            <div className="board-floating" style={{ left: floatAt.x, top: floatAt.y }} onMouseDown={(e) => e.stopPropagation()}>
              <button className="pill-btn on" onClick={() => void ask()} title={t("board_ask_hint")}>
                <MessageSquare size={11} /> {t("board_ask")}
              </button>
              <button className="pill-btn" onClick={() => { for (const s of sel) void edit({ op: "remove", id: s }); setSel([]); }}>
                <Trash2 size={11} /> {t("board_remove")}
              </button>
            </div>
          )}
        </div>
      </div>
      {dates.length > 0 && (
        <div className="board-timeline">
          <div className="board-timeline-title">{t("board_timeline")}</div>
          <div className="board-timeline-line">
            {dates.map((m, i) => (
              <div key={i} className={`board-milestone ${i === dates.length - 1 && dates.length > 1 ? "last" : ""}`} style={{ left: `${dates.length > 1 ? (i / (dates.length - 1)) * 100 : 0}%` }}>
                <span className="dot" />
                <b>{m.title}</b>
                <span className="dim">
                  {m.date}
                  {m.owner ? ` · ${m.owner}` : ""}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
