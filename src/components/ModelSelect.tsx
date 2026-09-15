import PillMenu, { type PillItem } from "./PillMenu";
import { shortModel } from "../lib/format";
import { t } from "../lib/i18n";
import { tierOf } from "../lib/support";

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
 *
 * The pill speaks in TIERS. A pick travels as "light"/"standard"/"heavy"/
 * "max" and the driver says it in each agent's own vocabulary (the
 * registry's tier table), so one window default drives a claude chat and
 * a gemini chat alike. `tiers` is what THIS chat's agent calls each tier;
 * `global` is the user's table, for reading picks stored as names
 * ("haiku" was light long before tiers travelled).
 */
export default function ModelSelect({
  value,
  liveModel,
  tiers,
  global = tiers,
  appliesTo,
  windowDefault,
  onSelect,
  disabled,
}: {
  /** Selected model: a tier key, a name from the global table, an explicit
   *  id, or "" (auto/router). */
  value: string;
  /** Model that produced the last turn — the truth the pill shows. */
  liveModel?: string | null;
  /** What the chat's agent calls each tier. */
  tiers: ModelTiers;
  /** The user's global table, to read a pick stored as a name. */
  global?: ModelTiers;
  appliesTo?: string;
  windowDefault?: string;
  onSelect: (model: string) => void;
  /** The reason the pill is off for the agent in this chat (its plugin
   *  carries no directives); the pill stays, disabled, with this hover. */
  disabled?: string;
}) {
  // The pick as a tier when it is one; the raw value (an explicit id)
  // otherwise, so the menu matches a row by tier and the label still
  // reads something honest for an id no tier stands for.
  const tier = tierOf(value, global);
  const current = tier ?? value;
  const named = tier ? tiers[tier] : value;
  const shown =
    shortModel(liveModel ?? (named || undefined)) === "?"
      ? t("model_auto")
      : shortModel(liveModel ?? named);
  const defaultTier = tierOf(windowDefault, global) ?? (windowDefault || "");
  const items: PillItem[] = [
    { value: "", name: t("model_auto"), hint: t("model_auto_hint"), title: t("model_auto_title") },
    { value: "light", name: shortModel(tiers.light), hint: t("tier_light") },
    { value: "standard", name: shortModel(tiers.standard), hint: t("tier_standard") },
    { value: "heavy", name: shortModel(tiers.heavy), hint: t("tier_heavy") },
    { value: "max", name: shortModel(tiers.max), hint: t("tier_max") },
  ].map((i) => ({ ...i, isDefault: !!appliesTo && i.value === defaultTier }));

  return (
    <PillMenu
      label={shown}
      title={appliesTo ? t("model_pill_task", { name: appliesTo }) : t("model_pill_window")}
      disabled={disabled}
      head={
        appliesTo ? t("model_menu_task", { name: appliesTo.slice(0, 26) }) : t("model_menu_new")
      }
      items={items}
      value={current}
      onPick={onSelect}
    />
  );
}
