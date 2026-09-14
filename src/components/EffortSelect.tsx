import { useLayoutEffect, useRef } from "react";
import { PillPopover } from "./PillMenu";
import { t } from "../lib/i18n";

/** `claude --effort`, low to max. An ordinal scale, so it gets a ruler
 *  instead of a list: the shape says "one axis, five steps". */
const LEVELS = ["low", "medium", "high", "xhigh", "max"] as const;

const NAME = {
  low: "effort_low",
  medium: "effort_medium",
  high: "effort_high",
  xhigh: "effort_xhigh",
  max: "effort_max",
} as const;

/**
 * The effort pill, third in the composer row. Until now `--effort` was
 * reachable only by SAYING "capricha" or "rápido" — and those phrases
 * cover three of the five levels, so medium and xhigh had no way in.
 *
 * "" is not a sixth level: it is the absence of the flag, which hands the
 * choice back to the CLI. That is why the ruler can be empty, and why the
 * reset is a separate affordance rather than a stop on the axis.
 */
export default function EffortSelect({
  value,
  appliesTo,
  onSelect,
  disabled,
}: {
  /** Current level, or "" when no --effort is passed at all. */
  value: string;
  /** Focused live task name, when the change applies to it. */
  appliesTo?: string;
  onSelect: (effort: string) => void;
  /** The reason the pill is off for the agent in this chat (its plugin
   *  carries no directives); the pill stays, disabled, with this hover. */
  disabled?: string;
}) {
  const at = LEVELS.indexOf(value as (typeof LEVELS)[number]);
  const set = at >= 0;

  return (
    <PillPopover
      label={set ? t(NAME[LEVELS[at]]) : t("effort_label")}
      title={appliesTo ? t("effort_pill_task", { name: appliesTo }) : t("effort_pill_window")}
      disabled={disabled}
      dim={!set}
    >
      {(close) => (
        <Ruler
          at={at}
          set={set}
          head={
            appliesTo
              ? t("effort_menu_task", { name: appliesTo.slice(0, 26) })
              : t("effort_menu_new")
          }
          onSelect={onSelect}
          close={close}
        />
      )}
    </PillPopover>
  );
}

function Ruler({
  at,
  set,
  head,
  onSelect,
  close,
}: {
  at: number;
  set: boolean;
  head: string;
  onSelect: (effort: string) => void;
  close: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  // Focus so Esc and the arrow keys reach the ruler, not the composer.
  useLayoutEffect(() => {
    (box.current?.querySelector("input") ?? box.current)?.focus();
  }, []);

  return (
    <div
      className="mode-menu effort-pop"
      ref={box}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          close();
        }
      }}
    >
      <div className="mode-menu-head">{head}</div>
      <div className="eff-value">{set ? t(NAME[LEVELS[at]]) : t("effort_default")}</div>
      <div className="eff-ends">
        <span>{t("effort_faster")}</span>
        <span>{t("effort_smarter")}</span>
      </div>
      <div className="eff-track">
        <div className="eff-dots">
          {LEVELS.map((level, i) => (
            // The dots sit on the THUMB's travel, not on even thirds of the
            // track: a native thumb moves between its own half-widths.
            <i key={level} style={{ left: `calc(13px + (100% - 26px) * ${i} / 4)` }} />
          ))}
        </div>
        <input
          className={`eff-range${set ? "" : " unset"}`}
          type="range"
          min={0}
          max={LEVELS.length - 1}
          step={1}
          // Unset parks the (hidden) thumb in the middle, so the first click
          // anywhere on the track lands where the pointer is.
          value={set ? at : 2}
          onChange={(e) => onSelect(LEVELS[Number(e.target.value)])}
        />
      </div>
      <div className="eff-foot">
        <span>{set ? t("effort_cost") : t("effort_default_hint")}</span>
        {set && (
          <button className="eff-reset" onClick={() => onSelect("")}>
            {t("menu_default")}
          </button>
        )}
      </div>
    </div>
  );
}
