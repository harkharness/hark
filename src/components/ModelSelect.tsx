import { useState } from "react";
import { ChevronLeft } from "lucide-react";
import { PillMenuBody, PillPopover, type PillItem } from "./PillMenu";
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
/** `claude --effort`, low to max; "" is the CLI's own default. */
const EFFORTS = ["low", "medium", "high", "xhigh", "max"] as const;
const EFFORT_NAME = { low: "effort_low", medium: "effort_medium", high: "effort_high", xhigh: "effort_xhigh", max: "effort_max" } as const;
const EFFORT_HINT = { low: "effort_low_hint", medium: "effort_medium_hint", high: "effort_high_hint", xhigh: "effort_xhigh_hint", max: "effort_max_hint" } as const;

/** "haiku" reads as a word in a menu: first letter up, the rest as the CLI says it. */
const cap = (name: string) => (name ? name[0].toUpperCase() + name.slice(1) : name);

export default function ModelSelect({
  value,
  liveModel,
  tiers,
  global = tiers,
  appliesTo,
  windowDefault,
  onSelect,
  disabled,
  effort = "",
  onSelectEffort,
  effortOff,
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
  /** The effort in force ("" = the CLI's default); rides in the same
   *  control, as a submenu, the way the Claude Code app does. */
  effort?: string;
  onSelectEffort?: (effort: string) => void;
  /** The reason effort is off for this agent: the row stays, disabled. */
  effortOff?: string;
}) {
  const [page, setPage] = useState<"models" | "effort">("models");
  // The pick as a tier when it is one; the raw value (an explicit id)
  // otherwise, so the menu matches a row by tier and the label still
  // reads something honest for an id no tier stands for.
  const tier = tierOf(value, global);
  const current = tier ?? value;
  const named = tier ? tiers[tier] : value;
  const shown =
    shortModel(liveModel ?? (named || undefined)) === "?"
      ? t("model_auto")
      : cap(shortModel(liveModel ?? named));
  const defaultTier = tierOf(windowDefault, global) ?? (windowDefault || "");
  const effortSet = (EFFORTS as readonly string[]).includes(effort);
  const effortLabel = effortSet ? t(EFFORT_NAME[effort as (typeof EFFORTS)[number]]) : t("effort_default");
  const items: PillItem[] = [
    { value: "", name: t("model_auto"), hint: t("model_auto_hint"), title: t("model_auto_title") },
    { value: "light", name: cap(shortModel(tiers.light)), hint: t("tier_light") },
    { value: "standard", name: cap(shortModel(tiers.standard)), hint: t("tier_standard") },
    { value: "heavy", name: cap(shortModel(tiers.heavy)), hint: t("tier_heavy") },
    { value: "max", name: cap(shortModel(tiers.max)), hint: t("tier_max") },
  ].map((i) => ({ ...i, isDefault: i.value === defaultTier }));
  if (onSelectEffort) {
    items.push({ value: "__effort", name: t("menu_effort"), submenu: effortLabel, disabled: effortOff });
  }
  const effortItems: PillItem[] = [
    ...EFFORTS.map((e) => ({ value: e, name: t(EFFORT_NAME[e]), hint: t(EFFORT_HINT[e]), tag: e === "high" ? t("effort_recommended") : undefined })),
    { value: "", name: t("effort_default"), hint: t("effort_default_hint") },
  ];

  return (
    <PillPopover
      label={effortSet ? `${shown} · ${effortLabel}` : shown}
      title={appliesTo ? t("model_pill_task", { name: appliesTo }) : t("model_pill_window")}
      disabled={disabled}
    >
      {(close) =>
        page === "effort" ? (
          <div className="mode-menu-stack">
            <button className="mm-back" onClick={() => setPage("models")}>
              <ChevronLeft size={13} /> {t("menu_back")}
            </button>
            <PillMenuBody
              head={t("menu_effort")}
              note={t("effort_menu_hint")}
              items={effortItems}
              value={effort}
              onPick={(v) => {
                setPage("models");
                onSelectEffort?.(v);
              }}
              close={close}
            />
          </div>
        ) : (
          <PillMenuBody
            items={items}
            value={current}
            onPick={(v) => (v === "__effort" ? setPage("effort") : onSelect(v))}
            close={() => {
              setPage("models");
              close();
            }}
          />
        )
      }
    </PillPopover>
  );
}
