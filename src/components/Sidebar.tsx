import { useState } from "react";
import { Bell, BookOpen, Bug, ChevronDown, ChevronRight, ClipboardList, Code2, Home, Inbox, MoreVertical, Pin, Plus, Search, Settings as SettingsIcon, Sparkles, SquareKanban } from "lucide-react";
import { t } from "../lib/i18n";
import type { BoardTask, Project, SessionHit } from "../types";
import type { Day } from "../lib/inicio";

const DOT: Record<BoardTask["status"], string> = {
  doing: "◍",
  waiting: "◌",
  backlog: "○",
  done: "●",
};

/** Accent/case-insensitive haystack ("migração" matches "migracao"). */
/** The two doors of the window, as in Claude Code: Início is the one
 *  chat of Hark over every project; Code is projects › chats. */
export type Door = "inicio" | "code";

/**
 * Chats grouped by project, like a session sidebar. Clicking a task loads
 * its context into the chat; each project header can spawn a fresh chat;
 * the footer registers new project directories.
 */
export default function Sidebar({
  door = "code",
  onDoor,
  onCreate,
  days = [],
  dayShown,
  onOpenDay,
  demandsCount = 0,
  demandsOpen,
  onOpenDemands,
  unreadTitles = [],
  notifCount = 0,
  onOpenNotifs,
  onDropLink,
  onMoreChats,
  projects,
  colourOf,
  tasks,
  chats = [],
  activeTitle,
  activeSessionId,
  liveTitles,
  liveSessionIds = [],
  unreadSessionIds = [],
  pulsing = [],
  onOpen,
  onOpenChat,
  boardOpen,
  onToggleBoard,
  onOpenSettings,
  settingsOpen,
  onOpenGeneral,
  artifactsOpen,
  artifactsCount,
  onToggleArtifacts,
  onReport,
  generalActive,
  onRename,
  onPin,
  onArchive,
  onResume,
  onNewChat,
  onAddProject,
  onRemoveProject,
  onOpenFiles,
  onSearchChats,
  workspaceName,
}: {
  /** Which door is open; the toggle at the top switches it. */
  door?: Door;
  onDoor?: (door: Door) => void;
  /** "Criar" in the Code door: a new chat in a project (never the general chat). */
  onCreate?: () => void;
  /** The days before today of Hark's chat (Início door), newest first. */
  days?: Day[];
  dayShown?: string | null;
  onOpenDay?: (day: Day) => void;
  /** The list of the day (Início door): how many, and the view's toggle. */
  demandsCount?: number;
  demandsOpen?: boolean;
  onOpenDemands?: () => void;
  /** Chats with something unread behind the bell: strong title, red dot. */
  unreadTitles?: string[];
  notifCount?: number;
  onOpenNotifs?: () => void;
  /** A page dragged from the artifacts tree onto a chat: its link lands there. */
  onDropLink?: (target: { title: string; session_id?: string }, link: string) => void;
  /** Chats with something unread behind the bell, by session. */
  unreadSessionIds?: string[];
  /** "Ver todos os chats" of a project: the palette, filtered to it. */
  onMoreChats?: (project: Project) => void;
  projects: Project[];
  /** The project colour of a path (css token), by registry order. */
  colourOf?: (path: string) => string | undefined;
  tasks: BoardTask[];
  /** Claude Code history of the project (index), minus adopted sessions. */
  chats?: SessionHit[];
  /** Open the history palette — the deep search lives there, not here. */
  onSearchChats?: () => void;
  /** Shown at the top when the window is scoped to one project. */
  workspaceName?: string;
  activeTitle?: string;
  /** The open chat, by session: a history row needs no card to read active. */
  activeSessionId?: string;
  liveTitles: string[];
  /** Sessions with a worker on them: a history row shows the dot by session. */
  liveSessionIds?: string[];
  /** Rows that just received work by voice: they pulse for a beat. */
  pulsing?: string[];
  onOpen: (task: BoardTask) => void;
  /** The artifacts view (main window only): the documentation tree. */
  artifactsOpen?: boolean;
  artifactsCount?: number;
  onToggleArtifacts?: () => void;
  /** Click on a history chat: adopt it as a task and open it. */
  onOpenChat?: (chat: SessionHit) => void;
  /** The board panel is open in the rail (highlights the shortcut). */
  boardOpen?: boolean;
  /** Toggle the board panel — the prominent spot the topbar tab had. */
  onToggleBoard?: () => void;
  /** Settings (with costs inside) live behind the gear at the foot. */
  onOpenSettings?: () => void;
  settingsOpen?: boolean;
  /** Back to the window's general chat — the way out of a task now that
   *  clicking the chat you are already in no longer closes it. */
  onOpenGeneral?: () => void;
  /** The bug report dialog (bottom of the rail, like every desktop app). */
  onReport?: () => void;
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
  const inicio = door === "inicio";
  const RECENT_CHATS = 8;
  // Folded groups, remembered per machine: a project you are not in today
  // takes one line.
  const [collapsed, setCollapsed] = useState<string[]>(() => {
    try {
      const raw = localStorage.getItem("hark.side.collapsed");
      return raw ? (JSON.parse(raw) as string[]) : [];
    } catch {
      return [];
    }
  });
  const toggleCollapsed = (path: string) =>
    setCollapsed((old) => {
      const next = old.includes(path) ? old.filter((p) => p !== path) : [...old, path];
      try {
        localStorage.setItem("hark.side.collapsed", JSON.stringify(next));
      } catch {
        /* private window */
      }
      return next;
    });
  // The chat search open right now: which project and what was typed.
  // Empty query = the 5 most recent; typing digs through the whole index.

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
      className={`side-item ${task.status} ${activeTitle === task.title ? "active" : ""} ${pulsing.includes(task.title) ? "pulse" : ""} ${unreadTitles.includes(task.title) ? "unread" : ""}`}
      onDragOver={(e) => {
        if (onDropLink && e.dataTransfer.types.includes("text/hark-artifact")) e.preventDefault();
      }}
      onDrop={(e) => {
        const link = e.dataTransfer.getData("text/hark-artifact");
        if (link && onDropLink) {
          e.preventDefault();
          onDropLink({ title: task.title, session_id: task.session_ids.at(-1) }, link);
        }
      }}
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

  /** A chat of the index as a row (05/11): the conversation is the unit,
   *  the cards it carries decorate it — status dot, task count — and
   *  opening a chat with a card opens the card, so the thread keeps the
   *  name the board has. */
  const chatRow = (c: SessionHit) => {
    const cards = tasks.filter((task) => task.session_ids.includes(c.session_id));
    const card = cards[0];
    const live = liveSessionIds.includes(c.session_id) || (!!card && liveTitles.includes(card.title));
    const active = activeSessionId === c.session_id || (!!card && activeTitle === card.title);
    const unread = unreadSessionIds.includes(c.session_id) || (!!card && unreadTitles.includes(card.title));
    const pulse = !!card && pulsing.includes(card.title);
    const title = card?.title ?? c.title;
    return (
      <div
        key={c.session_id}
        className={`side-item chat ${card?.status ?? ""} ${active ? "active" : ""} ${pulse ? "pulse" : ""} ${unread ? "unread" : ""}`}
        onDragOver={(e) => {
          if (onDropLink && e.dataTransfer.types.includes("text/hark-artifact")) e.preventDefault();
        }}
        onDrop={(e) => {
          const link = e.dataTransfer.getData("text/hark-artifact");
          if (link && onDropLink) {
            e.preventDefault();
            onDropLink({ title, session_id: c.session_id }, link);
          }
        }}
      >
        <button
          className="side-open"
          onClick={() => (card ? onOpen(card) : onOpenChat?.(c))}
          title={c.last_prompt ?? title}
        >
          <span className={`side-dot ${live ? "live" : ""}`}>{card ? DOT[card.status] : "·"}</span>
          <span className="side-title">{title}</span>
          {cards.length > 0 && (
            <span className="side-tasks" title={t("side_tasks_hint", { n: cards.length })}>
              <ClipboardList size={10} /> {cards.length}
            </span>
          )}
          <span className="side-chat-ts">{c.last_ts ? c.last_ts.slice(5, 10).replace("-", "/") : ""}</span>
        </button>
        {card && (
          <button
            className="side-menu-btn"
            onClick={() => setMenu(menu === card.title ? null : card.title)}
            title={t("side_actions")}
          >
            <MoreVertical size={13} />
          </button>
        )}
        {card && menu === card.title && (
          <div className="side-menu">
            <button onClick={() => { setMenu(null); onOpen(card); }}>{t("side_open")}</button>
            <button onClick={() => { setMenu(null); onResume(card); }}>{t("side_resume")}</button>
            <button onClick={() => { setMenu(null); onPin(card); }}>
              {card.pinned ? t("side_unpin") : t("side_pin")}
            </button>
            <button className="danger" onClick={() => { setMenu(null); onArchive(card); }}>
              {t("side_archive")}
            </button>
          </div>
        )}
      </div>
    );
  };

  return (
    <nav className="sidebar" onMouseLeave={() => { setMenu(null); setProjMenu(null); }}>
      {/* The slot under the traffic lights. An element rather than padding
          because only an element can carry the drag attribute: this is one
          of the two places that move the window (docs/CHROME.md). */}
      <div className="side-drag" data-tauri-drag-region />
      {/* Everything but the drag slot and the foot scrolls here, so the
          foot never floats over a row: it is the last thing, outside the
          scroll, with the window's own bottom under it. */}
      <div className="side-scroll">
      {onDoor && (
        <div className="side-door" role="tablist">
          <button
            role="tab"
            className={`side-door-btn ${door === "inicio" ? "on" : ""}`}
            onClick={() => onDoor("inicio")}
            title={t("door_inicio_hint")}
          >
            <Home size={12} /> {t("door_inicio")}
          </button>
          <button
            role="tab"
            className={`side-door-btn ${door === "code" ? "on" : ""}`}
            onClick={() => onDoor("code")}
            title={t("door_code_hint")}
          >
            <Code2 size={12} /> {t("door_code")}
          </button>
        </div>
      )}
      {workspaceName && (
        <div className="side-head">
          <span className="side-head-name" title={workspaceName}>
            {workspaceName}
          </span>
          {onSearchChats && (
            <button
              className="side-head-btn"
              title={t("side_search", { n: chats.length })}
              onClick={onSearchChats}
            >
              <Search size={15} />
            </button>
          )}
        </div>
      )}
      <div className="side-nav">
      {inicio && onOpenGeneral && (
        <button
          className={`side-board ${generalActive ? "on" : ""}`}
          onClick={onOpenGeneral}
          title={t("side_today_hint")}
        >
          <Sparkles size={13} /> {t("side_today")}
        </button>
      )}
      {inicio && onOpenDemands && (
        <button
          className={`side-board ${demandsOpen ? "on" : ""}`}
          onClick={onOpenDemands}
          title={t("side_demands_hint")}
        >
          <Inbox size={13} /> {t("side_demands")}
          {demandsCount ? <span className="side-count">{demandsCount}</span> : null}
        </button>
      )}
      {inicio && onOpenNotifs && notifCount > 0 && (
        <button className="side-board" onClick={onOpenNotifs} title={t("side_notifs_hint")}>
          <Bell size={13} /> {t("side_notifs")}
          <span className="side-count" style={{ color: "var(--err, #f7768e)", fontWeight: 700 }}>{notifCount}</span>
        </button>
      )}
      {inicio && onToggleBoard && (
        <button
          className={`side-board ${boardOpen ? "on" : ""}`}
          onClick={onToggleBoard}
          title={t("side_board_inicio_hint")}
        >
          <SquareKanban size={13} /> {t("side_board")}
        </button>
      )}
      {!inicio && onCreate && (
        <button className="side-board side-create" onClick={onCreate} title={t("side_create_hint")}>
          <Plus size={13} /> {t("side_create")}
        </button>
      )}
      {!inicio && onSearchChats && (
        <button className="side-board" onClick={onSearchChats} title={t("side_search_hint")}>
          <Search size={13} /> {t("side_search_row")}
        </button>
      )}
      {onToggleArtifacts && (
        <button
          className={`side-board ${artifactsOpen ? "on" : ""}`}
          onClick={onToggleArtifacts}
          title={t("side_artifacts_hint")}
        >
          <BookOpen size={13} /> {t("side_artifacts")}
          {artifactsCount ? <span className="side-count">{artifactsCount}</span> : null}
        </button>
      )}
      {!inicio && onToggleBoard && (
        <button
          className={`side-board ${boardOpen ? "on" : ""}`}
          onClick={onToggleBoard}
          title={t("side_board_hint")}
        >
          <SquareKanban size={13} /> {t("side_board")}
        </button>
      )}
      </div>
      {inicio && days.length > 0 && (
        <section className="side-group">
          <div className="side-group-head">
            <span className="side-group-name">{t("side_days")}</span>
          </div>
          {days.map((d) => (
            <button
              key={d.date}
              className={`side-chat ${dayShown === d.date ? "on" : ""}`}
              title={d.first}
              onClick={() => onOpenDay?.(d)}
            >
              <span className="side-title">{d.label}</span>
              <span className="side-chat-ts">{d.count}</span>
            </button>
          ))}
        </section>
      )}
      {!inicio && projects.map((p) => {
        // Rows are chats (05/11): the project's recent sessions from the
        // index, decorated by the cards they carry. Only pinned cards keep
        // a row of their own, at the top; the rest live on the board.
        const group = ordered.filter((t) => inProject(t, p) && t.pinned);
        const projChats = chats.filter(
          (c) => c.cwd && (c.cwd === p.path || c.cwd.startsWith(`${p.path}/`)),
        );
        // The most recent, always. Search moved to the palette.
        const matches = projChats
          .filter((c) => !group.some((task) => task.session_ids.includes(c.session_id)))
          .slice(0, RECENT_CHATS);
        const colour = colourOf?.(p.path);
        const folded = collapsed.includes(p.path);
        return (
          <section
            key={p.path}
            className="side-group"
            style={colour ? ({ "--proj": colour } as React.CSSProperties) : undefined}
          >
            <div className="side-group-head">
              <button
                className="side-group-caret"
                onClick={() => toggleCollapsed(p.path)}
                title={folded ? t("side_expand") : t("side_collapse")}
                aria-expanded={!folded}
              >
                {folded ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
              </button>
              <span className="side-group-name" title={p.path} onClick={() => toggleCollapsed(p.path)}>
                {/* In a project window the project name is already the
                    window title, so the group says what the rows ARE. */}
                {projects.length === 1 ? t("side_chats", { n: projChats.length }) : p.name}
              </span>
              {folded && group.length > 0 && <span className="side-count">{group.length}</span>}
              <button
                className="side-group-add"
                title={t("side_new_chat", { name: p.name })}
                onClick={() => onNewChat(p)}
              >
                <Plus size={12} />
              </button>
              <button
                className="side-menu-btn"
                title={t("side_actions")}
                onClick={() => setProjMenu(projMenu === p.path ? null : p.path)}
              >
                <MoreVertical size={13} />
              </button>
              {projMenu === p.path && (
                <div className="side-menu">
                  <button onClick={() => { setProjMenu(null); onNewChat(p); }}>{t("side_menu_new")}</button>
                  <button onClick={() => { setProjMenu(null); onOpenFiles(p); }}>{t("side_menu_files")}</button>
                  {onMoreChats && (
                    <button onClick={() => { setProjMenu(null); onMoreChats(p); }}>
                      {t("side_menu_all", { n: projChats.length })}
                    </button>
                  )}
                  <div className="side-menu-sep" />
                  <button onClick={() => { setProjMenu(null); toggleCollapsed(p.path); }}>
                    {folded ? t("side_expand") : t("side_collapse")}
                  </button>
                  <div className="side-menu-sep" />
                  <button
                    className="danger"
                    onClick={() => { setProjMenu(null); onRemoveProject(p); }}
                  >
                    {t("side_remove")}
                  </button>
                </div>
              )}
            </div>
            {!folded && (
              <>
                {group.length === 0 && projChats.length === 0 && (
                  <div className="side-empty">{t("side_empty")}</div>
                )}
                {group.map(item)}

                {/* The sidebar shows the RECENT chats; digging through the
                    whole history is the palette's job (the magnifier up top),
                    which gets the window's attention instead of a dropdown
                    inside a 260px rail. */}
                {matches.map(chatRow)}
              </>
            )}
          </section>
        );
      })}

      {!inicio && leftovers.length > 0 && (
        <section className="side-group">
          <div className="side-group-head">
            <span className="side-group-name">{t("side_others")}</span>
          </div>
          {leftovers.map(item)}
        </section>
      )}

      {inicio ? null : adding ? (
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
      </div>
      {(onReport || onOpenSettings) && (
        <div className="side-foot">
          {onReport && (
            <button
              className="side-head-btn"
              onClick={onReport}
              title={t("report_title")}
              aria-label={t("report_title")}
            >
              <Bug size={15} />
            </button>
          )}
          {onOpenSettings && (
            <button
              className={`side-head-btn ${settingsOpen ? "on" : ""}`}
              onClick={onOpenSettings}
              title={t("settings_btn")}
              aria-label={t("settings_btn")}
            >
              <SettingsIcon size={15} />
            </button>
          )}
        </div>
      )}
    </nav>
  );
}
