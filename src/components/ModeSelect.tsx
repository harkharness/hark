import PillMenu, { type PillItem } from "./PillMenu";
import { t } from "../lib/i18n";

const MODES = [
  { flag: "manual", name: "mode_manual", hint: "mode_manual_hint" },
  { flag: "acceptEdits", name: "mode_accept", hint: "mode_accept_hint" },
  { flag: "plan", name: "mode_plan", hint: "mode_plan_hint" },
  { flag: "auto", name: "mode_auto", hint: "mode_auto_hint" },
  { flag: "bypass", name: "mode_bypass", hint: "mode_bypass_hint" },
] as const;

/**
 * The permission-mode pill (Claude Code's composer selector). With a live
 * worker focused it switches THAT task (process restart, no message);
 * otherwise it sets the default for new tasks born in this window.
 */
export default function ModeSelect({
  value,
  appliesTo,
  windowDefault,
  onSelect,
  disabled,
  bypassOff,
}: {
  /** Current mode flag ("manual" | "acceptEdits" | "plan" | "auto" | "bypass"). */
  value: string;
  /** Focused live task name, when the change applies to it. */
  appliesTo?: string;
  /** This window's default — badged while the menu is aimed at a task. */
  windowDefault?: string;
  onSelect: (flag: string) => void;
  /** The reason the pill is off for the agent in this chat (its plugin
   *  carries no directives); the pill stays, disabled, with this hover. */
  disabled?: string;
  /** The reason bypass alone is off here: its deny floor is claude's and
   *  does not cross ACP. The row stays, disabled, with this hover. */
  bypassOff?: string;
}) {
  const current = MODES.find((m) => m.flag === value) ?? MODES[0];
  const items: PillItem[] = MODES.map((m) => ({
    value: m.flag,
    name: t(m.name),
    hint: t(m.hint),
    // "aceita tudo" is a warning, not a description.
    danger: m.flag === "bypass",
    disabled: m.flag === "bypass" ? bypassOff : undefined,
    isDefault: !!appliesTo && m.flag === windowDefault,
  }));

  return (
    <PillMenu
      label={t(current.name)}
      title={appliesTo ? t("mode_pill_task", { name: appliesTo }) : t("mode_pill_window")}
      disabled={disabled}
      head={appliesTo ? t("mode_menu_task", { name: appliesTo.slice(0, 26) }) : t("mode_menu_new")}
      items={items}
      value={value}
      onPick={onSelect}
    />
  );
}
