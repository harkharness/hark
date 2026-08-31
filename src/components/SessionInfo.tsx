import { useEffect, useState } from "react";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { ContextWeight, SpendAgg, StatusLine } from "../types";

type Stats = { title: string | null; size_mb: number; entries: number; last_ts: string | null };

function resetIn(raw?: string | null): string | undefined {
  if (!raw) return undefined;
  const ms = /^\d+$/.test(raw) ? Number(raw) * 1000 : Date.parse(raw);
  if (!Number.isFinite(ms)) return undefined;
  const mins = Math.round((ms - Date.now()) / 60000);
  if (mins <= 0) return t("reset_now");
  if (mins < 60) return t("reset_min", { m: mins });
  const h = Math.floor(mins / 60);
  return h < 24 ? t("reset_h", { h, m: mins % 60 }) : t("reset_d", { d: Math.round(h / 24) });
}

const limitName = (key: string) =>
  key === "five_hour"
    ? t("lim_5h_short")
    : key === "seven_day"
      ? t("lim_week_short")
      : t("lim_week_model", { m: key.replace(/^seven_day_?/, "") });

/**
 * The costs popover of a project window: the context window of the focused
 * session drawn like the CLI draws it, this project's measured spend, and
 * where inside the project it went. Everything local, zero tokens.
 */
export default function SessionInfo({
  taskTitle,
  sessionId,
  projectName,
  workspace,
  costs,
  onClose,
  onDetail,
}: {
  taskTitle?: string;
  sessionId?: string;
  projectName?: string;
  /** Project root: scopes the ledger to this project (and below). */
  workspace?: string;
  costs: Record<string, number>;
  onClose: () => void;
  /** "Ver detalhamento" → the /usage card lands in the thread. */
  onDetail?: () => void;
}) {
  const [stats, setStats] = useState<Stats | null>(null);
  const [weight, setWeight] = useState<ContextWeight | null>(null);
  const [ledger, setLedger] = useState<{ day: number; week: number } | null>(null);
  const [byTask, setByTask] = useState<SpendAgg[]>([]);
  const [limits, setLimits] = useState<StatusLine | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    ipc.sessionStats(sessionId).then(setStats).catch(() => setStats(null));
    ipc.sessionContextWeight(sessionId).then(setWeight).catch(() => setWeight(null));
  }, [sessionId]);

  useEffect(() => {
    const day = new Date(Date.now() - 24 * 3600e3).toISOString();
    const week = new Date(Date.now() - 7 * 24 * 3600e3).toISOString();
    const sum = (aggs: SpendAgg[]) => aggs.reduce((a, b) => a + b.cost_usd, 0);
    Promise.all([
      ipc.spendSummary(day, "kind", "live", workspace),
      ipc.spendSummary(week, "kind", "live", workspace),
      ipc.spendSummary(week, "label", "live", workspace),
    ])
      .then(([d, w, tasks]) => {
        setLedger({ day: sum(d), week: sum(w) });
        setByTask(tasks.filter((agg) => agg.cost_usd > 0).slice(0, 5));
      })
      .catch(() => {});
    ipc.subscriptionLimits().then(setLimits).catch(() => setLimits(null));
  }, [workspace]);

  const windowTotal = Object.values(costs).reduce((a, b) => a + b, 0);
  const contextSegments: Segment[] = weight
    ? [
        { label: t("seg_cache_read_plain"), value: weight.cache_read, color: "var(--ok)" },
        { label: t("seg_cache_new"), value: weight.cache_created, color: "var(--warn)" },
        { label: t("seg_prompt"), value: weight.input, color: "var(--accent)" },
      ]
    : [];
  const capacity = weight?.context_window ?? undefined;

  return (
    <div className="scope-pop" onMouseLeave={onClose}>
      <div className="scope-head">
        <b>{projectName ? `${t("si_costs")} · ${projectName}` : t("si_costs")}</b>
        <span className="hint">{taskTitle ?? t("si_no_task")}</span>
      </div>

      {weight && weight.last_total_tokens > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("si_ctx")}</span>
            <b>
              {fmtTok(weight.last_total_tokens)}
              {capacity ? ` / ${fmtTok(capacity)}` : ""}
              {weight.pct != null ? ` (${Math.round(weight.pct * 100)}%)` : ""}
            </b>
          </div>
          <StackedBar segments={contextSegments} capacity={capacity} height={9} />
          <Legend segments={contextSegments} capacity={capacity} free={t("si_free")} />
          {weight.pct != null && weight.pct > 0.7 && (
            <p className="hint warn">{t("si_heavy")}</p>
          )}
        </section>
      )}

      {ledger && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("costs_measured")}{projectName ? ` ${t("costs_in_project")}` : ""}</span>
            <b>{fmtUsd(ledger.day)}</b>
          </div>
          <div className="scope-pair">
            <span>{t("si_24h")}</span>
            <b>{fmtUsd(ledger.day)}</b>
          </div>
          <div className="scope-pair">
            <span>{t("si_7d")}</span>
            <b>{fmtUsd(ledger.week)}</b>
          </div>
          <div className="scope-pair">
            <span>{t("si_window")}</span>
            <b>{fmtUsd(windowTotal)}</b>
          </div>
        </section>
      )}

      {byTask.length > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("si_by_task")}</span>
          </div>
          {byTask.map((agg) => (
            <MeterRow
              key={agg.key}
              name={agg.key}
              value={agg.cost_usd}
              max={byTask[0].cost_usd}
              detail={fmtUsd(agg.cost_usd)}
              note={`${agg.turns} ${t("n_turns")}`}
            />
          ))}
        </section>
      )}

      {limits && limits.limits.length > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("c_limits")}</span>
          </div>
          {limits.limits.map((l) => (
            <Gauge
              key={l.key}
              name={limitName(l.key)}
              used={l.used}
              caption={resetIn(l.resets_at)}
            />
          ))}
        </section>
      )}

      {stats && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("si_focused")}</span>
            <b className={stats.size_mb > 2 ? "warn" : ""}>{stats.size_mb.toFixed(1)} MB</b>
          </div>
          <div className="scope-pair">
            <span>{t("si_events")}</span>
            <b>{stats.entries}</b>
          </div>
          {stats.last_ts && (
            <div className="scope-pair">
              <span>{t("si_last")}</span>
              <b>{stats.last_ts.slice(0, 16).replace("T", " ")}</b>
            </div>
          )}
        </section>
      )}

      {onDetail && (
        <button
          className="scope-detail"
          onClick={() => {
            onDetail();
            onClose();
          }}
        >
          {t("si_detail")} →
        </button>
      )}
      <p className="scope-foot">{t("si_foot")}</p>
    </div>
  );
}
