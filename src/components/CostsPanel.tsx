import { useCallback, useEffect, useState } from "react";
import { Info } from "lucide-react";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import { ModelTable } from "./UsageCard";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { BridgeStatus, SpendAgg, StatusLine } from "../types";

/** A jsonl aggregate as a table line (tokens only — no USD in the logs). */
function aggToLine(a: SpendAgg) {
  return {
    model: a.key,
    input: a.input,
    output: a.output,
    cache_read: a.cache_read,
    cache_created: a.cache_created,
    cost_usd: 0,
    turns: a.turns,
  };
}

type Window = "day" | "week";
type Group = "kind" | "workspace" | "label" | "model";

const GROUP_KEY = {
  kind: "g_kind",
  workspace: "g_workspace",
  label: "g_label",
  model: "g_model",
} as const;

/** Human names for the subscription windows the CLI reports. */
function limitName(key: string): string {
  if (key === "five_hour") return t("lim_5h");
  if (key === "seven_day") return t("lim_week");
  const model = key.replace(/^seven_day_?/, "");
  return model ? t("lim_week_model", { m: model }) : key;
}

/** "reinicia em 2h 34min" from an ISO date or epoch seconds. */
function resetIn(raw?: string | null): string | undefined {
  if (!raw) return undefined;
  const ms = /^\d+$/.test(raw) ? Number(raw) * 1000 : Date.parse(raw);
  if (!Number.isFinite(ms)) return undefined;
  const mins = Math.round((ms - Date.now()) / 60000);
  if (mins <= 0) return t("reset_now");
  if (mins < 60) return t("reset_min", { m: mins });
  const h = Math.floor(mins / 60);
  if (h < 24) return t("reset_h", { h, m: mins % 60 });
  return t("reset_d", { d: Math.round(h / 24) });
}

function sinceOf(window: Window): string {
  return new Date(Date.now() - (window === "day" ? 24 : 168) * 3600e3).toISOString();
}

const KIND_KEY: Record<string, string> = {
  ask: "k_ask",
  worker: "k_worker",
  gate: "k_gate",
  dispatch: "k_dispatch",
  session: "k_session",
  local: "k_local",
};
const kindLabel = (key: string) => (KIND_KEY[key] ? t(KIND_KEY[key] as never) : key);

const PALETTE = ["var(--accent)", "var(--ok)", "var(--warn)", "#b48ead", "#88c0d0", "#d08770"];

/**
 * The costs tab (mother window): global, measured, and readable at a
 * glance. Three questions in order — how much did I spend, how close am I
 * to the subscription ceiling, and where is the weight coming from. USD
 * (live rows) and token history (session logs) are never summed.
 */
