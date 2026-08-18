import { useCallback, useEffect, useRef, useState } from "react";
import { ExternalLink, Mic } from "lucide-react";
import Board from "./components/Board";
import CostsPanel from "./components/CostsPanel";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import { useVoxEvents } from "./hooks/useVoxEvents";
import * as ipc from "./lib/ipc";
import type { BoardTask, Msg, Overview, Project, RateLimitState, SessionHit } from "./types";

type MotherTab = "voz" | "board" | "custos";

/**
 * The mother window: the voice of Vox AND the global views. Three tabs:
 * voz (orb + mic), board (every project's demands — demands belong to the
 * user, not to a directory) and custos (the whole ledger). Project windows
 * are filtered views; closing the mother closes everything.
 *
 * Spoken commands resolve locally first (task_command: open project, new
 * chat, board/costs) — zero tokens; only real questions reach the ask
 * pipeline.
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
  const [tab, setTab] = useState<MotherTab>("voz");
  // Sessions matching a recovery request, waiting for the user to pick one.
  const [picks, setPicks] = useState<{ query: string; candidates: SessionHit[] } | null>(null);
  const speakRef = useRef(true);
  // The global hotkey/Esc handlers must see fresh state.
  const micRef = useRef<() => void>(() => {});
  const recordingRef = useRef(false);

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

  // Esc anywhere in this window: recording → cut the capture (transcribe
  // what was said); otherwise → cut the voice.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      if (recordingRef.current) ipc.hearStop().catch(() => {});
      else ipc.speakStop().catch(() => {});
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

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
    announce: true,
    onHotkeyMic: useCallback(() => {
      setTab("voz");
      micRef.current();
    }, []),
    onMainTab: useCallback((t: string) => {
      if (t === "board" || t === "custos") setTab(t);
    }, []),
    speakRef,
    refresh,
  });

  const say = useCallback((text: string) => {
    ipc.speak(text).catch(() => {});
  }, []);

  function openProject(p: Project, task?: { title: string; session?: string }) {
    ipc
      .openProjectWindow(p.name, p.path, task?.title, task?.session)
      .catch((err) => push({ who: "sys", text: `janela: ${err}` }));
  }

  /**
   * Recover an existing session: it becomes a board task named after the
   * SESSION and opens in its project's window, chat loaded. All local —
   * the index already knows every session on this machine.
   */
  async function recoverSession(hit: SessionHit) {
    setPicks(null);
    let task: { title: string; workspace?: string | null };
    try {
      task = await ipc.taskFromSession(hit.session_id);
    } catch (err) {
      push({ who: "sys", text: `não consegui registrar a sessão: ${err}` });
      return;
    }
    const ws = task.workspace;
    const project = ws
      ? (overview?.projects ?? []).find((p) => ws === p.path || ws.startsWith(`${p.path}/`))
      : undefined;
    refresh();
    if (!project) {
      push({
        who: "sys",
        text: `"${task.title}" virou task, mas ${ws ?? "o diretório dela"} não é um projeto registrado — adiciona ele pra abrir o chat`,
      });
      say(`Criei a task ${task.title}, mas o projeto dela não está registrado.`);
      return;
    }
    openProject(project, { title: task.title, session: hit.session_id });
    say(`Retomando ${task.title} em ${project.name}.`);
  }

  /**
   * A card click on the global board: open the window of the project that
   * task belongs to, with its chat already loaded — back to work in one
   * click, no hunting for the session.
   */
  async function openTask(t: BoardTask) {
    const resolve = (dir?: string | null) =>
      dir
        ? (overview?.projects ?? []).find((p) => dir === p.path || dir.startsWith(`${p.path}/`))
        : undefined;
    let project = resolve(t.workspace);
    let session = t.session_ids.at(-1);
    if (!project) {
      // Older tasks carry no workspace: the session's own cwd says where
      // the work lives.
      const hit = await ipc.findSession(t.title).catch(() => null);
      project = resolve(hit?.cwd);
      session = session ?? hit?.session_id;
    }
    if (!project) {
      push({
        who: "sys",
        text: `"${t.title}" não aponta pra nenhum projeto registrado; adiciona o projeto pra abrir o chat`,
      });
      say("Essa task não tem projeto registrado.");
      return;
    }
    openProject(project, { title: t.title, session });
  }

  /** Local commands (zero tokens) the mother can execute herself. */
  async function runCommand(text: string): Promise<boolean> {
    const cmd = await ipc.taskCommand(text).catch(() => null);
    if (!cmd) return false;
    push({ who: "user", text });
    if (cmd.kind === "open_project") {
      openProject({ name: cmd.title, path: cmd.path });
      if (cmd.instruction) {
        try {
          await ipc.chatStart(cmd.path, cmd.instruction);
          say(`Abrindo ${cmd.title} e iniciando o trabalho.`);
        } catch (err) {
          push({ who: "sys", text: `chat: ${err}` });
          say(`Abri ${cmd.title}, mas o chat falhou. Olha a janela.`);
        }
      } else {
        say(`Abrindo o projeto ${cmd.title}.`);
      }
    } else if (cmd.kind === "new_chat") {
      openProject({ name: cmd.title, path: cmd.path });
      if (cmd.instruction) {
        try {
          await ipc.chatStart(cmd.path, cmd.instruction);
          say(`Chat iniciado em ${cmd.title}.`);
        } catch (err) {
          push({ who: "sys", text: `chat: ${err}` });
          say("O chat não subiu, olha a janela.");
        }
      } else {
        say(`Abri ${cmd.title}. Diga a primeira tarefa lá.`);
      }
    } else if (cmd.kind === "open_hq") {
      setTab(cmd.tab);
      say(cmd.tab === "custos" ? "Custos na tela." : "Quadro na tela.");
    } else if (cmd.kind === "project_added") {
      push({ who: "sys", text: `projeto ${cmd.title} adicionado (${cmd.path})` });
      say(`Projeto ${cmd.title} adicionado.`);
      refresh();
    } else if (cmd.kind === "project_error") {
      push({ who: "sys", text: cmd.title });
      say("Não consegui adicionar esse projeto.");
    } else if (cmd.kind === "open_file") {
      // The mother has no editor: send the user to the project window.
      const target = cmd.project
        ? (overview?.projects ?? []).find((p) =>
            p.name.toLowerCase().includes(cmd.project!.toLowerCase()),
          )
        : undefined;
      if (target) {
        openProject(target);
        say(`Abre o arquivo na janela de ${target.name}.`);
      } else {
        say("Arquivos eu abro na janela do projeto. Qual projeto?");
      }
    } else if (cmd.kind === "session_candidates") {
      if (cmd.candidates.length === 0) {
        push({ who: "sys", text: `nenhuma sessão fala sobre "${cmd.query}"` });
        say("Não achei sessão sobre isso.");
      } else if (cmd.candidates.length === 1) {
        await recoverSession(cmd.candidates[0]);
      } else {
        setTab("voz");
        setPicks({ query: cmd.query, candidates: cmd.candidates });
        say(`Achei ${cmd.candidates.length} sessões. Qual delas?`);
      }
    } else if (cmd.kind === "not_found") {
      push({ who: "sys", text: `nada bate com "${cmd.query}"` });
      say("Não achei esse projeto.");
    } else {
      // Board actions (open/switch/rename/pin/archive) show their result
      // on the board tab right here.
      setTab("board");
      refresh();
      say("Feito. Olha o quadro.");
    }
    return true;
  }

  async function submit(text: string) {
    if (!text.trim() || busy) return;
    setInput("");
    if (await runCommand(text)) return;
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
    recordingRef.current = true;
    try {
      const text = await ipc.hearOnce();
      if (text) await submit(text);
    } catch (err) {
      push({ who: "sys", text: `mic: ${err}` });
    } finally {
      setRecording(false);
      recordingRef.current = false;
    }
  }
  micRef.current = onMic;

  const mode: OrbMode = recording ? "listening" : speaking ? "speaking" : busy ? "busy" : "idle";
  const recent = messages.filter((m) => !("task" in m) || !m.task).slice(-4);

  return (
    <div className={tab === "voz" ? "mother" : "mother mother-wide"}>
      <nav className="tabs mother-tabs">
        <button className={tab === "voz" ? "active" : ""} onClick={() => setTab("voz")}>
          voz
        </button>
        <button className={tab === "board" ? "active" : ""} onClick={() => setTab("board")}>
          board
        </button>
        <button className={tab === "custos" ? "active" : ""} onClick={() => setTab("custos")}>
          custos
        </button>
      </nav>

      {tab === "board" ? (
        <Board
          tasks={overview?.board ?? []}
          projects={overview?.projects ?? []}
          onMove={(title, status) => ipc.boardMove(title, status).then(refresh).catch(() => {})}
          onOpen={openTask}
        />
      ) : tab === "custos" ? (
        <div className="costs-page">
          <CostsPanel />
        </div>
      ) : (
        <>
          <div className="mother-orb" onClick={onMic} title="clique ou fale (Esc corta)">
            <VoiceOrb mode={mode} />
          </div>
          <div className="mother-status">
            {busy ?? (recording ? "ouvindo… (Esc corta)" : speaking ? "falando…" : "pronto")}
            {spentToday != null && (
              <span className="mother-spend"> · hoje ${spentToday.toFixed(2)}</span>
            )}
            {rateLimit && rateLimit.status !== "allowed" && (
              <span className="warn">
                {" "}
                · {rateLimit.status === "rejected" ? "limite atingido" : "quase no limite"}
              </span>
            )}
          </div>

          <div className="mother-chat">
            {recent.map((m, i) => (
              <div key={i} className={`mother-msg ${m.who}`}>
                {"text" in m ? m.text : ""}
              </div>
            ))}
          </div>

          {picks && (
            <div className="mother-picks">
              <div className="mother-picks-head">
                sessões sobre “{picks.query}”
                <button onClick={() => setPicks(null)}>fechar</button>
              </div>
              {picks.candidates.map((c) => (
                <button key={c.session_id} onClick={() => recoverSession(c)}>
                  <b>{c.title}</b>
                  <span>
                    {c.last_ts ? c.last_ts.slice(0, 10) : ""}
                    {c.cwd ? ` · ${c.cwd.split("/").filter(Boolean).pop()}` : ""}
                  </span>
                </button>
              ))}
            </div>
          )}

          <div className="mother-input">
            <input
              placeholder='fale ou digite… ("abre o projeto vox", "quanto gastei hoje?")'
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") submit(input);
              }}
              disabled={!!busy}
            />
            <button
              className={`mic ${recording ? "recording" : ""}`}
              onClick={onMic}
              title="falar"
            >
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
        </>
      )}
    </div>
  );
}
