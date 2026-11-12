import { useEffect, useState } from "react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { LimitWindow, StatusLine } from "../types";

/** A limit window as the popover names it: 5 hours, weekly, weekly per model. */
export function limitLabel(key: string): string {
  if (key === "five_hour" || key === "5h") return t("usage_5h");
  if (key === "seven_day" || key === "7d") return t("usage_week");
  return t("usage_week_model", { m: key.replace(/^(seven_day|7d)_?/, "") });
}

/** "Resets at 13:20" today, "Resets Tue, 17:00" otherwise. */
export function resetLabel(raw: string | null | undefined, now = new Date()): string | undefined {
  if (!raw) return undefined;
  const ms = /^\d+$/.test(raw) ? Number(raw) * 1000 : Date.parse(raw);
  if (!Number.isFinite(ms)) return undefined;
  const d = new Date(ms);
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", hour12: false });
  if (d.toDateString() === now.toDateString()) return t("usage_resets", { t: time });
  return t("usage_resets_day", { d: d.toLocaleDateString(undefined, { weekday: "short" }), t: time });
}

/**
 * The usage popover of the chat's cost pill, the way the Claude Code app
 * shows it: one bar per subscription window, the percentage, when it
 * resets, and a way to the full costs page. The session bar is another
 * thing (environment, outputs, changes) and keeps its own button.
 */
export default function UsagePopover({ limits, onAll, onClose }: { limits?: StatusLine | null; onAll: () => void; onClose: () => void }) {
  const [data, setData] = useState<StatusLine | null | undefined>(limits);
  useEffect(() => {
    if (limits !== undefined) return;
    let live = true;
    ipc
      .subscriptionLimits()
      .then((l) => live && setData(l))
      .catch(() => live && setData(null));
    return () => {
      live = false;
    };
  }, [limits]);
  const rows: LimitWindow[] = data?.limits ?? [];
  return (
    <div className="usage-pop" role="dialog" aria-label={t("usage_title")} onKeyDown={(e) => e.key === "Escape" && onClose()}>
      <div className="usage-head">
        <span>{t("usage_title")}</span>
        <button className="usage-all" onClick={onAll}>
          {t("usage_all")}
        </button>
      </div>
      {data === undefined ? (
        <div className="usage-empty dim">…</div>
      ) : rows.length === 0 ? (
        <div className="usage-empty dim">{t("usage_none")}</div>
      ) : (
        rows.map((l) => {
          const pct = Math.round(Math.max(0, Math.min(1, l.used)) * 100);
          return (
            <div key={l.key} className="usage-row">
              <div className="usage-line">
                <span className="usage-name">{limitLabel(l.key)}</span>
                <span className="usage-meta dim">{resetLabel(l.resets_at)}</span>
                <b className={`usage-pct${pct >= 90 ? " warn" : ""}`}>{pct}%</b>
              </div>
              <div className="usage-bar">
                <i style={{ width: `${pct}%` }} />
              </div>
            </div>
          );
        })
      )}
    </div>
  );
}
