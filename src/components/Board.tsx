import type { BoardTask } from "../types";

/** The kanban tab: read/organize view over the invisible board. */
export default function Board({
  tasks,
  onMove,
}: {
  tasks: BoardTask[];
  onMove: (title: string, status: BoardTask["status"]) => void;
}) {
  return (
    <div className="kanban">
      {(["backlog", "doing", "waiting", "done"] as const).map((status) => {
        const items = tasks.filter((t) => t.status === status);
        return (
          <div
            key={status}
            className={`column ${status}`}
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => {
              const title = e.dataTransfer.getData("text/vox-task");
              if (title) onMove(title, status);
            }}
          >
            <h3>
              {status} <span className="count">{items.length}</span>
            </h3>
            {items.map((t) => (
              <div
                key={t.title}
                className="card"
                draggable
                onDragStart={(e) => e.dataTransfer.setData("text/vox-task", t.title)}
              >
                <div className="card-title">{t.title}</div>
                {t.note && <div className="card-note">{t.note}</div>}
                <div className="card-meta">
                  {t.updated_at.slice(0, 16).replace("T", " ")}
                  {t.session_ids.length > 0 && ` · ${t.session_ids.length} sessão(ões)`}
                </div>
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}
