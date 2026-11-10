import { Archive, ArrowRight, Check, Clock, Crosshair } from "lucide-react";
import { t } from "../lib/i18n";
import type { Demand } from "../types";

/** A board line that counts cards instead of naming one: no chat to focus. */
const isCard = (d: Demand) => d.source === "board" && d.link === null;

/** The groups of the day, in the order they are read. */
export const GROUPS: Demand["urgency"][] = ["needs_you", "review", "reminder", "scheduled", "working", "stalled"];

/** When a demand happened, short: the hour today, the day otherwise. */
export function when(at: string, now = new Date()): string {
  const d = new Date(at);
  if (!at || Number.isNaN(d.getTime())) return "";
  const sameDay = d.toDateString() === now.toDateString();
  return sameDay
    ? d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
    : d.toLocaleDateString(undefined, { day: "2-digit", month: "2-digit" });
}

/**
 * The list of the day (Início › Demandas): what asks for the person, from
 * the board, the agenda, the artifacts and their own requests. Grouped
 * by what they ask (need you, to review, reminders, scheduled, in
 * progress, stalled) so the group says once what every row used to
 * repeat; a row is the title and when, the why on hover, the verbs on
 * hover too. Ordered by the backend inside each group.
 */
export default function Demands({
  demands,
  colourOf,
  onGo,
  onDone,
  onLater,
  onFocus,
  onArchiveStale,
}: {
  demands: Demand[];
  colourOf?: (key: string) => string | undefined;
  /** "ir": open what the demand points at (a chat, a page). */
  onGo: (d: Demand) => void;
  /** "feito": a request is done; the board's cards are the team's. */
  onDone: (d: Demand) => void;
  /** "depois": sinks to the end of the list for the day. */
  onLater: (d: Demand) => void;
  /** "focar": Início's composer talks to that chat (board demands only). */
  onFocus?: (d: Demand) => void;
  /** "arquivar todos" on the forgotten-cards line. */
  onArchiveStale?: () => void;
}) {
  if (demands.length === 0) return <div className="demands-empty">{t("demands_empty")}</div>;
  const groups = GROUPS.map((g) => [g, demands.filter((d) => d.urgency === g)] as const).filter(([, list]) => list.length > 0);
  return (
    <div className="demands">
      {groups.map(([g, list]) => (
        <div key={g} className="demand-group">
          <h3 className="demand-group-title">
            {t(`demand_group_${g}`)} <span className="demand-group-n">{list.length}</span>
          </h3>
          {list.map((d) => {
            const colour = d.project ? colourOf?.(d.project) : undefined;
            const goes = d.link || d.source === "board";
            return (
              <div
                key={d.id}
                className={`demand ${d.urgency} ${goes ? "goes" : ""}`}
                style={colour ? ({ "--proj": colour } as React.CSSProperties) : undefined}
                title={d.why}
                onClick={() => goes && onGo(d)}
              >
                <span className="demand-dot" />
                <span className="demand-title">{d.title}</span>
                <span className="demand-when">{when(d.at)}</span>
                <span className="demand-acts" onClick={(e) => e.stopPropagation()}>
                  {onFocus && isCard(d) && (
                    <button className="pill-btn" onClick={() => onFocus(d)} title={t("demand_focus_hint")}>
                      <Crosshair size={11} /> {t("demand_focus")}
                    </button>
                  )}
                  {onArchiveStale && d.id === "board:forgotten" && (
                    <button className="pill-btn" onClick={onArchiveStale} title={t("demand_archive_all_hint")}>
                      <Archive size={11} /> {t("demand_archive_all")}
                    </button>
                  )}
                  {goes && (
                    <button className="pill-btn" onClick={() => onGo(d)} title={t("demand_go_hint")}>
                      <ArrowRight size={11} /> {t("demand_go")}
                    </button>
                  )}
                  {d.source === "request" ? (
                    <button className="pill-btn" onClick={() => onDone(d)} title={t("demand_done_hint")}>
                      <Check size={11} /> {t("demand_done")}
                    </button>
                  ) : (
                    <button className="pill-btn" onClick={() => onLater(d)} title={t("demand_later_hint")}>
                      <Clock size={11} /> {t("demand_later")}
                    </button>
                  )}
                </span>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
