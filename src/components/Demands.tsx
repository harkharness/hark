import { ArrowRight, Check, Clock, Crosshair } from "lucide-react";
import { t } from "../lib/i18n";
import type { Demand } from "../types";

const SOURCE_KEY = {
  board: "demand_src_board",
  agenda: "demand_src_agenda",
  artifact: "demand_src_artifact",
  request: "demand_src_request",
  session: "demand_src_session",
} as const;

/** A board line that counts cards instead of naming one: no chat to focus. */
const isCard = (d: Demand) => d.source === "board" && d.link === null;

/**
 * The list of the day (Início › Demandas): what asks for the person, from
 * the board, the agenda, the artifacts and their own requests. Ordered by
 * the backend; here each row is a line and three verbs.
 */
export default function Demands({
  demands,
  colourOf,
  onGo,
  onDone,
  onLater,
  onFocus,
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
}) {
  if (demands.length === 0) return <div className="demands-empty">{t("demands_empty")}</div>;
  return (
    <div className="demands">
      {demands.map((d) => {
        const colour = d.project ? colourOf?.(d.project) : undefined;
        return (
          <div
            key={d.id}
            className={`demand ${d.urgency}`}
            style={colour ? ({ "--proj": colour } as React.CSSProperties) : undefined}
          >
            <span className="demand-dot" />
            <span className="demand-title">{d.title}</span>
            <span className="demand-why">{d.why}</span>
            <span className="demand-src">{t(SOURCE_KEY[d.source])}</span>
            <span className="demand-acts">
              {onFocus && isCard(d) && (
                <button className="pill-btn" onClick={() => onFocus(d)} title={t("demand_focus_hint")}>
                  <Crosshair size={11} /> {t("demand_focus")}
                </button>
              )}
              {(d.link || d.source === "board") && (
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
  );
}
