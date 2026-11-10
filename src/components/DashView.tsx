import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, ArrowRight, Check, Copy, Database, FileText, History, Lock, MessageSquare, Pencil, Plus, RefreshCw, ShieldCheck, ShieldOff, Sparkles, Terminal, Trash2 } from "lucide-react";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { DashChart, DashOp, DashPanel, DashQuery, DashSource, DashView as View } from "../types";

const fmtNumber = (n: number) => n.toLocaleString(undefined, { maximumFractionDigits: 2 });
const fmtDelta = (d: number) => `${d >= 0 ? "▲" : "▼"} ${Math.abs(d * 100).toLocaleString(undefined, { maximumFractionDigits: 1 })}%`;

/** A number panel's line: the value, its change, or what is missing. */
function Number_({ p }: { p: DashPanel }) {
  if (p.error) return <div className="dash-err">{p.error}</div>;
  return (
    <>
      <div className="dash-big">{p.number != null ? fmtNumber(p.number) : "—"}</div>
      {p.delta != null && <div className={`dash-delta ${p.delta >= 0 ? "up" : "down"}`}>{fmtDelta(p.delta)}</div>}
    </>
  );
}

function Bars({ p }: { p: DashPanel }) {
  const values = p.values ?? [];
  const max = Math.max(1, ...values);
  const last = values.length - 1;
  return (
    <div className="dash-bars" role="img" aria-label={p.title}>
      {values.map((v, i) => (
        <div key={i} className="dash-bar-col" title={`${p.labels?.[i] ?? i + 1}: ${fmtNumber(v)}`}>
          <div className={`dash-bar ${i === last ? "last" : ""}`} style={{ height: `${(v / max) * 100}%` }} />
        </div>
      ))}
    </div>
  );
}

