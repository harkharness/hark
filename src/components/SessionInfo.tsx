import { useEffect, useState } from "react";
import * as ipc from "../lib/ipc";

type Stats = { title: string | null; size_mb: number; entries: number; last_ts: string | null };

/**
 * Scope popover: what am I focused on, how heavy is that session on a
 * resume, and what this window has spent so far. All local, zero tokens.
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
  /** Project root path: when set, the ledger summary is FILTERED to this
   * project (project windows show their slice; the mother shows it all). */
  workspace?: string;
  costs: Record<string, number>;
  onClose: () => void;
}) {
  const [stats, setStats] = useState<Stats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [ledger, setLedger] = useState<{ day: number; week: number } | null>(null);
  const [contextPct, setContextPct] = useState<number | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    ipc
      .sessionStats(sessionId)
      .then(setStats)
      .catch((err) => setError(String(err)));
    ipc
      .sessionContextWeight(sessionId)
      .then((w) => setContextPct(w.pct))
      .catch(() => {});
  }, [sessionId]);

  useEffect(() => {
    // Persistent ledger: survives the window closing (unlike `costs`).
    // With a workspace, group by workspace and keep only this project's
    // slice (rows record the worker cwd, so prefix-match covers subdirs).
    const day = new Date(Date.now() - 24 * 3600e3).toISOString();
    const week = new Date(Date.now() - 7 * 24 * 3600e3).toISOString();
    const group = workspace ? "workspace" : "kind";
    const slice = (aggs: { key: string; cost_usd: number }[]) =>
      aggs
        .filter((a) => !workspace || a.key === workspace || a.key.startsWith(`${workspace}/`))
        .reduce((sum, a) => sum + a.cost_usd, 0);
    Promise.all([
      ipc.spendSummary(day, group, "live"),
      ipc.spendSummary(week, group, "live"),
    ])
      .then(([d, w]) => setLedger({ day: slice(d), week: slice(w) }))
      .catch(() => {});
  }, [workspace]);

  const total = Object.values(costs).reduce((a, b) => a + b, 0);
  const spent = Object.entries(costs).sort((a, b) => b[1] - a[1]);

  return (
    <div className="scope-pop" onMouseLeave={onClose}>
      <div className="scope-row head">
        <span>{taskTitle ?? "nenhuma task focada"}</span>
      </div>
      {projectName && (
        <div className="scope-row">
          <span>projeto</span>
          <b>{projectName}</b>
        </div>
      )}
      {stats && (
        <>
          {stats.title && stats.title !== taskTitle && (
            <div className="scope-row">
              <span>sessão</span>
              <b>{stats.title.slice(0, 34)}</b>
            </div>
          )}
          <div className="scope-row">
            <span>peso do resume</span>
            <b className={stats.size_mb > 2 ? "warn" : ""}>{stats.size_mb.toFixed(1)} MB</b>
          </div>
          <div className="scope-row">
            <span>eventos no log</span>
            <b>{stats.entries}</b>
          </div>
          {stats.last_ts && (
            <div className="scope-row">
              <span>última atividade</span>
              <b>{stats.last_ts.slice(0, 16).replace("T", " ")}</b>
            </div>
          )}
        </>
      )}
      {contextPct != null && (
        <div className="scope-row">
          <span>janela de contexto</span>
          <b className={contextPct > 0.7 ? "warn" : ""}>{Math.round(contextPct * 100)}%</b>
        </div>
      )}
      {error && <div className="scope-row"><span className="warn">{error}</span></div>}
      {ledger && (
        <>
          <div className="scope-row head">
            <span>{workspace ? "gasto do projeto (ledger)" : "gasto medido (ledger)"}</span>
          </div>
          <div className="scope-row">
            <span>últimas 24h</span>
            <b>${ledger.day.toFixed(4)}</b>
          </div>
          <div className="scope-row">
            <span>últimos 7 dias</span>
            <b>${ledger.week.toFixed(4)}</b>
          </div>
        </>
      )}
      <div className="scope-row head">
        <span>gasto nesta janela</span>
        <b>${total.toFixed(4)}</b>
      </div>
      {spent.slice(0, 6).map(([label, usd]) => (
        <div className="scope-row" key={label}>
          <span>{label.slice(0, 30)}</span>
          <b>${usd.toFixed(4)}</b>
        </div>
      ))}
      {spent.length === 0 && (
        <div className="scope-row">
          <span>nada gasto ainda</span>
        </div>
      )}
    </div>
  );
}
