import { useState } from "react";
import type { BoardTask } from "./types";

const DOT: Record<BoardTask["status"], string> = {
  doing: "◍",
  waiting: "◌",
  backlog: "○",
  done: "●",
};

/**
 * Task list, one line per task, like a chat sidebar. Clicking reads the
 * thread (never executes); the ⋮ menu holds the local actions.
 */
export default function Sidebar({
  tasks,
  activeTitle,
  liveTitles,
  onOpen,
  onRename,
  onPin,
  onArchive,
  onResume,
}: {
  tasks: BoardTask[];
  activeTitle?: string;
  liveTitles: string[];
  onOpen: (task: BoardTask) => void;
  onRename: (task: BoardTask, title: string) => void;
  onPin: (task: BoardTask) => void;
  onArchive: (task: BoardTask) => void;
  onResume: (task: BoardTask) => void;
}) {
  const [menu, setMenu] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);

  const ordered = [...tasks].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return b.updated_at.localeCompare(a.updated_at);
  });

  return (
    <nav className="sidebar" onMouseLeave={() => setMenu(null)}>
      <h3>tasks</h3>
      {ordered.length === 0 && <div className="side-empty">nenhuma task ainda</div>}
      {ordered.map((t) => (
        <div
          key={t.title}
          className={`side-item ${t.status} ${activeTitle === t.title ? "active" : ""}`}
        >
          {renaming === t.title ? (
            <input
              className="side-rename"
              autoFocus
              defaultValue={t.title}
              onBlur={(e) => {
                if (e.target.value.trim() && e.target.value !== t.title) {
                  onRename(t, e.target.value.trim());
                }
                setRenaming(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                if (e.key === "Escape") setRenaming(null);
              }}
            />
          ) : (
            <>
              <button className="side-open" onClick={() => onOpen(t)} title="ler a thread">
                <span className={`side-dot ${liveTitles.includes(t.title) ? "live" : ""}`}>
                  {DOT[t.status]}
                </span>
                {t.pinned && <span className="side-pin">📌</span>}
                <span className="side-title">{t.title}</span>
              </button>
              <button
                className="side-menu-btn"
                onClick={() => setMenu(menu === t.title ? null : t.title)}
                title="ações"
              >
                ⋮
              </button>
            </>
          )}

          {menu === t.title && (
            <div className="side-menu">
              <button onClick={() => { setMenu(null); onOpen(t); }}>Ler thread</button>
              <button onClick={() => { setMenu(null); onResume(t); }}>Retomar</button>
              <button onClick={() => { setMenu(null); setRenaming(t.title); }}>
                Mudar o nome
              </button>
              <button onClick={() => { setMenu(null); onPin(t); }}>
                {t.pinned ? "Desafixar" : "Fixar"}
              </button>
              <button className="danger" onClick={() => { setMenu(null); onArchive(t); }}>
                Arquivar
              </button>
            </div>
          )}
        </div>
      ))}
    </nav>
  );
}
