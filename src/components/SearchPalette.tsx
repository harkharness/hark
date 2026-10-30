import { useEffect, useMemo, useState } from "react";
import { BookOpen, FolderGit2, History, MessageSquare, MessageSquarePlus, SquareKanban, Zap } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import { KINDS, sections, type SearchItem, type SearchKind } from "../lib/search";
import type { BoardTask, Project, SessionHit } from "../types";

const TABS: (SearchKind | "all")[] = ["all", ...KINDS];
const TAB_LABEL: Record<SearchKind | "all", string> = {
  all: "search_all",
  task: "search_tasks",
  page: "search_pages",
  project: "search_projects",
  session: "search_sessions",
  action: "search_actions",
};
const KIND_ICON: Record<SearchKind, typeof BookOpen> = {
  task: MessageSquare,
  page: BookOpen,
  project: FolderGit2,
  session: History,
  action: Zap,
};

function ago(iso?: string): string {
  if (!iso) return "";
  const mins = Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60000));
  if (!Number.isFinite(mins)) return "";
  if (mins < 60) return t("time_min", { n: mins });
  if (mins < 1440) return t("time_hour", { n: Math.floor(mins / 60) });
  return t("time_day", { n: Math.floor(mins / 1440) });
}

/**
 * "Procurar": one palette over everything this window knows — the board's
 * chats, the artifact pages, the projects, the Claude Code history and a
 * few actions — with a tab per kind, the way Claude Code's app does it.
 * Before a word is typed it shows what needs you, what moved last and
 * what you can do. Local index, zero tokens. Cmd+K opens it; Cmd+P is
 * the files window.
 */
export default function SearchPalette({
  tasks,
  chats,
  projects,
  attention,
  onOpenTask,
  onOpenChat,
  onOpenProject,
  onOpenPage,
  onAction,
  onClose,
}: {
  tasks: BoardTask[];
  chats: SessionHit[];
  projects: Project[];
  /** Task titles waiting on the person (a permission). */
  attention: Set<string>;
  onOpenTask: (task: BoardTask) => void;
  onOpenChat: (chat: SessionHit) => void;
  onOpenProject: (project: Project) => void;
  onOpenPage: (space: string, id: string) => void;
  onAction: (id: "new-chat" | "board" | "artifacts" | "settings") => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<SearchKind | "all">("all");
  const [sel, setSel] = useState(0);
  const [pages, setPages] = useState<SearchItem[]>([]);

  // Pages come from the index on open: a few reads, no tokens.
  useEffect(() => {
    let live = true;
    ipc
      .artifactsSpaces()
      .then((spaces) => Promise.all(spaces.map((s) => ipc.artifactsTree(s.name).then((tree) => ({ space: s.name, tree })))))
      .then((all) => {
        if (!live) return;
        setPages(
          all.flatMap(({ space, tree }) =>
            tree.nodes.map((n) => ({
              kind: "page" as const,
              id: `${space}/${n.id}`,
              title: n.title,
              sub: [space, ...tree.nodes.filter((p) => p.id === n.parent).map((p) => p.title)].join(" › "),
              when: n.updated_at,
            })),
          ),
        );
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, []);

  const items = useMemo<SearchItem[]>(() => {
    const taskItems: SearchItem[] = tasks.map((task) => ({
      kind: "task",
      id: `task:${task.title}`,
      title: task.title,
      sub: task.note ?? undefined,
      when: task.updated_at,
      attention: attention.has(task.title),
    }));
    const projectItems: SearchItem[] = projects.map((p) => ({ kind: "project", id: `project:${p.path}`, title: p.name, sub: p.path }));
    const sessionItems: SearchItem[] = chats.map((c) => ({
      kind: "session",
      id: `session:${c.session_id}`,
      title: c.title,
      sub: c.last_prompt ?? undefined,
      when: c.last_ts ?? undefined,
    }));
    const actions: SearchItem[] = [
      { kind: "action", id: "action:new-chat", title: t("search_act_new_chat") },
      { kind: "action", id: "action:board", title: t("search_act_board") },
      { kind: "action", id: "action:artifacts", title: t("search_act_artifacts") },
      { kind: "action", id: "action:settings", title: t("search_act_settings") },
    ];
    return [...taskItems, ...pages, ...projectItems, ...sessionItems, ...actions];
  }, [tasks, pages, projects, chats, attention]);

  const groups = useMemo(() => sections(items, query, tab), [items, query, tab]);
  const flat = groups.flatMap((g) => g.items);

  function pick(item: SearchItem) {
    if (item.kind === "task") {
      const task = tasks.find((x) => x.title === item.title);
      if (task) onOpenTask(task);
    } else if (item.kind === "session") {
      const chat = chats.find((c) => `session:${c.session_id}` === item.id);
      if (chat) onOpenChat(chat);
    } else if (item.kind === "project") {
      const p = projects.find((x) => `project:${x.path}` === item.id);
      if (p) onOpenProject(p);
    } else if (item.kind === "page") {
      const at = item.id.indexOf("/");
      onOpenPage(item.id.slice(0, at), item.id.slice(at + 1));
    } else {
      onAction(item.id.replace("action:", "") as "new-chat" | "board" | "artifacts" | "settings");
    }
    onClose();
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="quickopen search-pal" onClick={(e) => e.stopPropagation()}>
        <div className="search-tabs">
          {TABS.map((k) => (
            <button key={k} className={`pill-btn ${tab === k ? "on" : ""}`} onClick={() => { setTab(k); setSel(0); }}>
              {t(TAB_LABEL[k] as "search_all")}
            </button>
          ))}
        </div>
        <input
          autoFocus
          placeholder={t("search_ph")}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSel(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const delta = e.key === "ArrowDown" ? 1 : -1;
              if (flat.length > 0) setSel((sel + delta + flat.length) % flat.length);
            }
            if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
              if (query !== "" && (e.target as HTMLInputElement).selectionStart !== (e.target as HTMLInputElement).value.length) return;
              e.preventDefault();
              const at = TABS.indexOf(tab);
              setTab(TABS[(at + (e.key === "ArrowRight" ? 1 : -1) + TABS.length) % TABS.length]);
              setSel(0);
            }
            if (e.key === "Enter" && flat[sel]) pick(flat[sel]);
          }}
        />
        <div className="quickopen-list">
          {groups.map((g) => (
            <div key={g.id} className="search-group">
              <div className="search-group-head">{t(`search_sec_${g.id}` as "search_sec_recent")}</div>
              {g.items.map((item) => {
                const i = flat.indexOf(item);
                const Icon = KIND_ICON[item.kind];
                return (
                  <button key={item.id} className={i === sel ? "sel" : ""} onMouseEnter={() => setSel(i)} onClick={() => pick(item)}>
                    {item.kind === "action" && item.id === "action:new-chat" ? <MessageSquarePlus size={14} /> : item.id === "action:board" ? <SquareKanban size={14} /> : <Icon size={14} />}
                    <span className="pal-title">
                      {item.title}
                      {item.sub && <span className="pal-sub"> · {item.sub}</span>}
                    </span>
                    {item.attention && <span className="pal-dot" />}
                    <span className="pal-when">{ago(item.when)}</span>
                  </button>
                );
              })}
            </div>
          ))}
          {flat.length === 0 && <div className="side-empty">{t("nothing_found")}</div>}
        </div>
        <div className="search-foot dim">
          <span>Esc · {t("search_close")}</span>
          <span>← → · {t("search_change_type")}</span>
          <span>↵ · {t("search_open")}</span>
        </div>
      </div>
    </div>
  );
}
