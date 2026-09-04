import { useState } from "react";
import { ChevronUp } from "lucide-react";
import { shortModel } from "../lib/format";
import { t } from "../lib/i18n";

/** The four tiers Hark routes between; names come from config [models]
 *  (with the router's own defaults as fallback — intent.rs). */
export type ModelTiers = { light: string; standard: string; heavy: string; max: string };

/**
 * The model pill beside the mode pill: shows the model actually producing
 * the focused thread's turns; picking one switches THAT task (process
 * restart on the same session), or sets this window's default for new
 * chats when nothing is focused. "auto" hands the choice back to the
 * router (spoken cues can still route UP).
 */
export default function ModelSelect({
  value,
  liveModel,
  tiers,
  appliesTo,
  onSelect,
}: {
  /** Selected model ("" = auto/router). */
  value: string;
  /** Model that produced the last turn — the truth the pill shows. */
  liveModel?: string | null;
  tiers: ModelTiers;
  appliesTo?: string;
  onSelect: (model: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const shown = shortModel(liveModel ?? (value || undefined)) === "?"
    ? t("model_auto")
    : shortModel(liveModel ?? value);
  const options: { model: string; name: string; hint: string }[] = [
    { model: "", name: t("model_auto"), hint: t("model_auto_hint") },
    { model: tiers.light, name: shortModel(tiers.light), hint: t("tier_light") },
    { model: tiers.standard, name: shortModel(tiers.standard), hint: t("tier_standard") },
    { model: tiers.heavy, name: shortModel(tiers.heavy), hint: t("tier_heavy") },
    { model: tiers.max, name: shortModel(tiers.max), hint: t("tier_max") },
  ];

  return (
    <span className="mode-anchor">
      <button
        className="mode-pill"
        title={appliesTo ? t("model_pill_task", { name: appliesTo }) : t("model_pill_window")}
        onClick={() => setOpen((o) => !o)}
      >
        {shown} <ChevronUp size={11} />
      </button>
      {open && (
        <div className="mode-menu" onMouseLeave={() => setOpen(false)}>
          <div className="mode-menu-head">
            {appliesTo ? t("mode_menu_task", { name: appliesTo.slice(0, 26) }) : t("mode_menu_new")}
          </div>
          {options.map((m) => (
            <button
              key={m.model || "auto"}
              className={m.model === value ? "on" : ""}
              onClick={() => {
                setOpen(false);
                onSelect(m.model);
              }}
            >
              <b>{m.name}</b>
              <span>{m.hint}</span>
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
