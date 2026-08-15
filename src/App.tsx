import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
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
      setEvents((old) => [...old.slice(-199), e.payload]);
    });
    const un2 = listen<PermissionAsk>("vox-permission", (e) => {
      setPending({ kind: "permission", ask: e.payload });
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
      const out = await invoke<DispatchOutcome>("dispatch_text", {
        instruction,
        sessionId: sessionId ?? null,
      });
      if (out.status === "done") {
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

  async function submit(text: string, img: string | null) {
    if (!text.trim() || busy) return;
    push({ who: "user", text, image: img ?? undefined });
    setInput("");
    setImage(null);
    const route = await invoke<string>("route_text", { text });
    if (route === "dispatch") {
      setPending({ kind: "confirm-dispatch", instruction: text });
    } else {
      runAsk(text, img);
    }
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

  async function answerPermission(allow: boolean) {
    if (pending?.kind !== "permission") return;
    const { request_id } = pending.ask;
    setPending(null);
    await invoke("approve", { requestId: request_id, allow }).catch(() => {});
  }

  return (
    <div className="app">
      <div className="topbar">
        <span className="title">VOX</span>
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

      <div className="transcript">
        {messages.map((m, i) => (
          <div key={i} className={`msg ${m.who}`}>
            {m.who === "sys" ? (
              <span>{m.text}</span>
            ) : (
              <div className="bubble">
                <span className="tag">{m.who === "user" ? "você" : "vox"}</span>
                {m.who === "vox" ? (
                  <>
                    <div className="fala">{m.text}</div>
                    {m.detalhes && <div className="detalhes">{m.detalhes}</div>}
                    {m.itens && m.itens.length > 0 && (
                      <ul className="itens">
                        {m.itens.map((it, j) => (
                          <li key={j}>{it}</li>
                        ))}
                      </ul>
                    )}
                    {(m.cost != null || m.model) && (
                      <span className="cost">
                        {shortModel(m.model)} · ${(m.cost ?? 0).toFixed(4)}
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

      <div className="events">
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
        {image && (
          <span className="thumb">
            <img src={image} alt="attachment" />
            <button onClick={() => setImage(null)}>×</button>
          </span>
        )}
        <input
          type="text"
          placeholder='pergunte ("pendências de hoje?") ou mande ("continua a migração do X")… cole um print com Cmd+V'
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onPaste={onPaste}
          onKeyDown={(e) => e.key === "Enter" && submit(input, image)}
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

      {pending?.kind === "permission" && (
        <div className="modal-backdrop">
          <div className="modal">
            <h2>
              🔐 {pending.ask.tool_name} pede permissão ({pending.ask.task_id})
            </h2>
            <pre>{prettyJson(pending.ask.input)}</pre>
            <div className="row">
              <button className="deny" onClick={() => answerPermission(false)}>
                negar
              </button>
              <button className="allow" onClick={() => answerPermission(true)}>
                permitir
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

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
