import { useEffect, useState } from "react";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import * as ipc from "../lib/ipc";
import type { ContextWeight, SpendAgg, StatusLine } from "../types";

type Stats = { title: string | null; size_mb: number; entries: number; last_ts: string | null };

function resetIn(raw?: string | null): string | undefined {
  if (!raw) return undefined;
  const ms = /^\d+$/.test(raw) ? Number(raw) * 1000 : Date.parse(raw);
  if (!Number.isFinite(ms)) return undefined;
  const mins = Math.round((ms - Date.now()) / 60000);
  if (mins <= 0) return "reinicia agora";
  if (mins < 60) return `reinicia em ${mins}min`;
  const h = Math.floor(mins / 60);
  return h < 24 ? `reinicia em ${h}h ${mins % 60}min` : `reinicia em ${Math.round(h / 24)}d`;
}

const limitName = (key: string) =>
  key === "five_hour"
    ? "5 horas"
    : key === "seven_day"
      ? "semanal"
      : `semanal · ${key.replace(/^seven_day_?/, "")}`;

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
}: {
  taskTitle?: string;
  sessionId?: string;
  projectName?: string;
  /** Project root: scopes the ledger to this project (and below). */
  workspace?: string;
  costs: Record<string, number>;
  onClose: () => void;
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
        setByTask(tasks.filter((t) => t.cost_usd > 0).slice(0, 5));
      })
      .catch(() => {});
    ipc.subscriptionLimits().then(setLimits).catch(() => setLimits(null));
  }, [workspace]);

  const windowTotal = Object.values(costs).reduce((a, b) => a + b, 0);
  const contextSegments: Segment[] = weight
    ? [
        { label: "cache lido", value: weight.cache_read, color: "var(--ok)" },
        { label: "cache escrito", value: weight.cache_created, color: "var(--warn)" },
        { label: "prompt novo", value: weight.input, color: "var(--accent)" },
      ]
    : [];
  const capacity = weight?.context_window ?? undefined;

  return (
    <div className="scope-pop" onMouseLeave={onClose}>
      <div className="scope-head">
        <b>{projectName ? `custos · ${projectName}` : "custos"}</b>
        <span className="hint">{taskTitle ?? "nenhuma task focada"}</span>
      </div>

      {weight && weight.last_total_tokens > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>janela de contexto</span>
            <b>
              {fmtTok(weight.last_total_tokens)}
              {capacity ? ` / ${fmtTok(capacity)}` : ""}
              {weight.pct != null ? ` (${Math.round(weight.pct * 100)}%)` : ""}
            </b>
          </div>
          <StackedBar segments={contextSegments} capacity={capacity} height={9} />
          <Legend segments={contextSegments} capacity={capacity} free="livre" />
          {weight.pct != null && weight.pct > 0.7 && (
            <p className="hint warn">
              sessão pesada: cada turno recarrega esse contexto. Vale recomeçar leve.
            </p>
          )}
        </section>
      )}

      {ledger && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>gasto medido{projectName ? " neste projeto" : ""}</span>
            <b>{fmtUsd(ledger.day)}</b>
          </div>
          <div className="scope-pair">
            <span>últimas 24h</span>
            <b>{fmtUsd(ledger.day)}</b>
          </div>
          <div className="scope-pair">
            <span>últimos 7 dias</span>
            <b>{fmtUsd(ledger.week)}</b>
          </div>
          <div className="scope-pair">
            <span>nesta janela aberta</span>
            <b>{fmtUsd(windowTotal)}</b>
          </div>
        </section>
      )}

      {byTask.length > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>por task · 7 dias</span>
          </div>
          {byTask.map((t) => (
            <MeterRow
              key={t.key}
              name={t.key}
              value={t.cost_usd}
              max={byTask[0].cost_usd}
              detail={fmtUsd(t.cost_usd)}
              note={`${t.turns} turnos`}
            />
          ))}
        </section>
      )}

      {limits && limits.limits.length > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>limites da assinatura</span>
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
            <span>sessão focada</span>
            <b className={stats.size_mb > 2 ? "warn" : ""}>{stats.size_mb.toFixed(1)} MB</b>
          </div>
          <div className="scope-pair">
            <span>eventos no log</span>
            <b>{stats.entries}</b>
          </div>
          {stats.last_ts && (
            <div className="scope-pair">
              <span>última atividade</span>
              <b>{stats.last_ts.slice(0, 16).replace("T", " ")}</b>
            </div>
          )}
        </section>
      )}

      <p className="scope-foot">
        USD medido pelo CLI nos turnos do Vox. Custos completos na janela mãe.
      </p>
    </div>
  );
}
