import { useCallback, useEffect, useState } from "react";
import { Info } from "lucide-react";
import { Gauge, Legend, MeterRow, StackedBar, fmtTok, fmtUsd, type Segment } from "./Meter";
import * as ipc from "../lib/ipc";
import type { BridgeStatus, SpendAgg, StatusLine } from "../types";

type Window = "day" | "week";
type Group = "kind" | "workspace" | "label" | "model";

const GROUP_LABEL: Record<Group, string> = {
  kind: "por tipo",
  workspace: "por projeto",
  label: "por task",
  model: "por modelo",
};

/** Human names for the subscription windows the CLI reports. */
function limitName(key: string): string {
  if (key === "five_hour") return "janela de 5 horas";
  if (key === "seven_day") return "semanal · todos os modelos";
  const model = key.replace(/^seven_day_?/, "");
  return model ? `semanal · ${model}` : key;
}

/** "reinicia em 2h 34min" from an ISO date or epoch seconds. */
function resetIn(raw?: string | null): string | undefined {
  if (!raw) return undefined;
  const ms = /^\d+$/.test(raw) ? Number(raw) * 1000 : Date.parse(raw);
  if (!Number.isFinite(ms)) return undefined;
  const mins = Math.round((ms - Date.now()) / 60000);
  if (mins <= 0) return "reinicia agora";
  if (mins < 60) return `reinicia em ${mins}min`;
  const h = Math.floor(mins / 60);
  if (h < 24) return `reinicia em ${h}h ${mins % 60}min`;
  return `reinicia em ${Math.round(h / 24)}d`;
}

function sinceOf(window: Window): string {
  return new Date(Date.now() - (window === "day" ? 24 : 168) * 3600e3).toISOString();
}

