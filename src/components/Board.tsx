import { useEffect, useRef, useState } from "react";
import { t } from "../lib/i18n";
import type { BoardTask, Project } from "../types";

const COLUMNS = ["backlog", "doing", "waiting", "done"] as const;
type Status = (typeof COLUMNS)[number];

/** Reading order when the board is a list: live work first, history last.
 *  Four columns side by side have no order — a single column does. */
const LIST_ORDER: Status[] = ["doing", "waiting", "backlog", "done"];

const COL_LABEL = {
  backlog: "col_backlog",
  doing: "col_doing",
  waiting: "col_waiting",
  done: "col_done",
} as const;

/** Below this the four columns stop being columns: at ~90px each the
 *  titles break one letter per line. Measured on the board's OWN box, not
 *  the window — the same board is a rail panel and a full-width tab. */
const NARROW_AT = 760;

/** "2026-08-31T01:18:44Z" → "31/08 01:18": the year and the seconds are
 *  never what you are looking for on a card. */
function shortWhen(iso: string): string {
  const [date, time] = iso.split("T");
  const [, month, day] = date.split("-");
  return `${day}/${month} ${(time ?? "").slice(0, 5)}`;
}

/** The kanban tab: read/organize view over the invisible board. Wide, it
 *  is four columns; narrow, the same data reads as grouped lists. */
export default function Board({
  tasks,
  projects,
  onMove,
  onOpen,
  onSubtask,
}: {
  tasks: BoardTask[];
  /** Given on the global board: each card shows the project it belongs to. */
  projects?: Project[];
  onMove: (title: string, status: BoardTask["status"]) => void;
  /** Click on a card = go back to work on that task. */
  onOpen?: (task: BoardTask) => void;
  /** Tick/untick one step of the task's plan checklist. */
  onSubtask?: (title: string, index: number, done: boolean) => void;
}) {
  // The drop target has to be obvious BEFORE the release, so the column
  // under the pointer lights up in its own color while dragging.
  const [over, setOver] = useState<BoardTask["status"] | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  /** Groups the user folded by hand; the default depends on the layout. */
  const [folded, setFolded] = useState<Partial<Record<Status, boolean>>>({});
  const [narrow, setNarrow] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      setNarrow(entry.contentRect.width < NARROW_AT);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  /** Wide, everything is open — folding is what a single column needs.
   *  Narrow, history and empty groups start out of the way. */
  function isOpen(status: Status, count: number): boolean {
    if (!narrow) return true;
    return folded[status] ?? (count > 0 && status !== "done");
  }

  /** Name of the project a task lives in (registered one first). */
  function tagOf(task: BoardTask): string | undefined {
    const ws = task.workspace;
    if (!ws) return undefined;
    const project = projects?.find((p) => ws === p.path || ws.startsWith(`${p.path}/`));
    return project?.name ?? ws.split("/").filter(Boolean).pop();
  }

  return (
    <div ref={rootRef} className={`kanban ${narrow ? "narrow" : ""}`}>
      {(narrow ? LIST_ORDER : COLUMNS).map((status) => {
        const items = tasks.filter((t) => t.status === status);
        const open = isOpen(status, items.length);
        return (
          <div
            key={status}
            className={`column ${status}${over === status ? " over" : ""}${open ? "" : " folded"}`}
            onDragOver={(e) => {
              e.preventDefault();
              e.dataTransfer.dropEffect = "move";
              if (over !== status) setOver(status);
            }}
            // No dragleave: WKWebView reports relatedTarget as null there,
            // so any child-crossing would kill the highlight. Moving to a
            // sibling column re-targets it; drop/dragEnd clear it.
            onDrop={(e) => {
              setOver(null);
              setDragging(null);
              const title = e.dataTransfer.getData("text/hark-task");
              if (title) onMove(title, status);
            }}
          >
            <h3
              // Only a list has anything to fold: the columns are all
              // visible at once by construction.
              onClick={narrow ? () => setFolded((f) => ({ ...f, [status]: !open })) : undefined}
            >
              <span className="col-dot" />
              {t(COL_LABEL[status])} <span className="count">{items.length}</span>
            </h3>
            {open &&
              /* NOTE: `task`, not `t` — `t()` is the i18n lookup in scope. */
              items.map((task) => {
                const tag = projects ? tagOf(task) : undefined;
                const steps = task.subtasks ?? [];
                const ticked = steps.filter((s) => s.done).length;
                return (
                  <div
                    key={task.title}
                    className={`card${dragging === task.title ? " dragging" : ""}${onOpen ? " clickable" : ""}`}
                    draggable
                    title={onOpen ? t("card_open") : undefined}
                    onClick={() => onOpen?.(task)}
                    onDragStart={(e) => {
                      e.dataTransfer.setData("text/hark-task", task.title);
                      e.dataTransfer.effectAllowed = "move";
                      setDragging(task.title);
                    }}
                    onDragEnd={() => {
                      setDragging(null);
                      setOver(null);
                    }}
                  >
                    <span className="card-dot" />
                    <div className="card-body">
                      {tag && (
                        <div className="card-tags">
                          <span className="card-tag">{tag}</span>
                        </div>
                      )}
                      <div className="card-title">{task.title}</div>
                      {task.note && <div className="card-note">{task.note}</div>}
                      <div className="card-foot">
                        {steps.length > 0 && (
                          <details className="card-subs" onClick={(e) => e.stopPropagation()}>
                            <summary title={t("card_steps")}>
                              <span className="card-bar">
                                <i style={{ width: `${(ticked / steps.length) * 100}%` }} />
                              </span>
                              {ticked}/{steps.length}
                            </summary>
                            <ul>
                              {steps.map((s, i) => (
                                <li key={i} className={s.done ? "done" : ""}>
                                  <label>
                                    <input
                                      type="checkbox"
                                      checked={s.done}
                                      onChange={(e) => onSubtask?.(task.title, i, e.target.checked)}
                                    />
                                    {s.text}
                                  </label>
                                </li>
                              ))}
                            </ul>
                          </details>
                        )}
                        {task.session_ids.length > 0 && (
                          <span className="card-sessions">
                            {task.session_ids.length} {t("card_sessions")}
                          </span>
                        )}
                        <span className="card-when">{shortWhen(task.updated_at)}</span>
                      </div>
                    </div>
                  </div>
                );
              })}
          </div>
        );
      })}
    </div>
  );
}
