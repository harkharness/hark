import { useEffect } from "react";
import { ArrowRight, Bell, Clock, Crosshair } from "lucide-react";
import { t } from "../lib/i18n";
import type { Notification } from "../types";

/** "há 2 min" / "09:14": a time the row can wear. */
function when(iso: string): string {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return "";
  const mins = Math.floor((Date.now() - at.getTime()) / 60_000);
  if (mins < 1) return t("time_now");
  if (mins < 60) return t("time_min", { n: mins });
  return at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/**
 * The bell's panel: unread rows first (strong), read rows under their own
 * heading (they fade from the panel ten minutes after reading), the
 * history behind "ver anteriores". Seeing the panel reads what it shows.
 */
export default function Notifications({
  rows,
  colourOf,
  onGo,
  onFocus,
  onReadAll,
  onHistory,
  onSeen,
}: {
  rows: Notification[];
  colourOf?: (key: string) => string | undefined;
  /** "ir": open what the row points at. */
  onGo: (n: Notification) => void;
  /** "focar": Início's composer talks to that chat (chat rows only). */
  onFocus: (n: Notification) => void;
  onReadAll: () => void;
  /** "ver anteriores": older rows, read, appended below. */
  onHistory: () => void;
  /** The panel is on screen: these unread ids are now read. */
  onSeen: (ids: string[]) => void;
}) {
  const unread = rows.filter((n) => !n.read_at);
  const read = rows.filter((n) => n.read_at);
  useEffect(() => {
    if (unread.length) onSeen(unread.map((n) => n.id));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows.map((n) => n.id).join(",")]);
  const row = (n: Notification, isUnread: boolean) => {
    const title = n.target.kind === "chat" ? n.target.title : n.target.kind === "page" ? n.target.id : n.text;
    const colour = n.project ? colourOf?.(n.project) : undefined;
    return (
      <div key={n.id} className={`notif ${isUnread ? "unread" : ""}`} style={colour ? ({ "--proj": colour } as React.CSSProperties) : undefined}>
        <span className="notif-dot" />
        <span className="notif-body">
          <span className="notif-head">
            <span className="notif-title">{title}</span>
            <span className="notif-when">{when(n.at)}</span>
          </span>
          <span className="notif-text">{n.text}</span>
        </span>
        <span className="notif-acts">
          <button className="pill-btn" onClick={() => onGo(n)}>
            <ArrowRight size={11} /> {t("demand_go")}
          </button>
          {n.target.kind === "chat" && (
            <button className="pill-btn" onClick={() => onFocus(n)}>
              <Crosshair size={11} /> {t("demand_focus")}
            </button>
          )}
        </span>
      </div>
    );
  };
  return (
    <div className="notifs">
      <div className="notifs-head">
        <span className="notifs-title">
          <Bell size={13} /> {t("notif_title")}
          {unread.length > 0 && <span className="notifs-badge">{t("notif_unread_n", { n: unread.length })}</span>}
        </span>
        <button className="link-btn" onClick={onReadAll}>{t("notif_read_all")}</button>
      </div>
      <div className="notifs-list">
        {rows.length === 0 && <div className="notifs-empty">{t("notif_empty")}</div>}
        {unread.map((n) => row(n, true))}
        {read.length > 0 && <div className="notifs-sub">{t("notif_read_heading", { n: 10 })}</div>}
        {read.map((n) => row(n, false))}
      </div>
      <div className="notifs-foot">
        <button className="pill-btn" onClick={onHistory}>
          <Clock size={11} /> {t("notif_history")}
        </button>
      </div>
    </div>
  );
}
