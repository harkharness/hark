import { useEffect, useRef, useState } from "react";
import { Globe } from "lucide-react";
import { t } from "../lib/i18n";
import { boardSummary, idleDays, inProject } from "../lib/board";
import type { BoardTask, Project } from "../types";

/** A permission a task is waiting on, as the card answers it. */
export type CardAsk = { requestId: string; tool: string; prodRisk?: string | null };

/** Done cards shown before "n older" folds the rest away. */
const DONE_SHOWN = 5;
/** A waiting card that sat this long says so. */
const IDLE_SAYS_AT = 2;

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
  colourOf,
  filter,
  onFilter,
  liveTitles,
  asking,
  costs,
  onAnswerPermission,
  onMove,
  onOpen,
  onSubtask,
}: {
  tasks: BoardTask[];
  /** Given on the global board: each card shows the project it belongs to. */
  projects?: Project[];
  /** The project colour of a path (css token): the card's edge. */
  colourOf?: (path?: string) => string | undefined;
  /** The scope's project: the board shows its cards only. */
  filter?: Project | null;
  /** The pills over the board set the scope (null = every project). */
  onFilter?: (project: Project | null) => void;
  /** Tasks with a worker running right now. */
  liveTitles?: string[];
  /** Tasks waiting on a permission, answered from the card. */
  asking?: Record<string, CardAsk>;
  /** What each task spent in this window. */
  costs?: Record<string, number>;
  onAnswerPermission?: (requestId: string, allow: boolean) => void;
  onMove: (title: string, status: BoardTask["status"]) => void;
  /** Click on a card = go back to work on that task. */
  onOpen?: (task: BoardTask) => void;
  /** Tick/untick one step of the task's plan checklist. */
  onSubtask?: (title: string, index: number, done: boolean) => void;
}) {
  const [moreDone, setMoreDone] = useState(false);
  const live = new Set(liveTitles ?? []);
  const asks = asking ?? {};
  const shown = filter ? tasks.filter((task) => inProject(task, filter)) : tasks;
  const summary = boardSummary(shown, live, new Set(Object.keys(asks)));
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

  /** The project a task lives in (registered one first). */
  function projectOf(task: BoardTask): Project | undefined {
    return task.workspace ? projects?.find((p) => inProject(task, p)) : undefined;
  }
  function tagOf(task: BoardTask): string | undefined {
    return projectOf(task)?.name ?? task.workspace?.split("/").filter(Boolean).pop();
  }

  return (
    <div ref={rootRef} className={`kanban-wrap ${narrow ? "narrow" : ""}`}>
      {projects && onFilter && (
        <div className="kanban-bar">
          <span className="kanban-pills">
            <button className={`pill-btn ${!filter ? "on" : ""}`} onClick={() => onFilter(null)}>
              <Globe size={11} /> {t("board_all")}
            </button>
            {projects.map((p) => {
              const colour = colourOf?.(p.path);
              return (
                <button
                  key={p.path}
                  className={`pill-btn ${filter?.path === p.path ? "on" : ""}`}
                  onClick={() => onFilter(filter?.path === p.path ? null : p)}
                  title={p.path}
                >
                  <span className="art-dot" style={colour ? { background: colour } : undefined} /> {p.name}
                </button>
              );
            })}
          </span>
          <span className="dim kanban-summary">
            {t("board_summary", { n: summary.cards })}
            {summary.live > 0 ? ` · ${t("board_live_n", { n: summary.live })}` : ""}
            {summary.asking > 0 ? ` · ${t("board_asking_n", { n: summary.asking })}` : ""}
          </span>
        </div>
      )}
      <div className={`kanban ${narrow ? "narrow" : ""}`}>
      {(narrow ? LIST_ORDER : COLUMNS).map((status) => {
        const all = shown.filter((t) => t.status === status);
        const items = status === "done" && !moreDone ? all.slice(0, DONE_SHOWN) : all;
        const hidden = all.length - items.length;
        const open = isOpen(status, all.length);
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
              {t(COL_LABEL[status])} <span className="count">{all.length}</span>
            </h3>
            {open &&
              /* NOTE: `task`, not `t` — `t()` is the i18n lookup in scope. */
              items.map((task) => {
                const tag = projects ? tagOf(task) : undefined;
                const colour = colourOf?.(projectOf(task)?.path);
                const steps = task.subtasks ?? [];
                const ticked = steps.filter((s) => s.done).length;
                const ask = asks[task.title];
                const isLive = live.has(task.title);
                const idle = status === "waiting" && !isLive && !ask ? idleDays(task.updated_at) : 0;
                const usd = costs?.[task.title];
                return (
                  <div
                    key={task.title}
                    className={`card${dragging === task.title ? " dragging" : ""}${onOpen ? " clickable" : ""}${ask ? " asking" : ""}`}
                    style={colour ? { borderLeftColor: colour } : undefined}
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
                      {(isLive || ask || idle >= IDLE_SAYS_AT || usd) && (
                        <div className="card-state">
                          {isLive && <span className="ok">{t("board_live")}</span>}
                          {ask && <span className={ask.prodRisk ? "err" : "warn"}>{t("board_asks", { tool: ask.tool })}</span>}
                          {idle >= IDLE_SAYS_AT && <span>{t("board_idle", { d: idle })}</span>}
                          {usd ? <span>≈ ${usd.toFixed(2)}</span> : null}
                        </div>
                      )}
                      {ask && onAnswerPermission && (
                        <div className="card-ask" onClick={(e) => e.stopPropagation()}>
                          <button onClick={() => onAnswerPermission(ask.requestId, false)}>{t("perm_deny")}</button>
                          <button className="allow" onClick={() => onAnswerPermission(ask.requestId, true)}>
                            {t("perm_once")}
                          </button>
                        </div>
                      )}
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
            {open && hidden > 0 && (
              <button className="kanban-more" onClick={() => setMoreDone(true)}>
                {t("board_older", { n: hidden })}
              </button>
            )}
          </div>
        );
      })}
      </div>
    </div>
  );
}