function Line({ p }: { p: DashPanel }) {
  const values = p.values ?? [];
  const w = 300;
  const h = 80;
  const max = Math.max(1, ...values);
  const min = Math.min(0, ...values);
  const span = max - min || 1;
  const pts = values.map((v, i) => [values.length > 1 ? (i / (values.length - 1)) * w : w / 2, h - ((v - min) / span) * h] as const);
  const d = pts.map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`).join(" ");
  return (
    <svg className="dash-line" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" role="img" aria-label={p.title}>
      <path d={`${d} L${w},${h} L0,${h} Z`} className="fill" />
      <path d={d} className="stroke" fill="none" />
    </svg>
  );
}

function Grid({ columns, rows }: { columns: string[]; rows: unknown[][] }) {
  return (
    <div className="dash-table-wrap">
      <table className="dash-table">
        <thead>
          <tr>
            {columns.map((c) => (
              <th key={c}>{c}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              {columns.map((_, j) => {
                const v = r[j];
                return (
                  <td key={j} className={typeof v === "number" ? "num" : ""}>
                    {typeof v === "number" ? fmtNumber(v) : v == null ? "" : String(v)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

const CHARTS: DashChart[] = ["number", "bar", "line", "table"];
type PanelDraft = { title: string; query: string; chart: DashChart; x: string; y: string; value: string; previous: string };
type QueryDraft = { id: string; source: string; sql: string; columns: string; limit: string };
type SourceDraft = { id: string; kind: DashSource["kind"]; path: string; command: string };

const orNull = (s: string) => (s.trim() ? s.trim() : null);

/** A panel's fields: its title, the query it draws, the chart and its columns. */
function PanelForm({ draft, queries, onChange, onSave, onCancel, extra }: { draft: PanelDraft; queries: DashQuery[]; onChange: (d: PanelDraft) => void; onSave: () => void; onCancel: () => void; extra?: React.ReactNode }) {
  const axes = draft.chart === "number" ? (["value", "previous"] as const) : draft.chart === "table" ? ([] as const) : (["x", "y"] as const);
  return (
    <div className="dash-form" onClick={(e) => e.stopPropagation()}>
      <label>
        <span className="dim">{t("dash_panel_title")}</span>
        <input autoFocus value={draft.title} onChange={(e) => onChange({ ...draft, title: e.target.value })} />
      </label>
      <label>
        <span className="dim">{t("dash_panel_query")}</span>
        <select value={draft.query} onChange={(e) => onChange({ ...draft, query: e.target.value })}>
          {queries.map((q) => (
            <option key={q.id} value={q.id}>{q.id}</option>
          ))}
        </select>
      </label>
      <label>
        <span className="dim">{t("dash_chart")}</span>
        <select value={draft.chart} onChange={(e) => onChange({ ...draft, chart: e.target.value as DashChart })}>
          {CHARTS.map((c) => (
            <option key={c} value={c}>{t(`dash_chart_${c}`)}</option>
          ))}
        </select>
      </label>
      {axes.map((a) => (
        <label key={a}>
          <span className="dim">{t(`dash_${a}`)}</span>
          <input className="mono" value={draft[a]} onChange={(e) => onChange({ ...draft, [a]: e.target.value })} />
        </label>
      ))}
      <span className="dash-form-acts">
        <button className="pill-btn on" onClick={onSave} disabled={!draft.title.trim() || !draft.query}>
          <Check size={11} /> {t("board_save")}
        </button>
        <button className="pill-btn" onClick={onCancel}>{t("board_cancel")}</button>
        {extra}
      </span>
    </div>
  );
}

/** A query's fields: sql for sqlite, a column pick for the rest. */
function QueryForm({ draft, sources, isNew, onChange, onSave, onCancel }: { draft: QueryDraft; sources: DashSource[]; isNew: boolean; onChange: (d: QueryDraft) => void; onSave: () => void; onCancel: () => void }) {
  const src = sources.find((s) => s.id === draft.source);
  const sqlish = src?.kind === "sqlite";
  return (
    <div className="dash-form">
      {isNew && (
        <label>
          <span className="dim">{t("dash_query_id")}</span>
          <input autoFocus className="mono" placeholder={t("dash_query_id")} value={draft.id} onChange={(e) => onChange({ ...draft, id: e.target.value })} />
        </label>
      )}
      <label>
        <span className="dim">{t("dash_source")}</span>
        <select value={draft.source} onChange={(e) => onChange({ ...draft, source: e.target.value })}>
          {sources.map((s) => (
            <option key={s.id} value={s.id}>{s.id} · {s.kind}</option>
          ))}
        </select>
      </label>
      {sqlish ? (
        <textarea className="mono" rows={4} placeholder={t("dash_sql_ph")} value={draft.sql} onChange={(e) => onChange({ ...draft, sql: e.target.value })} />
      ) : (
        <>
          <input className="mono" placeholder={t("dash_columns_ph")} value={draft.columns} onChange={(e) => onChange({ ...draft, columns: e.target.value })} />
          <label>
            <span className="dim">{t("dash_limit")}</span>
            <input className="mono" value={draft.limit} onChange={(e) => onChange({ ...draft, limit: e.target.value.replace(/[^0-9]/g, "") })} />
          </label>
        </>
      )}
      <span className="dash-form-acts">
        <button className="pill-btn on" onClick={onSave} disabled={!draft.source || (isNew && !draft.id.trim()) || (sqlish && !draft.sql.trim())}>
          <Check size={11} /> {t("board_save")}
        </button>
        <button className="pill-btn" onClick={onCancel}>{t("board_cancel")}</button>
      </span>
    </div>
  );
}

/** A source's fields: a file in the project, a sqlite file or a command. */
function SourceForm({ draft, isNew, onChange, onSave, onCancel }: { draft: SourceDraft; isNew: boolean; onChange: (d: SourceDraft) => void; onSave: () => void; onCancel: () => void }) {
  const cmd = draft.kind === "command";
  return (
    <div className="dash-form">
      {isNew && (
        <label>
          <span className="dim">{t("dash_query_id")}</span>
          <input autoFocus className="mono" placeholder={t("dash_query_id")} value={draft.id} onChange={(e) => onChange({ ...draft, id: e.target.value })} />
        </label>
      )}
      <label>
        <span className="dim">{t("dash_kind")}</span>
        <select value={draft.kind} onChange={(e) => onChange({ ...draft, kind: e.target.value as DashSource["kind"] })}>
          {(["csv", "json", "sqlite", "command"] as const).map((k) => (
            <option key={k} value={k}>{k}</option>
          ))}
        </select>
      </label>
      {cmd ? (
        <input className="mono" placeholder={t("dash_command")} value={draft.command} onChange={(e) => onChange({ ...draft, command: e.target.value })} />
      ) : (
        <input className="mono" placeholder={t("dash_path")} value={draft.path} onChange={(e) => onChange({ ...draft, path: e.target.value })} />
      )}
      <span className="dash-form-acts">
        <button className="pill-btn on" onClick={onSave} disabled={(isNew && !draft.id.trim()) || (cmd ? !draft.command.trim() : !draft.path.trim())}>
          <Check size={11} /> {t("board_save")}
        </button>
        <button className="pill-btn" onClick={onCancel}>{t("board_cancel")}</button>
      </span>
    </div>
  );
}

const SOURCE_ICON = { sqlite: Database, csv: FileText, json: FileText, command: Terminal } as const;

function SourceChip({ s }: { s: DashSource }) {
  const Icon = SOURCE_ICON[s.kind];
  return (
    <span className={`pill dash-source ${s.kind}`} title={s.kind === "command" ? t("dash_command_hint") : s.path ?? ""}>
      <Icon size={10} /> {s.kind === "command" ? <span className="mono">{s.command}</span> : <span className="mono">{s.path}</span>}
      {s.kind === "command" && <Lock size={9} />}
    </span>
  );
}

/**
 * The dashboard page: panels over the cached numbers (a number with its
 * change, bars, a line, a table), the sources as chips, when the
 * numbers were taken and "refresh now" (the only time a command source
 * runs). "Fontes e consultas" lists every query with its text, its
 * preview and the person's certification; a query the agent changed is
 * marked until certified or accepted.
 */
export default function DashView({
  space,
  id,
  onChanged,
  onAsk,
  onDoc,
}: {
  space: string;
  id: string;
  onChanged?: () => void;
  /** Text to take to a chat (the story), through "take to…". */
  onAsk?: (text: string) => void;
  /** A child document page was made from the story; open it. */
  onDoc?: (pageId: string) => void;
}) {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<"panels" | "sources">("panels");
  const [query, setQuery] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [story, setStory] = useState<{ text?: string; busy: boolean; error?: string }>({ busy: false });
  // The person's forms (fase 14.1): one open at a time.
  const [panelForm, setPanelForm] = useState<{ index: number | null; draft: PanelDraft } | null>(null);
  const [queryForm, setQueryForm] = useState<{ isNew: boolean; draft: QueryDraft } | null>(null);
  const [sourceForm, setSourceForm] = useState<{ isNew: boolean; draft: SourceDraft } | null>(null);
  const [editErr, setEditErr] = useState<string | null>(null);

  const load = useCallback(
    async (run = false) => {
      setRunning(run);
      try {
        setView(await ipc.dashView(space, id, run));
        setError(null);
      } catch (err) {
        setError(String(err));
      } finally {
        setRunning(false);
      }
    },
    [space, id],
  );
  useEffect(() => {
    void load(false);
  }, [load]);

  async function certify(q: string, on: boolean) {
    await ipc.dashCertify(space, id, q, on);
    await load(false);
    onChanged?.();
  }
  async function accept() {
    await ipc.artifactAccept(space, id);
    await load(false);
    onChanged?.();
  }
  async function edit(op: DashOp) {
    try {
      await ipc.dashEdit(space, id, op);
      setEditErr(null);
      setPanelForm(null);
      setQueryForm(null);
      setSourceForm(null);
      await load(false);
      onChanged?.();
    } catch (err) {
      setEditErr(String(err));
    }
  }
  const panelDraft = (p?: DashPanel): PanelDraft => ({ title: p?.title ?? "", query: p?.query ?? view?.queries[0]?.id ?? "", chart: p?.chart ?? "table", x: p?.x ?? "", y: p?.y ?? "", value: p?.value ?? "", previous: p?.previous ?? "" });
  const panelOp = (d: PanelDraft) => ({ title: d.title.trim(), query: d.query, chart: d.chart, x: orNull(d.x), y: orNull(d.y), value: orNull(d.value), previous: orNull(d.previous) });
  const queryDraft = (q?: DashQuery): QueryDraft => ({ id: q?.id ?? "", source: q?.source ?? view?.sources[0]?.id ?? "", sql: q?.sql ?? "", columns: (q?.columns ?? []).join(", "), limit: q?.limit != null ? String(q.limit) : "" });
  const queryOp = (d: QueryDraft) => {
    const sqlish = view?.sources.find((s) => s.id === d.source)?.kind === "sqlite";
    return { id: d.id.trim(), source: d.source, sql: sqlish ? orNull(d.sql) : null, columns: sqlish ? [] : d.columns.split(",").map((c) => c.trim()).filter(Boolean), limit: !sqlish && d.limit ? Number(d.limit) : null };
  };
  const sourceDraft = (s?: DashSource): SourceDraft => ({ id: s?.id ?? "", kind: s?.kind ?? "csv", path: s?.path ?? "", command: s?.command ?? "" });
  const sourceOp = (d: SourceDraft) => ({ id: d.id.trim(), kind: d.kind, path: d.kind === "command" ? null : orNull(d.path), command: d.kind === "command" ? orNull(d.command) : null });
  async function writeStory() {
    setStory({ busy: true });
    try {
      setStory({ busy: false, text: await ipc.dashStory(space, id) });
    } catch (err) {
      setStory({ busy: false, error: String(err) });
    }
  }
  async function storyToDoc() {
    if (!story.text || !view) return;
    const title = t("dash_story_doc_title", { t: view.title ?? id });
    const node = await ipc.artifactCreate(space, id, title, "doc");
    await ipc.artifactWrite(space, node.id, `# ${title}\n\n${story.text}\n`);
    onChanged?.();
    onDoc?.(node.id);
  }
  const panelCsv = (p: DashPanel) => {
    const q = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
    if (p.table) return [p.table.columns.map(q).join(","), ...p.table.rows.map((r) => r.map((v) => q(v == null ? "" : String(v))).join(","))].join("\n") + "\n";
    return ["label,value", ...(p.labels ?? []).map((l, i) => `${q(l)},${p.values?.[i] ?? ""}`)].join("\n") + "\n";
  };

  if (error) return <div className="dash-error">{t("dash_open_err", { e: error })}</div>;
  if (!view) return <div className="dash-loading dim">…</div>;
  if (view.error) {
    return (
      <div className="dash-broken">
        <div className="dash-error">{t("dash_file_err", { e: view.error })}</div>
        <pre className="mono">{view.raw}</pre>
      </div>
    );
  }
  const picked = view.queries.find((q) => q.id === query) ?? view.queries[0];
  const when = view.cache_at ? new Date(view.cache_at).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" }) : null;

  return (
    <div className="dash-view">
      {view.changed.length > 0 && (
        <div className="dash-banner">
          <Sparkles size={13} />
          <b>{t("dash_changed_n", { n: view.changed.length })}</b>
          <span className="dim">{t("dash_changed_hint")}</span>
          <span className="dash-banner-acts">
            <button className="pill-btn" onClick={() => { setScreen("sources"); setQuery(view.changed[0]); }}>
              {t("dash_see_queries")}
            </button>
            <button className="pill-btn on" onClick={() => void accept()} title={t("sheet_accept_hint")}>
              <Check size={11} /> {t("sheet_accept")}
            </button>
          </span>
        </div>
      )}
      <div className="dash-head">
        <span className="dash-tabs">
          <button className={`pill-btn ${screen === "panels" ? "on" : ""}`} onClick={() => setScreen("panels")}>
            {t("dash_panels_tab", { n: view.panels.length })}
          </button>
          <button className={`pill-btn ${screen === "sources" ? "on" : ""}`} onClick={() => setScreen("sources")}>
            {t("dash_sources_tab")}
          </button>
        </span>
        <span className="dash-when dim">
          {when ? t("dash_numbers_at", { when }) : t("dash_numbers_none")} ·{" "}
          <select className="dash-cadence" title={t("dash_cadence")} value={view.refresh} onChange={(e) => void edit({ op: "refresh", refresh: e.target.value as View["refresh"] })}>
            {(["manual", "hourly", "daily"] as const).map((r) => (
              <option key={r} value={r}>{t(`dash_refresh_${r}`)}</option>
            ))}
          </select>
          {view.stale && <span className="dash-stale"> · {t("dash_stale")}</span>}
        </span>
        <span className="dash-head-acts">
          <button className="pill-btn" onClick={() => void load(true)} disabled={running} title={t("dash_refresh_hint")}>
            <RefreshCw size={11} className={running ? "spin" : ""} /> {t("dash_refresh")}
          </button>
          <span className="dim" title={t("sheet_versions_hint")}>
            <History size={11} /> {t("sheet_versions", { n: view.versions })}
          </span>
        </span>
      </div>
      <div className="dash-sources">
        {screen === "panels" && view.sources.map((s) => (
          <SourceChip key={s.id} s={s} />
        ))}
        <span className="pill dim" title={t("dash_local_hint")}>
          <Lock size={9} /> {t("dash_local")}
        </span>
      </div>
      {screen === "panels" ? (
        <div className="dash-panels">
          {editErr && <div className="dash-err dash-edit-err">{t("dash_edit_err", { e: editErr })}</div>}
          {view.panels.length === 0 && <div className="dim dash-empty">{t("dash_no_panels")}</div>}
          {panelForm?.index == null && panelForm ? (
            <div className="dash-panel form">
              <PanelForm draft={panelForm.draft} queries={view.queries} onChange={(draft) => setPanelForm({ index: null, draft })} onSave={() => void edit({ op: "add_panel", ...panelOp(panelForm.draft) })} onCancel={() => setPanelForm(null)} />
            </div>
          ) : (
            <button className="dash-panel add" onClick={() => setPanelForm({ index: null, draft: panelDraft() })} disabled={view.queries.length === 0}>
              <Plus size={14} /> {t("dash_new_panel")}
            </button>
          )}
          <div className="dash-panel story">
            <div className="dash-panel-title">
              <span>
                <Sparkles size={11} /> {t("dash_story")}
              </span>
              <span className="dash-draft dim" title={t("dash_story_hint")}>
                {t("dash_story_hint").split(";")[1]?.trim() ?? ""}
              </span>
            </div>
            {story.text ? (
              <>
                <p className="dash-story-text">{story.text}</p>
                <span className="dash-story-acts">
                  <button className="pill-btn" onClick={() => void storyToDoc()} title={t("dash_story_to_doc_hint")}>
                    <FileText size={11} /> {t("dash_story_to_doc")}
                  </button>
                  {onAsk && (
                    <button className="pill-btn" onClick={() => onAsk(story.text ?? "")}>
                      <MessageSquare size={11} /> {t("dash_story_to_chat")}
                    </button>
                  )}
                  <button className="pill-btn" onClick={() => void writeStory()} disabled={story.busy}>
                    <RefreshCw size={11} /> {t("dash_story_write")}
                  </button>
                </span>
              </>
            ) : (
              <span className="dash-story-acts">
                {story.error && <span className="dash-err">{t("dash_story_err", { e: story.error })}</span>}
                <button className="pill-btn on" onClick={() => void writeStory()} disabled={story.busy || view.panels.length === 0} title={t("dash_story_hint")}>
                  <Sparkles size={11} className={story.busy ? "spin" : ""} /> {t("dash_story_write")}
                </button>
              </span>
            )}
          </div>
          {view.panels.map((p, i) =>
            panelForm?.index === i ? (
              <div key={i} className="dash-panel form">
                <PanelForm
                  draft={panelForm.draft}
                  queries={view.queries}
                  onChange={(draft) => setPanelForm({ index: i, draft })}
                  onSave={() => void edit({ op: "panel", index: i, ...panelOp(panelForm.draft) })}
                  onCancel={() => setPanelForm(null)}
                  extra={
                    <>
                      <button className="pill-btn" title={t("dash_move_left")} disabled={i === 0} onClick={() => void edit({ op: "move_panel", from: i, to: i - 1 })}><ArrowLeft size={11} /></button>
                      <button className="pill-btn" title={t("dash_move_right")} disabled={i === view.panels.length - 1} onClick={() => void edit({ op: "move_panel", from: i, to: i + 1 })}><ArrowRight size={11} /></button>
                      <button className="pill-btn" title={t("dash_remove_panel")} onClick={() => void edit({ op: "remove_panel", index: i })}><Trash2 size={11} /> {t("dash_remove_panel")}</button>
                    </>
                  }
                />
              </div>
            ) : (
            <div key={i} className={`dash-panel ${p.chart}`}>
              <div className="dash-panel-title">
                <span>{p.title}</span>
                <button className="dash-csv dash-gear" title={t("dash_edit_panel")} onClick={() => setPanelForm({ index: i, draft: panelDraft(p) })}>
                  <Pencil size={9} />
                </button>
                {p.chart !== "number" && !p.error && (
                  <button className="dash-csv" title={t("dash_copy_csv_hint")} onClick={() => void navigator.clipboard.writeText(panelCsv(p))}>
                    <Copy size={9} /> {t("dash_copy_csv")}
                  </button>
                )}
                {p.certified ? (
                  <span className="dash-cert" title={t("dash_certified_hint")}>
                    <ShieldCheck size={10} /> {t("dash_certified")}
                  </span>
                ) : (
                  <span className="dash-draft dim" title={t("dash_draft_hint")}>
                    {t("dash_draft")}
                  </span>
                )}
              </div>
              {p.chart === "number" ? (
                <Number_ p={p} />
              ) : p.error ? (
                <div className="dash-err">{p.error}</div>
              ) : p.chart === "bar" ? (
                <Bars p={p} />
              ) : p.chart === "line" ? (
                <Line p={p} />
              ) : (
                <Grid columns={p.table?.columns ?? []} rows={p.table?.rows ?? []} />
              )}
              {(p.chart === "bar" || p.chart === "line") && p.labels && p.labels.length > 0 && (
                <div className="dash-axis dim">
                  <span>{p.labels[0]}</span>
                  <span>{p.labels[p.labels.length - 1]}</span>
                </div>
              )}
            </div>
            ),
          )}
        </div>
      ) : (
        <div className="dash-sources-screen">
          <div className="dash-queries">
            <div className="dash-list-title dim">{t("dash_sources")}</div>
            {view.sources.map((s) => (
              <span key={s.id} className="dash-source-row">
                <SourceChip s={s} />
                <button title={t("dash_edit")} onClick={() => setSourceForm({ isNew: false, draft: sourceDraft(s) })}><Pencil size={9} /></button>
                <button title={t("dash_remove_source")} onClick={() => void edit({ op: "remove_source", id: s.id })}><Trash2 size={9} /></button>
              </span>
            ))}
            {sourceForm ? (
              <SourceForm draft={sourceForm.draft} isNew={sourceForm.isNew} onChange={(draft) => setSourceForm({ ...sourceForm, draft })} onSave={() => void edit(sourceForm.isNew ? { op: "add_source", ...sourceOp(sourceForm.draft) } : { op: "source", ...sourceOp(sourceForm.draft) })} onCancel={() => setSourceForm(null)} />
            ) : (
              <button className="pill-btn" onClick={() => setSourceForm({ isNew: true, draft: sourceDraft() })}>
                <Plus size={11} /> {t("dash_new_source")}
              </button>
            )}
            <div className="dash-list-title dim">{t("dash_queries")}</div>
            {view.queries.map((q) => (
              <button key={q.id} className={`dash-query ${picked?.id === q.id ? "on" : ""} ${q.changed ? "changed" : ""}`} onClick={() => setQuery(q.id)}>
                <span className="mono">{q.id}</span>
                {q.certified && <ShieldCheck size={10} className="ok" />}
                {q.error && <span className="dash-err-dot" title={q.error} />}
                <span className="dim">{q.source}</span>
              </button>
            ))}
            {queryForm?.isNew ? (
              <QueryForm draft={queryForm.draft} sources={view.sources} isNew onChange={(draft) => setQueryForm({ isNew: true, draft })} onSave={() => void edit({ op: "add_query", ...queryOp(queryForm.draft) })} onCancel={() => setQueryForm(null)} />
            ) : (
              <button className="pill-btn" onClick={() => setQueryForm({ isNew: true, draft: queryDraft() })} disabled={view.sources.length === 0}>
                <Plus size={11} /> {t("dash_new_query")}
              </button>
            )}
            {editErr && <div className="dash-err dash-edit-err">{t("dash_edit_err", { e: editErr })}</div>}
          </div>
          {picked && (
            <div className="dash-query-detail">
              <div className="dash-query-head">
                <span className="mono strong">{picked.id}</span>
                <span className="dim">· {picked.source}</span>
                {picked.changed && <span className="dash-changed-tag">{t("dash_query_changed")}</span>}
                <span className="dash-query-acts">
                  <button className="pill-btn" onClick={() => setQueryForm({ isNew: false, draft: queryDraft(picked) })}>
                    <Pencil size={11} /> {t("dash_edit_query")}
                  </button>
                  <button className="pill-btn" title={t("dash_remove_query")} onClick={() => void edit({ op: "remove_query", id: picked.id })}>
                    <Trash2 size={11} />
                  </button>
                  {picked.certified ? (
                    <button className="pill-btn" onClick={() => void certify(picked.id, false)} title={t("dash_uncertify_hint")}>
                      <ShieldOff size={11} /> {t("dash_uncertify")}
                    </button>
                  ) : (
                    <button className="pill-btn on" onClick={() => void certify(picked.id, true)} title={t("dash_certify_hint")}>
                      <ShieldCheck size={11} /> {t("dash_certify")}
                    </button>
                  )}
                </span>
              </div>
              {queryForm && !queryForm.isNew ? (
                <QueryForm draft={queryForm.draft} sources={view.sources} isNew={false} onChange={(draft) => setQueryForm({ isNew: false, draft })} onSave={() => void edit({ op: "query", ...queryOp(queryForm.draft) })} onCancel={() => setQueryForm(null)} />
              ) : picked.sql ? (
                <pre className="dash-sql mono">{picked.sql}</pre>
              ) : (
                <div className="dim dash-pick">{picked.columns.length ? t("dash_columns", { c: picked.columns.join(", ") }) : t("dash_all_columns")}</div>
              )}
              {picked.error ? (
                <div className="dash-err">{picked.error}</div>
              ) : picked.preview ? (
                <>
                  <div className="dash-list-title dim">{t("dash_preview", { n: picked.preview.rows.length })}</div>
                  <Grid columns={picked.preview.columns} rows={picked.preview.rows} />
                </>
              ) : (
                <div className="dim dash-pick">{t("dash_no_preview")}</div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
