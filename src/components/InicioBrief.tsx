import { Sparkles } from "lucide-react";
import { greeting } from "../lib/inicio";
import { lang, t } from "../lib/i18n";
import type { Demand } from "../types";

/**
 * What Hoje opens with when the day has no chat yet: the greeting and a
 * one-line count. The composer sits right under it (the column centres
 * them), and the glance of the day (InicioGlance) under the composer.
 * Built locally, zero tokens, never written to the journal.
 */
export default function InicioBrief({ userName, demands }: { userName: string; demands: Demand[] }) {
  const you = demands.filter((d) => d.urgency === "needs_you").length;
  const review = demands.filter((d) => d.urgency === "review").length;
  const sub = demands.length === 0 ? t("brief_nothing") : you && review ? t("brief_sub", { you, review }) : you ? t("brief_sub_you", { you }) : review ? t("brief_sub_review", { review }) : t("brief_demands_n", { n: demands.length, you });
  return (
    <div className="brief">
      <div className="brief-greet">
        <Sparkles size={22} /> {greeting(new Date(), userName, lang())}
      </div>
      <div className="brief-line">{sub}</div>
    </div>
  );
}
