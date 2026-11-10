import { FileText, MessageSquare, SquareKanban } from "lucide-react";
import { t } from "../lib/i18n";
import { when } from "./Demands";
import type { Demand } from "../types";

/** How many rows a box shows before "see all". */
const ROWS = 3;

/**
 * Under the composer on an empty Hoje: the hints, and three boxes with
 * what Claude's home does not have, at a glance: who needs you, what is
 * ready to review, what stalled. Three rows each, title and when, and a
 * way to the full list. Nothing here repeats what Demandas lists: it
 * points there.
 */
export default function InicioGlance({
  demands,
  colourOf,
  onGo,
  onAll,
  onArtifacts,
  onBoard,
  onAsk,
}: {
  demands: Demand[];
  colourOf?: (key: string) => string | undefined;
  onGo: (d: Demand) => void;
  onAll: () => void;
  onArtifacts: () => void;
  onBoard: () => void;
  onAsk: (text: string) => void;
}) {
  const hints: [string, string][] = [
    [t("brief_hint_today"), t("brief_hint_today_text")],
    [t("brief_hint_remind"), t("brief_hint_remind_text")],
    [t("brief_hint_focus"), t("brief_hint_focus_text")],
    [t("brief_hint_page"), t("brief_hint_page_text")],
  ];
  const you = demands.filter((d) => d.urgency === "needs_you");
  const review = demands.filter((d) => d.urgency === "review");
  const stalled = demands.filter((d) => d.urgency === "stalled");
  const box = (icon: React.ReactNode, title: string, list: Demand[], more: string, onMore: () => void) => (
    <div className="glance-box">
      <h3>
        {icon} {title} <span className="n">{list.length}</span>
      </h3>
      {list.slice(0, ROWS).map((d) => {
        const colour = d.project ? colourOf?.(d.project) : undefined;
        return (
          <button key={d.id} className="glance-row" onClick={() => onGo(d)} title={d.why}>
            <span className="demand-dot" style={colour ? { background: colour } : undefined} />
            <span className="t">{d.title}</span>
            <span className="w">{when(d.at)}</span>
          </button>
        );
      })}
      {list.length === 0 && <div className="dim glance-none">—</div>}
      <button className="link-btn glance-more" onClick={onMore}>
        {more}
      </button>
    </div>
  );
  return (
    <div className="glance">
      <div className="brief-hints">
        {hints.map(([label, text]) => (
          <button key={label} className="pill-btn" onClick={() => onAsk(text)}>
            {label}
          </button>
        ))}
      </div>
      {demands.length > 0 && (
        <div className="glance-strip">
          {box(<MessageSquare size={12} />, t("brief_box_you"), you, t("brief_see_n", { n: you.length }), onAll)}
          {box(<FileText size={12} />, t("brief_box_review"), review, t("brief_open_artifacts"), onArtifacts)}
          {box(<SquareKanban size={12} />, t("brief_box_stalled"), stalled, t("brief_open_board"), onBoard)}
        </div>
      )}
    </div>
  );
}
