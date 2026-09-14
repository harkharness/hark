import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Check, ChevronUp } from "lucide-react";
import { t } from "../lib/i18n";

/**
 * The composer's pill popovers — mode, model, effort. One shell, so the
 * three behave identically: click-outside and Esc close, digits pick,
 * arrows walk, Enter confirms.
 *
 * They open UPWARD (the pills live at the bottom of the window), so the
 * keyboard reads in DOM order: Down goes to the row below on screen.
 */
export function PillPopover({
  label,
  title,
  dim,
  disabled,
  children,
}: {
  /** Pill text — the current value, spelled the way the menu spells it. */
  label: string;
  /** Pill tooltip: which scope this change lands on. */
  title: string;
  /** True when nothing was chosen and the CLI's own default is in force. */
  dim?: boolean;
  /** The reason this pill cannot work for the agent in the chat (the
   *  plugin lacks the feature). The pill stays in place, disabled, and
   *  the reason is its hover — never hidden, never a click that does
   *  nothing. */
  disabled?: string;
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);

  // Closing on mouseleave (what these pills did before) fires the moment
  // the pointer crosses the gap between pill and menu. A real dismiss is
  // a click somewhere else, or Esc.
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      if (!anchor.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [open]);

  return (
    <span className="mode-anchor" ref={anchor}>
      <button
        className={`mode-pill${dim ? " unset" : ""}${disabled ? " unsupported" : ""}`}
        title={disabled ?? title}
        disabled={!!disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {label} <ChevronUp size={11} />
      </button>
      {open && !disabled && children(() => setOpen(false))}
    </span>
  );
}

/** One row of a pill menu. */
export type PillItem = {
  /** Stable value. "" is a legitimate one — "auto", "padrão". */
  value: string;
  name: string;
  hint?: string;
  /** The whole sentence, when `hint` is the one-line version of it. */
  title?: string;
  /** Marks the window default while the menu is aimed at a live task. */
  isDefault?: boolean;
  /** The hint is a warning, not a description (bypass). */
  danger?: boolean;
};

/**
 * The list body: header, rows, a check on the chosen one. Every row is
 * name + one-line hint + its digit — the hint NEVER wraps, which is what
 * keeps the rhythm even and the menu scannable.
 */
export function PillMenuBody({
  head,
  items,
  value,
  onPick,
  close,
}: {
  head: string;
  items: PillItem[];
  value: string;
  onPick: (value: string) => void;
  close: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [cursor, setCursor] = useState(() => Math.max(0, items.findIndex((i) => i.value === value)));

  // The menu takes focus so digits reach it instead of the composer.
  useLayoutEffect(() => {
    box.current?.focus();
  }, []);

  const pick = (v: string) => {
    close();
    onPick(v);
  };

  return (
    <div
      className="mode-menu"
      ref={box}
      tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          close();
          return;
        }
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          const step = e.key === "ArrowDown" ? 1 : -1;
          setCursor((c) => (c + step + items.length) % items.length);
          return;
        }
        if (e.key === "Enter") {
          e.preventDefault();
          pick(items[cursor].value);
          return;
        }
        const digit = Number(e.key);
        if (Number.isInteger(digit) && digit >= 1 && digit <= items.length) {
          e.preventDefault();
          pick(items[digit - 1].value);
        }
      }}
    >
      <div className="mode-menu-head">{head}</div>
      {items.map((item, i) => (
        <button
          key={item.value || "-"}
          className={`mm-row${item.value === value ? " on" : ""}${i === cursor ? " cursor" : ""}`}
          title={item.title}
          onMouseEnter={() => setCursor(i)}
          onClick={() => pick(item.value)}
        >
          <span className="mm-text">
            <span className="mm-name">{item.name}</span>
            {item.hint && (
              <span className={`mm-hint${item.danger ? " danger" : ""}`}>{item.hint}</span>
            )}
          </span>
          {item.isDefault && <span className="mm-tag">{t("menu_default")}</span>}
          {item.value === value && (
            <span className="mm-check">
              <Check size={13} strokeWidth={3} />
            </span>
          )}
          <span className="mm-key">{i + 1}</span>
        </button>
      ))}
    </div>
  );
}

/** Pill + list, the shape mode and model both use. */
export default function PillMenu({
  label,
  title,
  head,
  items,
  value,
  onPick,
  dim,
  disabled,
}: {
  label: string;
  title: string;
  head: string;
  items: PillItem[];
  value: string;
  onPick: (value: string) => void;
  dim?: boolean;
  /** See PillPopover: the reason the pill is off for this agent. */
  disabled?: string;
}): ReactNode {
  return (
    <PillPopover label={label} title={title} dim={dim} disabled={disabled}>
      {(close) => (
        <PillMenuBody head={head} items={items} value={value} onPick={onPick} close={close} />
      )}
    </PillPopover>
  );
}
