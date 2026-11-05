import { Sparkles } from "lucide-react";
import Demands from "./Demands";
import { greeting } from "../lib/inicio";
import { lang, t } from "../lib/i18n";
import type { Demand } from "../types";

/** How many demands the brief shows before "ver todas". */
const SHOWN = 5;

/**
 * What Hoje opens with when the day has no chat yet: the greeting, the
 * list of the day and three ways in. Built locally, zero tokens, never
 * written to the journal — the first message is still the person's.
 */
export default function InicioBrief({
  userName,
  demands,
  colourOf,
  onGo,
  onDone,
  onLater,
  onFocus,
  onAll,
  onAsk,
  onArchiveStale,
}: {
  userName: string;
  demands: Demand[];
  colourOf?: (key: string) => string | undefined;
  onGo: (d: Demand) => void;
  onDone: (d: Demand) => void;
  onLater: (d: Demand) => void;
  onFocus: (d: Demand) => void;
  /** The Demandas view, when the list is longer than the brief. */
  onAll: () => void;
  /** A hint clicked: its text lands in the composer. */
  onAsk: (text: string) => void;
  onArchiveStale?: () => void;
}) {
  const you = demands.filter((d) => d.urgency === "needs_you").length;
  const hints: [string, string][] = [
    [t("brief_hint_today"), t("brief_hint_today_text")],
    [t("brief_hint_remind"), t("brief_hint_remind_text")],
    [t("brief_hint_focus"), t("brief_hint_focus_text")],
  ];
  return (
    <div className="brief">
      <div className="brief-greet">
        <Sparkles size={16} /> {greeting(new Date(), userName, lang())}
      </div>
      <div className="brief-line">
        {demands.length === 0 ? t("brief_nothing") : t("brief_demands_n", { n: demands.length, you })}
      </div>
      {demands.length > 0 && (
        <Demands demands={demands.slice(0, SHOWN)} colourOf={colourOf} onGo={onGo} onDone={onDone} onLater={onLater} onFocus={onFocus} onArchiveStale={onArchiveStale} />
      )}
      {demands.length > SHOWN && (
        <button className="link-btn brief-all" onClick={onAll}>
          {t("brief_all", { n: demands.length })} ›
        </button>
      )}
      <div className="brief-hints">
        {hints.map(([label, text]) => (
          <button key={label} className="pill-btn" onClick={() => onAsk(text)}>
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
