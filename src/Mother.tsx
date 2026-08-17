import { useCallback, useEffect, useRef, useState } from "react";
import { ExternalLink, Mic } from "lucide-react";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import { useVoxEvents } from "./hooks/useVoxEvents";
import * as ipc from "./lib/ipc";
import type { Msg, Overview, Project, RateLimitState } from "./types";

/**
 * The mother window: the voice of Vox. A big orb, the mic, the measured
 * spend, and one button per project — each opens its own window (VSCode
 * model). Closing this window closes everything; project windows are
 * expendable, the workers underneath never die with a window.
 */
export default function Mother() {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [messages, setMessages] = useState<Msg[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [speaking, setSpeaking] = useState(false);
  const [rateLimit, setRateLimit] = useState<RateLimitState | null>(null);
  const [spentToday, setSpentToday] = useState<number | null>(null);
  const [input, setInput] = useState("");
  const speakRef = useRef(true);
  // The global hotkey handler must see fresh state.
  const micRef = useRef<() => void>(() => {});

  const push = useCallback((m: Msg) => setMessages((old) => [...old, m].slice(-30)), []);
  const refresh = useCallback(() => {
    ipc
      .overview()
      .then((o) => {
        setOverview(o);
        document.documentElement.dataset.theme = o.theme;
      })
      .catch(() => {});
    const day = new Date(Date.now() - 24 * 3600e3).toISOString();
    ipc
      .spendSummary(day, "kind", "live")
      .then((aggs) => setSpentToday(aggs.reduce((a, b) => a + b.cost_usd, 0)))
      .catch(() => {});
  }, []);
  useEffect(refresh, [refresh]);

  useVoxEvents({
    labelFor: (id) => id,
    push,
    setMessages,
    setLiveWorkers: () => {},
    pushRaw: () => {},
    addCost: () => {},
    onWorkerExit: () => {},
    onSessionStarted: () => {},
    onSpeaking: setSpeaking,
    onRateLimit: setRateLimit,
    onHotkeyMic: useCallback(() => micRef.current(), []),
    speakRef,
    refresh,
  });

  const say = useCallback((text: string) => {
    ipc.speak(text).catch(() => {});
  }, []);

  function openProject(p: Project) {
    ipc.openProjectWindow(p.name, p.path).catch((err) =>
      push({ who: "sys", text: `janela: ${err}` }),
    );
  }

  /** "abre o projeto vox" — the mother's own verb. */
  function matchOpenProject(text: string): Project | undefined {
    const m = text.toLowerCase().match(/^(?:abre|abrir|abra)\s+(?:a\s+janela\s+d[oe]\s+)?(?:o\s+)?projeto\s+(.+)$/);
    if (!m) return undefined;
    const query = m[1].trim();
    return (overview?.projects ?? []).find((p) => p.name.toLowerCase().includes(query));
  }

  async function submit(text: string) {
    if (!text.trim() || busy) return;
    setInput("");
    const target = matchOpenProject(text);
    if (target) {
      push({ who: "user", text });
      openProject(target);
      say(`Abrindo o projeto ${target.name}.`);
      return;
    }
    push({ who: "user", text });
    setBusy("perguntando…");
    try {
      const reply = await ipc.askText(text, null, null);
      push({ who: "vox", text: reply.fala, cost: reply.cost_usd, model: reply.model });
      say(reply.fala);
    } catch (err) {
      push({ who: "sys", text: `erro: ${err}` });
    } finally {
      setBusy(null);
      refresh();
    }
  }

  async function onMic() {
    if (recording || busy) return;
    setRecording(true);
    try {
      const text = await ipc.hearOnce();
      if (text) await submit(text);
    } catch (err) {
      push({ who: "sys", text: `mic: ${err}` });
    } finally {
      setRecording(false);
    }
  }
  micRef.current = onMic;

  const mode: OrbMode = recording ? "listening" : speaking ? "speaking" : busy ? "busy" : "idle";
  const recent = messages.filter((m) => !("task" in m) || !m.task).slice(-4);

  return (
    <div className="mother">
      <div className="mother-orb" onClick={onMic} title="clique ou fale (Esc corta)">
        <VoiceOrb mode={mode} />
      </div>
      <div className="mother-status">
        {busy ?? (recording ? "ouvindo…" : speaking ? "falando…" : "pronto")}
        {spentToday != null && <span className="mother-spend"> · hoje ${spentToday.toFixed(2)}</span>}
        {rateLimit && rateLimit.status !== "allowed" && (
          <span className="warn"> · {rateLimit.status === "rejected" ? "limite atingido" : "quase no limite"}</span>
        )}
      </div>

      <div className="mother-chat">
        {recent.map((m, i) => (
          <div key={i} className={`mother-msg ${m.who}`}>
            {"text" in m ? m.text : ""}
          </div>
        ))}
      </div>

      <div className="mother-input">
        <input
          placeholder='fale ou digite… ("abre o projeto vox", "quanto gastei hoje?")'
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit(input);
            if (e.key === "Escape") ipc.speakStop().catch(() => {});
          }}
          disabled={!!busy}
        />
        <button className={`mic ${recording ? "recording" : ""}`} onClick={onMic} title="falar">
          <Mic size={16} />
        </button>
      </div>

      <div className="mother-projects">
        {(overview?.projects ?? []).map((p) => (
          <button key={p.path} className="mother-project" onClick={() => openProject(p)}>
            <ExternalLink size={12} /> {p.name}
          </button>
        ))}
      </div>
    </div>
  );
}
