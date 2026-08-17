import { useEffect, useState } from "react";
import * as ipc from "../lib/ipc";
import type { SpendAgg } from "../types";

type Window = "day" | "week";
type Group = "kind" | "workspace" | "label" | "model";

const GROUP_LABEL: Record<Group, string> = {
  kind: "por tipo",
  workspace: "por projeto",
  label: "por task",
  model: "por modelo",
};

function sinceOf(window: Window): string {
  const ms = window === "day" ? 24 * 3600e3 : 7 * 24 * 3600e3;
  return new Date(Date.now() - ms).toISOString();
}

const fmtTok = (n: number) =>
  n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}k` : `${n}`;

/** Cache hit: how much of the input the cache absorbed. */
function cacheHit(a: { input: number; cache_read: number; cache_created: number }): number | null {
  const denom = a.input + a.cache_read + a.cache_created;
  return denom > 0 ? a.cache_read / denom : null;
}

/**
 * The persistent cost panel (board tab): what was spent (USD, measured by
 * the CLI on Vox turns) and what the machine consumed in tokens (session
 * logs). The two sources are never summed — methodology pinned below.
 */
export default function CostsPanel() {
  const [window, setWindow] = useState<Window>("day");
  const [group, setGroup] = useState<Group>("kind");
  const [live, setLive] = useState<SpendAgg[]>([]);
  const [tokens, setTokens] = useState<SpendAgg[]>([]);
  const [top, setTop] = useState<SpendAgg[]>([]);

  useEffect(() => {
    const since = sinceOf(window);
    ipc.spendSummary(since, group, "live").then(setLive).catch(() => setLive([]));
    ipc.spendSummary(since, "model", "jsonl").then(setTokens).catch(() => setTokens([]));
    ipc.spendTopSessions(since, 5).then(setTop).catch(() => setTop([]));
  }, [window, group]);

  const total = live.reduce((a, b) => a + b.cost_usd, 0);
  const totals = live.reduce(
    (acc, a) => ({
      input: acc.input + a.input,
      cache_read: acc.cache_read + a.cache_read,
      cache_created: acc.cache_created + a.cache_created,
    }),
    { input: 0, cache_read: 0, cache_created: 0 },
  );
  const hit = cacheHit(totals);

  return (
    <div className="costs">
      <div className="costs-head">
        <h3>custos</h3>
        <div className="costs-toggles">
          {(["day", "week"] as const).map((w) => (
            <button key={w} className={window === w ? "on" : ""} onClick={() => setWindow(w)}>
              {w === "day" ? "24h" : "7 dias"}
            </button>
          ))}
          <span className="sep" />
          {(Object.keys(GROUP_LABEL) as Group[]).map((g) => (
            <button key={g} className={group === g ? "on" : ""} onClick={() => setGroup(g)}>
              {GROUP_LABEL[g]}
            </button>
          ))}
        </div>
        <div className="costs-total">
          <b>${total.toFixed(4)}</b>
          {hit != null && <span title="quanto do input veio do cache (barato)"> · cache {Math.round(hit * 100)}%</span>}
        </div>
      </div>

      <div className="costs-grid">
        <section>
          <h4>gasto medido (USD · turnos do Vox)</h4>
          {live.length === 0 && <div className="side-empty">nada nesta janela</div>}
          <table>
            <tbody>
              {live.map((a) => (
                <tr key={a.key}>
                  <td className="k" title={a.key}>{a.key}</td>
                  <td className="n">${a.cost_usd.toFixed(4)}</td>
                  <td className="n">{a.turns} turnos</td>
                  <td className="n">{a.errors > 0 ? `${a.errors} erros` : ""}</td>
                  <td className="n dim">
                    in {fmtTok(a.input)} · out {fmtTok(a.output)} · cache {fmtTok(a.cache_read)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section>
          <h4>sessões mais caras (USD)</h4>
          {top.length === 0 && <div className="side-empty">nada nesta janela</div>}
          <table>
            <tbody>
              {top.map((a) => (
                <tr key={a.key}>
                  <td className="k" title={a.key}>{a.key}</td>
                  <td className="n">${a.cost_usd.toFixed(4)}</td>
                  <td className="n dim">{a.turns} turnos</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section>
          <h4>tokens da máquina (histórico dos session logs)</h4>
          <table>
            <tbody>
              {tokens.map((a) => (
                <tr key={a.key}>
                  <td className="k">{a.key.replace(/^claude-/, "")}</td>
                  <td className="n dim">in {fmtTok(a.input)}</td>
                  <td className="n dim">out {fmtTok(a.output)}</td>
                  <td className="n dim">cache_read {fmtTok(a.cache_read)}</td>
                  <td className="n dim">cache_novo {fmtTok(a.cache_created)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </div>

      <div className="costs-method">
        metodologia: USD é o valor medido pelo CLI nos turnos do Vox; tokens vêm dos
        session logs locais (máquina inteira, inclui uso fora do Vox); as duas fontes
        nunca são somadas. cache % = cache_read / (input + cache_read + cache_novo).
      </div>
    </div>
  );
}
