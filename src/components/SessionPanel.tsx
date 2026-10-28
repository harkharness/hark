import { useEffect, useState } from "react";
import { FileText, FolderGit2, Laptop, Package } from "lucide-react";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import type { Output } from "../lib/outputs";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { ContextWeight, RepoState, Scope, SpendAgg, StatusLine } from "../types";

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

const basename = (p: string) => p.split("/").filter(Boolean).at(-1) ?? p;
const dirname = (p: string) => p.replace(/\/[^/]*$/, "").replace(/^\/Users\/[^/]+/, "~");

/**
 * The session panel of the rail: what the thread on screen is running on
 * (environment), what it is carrying (context), what it produced
 * (outputs) and what it cost. Everything local, zero tokens. In the
 * "all projects" scope it describes the window instead of a session.
 */
export default function SessionPanel({
  scope,
  projectsCount,
  taskTitle,
  sessionId,
  workspace,
  repo,
  agentName,
  mode,
  model,
  effort,
  context,
  turnUsd,
  costs,
  outputs,
  onOpenPath,
  onDetail,
}: {
  scope: Scope;
  projectsCount: number;
  taskTitle?: string;
  sessionId?: string;
  /** Project root: scopes the ledger to this project (and below). */
  workspace?: string;
  repo?: RepoState;
  agentName?: string;
  mode?: string;
  model?: string;
  effort?: string;
  /** The live reading of the context window, when any source has one. */
  context?: { pct: number | null; window?: number | null; tokens?: number | null } | null;
  /** What the running turn has cost so far (null = unpriced). */
  turnUsd?: number | null;
  costs: Record<string, number>;
  outputs: Output[];
  onOpenPath: (path: string) => void;
  /** "Ver detalhamento" → the /usage card lands in the thread. */
  onDetail?: () => void;
}) {
  const [weight, setWeight] = useState<ContextWeight | null>(null);
  const [ledger, setLedger] = useState<{ day: number; week: number } | null>(null);
  const [byTask, setByTask] = useState<SpendAgg[]>([]);
  const [limits, setLimits] = useState<StatusLine | null>(null);

  useEffect(() => {
    if (!sessionId) {
      setWeight(null);
      return;
    }
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

  const threadCost = costs[taskTitle ?? ""] ?? 0;
  const windowTotal = Object.values(costs).reduce((a, b) => a + b, 0);
  const contextSegments: Segment[] = weight
    ? [
        { label: t("seg_cache_read_plain"), value: weight.cache_read, color: "var(--ok)" },
        { label: t("seg_cache_new"), value: weight.cache_created, color: "var(--warn)" },
        { label: t("seg_prompt"), value: weight.input, color: "var(--accent)" },
      ]
    : [];
  const capacity = weight?.context_window ?? context?.window ?? undefined;
  const projectName = scope.kind === "chat" ? scope.project?.name : scope.kind === "project" ? scope.project.name : undefined;
  const projectPath = scope.kind === "chat" ? scope.project?.path : scope.kind === "project" ? scope.project.path : undefined;
  const knobs = [mode, model, effort].filter(Boolean).join(" · ");

  return (
    <div className="session-panel">
      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("sec_env")}</span>
        </div>
        <div className="sp-row">
          <Laptop size={13} />
          <span>{t("env_machine")}</span>
        </div>
        <div className="sp-row">
          <FolderGit2 size={13} />
          {projectName ? (
            <span>
              {projectName} <span className="mono dim">{projectPath}</span>
            </span>
          ) : (
            <span>{t("env_projects", { n: projectsCount })}</span>
          )}
        </div>
        {repo && (
          <div className="sp-row">
            <span className="sp-ico" />
            <span>
              {repo.branch || t("repo_detached")}
              {repo.ahead || repo.behind ? (
                <span className="dim"> · {repo.ahead > 0 ? `↑${repo.ahead}` : ""}{repo.behind > 0 ? ` ↓${repo.behind}` : ""}</span>
              ) : null}
              {repo.dirty > 0 && <span className="dim"> · {t("env_dirty", { n: repo.dirty })}</span>}
            </span>
          </div>
        )}
        {agentName && (
          <div className="sp-row">
            <span className="sp-ico" />
            <span>
              {agentName}
              {knobs && <span className="dim"> · {knobs}</span>}
            </span>
          </div>
        )}
        {scope.kind === "all" && <p className="hint">{t("env_general")}</p>}
      </section>

      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("sec_ctx")}</span>
          {context?.pct != null ? (
            <b>
              {context.tokens ? fmtTok(context.tokens) : ""}
              {capacity ? ` / ${fmtTok(capacity)}` : ""}
              {` (${Math.round(context.pct * 100)}%)`}
            </b>
          ) : weight && weight.last_total_tokens > 0 ? (
            <b>
              {fmtTok(weight.last_total_tokens)}
              {capacity ? ` / ${fmtTok(capacity)}` : ""}
            </b>
          ) : (
            <b className="dim">{scope.kind === "all" ? t("ctx_local") : t("ctx_none")}</b>
          )}
        </div>
        {weight && weight.last_total_tokens > 0 && (
          <>
            <StackedBar segments={contextSegments} capacity={capacity} height={7} />
            <Legend segments={contextSegments} capacity={capacity} free={t("si_free")} />
            {weight.pct != null && weight.pct > 0.7 && <p className="hint warn">{t("si_heavy")}</p>}
          </>
        )}
        {(turnUsd != null || threadCost > 0) && (
          <div className="scope-pair">
            <span>{turnUsd != null ? t("ctx_turn") : t("ctx_thread")}</span>
            <b>{fmtUsd(turnUsd ?? threadCost)}</b>
          </div>
        )}
      </section>

      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("sec_outputs")}</span>
          {outputs.length > 0 && <b>{outputs.length}</b>}
        </div>
        {outputs.length === 0 ? (
          <p className="hint">{t("out_none")}</p>
        ) : (
          outputs.map((o) => (
            <button key={o.path} className="sp-out" onClick={() => onOpenPath(o.path)} title={o.path}>
              {o.kind === "delivered" ? <Package size={13} /> : <FileText size={13} />}
              <span className="sp-out-name">{basename(o.path)}</span>
              <span className="dim sp-out-dir">{dirname(o.path)}</span>
              <span className={`pill-kind ${o.kind}`}>{t(`out_${o.kind}`)}</span>
            </button>
          ))
        )}
      </section>

      {ledger && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>
              {t("costs_measured")}
              {projectName ? ` ${t("costs_in_project")}` : ""}
            </span>
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
          {onDetail && (
            <button className="sp-link" onClick={onDetail}>
              {t("si_detail")}
            </button>
          )}
        </section>
      )}

      {limits && limits.limits.length > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("c_limits")}</span>
          </div>
          {limits.limits.map((l) => (
            <Gauge key={l.key} name={limitName(l.key)} used={l.used} caption={resetIn(l.resets_at)} />
          ))}
        </section>
      )}
    </div>
  );
}
