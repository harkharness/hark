import { useState } from "react";
import { ChevronUp } from "lucide-react";
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
  onSelect,
}: {
  /** Current mode flag ("manual" | "acceptEdits" | "plan" | "auto" | "bypass"). */
  value: string;
  /** Focused live task name, when the change applies to it. */
  appliesTo?: string;
  onSelect: (flag: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const current = MODES.find((m) => m.flag === value) ?? MODES[0];

  return (
    <span className="mode-anchor">
      <button
        className="mode-pill"
        title={
          appliesTo
            ? t("mode_pill_task", { name: appliesTo })
            : t("mode_pill_window")
        }
        onClick={() => setOpen((o) => !o)}
      >
        {t(current.name)} <ChevronUp size={11} />
      </button>
      {open && (
        <div className="mode-menu" onMouseLeave={() => setOpen(false)}>
          <div className="mode-menu-head">
            {appliesTo ? t("mode_menu_task", { name: appliesTo.slice(0, 26) }) : t("mode_menu_new")}
          </div>
          {MODES.map((m) => (
            <button
              key={m.flag}
              className={m.flag === value ? "on" : ""}
              onClick={() => {
                setOpen(false);
                onSelect(m.flag);
              }}
            >
              <b>{t(m.name)}</b>
              <span>{t(m.hint)}</span>
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
