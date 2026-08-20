import { useState } from "react";
import { t } from "../lib/i18n";
import type { BoardTask, Project } from "../types";

const COLUMNS = ["backlog", "doing", "waiting", "done"] as const;
const COL_LABEL = {
  backlog: "col_backlog",
  doing: "col_doing",
  waiting: "col_waiting",
  done: "col_done",
} as const;

/** The kanban tab: read/organize view over the invisible board. */
export default function Board({
  tasks,
  projects,
  onMove,
  onOpen,
}: {
  tasks: BoardTask[];
  /** Given on the global board: each card shows the project it belongs to. */
  projects?: Project[];
  onMove: (title: string, status: BoardTask["status"]) => void;
  /** Click on a card = go back to work on that task. */
  onOpen?: (task: BoardTask) => void;
}) {
  // The drop target has to be obvious BEFORE the release, so the column
  // under the pointer lights up in its own color while dragging.
  const [over, setOver] = useState<BoardTask["status"] | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);

  /** Name of the project a task lives in (registered one first). */
  function tagOf(task: BoardTask): string | undefined {
    const ws = task.workspace;
    if (!ws) return undefined;
    const project = projects?.find((p) => ws === p.path || ws.startsWith(`${p.path}/`));
    return project?.name ?? ws.split("/").filter(Boolean).pop();
  }

  return (
    <div className="kanban">
      {COLUMNS.map((status) => {
        const items = tasks.filter((t) => t.status === status);
        return (
          <div
            key={status}
            className={`column ${status}${over === status ? " over" : ""}`}
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
              const title = e.dataTransfer.getData("text/vox-task");
              if (title) onMove(title, status);
            }}
          >
            <h3>
              {t(COL_LABEL[status])} <span className="count">{items.length}</span>
            </h3>
            {/* NOTE: `task`, not `t` — `t()` is the i18n lookup in scope. */}
            {items.map((task) => {
              const tag = projects ? tagOf(task) : undefined;
              return (
                <div
                  key={task.title}
                  className={`card${dragging === task.title ? " dragging" : ""}${onOpen ? " clickable" : ""}`}
                  draggable
                  title={onOpen ? t("card_open") : undefined}
                  onClick={() => onOpen?.(task)}
                  onDragStart={(e) => {
                    e.dataTransfer.setData("text/vox-task", task.title);
                    e.dataTransfer.effectAllowed = "move";
                    setDragging(task.title);
                  }}
                  onDragEnd={() => {
                    setDragging(null);
                    setOver(null);
                  }}
                >
                  {tag && (
                    <div className="card-tags">
                      <span className="card-tag">{tag}</span>
                    </div>
                  )}
                  <div className="card-title">{task.title}</div>
                  {task.note && <div className="card-note">{task.note}</div>}
                  <div className="card-meta">
                    {task.updated_at.slice(0, 16).replace("T", " ")}
                    {task.session_ids.length > 0 &&
                      ` · ${task.session_ids.length} ${t("card_sessions")}`}
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
