import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Markdown from "./Markdown";
import Reader from "./Reader";
import ToolCall, { ToolOutput } from "./ToolCall";
import type {
  Directives,
  DispatchOutcome,
  Msg,
  Overview,
  PermissionAsk,
  Reply,
  VoxEvent,
} from "./types";

type Pending =
  | { kind: "confirm-dispatch"; instruction: string }
  | { kind: "permission"; ask: PermissionAsk }
  | {
      kind: "choice";
      instruction: string;
      candidates: { session_id: string; title: string; last_ts: string }[];
    }
  | { kind: "resume-task"; title: string; sessionId?: string; instruction: string }
  | { kind: "task-summary"; taskId: string }
  | null;

export default function App() {
  const [messages, setMessages] = useState<Msg[]>([]);
  const [events, setEvents] = useState<VoxEvent[]>([]);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [input, setInput] = useState("");
  const [image, setImage] = useState<string | null>(null); // dataURL
  const [busy, setBusy] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [pending, setPending] = useState<Pending>(null);
  const [tab, setTab] = useState<"chat" | "board">("chat");
  // Read-only thread being viewed (board tab), never executes anything.
  const [reading, setReading] = useState<{
    sessionId: string;
    title: string;
    resume?: { title: string; note?: string };
  } | null>(null);
  // All live conversational workers, keyed by task_id.
  const [liveWorkers, setLiveWorkers] = useState<
    Record<
      string,
      {
        label: string;
        status: "running" | "turn_done" | "awaiting";
        directives: Directives;
      }
    >
  >({});
  const [focused, setFocused] = useState<string | null>(null);
  const workersRef = useRef(liveWorkers);
  workersRef.current = liveWorkers;
  const focusedRef = useRef(focused);
  focusedRef.current = focused;
  const [speak, setSpeak] = useState(true);
  const speakRef = useRef(speak);
  speakRef.current = speak;
  const endRef = useRef<HTMLDivElement>(null);

  const push = useCallback((m: Msg) => setMessages((old) => [...old, m]), []);
  const refresh = useCallback(() => {
    invoke<Overview>("overview").then(setOverview).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
    const un1 = listen<VoxEvent>("vox", (e) => {
      const ev = e.payload;
      setEvents((old) => [...old.slice(-199), ev]);
      // Rich live transcript for conversational workers.
      if (ev.kind === "assistant_text") {
        push({ who: "vox", text: ev.text, task: ev.task_id });
      } else if (ev.kind === "worker") {
        push({ who: "tool", name: ev.name, input: ev.input, task: ev.task_id });
      } else if (ev.kind === "tool_result") {
        push({
          who: "output",
          content: ev.content,
          error: ev.is_error,
          task: ev.task_id,
        });
      } else if (ev.kind === "worker_turn") {
        push({
          who: "vox",
          text: ev.text,
          cost: ev.cost_usd,
          model: ev.model,
          task: ev.task_id,
        });
        setLiveWorkers((old) =>
          old[ev.task_id]
            ? { ...old, [ev.task_id]: { ...old[ev.task_id], status: "turn_done" } }
            : old,
        );
        const label = workersRef.current[ev.task_id]?.label ?? ev.task_id.slice(0, 12);
        if (speakRef.current)
          invoke("speak", {
            text: ev.is_error
              ? `A task ${label} falhou, olha a tela.`
              : `Task ${label} terminou o turno.`,
          }).catch(() => {});
        refresh();
      } else if (ev.kind === "worker_exit") {
        push({ who: "sys", text: `worker ${ev.task_id} encerrado` });
        setLiveWorkers((old) => {
          const next = { ...old };
          delete next[ev.task_id];
          return next;
        });
        if (focusedRef.current === ev.task_id) setFocused(null);
        refresh();
      }
    });
    // Permission requests land INLINE in the thread, never as a blocking
    // modal: other workers must keep streaming while one waits.
    const un2 = listen<PermissionAsk>("vox-permission", (e) => {
      const ask = e.payload;
      push({
        who: "permission",
        requestId: ask.request_id,
        tool: ask.tool_name,
        input: ask.input,
        task: ask.task_id,
      });
      setLiveWorkers((old) =>
        old[ask.task_id]
          ? { ...old, [ask.task_id]: { ...old[ask.task_id], status: "awaiting" } }
          : old,
      );
      if (speakRef.current)
        invoke("speak", { text: `${ask.tool_name} pede permissão.` }).catch(() => {});
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, [refresh]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const say = useCallback((text: string) => {
    if (speakRef.current) invoke("speak", { text }).catch(() => {});
  }, []);

  async function runAsk(question: string, img: string | null) {
    setBusy("perguntando…");
    try {
      const reply = await invoke<Reply>("ask_text", {
        question,
        imageB64: img ? img.split(",")[1] : null,
        mediaType: img ? img.slice(5, img.indexOf(";")) : null,
      });
      push({
        who: "vox",
        text: reply.fala,
        detalhes: reply.detalhes,
        itens: reply.itens,
        cost: reply.cost_usd,
        model: reply.model,
      });
      say(reply.fala);
    } catch (err) {
      push({ who: "sys", text: `erro: ${err}` });
    } finally {
      setBusy(null);
      refresh();
    }
  }

  async function runDispatch(instruction: string, sessionId?: string) {
    setBusy("despachando…");
    push({ who: "sys", text: `dispatch: ${instruction}` });
    try {
      const out = await invoke<DispatchOutcome>("worker_start", {
        instruction,
        sessionId: sessionId ?? null,
      });
      if (out.status === "started") {
        const label = instruction.split(/\s+/).slice(0, 5).join(" ");
        setLiveWorkers((old) => ({
          ...old,
          [out.task_id]: { label, status: "running", directives: out.directives },
        }));
        setFocused(out.task_id);
        push({
          who: "sys",
          text: `worker ${out.task_id} ativo (${label}); input responde a ele enquanto focado`,
        });
      } else if (out.status === "done") {
        push({ who: "vox", text: out.summary, cost: out.cost_usd });
        say("Tarefa concluída.");
      } else if (out.status === "failed") {
        push({ who: "sys", text: `worker falhou: ${out.summary}` });
        say("A tarefa falhou. Detalhes na tela.");
      } else if (out.status === "choice") {
        setPending({ kind: "choice", instruction, candidates: out.candidates });
      } else if (out.status === "busy") {
        push({
          who: "sys",
          text: `sessão ${out.session_id} está aberta num terminal; feche-a primeiro`,
        });
        say("A sessão alvo está aberta num terminal.");
      } else {
        push({ who: "sys", text: "nenhuma sessão bate com essa instrução" });
        say("Não achei sessão pra isso.");
      }
    } catch (err) {
      push({ who: "sys", text: `erro: ${err}` });
    } finally {
      setBusy(null);
      refresh();
    }
  }

  const QUESTION_START =
    /^(quais|qual|como|o que|onde|quando|por que|porque|quem|quanto|lista|resumo|status)\b/i;

  async function submit(text: string, img: string | null) {
    if (!text.trim() || busy) return;
    const route = await invoke<string>("route_text", { text });
    // Action verbs ALWAYS open a new parallel worker ("enquanto isso faz X"),
    // even while another one is focused.
    if (route === "dispatch") {
      push({ who: "user", text });
      setInput("");
      setPending({ kind: "confirm-dispatch", instruction: text });
      return;
    }
    // Explicit questions go to ask; anything else while focused is a reply
    // to the focused worker.
    const isQuestion = /\?\s*$/.test(text) || QUESTION_START.test(text.trim());
    if (focused && !isQuestion) {
      push({ who: "user", text, task: focused });
      setInput("");
      // The reply may carry directives ("planeja isso"): keep the chip in sync.
      await invoke<Directives>("worker_send", { taskId: focused, text })
        .then((directives) =>
          setLiveWorkers((old) =>
            old[focused] ? { ...old, [focused]: { ...old[focused], directives } } : old,
          ),
        )
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    push({ who: "user", text, image: img ?? undefined });
    setInput("");
    setImage(null);
    runAsk(text, img);
  }

  async function stopWorker(taskId: string) {
    await invoke("worker_stop", { taskId }).catch(() => {});
    setLiveWorkers((old) => {
      const next = { ...old };
      delete next[taskId];
      return next;
    });
    if (focused === taskId) setFocused(null);
    refresh();
  }

  async function onMic() {
    if (recording || busy) return;
    setRecording(true);
    try {
      const text = await invoke<string>("hear_once");
      if (text) {
        setInput(text);
        await submit(text, image);
      }
    } catch (err) {
      push({ who: "sys", text: `mic: ${err}` });
    } finally {
      setRecording(false);
    }
  }

  function onPaste(e: React.ClipboardEvent) {
    const item = Array.from(e.clipboardData.items).find((i) =>
      i.type.startsWith("image/"),
    );
    if (!item) return;
    const file = item.getAsFile();
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => setImage(reader.result as string);
    reader.readAsDataURL(file);
  }

  /** Answer an inline permission card and mark it decided in place. */
  async function answerPermission(requestId: string, allow: boolean) {
    setMessages((old) =>
      old.map((m) =>
        m.who === "permission" && m.requestId === requestId
          ? { ...m, decision: allow ? "allow" : "deny" }
          : m,
      ),
    );
    setLiveWorkers((old) => {
      const entry = Object.entries(old).find(([, w]) => w.status === "awaiting");
      return entry
        ? { ...old, [entry[0]]: { ...entry[1], status: "running" } }
        : old;
    });
    await invoke("approve", { requestId, allow }).catch(() => {});
  }

  /** Newest permission still waiting for a decision. */
  function pendingPermission() {
    return [...messages]
      .reverse()
      .find((m) => m.who === "permission" && !m.decision);
  }

  return (
    <div className="app">
      <div className="topbar">
        <span className="title">VOX</span>
        <nav className="tabs">
          <button
            className={tab === "chat" ? "active" : ""}
            onClick={() => setTab("chat")}
          >
            chat
          </button>
          <button
            className={tab === "board" ? "active" : ""}
            onClick={() => {
              setTab("board");
              refresh();
            }}
          >
            board
          </button>
        </nav>
        <select
          value={overview?.active ?? "all"}
          onChange={(e) => {
            invoke("use_context", { name: e.target.value }).then(refresh);
          }}
        >
          {["all", ...(overview?.contexts ?? [])].map((c) => (
            <option key={c}>{c}</option>
          ))}
        </select>
        <label>
          <input
            type="checkbox"
            checked={speak}
            onChange={(e) => setSpeak(e.target.checked)}
          />{" "}
          voz
        </label>
        <span className={`state ${busy ? "busy" : ""}`}>
          {busy ?? (recording ? "🎤 gravando…" : "pronto")}
        </span>
      </div>

      {tab === "board" && reading && (
        <Reader
          sessionId={reading.sessionId}
          title={reading.title}
          onClose={() => setReading(null)}
          onResume={
            reading.resume &&
            (() => {
              const { title, note } = reading.resume!;
              setReading(null);
              setPending({
                kind: "resume-task",
                title,
                sessionId: reading.sessionId,
                instruction: `Continua a tarefa: ${title}.${note ? ` Contexto: ${note}.` : ""}`,
              });
            })
          }
        />
      )}

      {tab === "board" && !reading && (
        <div className="kanban">
          {(["backlog", "doing", "waiting", "done"] as const).map((status) => {
            const items = (overview?.board ?? []).filter(
              (t) => t.status === status,
            );
            return (
              <div
                key={status}
                className={`column ${status}`}
                onDragOver={(e) => e.preventDefault()}
                onDrop={async (e) => {
                  const title = e.dataTransfer.getData("text/vox-task");
                  if (!title) return;
                  await invoke("board_move", { title, status }).catch(() => {});
                  refresh();
                }}
              >
                <h3>
                  {status} <span className="count">{items.length}</span>
                </h3>
                {items.map((t) => (
                  <div
                    key={t.title}
                    className="card"
                    draggable
                    onDragStart={(e) =>
                      e.dataTransfer.setData("text/vox-task", t.title)
                    }
                  >
                    <div className="card-title">{t.title}</div>
                    {t.note && <div className="card-note">{t.note}</div>}
                    <div className="card-meta">
                      {t.updated_at.slice(0, 16).replace("T", " ")}
                      {t.session_ids.length > 0 &&
                        ` · ${t.session_ids.length} sessão(ões)`}
                    </div>
                    <div className="card-actions">
                      {t.session_ids.length > 0 && (
                        <button
                          title="ler o histórico (não executa nada)"
                          onClick={() =>
                            setReading({
                              sessionId: t.session_ids.at(-1)!,
                              title: t.title,
                              resume: { title: t.title, note: t.note },
                            })
                          }
                        >
                          📖 ler
                        </button>
                      )}
                      <button
                        title="retomar (despacha na sessão vinculada)"
                        onClick={() =>
                          setPending({
                            kind: "resume-task",
                            title: t.title,
                            sessionId: t.session_ids.at(-1),
                            instruction: `Continua a tarefa: ${t.title}.${
                              t.note ? ` Contexto: ${t.note}.` : ""
                            }`,
                          })
                        }
                      >
                        ▶ retomar
                      </button>
                      <button
                        title="arquivar (remove do board)"
                        onClick={async () => {
                          await invoke("board_archive", { title: t.title }).catch(
                            () => {},
                          );
                          refresh();
                        }}
                      >
                        arquivar
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      )}

      <div className="transcript" style={tab === "board" ? { display: "none" } : undefined}>
        {messages.map((m, i) => (
          <div key={i} className={`msg ${m.who}`}>
            {m.who === "sys" ? (
              <span>{m.text}</span>
            ) : m.who === "tool" ? (
              <ToolCall name={m.name} input={m.input} />
            ) : m.who === "output" ? (
              <ToolOutput content={m.content} isError={m.error} />
            ) : m.who === "permission" ? (
              <div className={`permission ${m.decision ?? "waiting"}`}>
                <div className="perm-head">
                  🔐 {m.tool} pede permissão
                  {m.task && <span className="tasktag">{liveWorkers[m.task]?.label ?? m.task.slice(0, 12)}</span>}
                </div>
                <ToolCall name={m.tool} input={m.input} />
                {m.decision ? (
                  <div className={`perm-done ${m.decision}`}>
                    {m.decision === "allow" ? "✓ permitido" : "✗ negado"}
                  </div>
                ) : (
                  <div className="perm-actions">
                    <button
                      className="deny"
                      onClick={() => answerPermission(m.requestId, false)}
                    >
                      negar <kbd>n</kbd>
                    </button>
                    <button
                      className="allow"
                      onClick={() => answerPermission(m.requestId, true)}
                    >
                      permitir <kbd>y</kbd>
                    </button>
                  </div>
                )}
              </div>
            ) : (
              <div className="bubble">
                <span className="tag">
                  {m.who === "user" ? "você" : "vox"}
                  {m.task ? ` → ${m.task.slice(0, 12)}` : ""}
                </span>
                {m.who === "vox" ? (
                  <>
                    <div className="fala">
                      <Markdown>{m.text}</Markdown>
                    </div>
                    {m.detalhes && (
                      <div className="detalhes">
                        <Markdown>{m.detalhes}</Markdown>
                      </div>
                    )}
                    {m.itens && m.itens.length > 0 && (
                      <ul className="itens">
                        {m.itens.map((it, j) => (
                          <li key={j}>{it}</li>
                        ))}
                      </ul>
                    )}
                    {(m.cost != null || m.model) && (
                      <span className="cost">
                        {[
                          shortModel(m.model),
                          ...directiveLabels(
                            m.task ? liveWorkers[m.task]?.directives : undefined,
                          ),
                          `$${(m.cost ?? 0).toFixed(4)}`,
                        ].join(" · ")}
                      </span>
                    )}
                  </>
                ) : (
                  <>
                    <div>{m.text}</div>
                    {m.image && <img className="paste" src={m.image} alt="pasted" />}
                  </>
                )}
              </div>
            )}
          </div>
        ))}
        <div ref={endRef} />
      </div>

      <div className="events" style={tab === "board" ? { display: "none" } : undefined}>
        {overview && overview.board.length > 0 && (
          <div className="board">
            <h3>board</h3>
            {overview.board
              .filter((t) => t.status !== "done")
              .slice(0, 10)
              .map((t) => (
                <div key={t.title} className={`task ${t.status}`}>
                  <span className="status">{t.status}</span> {t.title}
                  {t.note && <div className="note">{t.note}</div>}
                </div>
              ))}
          </div>
        )}
        <h3>eventos</h3>
        {events.slice(-40).map((e, i) => (
          <div key={i} className="event">
            {e.kind === "tool" || e.kind === "worker" ? (
              <>
                <span className="name">
                  {"task_id" in e ? `[${e.task_id}] ` : ""}
                  {e.name}
                </span>
                <pre>{e.input}</pre>
              </>
            ) : e.kind === "tool_result" ? (
              <span>{e.is_error ? "✗" : "✓"} {e.content.slice(0, 120)}</span>
            ) : e.kind === "assistant_text" || e.kind === "worker_turn" ? (
              <span>{e.text.slice(0, 160)}</span>
            ) : e.kind === "worker_exit" ? (
              <span>worker {e.task_id} saiu</span>
            ) : (
              <span>{e.text}</span>
            )}
          </div>
        ))}
        {overview && overview.workers.length > 0 && (
          <div className="workers">
            <h3>workers</h3>
            {overview.workers.map((w) => (
              <div key={w.task_id} className={`worker ${w.status}`}>
                <div className="name">
                  {w.task_id} · {w.status}
                </div>
                <div>{w.summary}</div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="inputbar">
        {Object.entries(liveWorkers).map(([taskId, w]) => (
          <span
            key={taskId}
            className={`worker-chip ${w.status} ${focused === taskId ? "focused" : ""}`}
            onClick={() => setFocused(focused === taskId ? null : taskId)}
            title={focused === taskId ? "focado (clique para soltar)" : "clique para focar"}
          >
            <span className="dot" />
            {w.status === "awaiting" ? "🔐 " : ""}
            {w.label}
            {directiveLabels(w.directives).length > 0 && (
              <span className="chip-mode">{directiveLabels(w.directives).join(" ")}</span>
            )}
            <button
              className="info"
              title="resumo da task"
              onClick={(e) => {
                e.stopPropagation();
                const session = overview?.workers.find(
                  (w) => w.task_id === taskId,
                )?.session_id;
                if (session) {
                  setTab("board");
                  setReading({ sessionId: session, title: w.label });
                } else {
                  setPending({ kind: "task-summary", taskId });
                }
              }}
            >
              ℹ
            </button>
            <button
              className="close"
              title="finalizar worker"
              onClick={(e) => {
                e.stopPropagation();
                stopWorker(taskId);
              }}
            >
              ×
            </button>
          </span>
        ))}
        {image && (
          <span className="thumb">
            <img src={image} alt="attachment" />
            <button onClick={() => setImage(null)}>×</button>
          </span>
        )}
        <input
          type="text"
          placeholder={
            focused
              ? `→ ${liveWorkers[focused]?.label ?? focused} (perguntas e novos comandos ainda funcionam)`
              : 'pergunte ("pendências de hoje?") ou mande ("continua a migração do X")… cole um print com Cmd+V'
          }
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onPaste={onPaste}
          onKeyDown={(e) => {
            // Empty input + pending permission: y/n decide it, like a terminal.
            if (!input) {
              const perm = pendingPermission();
              if (perm?.who === "permission" && (e.key === "y" || e.key === "n")) {
                e.preventDefault();
                answerPermission(perm.requestId, e.key === "y");
                return;
              }
            }
            if (e.key === "Enter") submit(input, image);
          }}
          disabled={!!busy}
        />
        <button
          className={`mic ${recording ? "recording" : ""}`}
          onClick={onMic}
          title="falar"
        >
          🎤
        </button>
        <button onClick={() => submit(input, image)} disabled={!!busy}>
          enviar
        </button>
      </div>

      {pending?.kind === "confirm-dispatch" && (
        <div className="modal-backdrop">
          <div className="modal">
            <h2>Despachar tarefa?</h2>
            <pre>{pending.instruction}</pre>
            <div className="row">
              <button className="plain" onClick={() => setPending(null)}>
                cancelar
              </button>
              <button
                className="allow"
                onClick={() => {
                  const inst = pending.instruction;
                  setPending(null);
                  runDispatch(inst);
                }}
              >
                confirmar
              </button>
            </div>
          </div>
        </div>
      )}

      {pending?.kind === "choice" && (
        <div className="modal-backdrop">
          <div className="modal">
            <h2>Qual sessão?</h2>
            {pending.candidates.map((c) => (
              <button
                key={c.session_id}
                className="choice"
                onClick={() => {
                  const inst = pending.instruction;
                  setPending(null);
                  runDispatch(inst, c.session_id);
                }}
              >
                {c.title || c.session_id} · {c.last_ts}
              </button>
            ))}
            <div className="row">
              <button className="plain" onClick={() => setPending(null)}>
                cancelar
              </button>
            </div>
          </div>
        </div>
      )}

      {pending?.kind === "resume-task" && (
        <div className="modal-backdrop">
          <div className="modal">
            <h2>▶ Retomar: {pending.title}</h2>
            <textarea
              className="resume-input"
              value={pending.instruction}
              onChange={(e) =>
                setPending({ ...pending, instruction: e.target.value })
              }
              rows={4}
            />
            <div className="row">
              <button className="plain" onClick={() => setPending(null)}>
                cancelar
              </button>
              <button
                className="allow"
                onClick={() => {
                  const { instruction, sessionId } = pending;
                  setPending(null);
                  setTab("chat");
                  runDispatch(instruction, sessionId);
                }}
              >
                despachar
              </button>
            </div>
          </div>
        </div>
      )}

      {pending?.kind === "task-summary" && (
        <div className="modal-backdrop" onClick={() => setPending(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>
              ℹ {liveWorkers[pending.taskId]?.label ?? pending.taskId}
              {liveWorkers[pending.taskId] &&
                ` · ${liveWorkers[pending.taskId].status === "running" ? "rodando" : "aguardando você"}`}
            </h2>
            <pre>
              {messages
                .filter((m) => "task" in m && m.task === pending.taskId)
                .slice(-14)
                .map((m) => {
                  switch (m.who) {
                    case "user":
                      return `você: ${m.text}`;
                    case "tool":
                      return `  ⚙ ${m.name}`;
                    case "output":
                      return `  ${m.error ? "✗" : "✓"} ${m.content.split("\n")[0]}`;
                    case "permission":
                      return `  🔐 ${m.tool} ${m.decision ?? "aguardando"}`;
                    default:
                      return `vox: ${"text" in m ? m.text : ""}`;
                  }
                })
                .join("\n") || "(sem eventos ainda)"}
            </pre>
            <div className="row">
              <button
                className="plain"
                onClick={() => {
                  setFocused(pending.taskId);
                  setPending(null);
                }}
              >
                focar nela
              </button>
              <button className="plain" onClick={() => setPending(null)}>
                fechar
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}

const MODE_LABEL: Record<string, string> = {
  manual: "manual",
  acceptEdits: "edições ok",
  plan: "plano",
  auto: "auto",
  bypass: "sem trava",
};

/** Session directives as short footer chips, empty when using defaults. */
function directiveLabels(d?: Directives): string[] {
  if (!d) return [];
  return [
    d.mode ? MODE_LABEL[d.mode] : undefined,
    d.effort ? `esforço ${d.effort}` : undefined,
  ].filter((x): x is string => !!x);
}

/** Format the model id for the footer: "claude-sonnet-5" -> "sonnet-5". */
function shortModel(model?: string): string {
  if (!model) return "?";
  // "claude-sonnet-5" -> "sonnet-5"
  return model.replace(/^claude-/, "");
}

function prettyJson(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}
