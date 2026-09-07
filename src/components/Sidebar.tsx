import { useState } from "react";
import { ArrowUp, FolderTree, GitBranch, MessageSquare, MoreVertical, Pin, Plus, SquareKanban } from "lucide-react";
import { t } from "../lib/i18n";
import type { BoardTask, Project, RepoState, SessionHit } from "../types";

const DOT: Record<BoardTask["status"], string> = {
  doing: "◍",
  waiting: "◌",
  backlog: "○",
  done: "●",
};

/** Accent/case-insensitive haystack ("migração" matches "migracao"). */
const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");

/** Every typed word must appear in the chat's title or last prompt. */
function chatMatches(chat: SessionHit, query: string): boolean {
  const hay = fold(`${chat.title} ${chat.last_prompt ?? ""}`);
  return fold(query)
    .split(/\s+/)
    .filter(Boolean)
    .every((term) => hay.includes(term));
}

/**
 * Chats grouped by project, like a session sidebar. Clicking a task loads
 * its context into the chat; each project header can spawn a fresh chat;
 * the footer registers new project directories.
 */
export default function Sidebar({
  projects,
  tasks,
  chats = [],
  activeTitle,
  liveTitles,
  onOpen,
  onOpenChat,
  boardOpen,
  onToggleBoard,
  onOpenGeneral,
  generalActive,
  onRename,
  onPin,
  onArchive,
  onResume,
  onNewChat,
  onAddProject,
  onRemoveProject,
  onOpenFiles,
  repoFor,
}: {
  projects: Project[];
  tasks: BoardTask[];
  /** Git state of a task's repository, when there is one. */
  repoFor?: (task: BoardTask) => RepoState | undefined;
  /** Claude Code history of the project (index), minus adopted sessions. */
  chats?: SessionHit[];
  activeTitle?: string;
  liveTitles: string[];
  onOpen: (task: BoardTask) => void;
  /** Click on a history chat: adopt it as a task and open it. */
  onOpenChat?: (chat: SessionHit) => void;
  /** The board panel is open in the rail (highlights the shortcut). */
  boardOpen?: boolean;
  /** Toggle the board panel — the prominent spot the topbar tab had. */
  onToggleBoard?: () => void;
  /** Back to the window's general chat — the way out of a task now that
   *  clicking the chat you are already in no longer closes it. */
  onOpenGeneral?: () => void;
  generalActive?: boolean;
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
  // The chat search open right now: which project and what was typed.
  // Empty query = the 5 most recent; typing digs through the whole index.
  const [search, setSearch] = useState<{ path: string; q: string } | null>(null);

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

  // NOTE: named `task`, not `t` — `t()` is the i18n lookup in scope here.
  const item = (task: BoardTask) => (
    <div
      key={task.title}
      className={`side-item ${task.status} ${activeTitle === task.title ? "active" : ""}`}
    >
      {renaming === task.title ? (
        <input
          className="side-rename"
          autoFocus
          defaultValue={task.title}
          onBlur={(e) => {
            if (e.target.value.trim() && e.target.value !== task.title) {
              onRename(task, e.target.value.trim());
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
          <button className="side-open" onClick={() => onOpen(task)} title={t("side_open_chat")}>
            <span className={`side-dot ${liveTitles.includes(task.title) ? "live" : ""}`}>
              {DOT[task.status]}
            </span>
            {task.pinned && <span className="side-pin"><Pin size={10} /></span>}
            <span className="side-title">{task.title}</span>
            {/* One glance says where the WORK is: uncommitted files, or
                commits that never left this machine. Silent when clean —
                a row of green ticks would be noise. */}
            {(() => {
              const repo = repoFor?.(task);
              if (!repo || (repo.dirty === 0 && repo.ahead === 0)) return null;
              if (repo.dirty > 0) {
                return (
                  <span className="side-git warn" title={t("repo_dirty", { n: repo.dirty })}>
                    {repo.dirty}<GitBranch size={10} />
                  </span>
                );
              }
              return (
                <span className="side-git" title={t("repo_ahead", { n: repo.ahead })}>
                  {repo.ahead}<ArrowUp size={10} />
                </span>
              );
            })()}
          </button>
          <button
            className="side-menu-btn"
            onClick={() => setMenu(menu === task.title ? null : task.title)}
            title={t("side_actions")}
          >
            <MoreVertical size={13} />
          </button>
        </>
      )}

      {menu === task.title && (
        <div className="side-menu">
          <button onClick={() => { setMenu(null); onOpen(task); }}>{t("side_open")}</button>
          <button onClick={() => { setMenu(null); onResume(task); }}>{t("side_resume")}</button>
          <button onClick={() => { setMenu(null); setRenaming(task.title); }}>
            {t("side_rename")}
          </button>
          <button onClick={() => { setMenu(null); onPin(task); }}>
            {task.pinned ? t("side_unpin") : t("side_pin")}
          </button>
          <button className="danger" onClick={() => { setMenu(null); onArchive(task); }}>
            {t("side_archive")}
          </button>
        </div>
      )}
    </div>
  );

  return (
    <nav className="sidebar" onMouseLeave={() => { setMenu(null); setProjMenu(null); }}>
      {onToggleBoard && (
        <button
          className={`side-board ${boardOpen ? "on" : ""}`}
          onClick={onToggleBoard}
          title={t("side_board_hint")}
        >
          <SquareKanban size={13} /> {t("side_board")}
        </button>
      )}
      {onOpenGeneral && (
        <button
          className={`side-board ${generalActive ? "on" : ""}`}
          onClick={onOpenGeneral}
          title={t("side_general_hint")}
        >
          <MessageSquare size={13} /> {t("side_general")}
        </button>
      )}
      {projects.map((p) => {
        const group = ordered.filter((t) => inProject(t, p));
        // Full Claude Code history of this project (chats not yet on the
        // board), reachable through the search box — like `claude --resume`,
        // but typed. Empty query shows the 5 most recent.
        const projChats = chats.filter(
          (c) => c.cwd && (c.cwd === p.path || c.cwd.startsWith(`${p.path}/`)),
        );
        const query = search?.path === p.path ? search.q : null;
        const matches =
          query === null
            ? []
            : query.trim() === ""
              ? projChats.slice(0, 5)
              : projChats.filter((c) => chatMatches(c, query)).slice(0, 8);
        return (
          <section key={p.path} className="side-group">
            <div className="side-group-head">
              <span className="side-group-name" title={p.path}>
                {p.name}
              </span>
              <button
                className="side-group-add"
                title={t("side_new_chat", { name: p.name })}
                onClick={() => onNewChat(p)}
              >
                <Plus size={12} />
              </button>
              <button
                className="side-group-add"
                title={t("side_files")}
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
                  <button
                    className="danger"
                    onClick={() => { setProjMenu(null); onRemoveProject(p); }}
                  >
                    {t("side_remove")}
                  </button>
                </div>
              )}
            </div>
            {group.length === 0 && projChats.length === 0 && (
              <div className="side-empty">{t("side_empty")}</div>
            )}
            {group.map(item)}

            {projChats.length > 0 && (
              <div className="side-search">
                <input
                  placeholder={t("side_search", { n: projChats.length })}
                  value={query ?? ""}
                  onFocus={() => setSearch({ path: p.path, q: query ?? "" })}
                  // Rows use onMouseDown (fires before blur), so closing
                  // here never eats the click.
                  onBlur={() => setSearch(null)}
                  onChange={(e) => setSearch({ path: p.path, q: e.target.value })}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") (e.target as HTMLInputElement).blur();
                    if (e.key === "Enter" && matches[0]) {
                      onOpenChat?.(matches[0]);
                      (e.target as HTMLInputElement).blur();
                    }
                  }}
                />
                {query !== null && (
                  <div className="side-search-drop">
                    {matches.length === 0 && (
                      <div className="side-empty">{t("side_no_match")}</div>
                    )}
                    {matches.map((c) => (
                      <button
                        key={c.session_id}
                        className="side-chat"
                        title={c.last_prompt ?? c.title}
                        onMouseDown={(e) => {
                          e.preventDefault();
                          onOpenChat?.(c);
                          setSearch(null);
                        }}
                      >
                        <span className="side-title">{c.title}</span>
                        <span className="side-chat-ts">
                          {c.last_ts ? c.last_ts.slice(5, 10).replace("-", "/") : ""}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </section>
        );
      })}

      {leftovers.length > 0 && (
        <section className="side-group">
          <div className="side-group-head">
            <span className="side-group-name">{t("side_others")}</span>
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
          {t("side_add_project")}
        </button>
      )}
    </nav>
  );
}
