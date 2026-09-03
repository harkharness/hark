import { useState } from "react";
import { Gauge, MeterRow, fmtTok, fmtUsd } from "./Meter";
import * as ipc from "../lib/ipc";
import { shortModel } from "../lib/format";
import { t } from "../lib/i18n";
import type { StatusLine, UsageModelLine, UsageReport } from "../types";

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

const kindName = (key: string): string =>
  ({
    ask: t("k_ask"),
    worker: t("k_worker"),
    gate: t("k_gate"),
    dispatch: t("k_dispatch"),
    session: t("k_session"),
    local: t("k_local"),
  })[key] ?? key;

function fmtDur(ms: number): string {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return m < 60 ? `${m}min ${s % 60}s` : `${Math.floor(m / 60)}h ${m % 60}min`;
}

/** Per-model table; jsonl rows have no USD, so cost renders "–" there. */
function ModelTable({ lines, cost }: { lines: UsageModelLine[]; cost: boolean }) {
  return (
    <div className="usage-table-wrap">
      <table className="usage-table">
        <thead>
          <tr>
            <th />
            <th>{t("u_in")}</th>
            <th>{t("u_out")}</th>
            <th>{t("u_cache_r")}</th>
            <th>{t("u_cache_w")}</th>
            {cost && <th>{t("u_cost")}</th>}
          </tr>
        </thead>
        <tbody>
          {lines.map((m) => (
            <tr key={m.model}>
              <td className="usage-model">{shortModel(m.model)}</td>
              <td>{fmtTok(m.input)}</td>
              <td>{fmtTok(m.output)}</td>
              <td>{fmtTok(m.cache_read)}</td>
              <td>{fmtTok(m.cache_created)}</td>
              {cost && <td>{m.cost_usd > 0 ? fmtUsd(m.cost_usd) : "–"}</td>}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** The /usage card, drawn in the thread: subscription windows, this
 *  session, the machine's last 24h (EVERY agent session on this box, from
 *  the local index — exact, not approximate), and hark's own spend. All
 *  local, zero tokens. */
export default function UsageCard({ report }: { report: UsageReport }) {
  const s = report.session;
  // The bridge can be installed right from the card; the gauges appear in
  // place on success (the message itself is static — this state is not).
  const [limits, setLimits] = useState<StatusLine | null>(report.limits);
  const [bridgeBusy, setBridgeBusy] = useState(false);
  const [bridgeNote, setBridgeNote] = useState<string | null>(null);

  async function installBridge() {
    setBridgeBusy(true);
    try {
      await ipc.statuslineBridgeInstall();
      setBridgeNote(t("u_bridge_done"));
      const fresh = await ipc.subscriptionLimits().catch(() => null);
      if (fresh) setLimits(fresh);
    } catch (err) {
      setBridgeNote(String(err));
    } finally {
      setBridgeBusy(false);
    }
  }

  return (
    <div className="usage-card">
      <div className="usage-head">
        <b>{t("u_title")}</b>
        <span className="hint">{t("u_local")}</span>
      </div>

      {limits && limits.limits.length > 0 ? (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("c_limits")}</span>
          </div>
          {limits.limits.map((l) => (
            <Gauge key={l.key} name={limitName(l.key)} used={l.used} caption={resetIn(l.resets_at)} />
          ))}
        </section>
      ) : (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("c_limits")}</span>
          </div>
          <p className="hint">{bridgeNote ?? t("u_no_bridge")}</p>
          {!bridgeNote && (
            <button className="scope-detail" onClick={installBridge} disabled={bridgeBusy}>
              {bridgeBusy ? "…" : t("u_bridge_btn")}
            </button>
          )}
        </section>
      )}

      {s && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("u_session")}</span>
            <b>{s.cost_usd > 0 ? fmtUsd(s.cost_usd) : ""}</b>
          </div>
          <div className="usage-facts">
            {s.turns > 0 && (
              <span>
                {s.turns} {t("n_turns")}
              </span>
            )}
            {s.duration_ms > 0 && <span>API {fmtDur(s.duration_ms)}</span>}
            {s.cache_hit != null && (
              <span>
                cache hit <b>{Math.round(s.cache_hit * 100)}%</b>
              </span>
            )}
          </div>
          {s.context.pct != null && (
            <Gauge
              name={t("u_ctx")}
              used={s.context.pct * 100}
              caption={`${fmtTok(s.context.total)}${s.context.window ? ` / ${fmtTok(s.context.window)}` : ""}`}
            />
          )}
          {s.models.length > 0 && <ModelTable lines={s.models} cost />}
        </section>
      )}

      {report.machine.models.length > 0 && (
        <section className="scope-block">
          <div className="scope-block-head">
            <span>{t("u_machine")}</span>
            <span className="hint">
              {report.machine.turns} {t("n_turns")}
            </span>
          </div>
          <ModelTable lines={report.machine.models} cost={false} />
          {report.machine.workspaces.length > 0 && (
            <>
              <div className="scope-block-head usage-sub">
                <span>{t("u_ws")}</span>
              </div>
              {report.machine.workspaces.map((w) => {
                const toks = w.input + w.output + w.cache_read;
                const max = Math.max(
                  ...report.machine.workspaces.map((x) => x.input + x.output + x.cache_read),
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
            </>
          )}
        </section>
      )}

      <section className="scope-block">
        <div className="scope-block-head">
          <span>{t("u_day")}</span>
          <b>{fmtUsd(report.day.total_usd)}</b>
        </div>
        <div className="usage-facts">
          <span>
            {t("si_7d")} <b>{fmtUsd(report.day.week_usd)}</b>
          </span>
        </div>
        {report.day.kinds
          .filter((k) => k.cost_usd > 0)
          .map((k) => (
            <MeterRow
              key={k.key}
              name={kindName(k.key)}
              value={k.cost_usd}
              max={Math.max(...report.day.kinds.map((x) => x.cost_usd), 0.0001)}
              detail={fmtUsd(k.cost_usd)}
              note={`${k.turns} ${t("n_turns")}`}
            />
          ))}
        {report.day.top.length > 0 && (
          <>
            <div className="scope-block-head usage-sub">
              <span>{t("u_top")}</span>
            </div>
            {report.day.top.map((a) => (
              <MeterRow
                key={a.key}
                name={a.key}
                value={a.cost_usd}
                max={report.day.top[0].cost_usd}
                detail={fmtUsd(a.cost_usd)}
                note={`${a.turns} ${t("n_turns")}`}
              />
            ))}
          </>
        )}
        {report.day.total_usd === 0 && report.day.week_usd === 0 && (
          <p className="hint">{t("u_quiet")}</p>
        )}
      </section>
    </div>
  );
}