const KIND_LABEL: Record<string, string> = {
  ask: "perguntas ao vox",
  worker: "trabalho (workers)",
  gate: "avaliador",
  dispatch: "despachos",
  session: "sessões (histórico)",
  local: "respostas locais",
};

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
    label: KIND_LABEL[a.key] ?? a.key,
    value: a.cost_usd,
    color: PALETTE[i % PALETTE.length],
    detail: fmtUsd(a.cost_usd),
  }));
  const inputSegments: Segment[] = [
    { label: "cache lido (barato)", value: io.cache_read, color: "var(--ok)" },
    { label: "cache escrito", value: io.cache_created, color: "var(--warn)" },
    { label: "prompt novo", value: io.input, color: "var(--accent)" },
  ];

  async function installBridge() {
    setBusy("instalando…");
    try {
      const replaced = await ipc.statuslineBridgeInstall();
      setBusy(
        replaced
          ? `pronto — sua status line (${replaced}) continua rodando por baixo`
          : "pronto — reinicie uma sessão do Claude Code para preencher",
      );
      load();
    } catch (err) {
      setBusy(`falhou: ${err}`);
    }
  }

  return (
    <div className="costs">
      <header className="costs-bar">
        <div className="costs-hero">
          <span className="costs-label">
            gasto medido {workspace ? "no projeto" : ""} · {window === "day" ? "24h" : "7 dias"}
          </span>
          <b className="costs-money">{fmtUsd(total)}</b>
          <span className="costs-sub">
            {turns} turno{turns === 1 ? "" : "s"}
            {errors > 0 && <span className="warn"> · {errors} com erro</span>}
            {hit != null && <span className="ok"> · cache absorveu {Math.round(hit * 100)}%</span>}
          </span>
        </div>
        <div className="costs-toggles">
          {(["day", "week"] as const).map((w) => (
            <button key={w} className={window === w ? "on" : ""} onClick={() => setWindow(w)}>
              {w === "day" ? "24h" : "7 dias"}
            </button>
          ))}
        </div>
      </header>

      <div className="costs-cards">
        <section className="card">
          <h4>para onde o dinheiro foi</h4>
          <div className="costs-groups">
            {(Object.keys(GROUP_LABEL) as Group[]).map((g) => (
              <button key={g} className={group === g ? "on" : ""} onClick={() => setGroup(g)}>
                {GROUP_LABEL[g]}
              </button>
            ))}
          </div>
          {live.length === 0 ? (
            <p className="empty">nada medido nesta janela</p>
          ) : (
            <>
              <StackedBar segments={costSegments} height={10} />
              <Legend segments={costSegments} />
              <div className="costs-rows">
                {live.slice(0, 8).map((a, i) => (
                  <MeterRow
                    key={a.key}
                    name={KIND_LABEL[a.key] ?? a.key.replace(/^claude-/, "")}
                    value={a.cost_usd}
                    max={live[0].cost_usd}
                    detail={fmtUsd(a.cost_usd)}
                    color={PALETTE[i % PALETTE.length]}
                    note={`${a.turns} turnos · in ${fmtTok(a.input)} · out ${fmtTok(a.output)} · cache ${fmtTok(a.cache_read)}${a.errors > 0 ? ` · ${a.errors} erro(s)` : ""}`}
                  />
                ))}
              </div>
            </>
          )}
        </section>

        <section className="card">
          <h4>limites da assinatura</h4>
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
                  janela de contexto da sessão ativa: {Math.round(limits.context_used * 100)}%
                  {limits.model ? ` · ${limits.model}` : ""}
                </p>
              )}
            </>
          ) : (
            <div className="bridge-offer">
              <p>
                O CLI só publica o percentual das janelas de uso (5h, semanal) na status line.
                O Vox pode ler esse dado com uma ponte local: um script que copia o payload
                para um arquivo e <b>continua chamando a sua status line atual</b>.
              </p>
              <p className="hint">
                Altera <code>~/.claude/settings.json</code> — com backup automático, e
                reversível a qualquer momento.
              </p>
              <div className="bridge-actions">
                {bridge?.installed ? (
                  <>
                    <span className="ok">ponte instalada</span>
                    <span className="hint">
                      {bridge.age_secs == null
                        ? "aguardando a primeira sessão do Claude Code"
                        : `último dado há ${Math.round(bridge.age_secs / 60)}min`}
                    </span>
                    <button
                      onClick={() =>
                        ipc.statuslineBridgeUninstall().then(load).catch(() => {})
                      }
                    >
                      remover
                    </button>
                  </>
                ) : (
                  <button className="primary" onClick={installBridge}>
                    instalar a ponte
                  </button>
                )}
                {busy && <span className="hint">{busy}</span>}
              </div>
            </div>
          )}
        </section>

        <section className="card">
          <h4>composição do input</h4>
          {inputTotal === 0 ? (
            <p className="empty">sem turnos nesta janela</p>
          ) : (
            <>
              <StackedBar segments={inputSegments} height={10} />
              <Legend segments={inputSegments} />
              <p className="hint">
                cache lido é a parte barata: quanto maior a fatia verde, menos você paga pelo
                mesmo contexto. Saída gerada: {fmtTok(io.output)} tokens.
              </p>
            </>
          )}
        </section>

        <section className="card">
          <h4>sessões mais caras</h4>
          {top.length === 0 ? (
            <p className="empty">nada nesta janela</p>
          ) : (
            <div className="costs-rows">
              {top.map((a) => (
                <MeterRow
                  key={a.key}
                  name={a.key}
                  value={a.cost_usd}
                  max={top[0].cost_usd}
                  detail={fmtUsd(a.cost_usd)}
                  note={`${a.turns} turnos · cache ${fmtTok(a.cache_read)}`}
                />
              ))}
            </div>
          )}
        </section>

        <section className="card wide">
          <h4>tokens da máquina · histórico dos session logs</h4>
          {tokens.length === 0 ? (
            <p className="empty">nenhum log indexado</p>
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
                    note={`cache lido ${fmtTok(a.cache_read)} · cache novo ${fmtTok(a.cache_created)} · saída ${fmtTok(a.output)}`}
                  />
                );
              })}
            </div>
          )}
          <p className="hint">
            Esta seção mede a máquina inteira (todo uso do Claude Code, dentro ou fora do
            Vox) e não tem USD: os logs guardam tokens, não preço.
          </p>
        </section>
      </div>

      <button className="costs-method-toggle" onClick={() => setMethod((m) => !m)}>
        <Info size={12} /> como esses números são calculados
      </button>
      {method && (
        <div className="costs-method">
          <p>
            <b>USD</b> vem exclusivamente do valor que o CLI reporta em cada turno do Vox
            (nunca de tabela de preço): assinatura, não API.
          </p>
          <p>
            <b>Tokens da máquina</b> vêm dos session logs locais e cobrem todo o uso do
            Claude Code neste computador. As duas fontes <b>nunca são somadas</b> — medem
            coisas diferentes.
          </p>
          <p>
            <b>cache absorveu</b> = cache lido ÷ (prompt novo + cache lido + cache escrito).
          </p>
          <p>
            <b>Limites da assinatura</b> só aparecem com a ponte da status line instalada, e
            somem se o dado tiver mais de 10 minutos (dado velho engana).
          </p>
        </div>
      )}
    </div>
  );
}
