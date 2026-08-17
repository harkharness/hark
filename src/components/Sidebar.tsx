import { useState } from "react";
import { FolderTree, MoreVertical, Pin, Plus } from "lucide-react";
import type { BoardTask, Project } from "../types";

const DOT: Record<BoardTask["status"], string> = {
  doing: "◍",
  waiting: "◌",
  backlog: "○",
  done: "●",
};

/**
 * Chats grouped by project, like a session sidebar. Clicking a task loads
 * its context into the chat; each project header can spawn a fresh chat;
 * the footer registers new project directories.
 */
export default function Sidebar({
  projects,
  tasks,
  activeTitle,
  liveTitles,
  onOpen,
  onRename,
  onPin,
  onArchive,
  onResume,
  onNewChat,
  onAddProject,
  onRemoveProject,
  onOpenFiles,
}: {
  projects: Project[];
  tasks: BoardTask[];
  activeTitle?: string;
  liveTitles: string[];
  onOpen: (task: BoardTask) => void;
  onRename: (task: BoardTask, title: string) => void;
  onPin: (task: BoardTask) => void;
  onArchive: (task: BoardTask) => void;
  onResume: (task: BoardTask) => void;
  onNewChat: (project: Project) => void;
  onAddProject: (path: string) => void;
  onRemoveProject: (project: Project) => void;
  /** Opens the "Arquivos" window scoped to this project. */
  onOpenFiles: (project: Project) => void;
}) {
  const [menu, setMenu] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [projMenu, setProjMenu] = useState<string | null>(null);

  // Finished tasks sink to the bottom and vanish after a week (still
  // recoverable by search/voice: "retoma a task do hydrator").
  const WEEK_MS = 7 * 24 * 3600 * 1000;
  const visible = (t: BoardTask) =>
    t.status !== "done" || Date.now() - Date.parse(t.updated_at) < WEEK_MS;
  const ordered = [...tasks].filter(visible).sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    const doneA = a.status === "done" ? 1 : 0;
    const doneB = b.status === "done" ? 1 : 0;
    if (doneA !== doneB) return doneA - doneB;
    return b.updated_at.localeCompare(a.updated_at);
  });
  const inProject = (t: BoardTask, p: Project) =>
    !!t.workspace && (t.workspace === p.path || t.workspace.startsWith(`${p.path}/`));
  const claimed = new Set(
    ordered.filter((t) => projects.some((p) => inProject(t, p))).map((t) => t.title),
  );
  const leftovers = ordered.filter((t) => !claimed.has(t.title));

  const item = (t: BoardTask) => (
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
          <button className="side-open" onClick={() => onOpen(t)} title="abrir no chat">
            <span className={`side-dot ${liveTitles.includes(t.title) ? "live" : ""}`}>
              {DOT[t.status]}
            </span>
            {t.pinned && <span className="side-pin"><Pin size={10} /></span>}
            <span className="side-title">{t.title}</span>
          </button>
          <button
            className="side-menu-btn"
            onClick={() => setMenu(menu === t.title ? null : t.title)}
            title="ações"
          >
            <MoreVertical size={13} />
          </button>
        </>
      )}

      {menu === t.title && (
        <div className="side-menu">
          <button onClick={() => { setMenu(null); onOpen(t); }}>Abrir no chat</button>
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
  );

  return (
    <nav className="sidebar" onMouseLeave={() => { setMenu(null); setProjMenu(null); }}>
      {projects.map((p) => {
        const group = ordered.filter((t) => inProject(t, p));
        return (
          <section key={p.path} className="side-group">
            <div className="side-group-head">
              <span className="side-group-name" title={p.path}>
                {p.name}
              </span>
              <button
                className="side-group-add"
                title={`novo chat em ${p.name}`}
                onClick={() => onNewChat(p)}
              >
                <Plus size={12} />
              </button>
              <button
                className="side-group-add"
                title="arquivos do projeto (abre a janela Arquivos)"
                onClick={() => onOpenFiles(p)}
              >
                <FolderTree size={12} />
              </button>
              <button
                className="side-menu-btn"
                onClick={() => setProjMenu(projMenu === p.path ? null : p.path)}
              >
                <MoreVertical size={13} />
              </button>
              {projMenu === p.path && (
                <div className="side-menu">
                  <button onClick={() => { setProjMenu(null); onNewChat(p); }}>
                    Novo chat aqui
                  </button>
                  <button
                    className="danger"
                    onClick={() => { setProjMenu(null); onRemoveProject(p); }}
                  >
                    Remover da lista
                  </button>
                </div>
              )}
            </div>
            {group.length === 0 && <div className="side-empty">sem chats ainda</div>}
            {group.map(item)}
          </section>
        );
      })}

      {leftovers.length > 0 && (
        <section className="side-group">
          <div className="side-group-head">
            <span className="side-group-name">outros</span>
          </div>
          {leftovers.map(item)}
        </section>
      )}

      {adding ? (
        <input
          className="side-rename"
          autoFocus
          placeholder="~/Projects/…"
          onBlur={() => setAdding(false)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setAdding(false);
            if (e.key === "Enter") {
              const value = (e.target as HTMLInputElement).value.trim();
              if (value) onAddProject(value);
              setAdding(false);
            }
          }}
        />
      ) : (
        <button className="side-add-project" onClick={() => setAdding(true)}>
          + projeto
        </button>
      )}
    </nav>
  );
}
