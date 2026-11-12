import { useEffect, useState, type ReactNode } from "react";
import { CheckCircle2, ChevronDown, ChevronRight, File, FileText, FolderGit2, GitCommitHorizontal, Laptop, Package, Plug, SquareTerminal } from "lucide-react";
import { KIND_ICON } from "./kindIcon";
import DiffPanel from "./DiffPanel";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import type { Output } from "../lib/outputs";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { BgTask, ContextWeight, McpServer, RepoState, Scope, SpendAgg, StatusLine } from "../types";

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

const SECTIONS_KEY = "hark.session.sections";
type SectionId = "env" | "ctx" | "connectors" | "outputs" | "changes" | "bg" | "costs" | "limits";
const OPEN_BY_DEFAULT: SectionId[] = ["env", "outputs", "changes", "bg"];

function readOpen(): Record<string, boolean> {
  try {
    const raw = window.localStorage.getItem(SECTIONS_KEY);
    return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
  } catch {
    return {};
  }
}

/** One section of the bar: a head that folds it, a summary shown while folded. */
function Section({ id, title, summary, open, onToggle, children }: { id: SectionId; title: string; summary?: ReactNode; open: boolean; onToggle: () => void; children: ReactNode }) {
  return (
    <section className={`scope-block sp-sec ${open ? "open" : "closed"}`} data-section={id}>
      <button className="scope-block-head sp-sec-head" onClick={onToggle} aria-expanded={open}>
        <span className="sp-sec-title">
          {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />} {title}
        </span>
        {summary != null && <b className="sp-sec-sum">{summary}</b>}
      </button>
      {open && <div className="sp-sec-body">{children}</div>}
    </section>
  );
}

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
  changes,
  onCommit,
  connectors,
  background,
  onPlugins,
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
  /** The session's git changes (the Alterações section): the workspace
   *  git resolves, and a tick that re-reads when a worker wrote a file. */
  changes?: { workspace?: string; tick: number } | null;
  /** "Fazer commit das alterações": the ask goes to the chat. */
  onCommit?: () => void;
  /** The MCP servers the session opened, with their state. */
  connectors?: McpServer[];
  /** Commands the agent left running in the background, and the finished ones. */
  background?: BgTask[];
  /** Settings › plugins, where a connector is reconnected. */
  onPlugins?: () => void;
}) {
  const [open, setOpen] = useState<Record<string, boolean>>(readOpen);
  const isOpen = (id: SectionId) => open[id] ?? OPEN_BY_DEFAULT.includes(id);
  const toggle = (id: SectionId) =>
    setOpen((o) => {
      const next = { ...o, [id]: !isOpen(id) };
      try {
        window.localStorage.setItem(SECTIONS_KEY, JSON.stringify(next));
      } catch {
        /* a private window keeps nothing */
      }
      return next;
    });
  const [sum, setSum] = useState<{ added: number; removed: number; files: number } | null>(null);
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

  const ctxSummary =
    context?.pct != null
      ? `${context.tokens ? fmtTok(context.tokens) : ""}${capacity ? ` / ${fmtTok(capacity)}` : ""} (${Math.round(context.pct * 100)}%)`
      : weight && weight.last_total_tokens > 0
        ? `${fmtTok(weight.last_total_tokens)}${capacity ? ` / ${fmtTok(capacity)}` : ""}`
        : null;

  return (
    <div className="session-panel">
      <Section id="env" title={t("sec_env")} open={isOpen("env")} onToggle={() => toggle("env")}>
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
      </Section>

      <Section
        id="ctx"
        title={t("sec_ctx")}
        summary={ctxSummary ?? <span className="dim">{scope.kind === "all" ? t("ctx_local") : t("ctx_none")}</span>}
        open={isOpen("ctx")}
        onToggle={() => toggle("ctx")}
      >
        {weight && weight.last_total_tokens > 0 ? (
          <>
            <StackedBar segments={contextSegments} capacity={capacity} height={7} />
            <Legend segments={contextSegments} capacity={capacity} free={t("si_free")} />
            {weight.pct != null && weight.pct > 0.7 && <p className="hint warn">{t("si_heavy")}</p>}
          </>
        ) : (
          <p className="hint">{scope.kind === "all" ? t("ctx_local") : t("ctx_none")}</p>
        )}
        {(turnUsd != null || threadCost > 0) && (
          <div className="scope-pair">
            <span>{turnUsd != null ? t("ctx_turn") : t("ctx_thread")}</span>
            <b>{fmtUsd(turnUsd ?? threadCost)}</b>
          </div>
        )}
      </Section>

      <Section
        id="connectors"
        title={t("sec_connectors")}
        summary={
          connectors && connectors.length > 0 ? (
            <>
              {connectors.length}
              {connectors.some((c) => c.status !== "connected") && <span className="warn"> · {t("sp_conn_reconnect_n", { n: connectors.filter((c) => c.status !== "connected").length })}</span>}
            </>
          ) : undefined
        }
        open={isOpen("connectors")}
        onToggle={() => toggle("connectors")}
      >
        {!connectors || connectors.length === 0 ? (
          <p className="hint">{t("sp_conn_none")}</p>
        ) : (
          connectors.map((c) => (
            <div key={c.name} className="sp-row" title={c.status}>
              <Plug size={13} />
              <span>{c.name}</span>
              <span className={`sp-dot ${c.status === "connected" ? "ok" : "err"}`} />
            </div>
          ))
        )}
        {onPlugins && (
          <button className="sp-link" onClick={onPlugins} title={t("sp_conn_hint")}>
            {t("sp_conn_plugins")}
          </button>
        )}
      </Section>

      <Section id="outputs" title={t("sec_outputs")} summary={outputs.length > 0 ? outputs.length : undefined} open={isOpen("outputs")} onToggle={() => toggle("outputs")}>
        {outputs.length === 0 ? (
          <p className="hint">{t("out_none")}</p>
        ) : (
          outputs.map((o) => {
            // A page is named by its title with the icon of its kind; a
            // markdown of the workspace is a doc; anything else, a file.
            const PageIcon = o.page ? KIND_ICON[o.page.kind] : null;
            return (
              <button key={o.path} className="sp-out" onClick={() => onOpenPath(o.path)} title={o.path}>
                {PageIcon && o.page ? (
                  <PageIcon size={13} className={`art-kind ${o.page.kind}`} />
                ) : o.kind === "delivered" ? (
                  <Package size={13} />
                ) : /\.(md|markdown)$/i.test(o.path) ? (
                  <FileText size={13} className="sp-out-doc" />
                ) : (
                  <File size={13} />
                )}
                <span className="sp-out-name">{o.page ? o.page.title : basename(o.path)}</span>
                <span className="dim sp-out-dir">{o.page ? t("out_page_in", { space: o.page.space }) : dirname(o.path)}</span>
                <span className={`pill-kind ${o.kind}`}>{t(`out_${o.kind}`)}</span>
              </button>
            );
          })
        )}
      </Section>

      <Section
        id="changes"
        title={t("sec_changes")}
        summary={
          sum && sum.files > 0 ? (
            <span className="sp-sum-diff">
              <span className="add">+{sum.added.toLocaleString()}</span> <span className="del">−{sum.removed.toLocaleString()}</span>
            </span>
          ) : undefined
        }
        open={isOpen("changes")}
        onToggle={() => toggle("changes")}
      >
        {changes?.workspace ? (
          <>
            {onCommit && sum && sum.files > 0 && (
              <button className="pill-btn on sp-commit" onClick={onCommit} title={t("sp_commit_text")}>
                <GitCommitHorizontal size={11} /> {t("sp_commit")}
              </button>
            )}
            <div className="sp-changes">
              <DiffPanel workspace={changes.workspace} repo={repo} tick={changes.tick} onOpenPath={onOpenPath} onTotals={setSum} />
            </div>
          </>
        ) : (
          <p className="hint">{t("sp_no_repo")}</p>
        )}
      </Section>

      <Section
        id="bg"
        title={t("sec_bg")}
        summary={background && background.some((b) => !b.doneAt) ? background.filter((b) => !b.doneAt).length : undefined}
        open={isOpen("bg")}
        onToggle={() => toggle("bg")}
      >
        {!background || background.length === 0 ? (
          <p className="hint">{t("sp_bg_none")}</p>
        ) : (
          <>
            {background
              .filter((b) => !b.doneAt)
              .map((b) => (
                <div key={b.id} className="sp-row" title={b.command}>
                  <SquareTerminal size={13} />
                  <span>{b.description || b.command}</span>
                  <span className="dim sp-bg-when">{Math.max(1, Math.round((Date.now() - b.startedAt) / 1000))} s ●</span>
                </div>
              ))}
            {background.some((b) => b.doneAt) && (
              <div className="sp-row" title={background.filter((b) => b.doneAt).map((b) => b.summary ?? b.description).join("\n")}>
                <CheckCircle2 size={13} />
                <span>{t("sp_bg_done_n", { n: background.filter((b) => b.doneAt).length })}</span>
              </div>
            )}
          </>
        )}
      </Section>

      {ledger && (
        <Section id="costs" title={`${t("sec_costs")}${projectName ? ` ${t("costs_in_project")}` : ""}`} summary={fmtUsd(ledger.day)} open={isOpen("costs")} onToggle={() => toggle("costs")}>
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
        </Section>
      )}

      {limits && limits.limits.length > 0 && (
        <Section id="limits" title={t("sec_limits")} open={isOpen("limits")} onToggle={() => toggle("limits")}>
          {limits.limits.map((l) => (
            <Gauge key={l.key} name={limitName(l.key)} used={l.used} caption={resetIn(l.resets_at)} />
          ))}
        </Section>
      )}
    </div>
  );
}
