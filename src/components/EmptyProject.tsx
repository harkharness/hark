import { useEffect, useState } from "react";
import { LayoutGrid, Mic, Play, Search } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { BoardTask, Project } from "../types";

/** "há 2 h" from an ISO timestamp — coarse on purpose. */
function ago(iso: string): string {
  const mins = Math.floor((Date.now() - Date.parse(iso)) / 60_000);
  if (!Number.isFinite(mins) || mins < 1) return t("time_now");
  if (mins < 60) return t("time_min", { n: mins });
  if (mins < 1440) return t("time_hour", { n: Math.floor(mins / 60) });
  return t("time_day", { n: Math.floor(mins / 1440) });
}

/**
 * The empty chat is an INVITATION, not a void: the project's name, its
 * real numbers, and four doors — resume the latest task, speak work,
 * search the chats, open the board. Disappears as soon as a chat loads.
 */
export default function EmptyProject({
  project,
  board,
  onResume,
  onSpeak,
  onSearch,
  onBoard,
}: {
  project?: Project;
  board: BoardTask[];
  onResume: (task: BoardTask) => void;
  onSpeak: () => void;
  onSearch: () => void;
  onBoard: () => void;
}) {
  const [chats, setChats] = useState<number | null>(null);
  const [hotkey, setHotkey] = useState("cmd+shift+space");
  useEffect(() => {
    if (project) {
      ipc
        .projectSessions(project.path)
        .then((hits) => setChats(hits.length))
        .catch(() => setChats(null));
    }
    ipc
      .configRead()
      .then((snap) => setHotkey(snap.values.hotkey || "cmd+shift+space"))
      .catch(() => {});
  }, [project?.path]);

  const latest = [...board]
    .filter((task) => task.status !== "done")
    .sort((a, b) => b.updated_at.localeCompare(a.updated_at))[0];
  const doing = board.filter((task) => task.status === "doing").length;
  const waiting = board.filter((task) => task.status === "waiting").length;
  const prettyHotkey = hotkey
    .replace(/cmd/i, "⌘")
    .replace(/shift/i, "⇧")
    .replace(/alt|option/i, "⌥")
    .replace(/\+/g, "")
    .replace(/space/i, "Espaço");

  return (
    <div className="empty-project">
      <div className="ep-hero">
        <span className="ep-orb">
          <span style={{ height: 12 }} />
          <span style={{ height: 18 }} />
          <span style={{ height: 10 }} />
        </span>
        <div className="ep-name">{project?.name ?? "vox"}</div>
        <div className="ep-sub">
          {chats != null && `${t("ep_chats", { n: chats })} · `}
          {board.length > 0 && `${t("ep_tasks", { n: board.length })} · `}
          {t("ep_what")}
        </div>
      </div>

      <div className="ep-doors">
        {latest && (
          <button className="ep-door" onClick={() => onResume(latest)}>
            <span className="ep-kicker ok">
              <Play size={11} /> {t("ep_resume")}
            </span>
            <span className="ep-title">{latest.title}</span>
            <span className="ep-hint">
              {ago(latest.updated_at)}
              {latest.session_ids.length > 0 && ` · ${t("chat_session_live")}`}
            </span>
          </button>
        )}
        <button className="ep-door" onClick={onSpeak}>
          <span className="ep-kicker accent">
            <Mic size={11} /> {t("ep_speak")}
          </span>
          <span className="ep-title">{t("ep_speak_ex")}</span>
          <span className="ep-hint">{t("ep_speak_hint", { key: prettyHotkey })}</span>
        </button>
        <button className="ep-door" onClick={onSearch}>
          <span className="ep-kicker purple">
            <Search size={11} /> {t("ep_search")}
          </span>
          <span className="ep-title">
            {chats != null ? t("ep_search_n", { n: chats }) : t("ep_search_any")}
          </span>
          <span className="ep-hint">{t("ep_search_hint")}</span>
        </button>
        <button className="ep-door" onClick={onBoard}>
          <span className="ep-kicker warn">
            <LayoutGrid size={11} /> {t("ep_board")}
          </span>
          <span className="ep-title">{t("ep_board_n", { d: doing, w: waiting })}</span>
          <span className="ep-hint">{t("ep_board_hint")}</span>
        </button>
      </div>
    </div>
  );
}
