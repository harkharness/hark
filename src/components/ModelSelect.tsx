import PillMenu, { type PillItem } from "./PillMenu";
import { shortModel } from "../lib/format";
import { t } from "../lib/i18n";

/** The four tiers Hark routes between; names come from config [models]
 *  (with the router's own defaults as fallback — intent.rs). */
export type ModelTiers = { light: string; standard: string; heavy: string; max: string };

/** The router's own defaults (intent.rs). Both the project windows and
 *  the mother read them from here so the two pills cannot offer
 *  different models. */
export const DEFAULT_TIERS: ModelTiers = {
  light: "haiku",
  standard: "sonnet",
  heavy: "opus",
  max: "fable",
};

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
  windowDefault,
  onSelect,
}: {
  /** Selected model ("" = auto/router). */
  value: string;
  /** Model that produced the last turn — the truth the pill shows. */
  liveModel?: string | null;
  tiers: ModelTiers;
  appliesTo?: string;
  windowDefault?: string;
  onSelect: (model: string) => void;
}) {
  const shown =
    shortModel(liveModel ?? (value || undefined)) === "?"
      ? t("model_auto")
      : shortModel(liveModel ?? value);
  const items: PillItem[] = [
    { value: "", name: t("model_auto"), hint: t("model_auto_hint"), title: t("model_auto_title") },
    { value: tiers.light, name: shortModel(tiers.light), hint: t("tier_light") },
    { value: tiers.standard, name: shortModel(tiers.standard), hint: t("tier_standard") },
    { value: tiers.heavy, name: shortModel(tiers.heavy), hint: t("tier_heavy") },
    { value: tiers.max, name: shortModel(tiers.max), hint: t("tier_max") },
  ].map((i) => ({ ...i, isDefault: !!appliesTo && i.value === (windowDefault ?? "") }));

  return (
    <PillMenu
      label={shown}
      title={appliesTo ? t("model_pill_task", { name: appliesTo }) : t("model_pill_window")}
      head={
        appliesTo ? t("model_menu_task", { name: appliesTo.slice(0, 26) }) : t("model_menu_new")
      }
      items={items}
      value={value}
      onPick={onSelect}
    />
  );
}
