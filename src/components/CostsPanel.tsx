import { useCallback, useEffect, useState } from "react";
import { Info } from "lucide-react";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import * as ipc from "../lib/ipc";
import { t } from "../lib/i18n";
import type { BridgeStatus, SpendAgg, StatusLine } from "../types";

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
  const [top, setTop] = useState<SpendAgg[]>([]);
  const [limits, setLimits] = useState<StatusLine | null>(null);
  const [bridge, setBridge] = useState<BridgeStatus | null>(null);
  const [method, setMethod] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    const since = sinceOf(window);
    ipc.spendSummary(since, group, "live", workspace).then(setLive).catch(() => setLive([]));
    ipc
      .spendSummary(since, "model", "jsonl", workspace)
      .then(setTokens)
      .catch(() => setTokens([]));
    ipc.spendTopSessions(since, 6).then(setTop).catch(() => setTop([]));
    ipc.subscriptionLimits().then(setLimits).catch(() => setLimits(null));
    ipc.statuslineBridgeStatus().then(setBridge).catch(() => setBridge(null));
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
                {bridge?.installed ? (
                  <>
                    <span className="ok">{t("br_installed")}</span>
                    <span className="hint">
                      {bridge.age_secs == null
                        ? t("br_waiting")
                        : t("br_age", { m: Math.round(bridge.age_secs / 60) })}
                    </span>
                    <button
                      onClick={() =>
                        ipc.statuslineBridgeUninstall().then(load).catch(() => {})
                      }
                    >
                      {t("br_remove")}
                    </button>
                  </>
                ) : (
                  <button className="primary" onClick={installBridge}>
                    {t("br_install")}
                  </button>
                )}
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
          <h4>{t("c_machine")}</h4>
          {tokens.length === 0 ? (
            <p className="empty">{t("e_logs")}</p>
          ) : (
            <div className="costs-rows two">
              {tokens.slice(0, 8).map((a) => {
                const totalTok = a.input + a.output + a.cache_read + a.cache_created;
                return (
                  <MeterRow
                    key={a.key}
                    name={a.key.replace(/^claude-/, "")}
                    value={totalTok}
                    max={
                      tokens[0].input +
                      tokens[0].output +
                      tokens[0].cache_read +
                      tokens[0].cache_created
                    }
                    detail={fmtTok(totalTok)}
                    color="var(--dim)"
                    note={t("mt_note", { a: fmtTok(a.cache_read), b: fmtTok(a.cache_created), c: fmtTok(a.output) })}
                  />
                );
              })}
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