export default function CostsPanel({ workspace }: { workspace?: string }) {
  const [window, setWindow] = useState<Window>("day");
  const [group, setGroup] = useState<Group>("kind");
  const [live, setLive] = useState<SpendAgg[]>([]);
  const [tokens, setTokens] = useState<SpendAgg[]>([]);
  const [machineWs, setMachineWs] = useState<SpendAgg[]>([]);
  const [top, setTop] = useState<SpendAgg[]>([]);
  const [limits, setLimits] = useState<StatusLine | null>(null);
  const [bridge, setBridge] = useState<BridgeStatus | null>(null);
  const [method, setMethod] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [savings, setSavings] = useState<import("../lib/ipc").SavingsOut | null>(null);
  const [eco, setEco] = useState<import("../lib/ipc").EcoOut | null>(null);

  const load = useCallback(() => {
    const since = sinceOf(window);
    ipc.spendSummary(since, group, "live", workspace).then(setLive).catch(() => setLive([]));
    ipc
      .spendSummary(since, "model", "jsonl", workspace)
      .then(setTokens)
      .catch(() => setTokens([]));
    ipc
      .spendSummary(since, "workspace", "jsonl", workspace)
      .then((aggs) =>
        setMachineWs(
          aggs
            .sort(
              (a, b) =>
                b.input + b.output + b.cache_read - (a.input + a.output + a.cache_read),
            )
            .slice(0, 5),
        ),
      )
      .catch(() => setMachineWs([]));
    ipc.spendTopSessions(since, 6).then(setTop).catch(() => setTop([]));
    ipc.subscriptionLimits().then(setLimits).catch(() => setLimits(null));
    ipc.statuslineBridgeStatus().then(setBridge).catch(() => setBridge(null));
    ipc.savingsSummary(since).then(setSavings).catch(() => setSavings(null));
    ipc.ecoStatus().then(setEco).catch(() => setEco(null));
  }, [window, group, workspace]);
  useEffect(load, [load]);

  const total = live.reduce((a, b) => a + b.cost_usd, 0);
  const turns = live.reduce((a, b) => a + b.turns, 0);
  const errors = live.reduce((a, b) => a + b.errors, 0);
  const io = live.reduce(
    (acc, a) => ({
      input: acc.input + a.input,
      output: acc.output + a.output,
      cache_read: acc.cache_read + a.cache_read,
      cache_created: acc.cache_created + a.cache_created,
    }),
    { input: 0, output: 0, cache_read: 0, cache_created: 0 },
  );
  const inputTotal = io.input + io.cache_read + io.cache_created;
  const hit = inputTotal > 0 ? io.cache_read / inputTotal : null;
  const machineTok = tokens.reduce(
    (a, x) => a + x.input + x.output + x.cache_read + x.cache_created,
    0,
  );

  const costSegments: Segment[] = live.slice(0, 6).map((a, i) => ({
    label: kindLabel(a.key),
    value: a.cost_usd,
    color: PALETTE[i % PALETTE.length],
    detail: fmtUsd(a.cost_usd),
  }));
  const inputSegments: Segment[] = [
    { label: t("seg_cache_read"), value: io.cache_read, color: "var(--ok)" },
    { label: t("seg_cache_new"), value: io.cache_created, color: "var(--warn)" },
    { label: t("seg_prompt"), value: io.input, color: "var(--accent)" },
  ];

  async function installBridge() {
    setBusy(t("br_installing"));
    try {
      const replaced = await ipc.statuslineBridgeInstall();
      setBusy(replaced ? t("br_ok_replaced", { s: replaced }) : t("br_ok"));
      load();
    } catch (err) {
      setBusy(t("br_fail", { e: String(err) }));
    }
  }

  return (
    <div className="costs">
      <header className="costs-bar">
        <div className="costs-hero">
          <span className="costs-label">
            {t("costs_measured")} {workspace ? t("costs_in_project") : ""} ·{" "}
            {window === "day" ? "24h" : t("win_7d")}
          </span>
          <b className="costs-money">{fmtUsd(total)}</b>
          <span className="costs-sub">
            {turns} {turns === 1 ? t("n_turn") : t("n_turns")}
            {errors > 0 && <span className="warn"> · {errors} {t("n_with_error")}</span>}
            {hit != null && (
              <span className="ok"> · {t("costs_cache_absorbed", { p: Math.round(hit * 100) })}</span>
            )}
            {/* The machine never sleeps even when hark's USD is quiet:
                the jsonl tokens keep the hero honest instead of empty. */}
            {machineTok > 0 && (
              <span> · {t("c_machine_short", { t: fmtTok(machineTok) })}</span>
            )}
          </span>
        </div>
        <div className="costs-toggles">
          {(["day", "week"] as const).map((w) => (
            <button key={w} className={window === w ? "on" : ""} onClick={() => setWindow(w)}>
              {w === "day" ? "24h" : t("win_7d")}
            </button>
          ))}
        </div>
      </header>

      <div className="costs-cards">
        <section className="card">
          <h4>{t("c_where")}</h4>
          <div className="costs-groups">
            {(Object.keys(GROUP_KEY) as Group[]).map((g) => (
              <button key={g} className={group === g ? "on" : ""} onClick={() => setGroup(g)}>
                {t(GROUP_KEY[g])}
              </button>
            ))}
          </div>
          {live.length === 0 ? (
            <p className="empty">{t("e_window")}</p>
          ) : (
            <>
              <StackedBar segments={costSegments} height={10} />
              <Legend segments={costSegments} />
              <div className="costs-rows">
                {live.slice(0, 8).map((a, i) => (
                  <MeterRow
                    key={a.key}
                    name={KIND_KEY[a.key] ? kindLabel(a.key) : a.key.replace(/^claude-/, "")}
                    value={a.cost_usd}
                    max={live[0].cost_usd}
                    detail={fmtUsd(a.cost_usd)}
                    color={PALETTE[i % PALETTE.length]}
                    note={`${a.turns} ${t("n_turns")} · in ${fmtTok(a.input)} · out ${fmtTok(a.output)} · cache ${fmtTok(a.cache_read)}${a.errors > 0 ? ` · ${a.errors} ✗` : ""}`}
                  />
                ))}
              </div>
            </>
          )}
        </section>

        {savings && savings.total_usd > 0 && (
          <section className="card">
            <h4>{t("c_saved")}</h4>
            <div className="saved-hero">
              <b className="costs-money ok">{fmtUsd(savings.total_usd)}</b>
              <span className="costs-sub">{t("c_saved_sub")}</span>
            </div>
            <div className="costs-rows">
              <MeterRow
                name={t("s_gate")}
                value={savings.avoided_gate_usd}
                max={savings.total_usd}
                detail={fmtUsd(savings.avoided_gate_usd)}
                color="var(--ok)"
              />
              <MeterRow
                name={t("s_local")}
                value={savings.avoided_local_usd}
                max={savings.total_usd}
                detail={fmtUsd(savings.avoided_local_usd)}
                color="var(--accent)"
              />
              <MeterRow
                name={t("s_cache")}
                value={savings.avoided_cache_usd}
                max={savings.total_usd}
                detail={`${fmtUsd(savings.avoided_cache_usd)} ~`}
                color="var(--warn)"
              />
            </div>
            <details className="saved-method">
              <summary>{t("c_method")}</summary>
              <ul>
                {savings.methodology.map((m, i) => (
                  <li key={i}>{m}</li>
                ))}
              </ul>
            </details>
          </section>
        )}

        {eco && (
          <section className="card">
            <h4>{t("c_eco")}</h4>
            <div className="eco-row">
              {(["rtk", "ponytail", "caveman", "tokensave"] as const).map((tool) => (
                <span key={tool} className={`eco-chip ${eco.status[tool] ? "on" : ""}`}>
                  {tool} {eco.status[tool] ? "✓" : "—"}
                </span>
              ))}
            </div>
            <p className="hint">
              {eco.envs.length > 0
                ? t("c_eco_envs", { envs: eco.envs.map(([k, v]) => `${k}=${v}`).join(" · ") })
                : t("c_eco_none")}
            </p>
            <p className="hint">{t("c_eco_fp", { fp: eco.fingerprint })}</p>
          </section>
        )}

        <section className="card">
          <h4>{t("c_limits")}</h4>
          {limits && limits.limits.length > 0 ? (
            <>
              {limits.limits.map((l) => (
                <Gauge
                  key={l.key}
                  name={limitName(l.key)}
                  used={l.used}
                  caption={resetIn(l.resets_at)}
                />
              ))}
              {limits.context_used != null && (
                <p className="hint">
                  {t("ctx_active", { p: Math.round(limits.context_used * 100) })}
                  {limits.model ? ` · ${limits.model}` : ""}
                </p>
              )}
            </>
          ) : bridge?.installed ? (
            // Installed, payload pending: a compact status — the sales
            // pitch is for machines that DON'T have the bridge yet.
            <div className="bridge-offer">
              <p>
                <span className="ok">{t("br_installed")}</span>
                {" · "}
                <span className="hint">
                  {bridge.age_secs == null
                    ? t("br_waiting")
                    : t("br_age", { m: Math.round(bridge.age_secs / 60) })}
                </span>
              </p>
              <div className="bridge-actions">
                <button
                  onClick={() => ipc.statuslineBridgeUninstall().then(load).catch(() => {})}
                >
                  {t("br_remove")}
                </button>
                {busy && <span className="hint">{busy}</span>}
              </div>
            </div>
          ) : (
            <div className="bridge-offer">
              <p>
                {t("br_p1_pre")}
                <b>{t("br_p1_bold")}</b>.
              </p>
              <p className="hint">
                {t("br_hint_pre")}<code>~/.claude/settings.json</code>{t("br_hint_post")}
              </p>
              <div className="bridge-actions">
                <button className="primary" onClick={installBridge}>
                  {t("br_install")}
                </button>
                {busy && <span className="hint">{busy}</span>}
              </div>
            </div>
          )}
        </section>

        <section className="card">
          <h4>{t("c_input")}</h4>
          {inputTotal === 0 ? (
            <p className="empty">{t("e_turns")}</p>
          ) : (
            <>
              <StackedBar segments={inputSegments} height={10} />
              <Legend segments={inputSegments} />
              <p className="hint">{t("hint_cache", { t: fmtTok(io.output) })}</p>
            </>
          )}
        </section>

        <section className="card">
          <h4>{t("c_top")}</h4>
          {top.length === 0 ? (
            <p className="empty">{t("e_top")}</p>
          ) : (
            <div className="costs-rows">
              {top.map((a) => (
                <MeterRow
                  key={a.key}
                  name={a.key}
                  value={a.cost_usd}
                  max={top[0].cost_usd}
                  detail={fmtUsd(a.cost_usd)}
                  note={`${a.turns} ${t("n_turns")} · cache ${fmtTok(a.cache_read)}`}
                />
              ))}
            </div>
          )}
        </section>

        <section className="card wide">
          <h4>
            {t("c_machine")} · {window === "day" ? "24h" : t("win_7d")}
          </h4>
          {tokens.length === 0 ? (
            <p className="empty">{t("e_logs")}</p>
          ) : (
            <div className="machine-cols">
              <ModelTable lines={tokens.slice(0, 8).map(aggToLine)} cost={false} />
              {machineWs.length > 0 && (
                <div className="costs-rows">
                  <div className="scope-block-head">
                    <span>{t("u_ws")}</span>
                  </div>
                  {machineWs.map((w) => {
                    const toks = w.input + w.output + w.cache_read;
                    const max = Math.max(
                      ...machineWs.map((x) => x.input + x.output + x.cache_read),
                      1,
                    );
                    return (
                      <MeterRow
                        key={w.key}
                        name={w.key.split("/").pop() ?? w.key}
                        value={toks}
                        max={max}
                        detail={fmtTok(toks)}
                        note={`${w.turns} ${t("n_turns")}`}
                      />
                    );
                  })}
                </div>
              )}
            </div>
          )}
          <p className="hint">{t("hint_machine")}</p>
        </section>
      </div>

      <button className="costs-method-toggle" onClick={() => setMethod((m) => !m)}>
        <Info size={12} /> {t("method_toggle")}
      </button>
      {method && (
        <div className="costs-method">
          <p>
            <b>{t("meth1_term")}</b> {t("meth1")}
          </p>
          <p>
            <b>{t("meth2_term")}</b> {t("meth2")}
          </p>
          <p>
            <b>{t("meth3_term")}</b> {t("meth3")}
          </p>
          <p>
            <b>{t("meth4_term")}</b> {t("meth4")}
          </p>
        </div>
      )}
    </div>
  );
}
