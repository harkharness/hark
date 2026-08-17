import { useCallback, useEffect, useRef, useState } from "react";
import {
  Panel,
  PanelGroup,
  PanelResizeHandle,
  type ImperativePanelHandle,
} from "react-resizable-panels";
import { FolderOpen, SquareTerminal, Volume2, VolumeX } from "lucide-react";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import Board from "./components/Board";
import Composer from "./components/Composer";
import FilesEditor from "./components/FilesEditor";
import FilesPanel from "./components/FilesPanel";
import Modals, { type Pending } from "./components/Modals";
import PanelFrame from "./components/PanelFrame";
import QuickOpen from "./components/QuickOpen";
import Reader from "./components/Reader";
import SessionInfo from "./components/SessionInfo";
import Sidebar from "./components/Sidebar";
import TerminalPane from "./components/TerminalPane";
import Transcript from "./components/Transcript";
import WorkerChips from "./components/WorkerChips";
import { useVoxEvents } from "./hooks/useVoxEvents";
import * as ipc from "./lib/ipc";
import type {
  BoardTask,
  Directives,
  LiveWorker,
  Msg,
  OpenFile,
  Overview,
  Project,
} from "./types";

export default function App({ forcedProject }: { forcedProject?: Project }) {
  const [messages, setMessages] = useState<Msg[]>([]);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  // The global Esc handler must see the live value (no stale closure).
  const recordingRef = useRef(false);
  const [pending, setPending] = useState<Pending>(null);
  // Costs are global and live on the mother window; here only code|board.
  const [tab, setTab] = useState<"code" | "board">("code");
  // Read-only thread being viewed (board tab), never executes anything.
  const [reading, setReading] = useState<{
    sessionId: string;
    title: string;
    resume?: { title: string; note?: string };
  } | null>(null);
  // Task selected on the sidebar: messages continue it directly unless
  // the message explicitly names another one or is a question.
  const [focusedTask, setFocusedTask] = useState<{
    title: string;
    sessionId: string;
    note?: string;
  } | null>(null);
  // A "+" click on a project: the NEXT message opens a fresh session there.
  const [draftChat, setDraftChat] = useState<Project | null>(null);
  const [liveWorkers, setLiveWorkers] = useState<Record<string, LiveWorker>>({});
  const [focused, setFocused] = useState<string | null>(null);
  const [speak, setSpeak] = useState(true);
  // "Arquivo" window: up to 5 tabs, LRU-evicted, dirty tabs protected.
  const [openFiles, setOpenFiles] = useState<OpenFile[]>([]);
  const [activeFile, setActiveFile] = useState(0);
  const [dirtyPaths, setDirtyPaths] = useState<Set<string>>(new Set());
  const lastFocus = useRef(new Map<string, number>());
  // "Arquivos" window (project trees + filter).
  const [filesOpen, setFilesOpen] = useState(false);
  const [filesInitialProject, setFilesInitialProject] = useState<string | undefined>();
  /** Typed window taking the whole work area (menu stays). */
  const [expanded, setExpanded] = useState<"arquivo" | "terminal" | "arquivos" | null>(null);
  const [quickOpen, setQuickOpen] = useState(false);
  // Per-thread raw worker feed (the task's "terminal") and window spend.
  const [rawLog, setRawLog] = useState<Record<string, string[]>>({});
  const [costs, setCosts] = useState<Record<string, number>>({});
  const [termOpen, setTermOpen] = useState(false);
  const [scopeInfo, setScopeInfo] = useState(false);
  const [speaking, setSpeaking] = useState(false);
  const [rateLimit, setRateLimit] = useState<import("./types").RateLimitState | null>(null);
  /** Timestamp of the last Esc, for the double-Esc worker abort. */
  const lastEsc = useRef(0);

  const workersRef = useRef(liveWorkers);
  workersRef.current = liveWorkers;
  const focusedTaskRef = useRef(focusedTask);
  focusedTaskRef.current = focusedTask;
  const focusedRef = useRef(focused);
  focusedRef.current = focused;
  // stopWorker is declared below (hoisted); the ref keeps the Esc handler fresh.
  const stopWorkerRef = useRef<(taskId: string) => void>(() => {});
  stopWorkerRef.current = stopWorker;
  const speakRef = useRef(speak);
  speakRef.current = speak;
  const sidebarRef = useRef<ImperativePanelHandle>(null);
  // Tasks whose history was already injected once (avoid re-loading on refocus).
  const loadedTasks = useRef(new Set<string>());

  const push = useCallback((m: Msg) => setMessages((old) => [...old, m]), []);
  /** Thread key for worker events: the task LABEL, stable and readable. */
  const labelFor = useCallback(
    (taskId: string) => workersRef.current[taskId]?.label ?? taskId,
    [],
  );
  const refresh = useCallback(() => {
    ipc
      .overview()
      .then((o) => {
        setOverview(o);
        // Code-surface theme (chat blocks, editor, terminal) via CSS vars.
        document.documentElement.dataset.theme = o.theme;
      })
      .catch(() => {});
  }, []);
  const say = useCallback((text: string) => {
    if (speakRef.current) ipc.speak(text).catch(() => {});
  }, []);
  const pushRaw = useCallback((label: string, line: string) => {
    setRawLog((old) => ({ ...old, [label]: [...(old[label] ?? []), line].slice(-500) }));
  }, []);
  const addCost = useCallback((label: string, usd: number) => {
    setCosts((old) => ({ ...old, [label]: (old[label] ?? 0) + usd }));
  }, []);

  useEffect(refresh, [refresh]);

  useVoxEvents({
    labelFor,
    push,
    setMessages,
    setLiveWorkers,
    pushRaw,
    addCost,
    onSpeaking: setSpeaking,
    onRateLimit: setRateLimit,
    speakRef,
    refresh,
    onWorkerExit: useCallback((taskId: string) => {
      setLiveWorkers((old) => {
        const next = { ...old };
        delete next[taskId];
        return next;
      });
      setFocused((f) => (f === taskId ? null : f));
    }, []),
    onSessionStarted: useCallback((taskId: string, sessionId: string) => {
      // A fresh chat finally has a session: link the focused task to it.
      const label = workersRef.current[taskId]?.label;
      setFocusedTask((old) =>
        old && old.title === label ? { ...old, sessionId } : old,
      );
    }, []),
  });

  // Global shortcuts: Cmd+P quick-open, Cmd+B sidebar collapse,
  // Esc cuts the voice, double-Esc aborts the focused worker.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === "p") {
        e.preventDefault();
        setQuickOpen((q) => !q);
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "b") {
        e.preventDefault();
        const panel = sidebarRef.current;
        if (panel) panel.isCollapsed() ? panel.expand() : panel.collapse();
      }
      if (e.key === "Escape") {
        // Recording? Esc means "parei de falar": cut the capture and let
        // the transcription of what was said proceed. Nothing else.
        if (recordingRef.current) {
          ipc.hearStop().catch(() => {});
          return;
        }
        ipc.speakStop().catch(() => {});
        const now = Date.now();
        if (now - lastEsc.current < 900) {
          lastEsc.current = 0;
          const target = focusedRef.current;
          if (target) {
            push({ who: "sys", text: "⏹ worker abortado (Esc duplo)", task: labelFor(target) });
            stopWorkerRef.current(target);
          }
        } else {
          lastEsc.current = now;
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // A project window sees ONLY its project: sidebar, board, scope.
  const allProjects = overview?.projects ?? [];
  const projects = forcedProject
    ? allProjects.filter((p) => p.path === forcedProject.path).length > 0
      ? allProjects.filter((p) => p.path === forcedProject.path)
      : [forcedProject]
    : allProjects;
  const inForced = (workspace?: string | null) =>
    !!workspace &&
    !!forcedProject &&
    (workspace === forcedProject.path || workspace.startsWith(`${forcedProject.path}/`));
  const board = (overview?.board ?? []).filter((t) => !forcedProject || inForced(t.workspace));

  /** Project a board task lives in (by workspace prefix). */
  const projectOf = useCallback(
    (task?: BoardTask): Project | undefined => {
      const ws = task?.workspace;
      if (!ws) return undefined;
      return projects.find((p) => ws === p.path || ws.startsWith(`${p.path}/`));
    },
    [projects],
  );

  /** Project scoping @mentions / file commands right now. */
  const activeProject =
    forcedProject ??
    draftChat ??
    (focusedTask ? projectOf(board.find((t) => t.title === focusedTask.title)) : undefined);

  const directivesFor = useCallback(
    (taskLabel?: string): Directives | undefined =>
      taskLabel
        ? Object.values(workersRef.current).find((w) => w.label === taskLabel)?.directives
        : undefined,
    [],
  );

  async function runAsk(question: string, img: string | null) {
    setBusy("perguntando…");
    try {
      // The focused project scopes the snapshot: clicking a chat or
      // starting one IS the context selection (no manual picker).
      const reply = await ipc.askText(
        question,
        img ? img.split(",")[1] : null,
        img ? img.slice(5, img.indexOf(";")) : null,
        activeProject,
      );
      push({
        who: "vox",
        text: reply.fala,
        detalhes: reply.detalhes,
        itens: reply.itens,
        cost: reply.cost_usd,
        model: reply.model,
      });
      if (reply.cost_usd) addCost("vox (perguntas)", reply.cost_usd);
      say(reply.fala);
    } catch (err) {
      push({ who: "sys", text: `erro: ${err}` });
    } finally {
      setBusy(null);
      refresh();
    }
  }

  /** Register a started worker in the UI and focus its thread. */
  function adoptWorker(taskId: string, label: string, directives: Directives, sessionId: string) {
    setLiveWorkers((old) => ({ ...old, [taskId]: { label, status: "running", directives } }));
    setFocused(taskId);
    setFocusedTask((old) => (old?.title === label ? old : { title: label, sessionId }));
    loadedTasks.current.add(label);
    push({
      who: "sys",
      text: `worker ativo (${label}); mensagens continuam esta task`,
      task: label,
    });
  }

  async function runDispatch(instruction: string, sessionId?: string) {
    setBusy("despachando…");
    push({ who: "sys", text: `dispatch: ${instruction}` });
    try {
      const out = await ipc.workerStart(instruction, sessionId ?? null);
      if (out.status === "started") {
        const label =
          focusedTask && sessionId === focusedTask.sessionId
            ? focusedTask.title
            : instruction.split(/\s+/).slice(0, 5).join(" ");
        adoptWorker(out.task_id, label, out.directives, sessionId ?? "");
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

  /** First message of a "+" draft: opens a brand-new session in the project. */
  async function runNewChat(project: Project, instruction: string) {
    setBusy("abrindo sessão nova…");
    push({ who: "sys", text: `novo chat em ${project.name}` });
    try {
      const out = await ipc.chatStart(project.path, instruction);
      if (out.status === "started") {
        const label = instruction.split(/\s+/).slice(0, 5).join(" ");
        push({ who: "user", text: instruction, task: label });
        adoptWorker(out.task_id, label, out.directives, "");
      } else {
        push({ who: "sys", text: `não abriu: ${JSON.stringify(out)}` });
      }
    } catch (err) {
      push({ who: "sys", text: `erro: ${err}` });
    } finally {
      setDraftChat(null);
      setBusy(null);
      refresh();
    }
  }

  /**
   * Open a file as an editor tab (max 5). Eviction is LRU by last focus
   * and NEVER touches a tab with unsaved edits.
   */
  function openFile(file: OpenFile) {
    lastFocus.current.set(file.abs, Date.now());
    setOpenFiles((old) => {
      const existing = old.findIndex((f) => f.abs === file.abs);
      if (existing >= 0) {
        setActiveFile(existing);
        return old;
      }
      if (old.length < 5) {
        setActiveFile(old.length);
        return [...old, file];
      }
      const evictable = old
        .map((f, i) => ({ f, i }))
        .filter(({ f }) => !dirtyPaths.has(f.abs))
        .sort(
          (a, b) =>
            (lastFocus.current.get(a.f.abs) ?? 0) - (lastFocus.current.get(b.f.abs) ?? 0),
        );
      const victim = evictable[0];
      if (!victim) {
        push({
          who: "sys",
          text: "5 abas com edição não salva: salva alguma (Cmd+S) antes de abrir outro arquivo",
        });
        return old;
      }
      const next = [...old];
      next[victim.i] = file;
      setActiveFile(victim.i);
      return next;
    });
  }

  function closeFileTab(index: number) {
    setOpenFiles((old) => {
      const closing = old[index];
      if (closing) {
        lastFocus.current.delete(closing.abs);
        setDirtyPaths((d) => {
          const next = new Set(d);
          next.delete(closing.abs);
          return next;
        });
      }
      const next = old.filter((_, i) => i !== index);
      setActiveFile((a) => Math.max(0, a > index ? a - 1 : Math.min(a, next.length - 1)));
      return next;
    });
  }

  const onFileDirty = useCallback((abs: string, isDirty: boolean) => {
    setDirtyPaths((old) => {
      if (old.has(abs) === isDirty) return old;
      const next = new Set(old);
      if (isDirty) next.add(abs);
      else next.delete(abs);
      return next;
    });
  }, []);

  /** Open an ABSOLUTE path (from a tool call) in the local editor. */
  function openAbsolutePath(path: string) {
    const project = projects.find((p) => path === p.path || path.startsWith(`${p.path}/`));
    if (!project) {
      push({ who: "sys", text: `${path} está fora dos projetos registrados` });
      return;
    }
    openFile({ abs: path, rel: path.slice(project.path.length + 1), project });
  }

  /** Resolve a spoken/typed file query to a real file and open the viewer. */
  async function openFileByQuery(query: string, projectName?: string | null) {
    const named = projectName
      ? projects.find((p) => p.name.toLowerCase().includes(projectName.toLowerCase()))
      : undefined;
    const scope = named ? [named] : activeProject ? [activeProject] : projects;
    for (const project of scope) {
      const hits = await ipc.projectFiles(project.path, query, 1).catch(() => []);
      if (hits.length > 0) {
        openFile({ abs: `${project.path}/${hits[0]}`, rel: hits[0], project });
        setTab("code");
        say(`Abrindo ${hits[0].split("/").pop()}.`);
        return;
      }
    }
    push({ who: "sys", text: `nenhum arquivo bate com "${query}"` });
    say("Não achei esse arquivo.");
  }

  const QUESTION_START =
    /^(quais|qual|como|o que|onde|quando|por que|porque|quem|quanto|lista|resumo|status)\b/i;

  async function submit(text: string, img: string | null) {
    if (!text.trim() || busy) return;

    // Local commands first ("abre o arquivo X", "vai pra task Y", "novo
    // chat no projeto Z"): zero tokens, resolved on this machine.
    const cmd = await ipc.taskCommand(text).catch(() => null);
    if (cmd) {
      push({ who: "user", text });
      if (cmd.kind === "open" && cmd.session_id) {
        setTab("board");
        setReading({ sessionId: cmd.session_id, title: cmd.title });
        say(`Abrindo ${cmd.title}.`);
      } else if (cmd.kind === "switch") {
        let session = cmd.session_id;
        if (!session) {
          const hit = await ipc.findSession(cmd.title).catch(() => null);
          session = hit?.session_id;
        }
        if (!session) {
          push({ who: "sys", text: `sem sessão para "${cmd.title}"` });
          say("Não achei a sessão dessa task.");
          return;
        }
        await focusTask(cmd.title, session, cmd.note);
        say(`Na task ${cmd.title}.`);
        if (cmd.instruction) {
          await sendToFocusedTaskWith(cmd.title, session, cmd.instruction, null);
        }
      } else if (cmd.kind === "open_file") {
        await openFileByQuery(cmd.query, cmd.project);
      } else if (cmd.kind === "project_added") {
        push({ who: "sys", text: `projeto ${cmd.title} adicionado (${cmd.path})` });
        say(`Projeto ${cmd.title} adicionado.`);
        refresh();
      } else if (cmd.kind === "project_error") {
        push({ who: "sys", text: cmd.title });
        say("Não consegui adicionar esse projeto.");
      } else if (cmd.kind === "new_chat") {
        if (cmd.instruction) {
          await runNewChat({ name: cmd.title, path: cmd.path }, cmd.instruction);
        } else {
          startDraftChat({ name: cmd.title, path: cmd.path });
          say(`Novo chat em ${cmd.title}. Qual a primeira tarefa?`);
        }
      } else if (cmd.kind === "open_project") {
        await ipc.openProjectWindow(cmd.title, cmd.path).catch((err) =>
          push({ who: "sys", text: `janela: ${err}` }),
        );
        if (cmd.instruction) {
          await ipc.chatStart(cmd.path, cmd.instruction).catch((err) =>
            push({ who: "sys", text: `chat: ${err}` }),
          );
          say(`Abrindo ${cmd.title} e iniciando o trabalho.`);
        } else {
          say(`Abrindo o projeto ${cmd.title}.`);
        }
      } else if (cmd.kind === "open_hq") {
        if (cmd.tab === "board") {
          setTab("board");
          say("Quadro na tela.");
        } else {
          // Costs are global: they live on the mother window.
          await ipc.focusMain("custos").catch(() => {});
          say("Custos na janela mãe.");
        }
      } else if (cmd.kind === "not_found") {
        push({ who: "sys", text: `nada bate com "${cmd.query}"` });
        say("Não achei isso no quadro.");
      } else {
        push({ who: "sys", text: `${cmd.kind}: ${cmd.title}` });
        refresh();
      }
      return;
    }

    // A pending "+" draft: this message opens the new session.
    const isQuestion = /\?\s*$/.test(text) || QUESTION_START.test(text.trim());
    if (draftChat && !isQuestion) {
      await runNewChat(draftChat, text);
      return;
    }

    // Explicit questions always go to ask, focused or not.
    if (!isQuestion && focused) {
      push({ who: "user", text, image: img ?? undefined, task: focused && labelFor(focused) });
      await ipc
        .workerSend(
          focused,
          text,
          img ? img.split(",")[1] : null,
          img ? img.slice(5, img.indexOf(";")) : null,
        )
        .then((directives) =>
          setLiveWorkers((old) =>
            old[focused] ? { ...old, [focused]: { ...old[focused], directives } } : old,
          ),
        )
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    if (!isQuestion && focusedTask) {
      // THE GATE: before anything expensive runs on the focused session, a
      // cheap evaluator decides whether this message really belongs there.
      setBusy("avaliando…");
      const gate = await ipc
        .evaluate(text, focusedTask.title, focusedTask.sessionId)
        .catch(() => null);
      setBusy(null);
      if (gate) {
        if (gate.cost_usd) addCost("avaliador", gate.cost_usd);
        push({
          who: "sys",
          text: `avaliador: ${gate.acao} (${Math.round(gate.confianca * 100)}%) · ${gate.motivo}${gate.aviso ? ` · ⚠ ${gate.aviso}` : ""} · $${(gate.cost_usd ?? 0).toFixed(4)}`,
          task: focusedTask.title,
        });
        if (gate.acao === "meta_vox" || gate.acao === "pergunta") {
          push({ who: "user", text });
          runAsk(text, img);
          return;
        }
        if (gate.acao === "trocar_task" && gate.task_alvo) {
          push({
            who: "sys",
            text: `o avaliador sugere a task "${gate.task_alvo}"; use a sidebar ou "vai pra task ${gate.task_alvo}"`,
          });
          say(`Isso parece ser da task ${gate.task_alvo}.`);
          return;
        }
        if (gate.needs_confirmation) {
          setPending({
            kind: "confirm-dispatch",
            instruction: text,
            sessionId: focusedTask.sessionId,
            warning: gate.aviso ?? gate.motivo,
          });
          say("Preciso de confirmação antes de executar.");
          return;
        }
      }
      await sendToFocusedTaskWith(focusedTask.title, focusedTask.sessionId, text, img);
      return;
    }
    const route = await ipc.routeText(text);
    if (route === "dispatch") {
      push({ who: "user", text });
      setPending({ kind: "confirm-dispatch", instruction: text });
      return;
    }
    push({ who: "user", text, image: img ?? undefined });
    runAsk(text, img);
  }

  async function stopWorker(taskId: string) {
    await ipc.workerStop(taskId).catch(() => {});
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
    recordingRef.current = true;
    try {
      const text = await ipc.hearOnce();
      if (text) await submit(text, null);
    } catch (err) {
      push({ who: "sys", text: `mic: ${err}` });
    } finally {
      setRecording(false);
      recordingRef.current = false;
    }
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
      return entry ? { ...old, [entry[0]]: { ...entry[1], status: "running" } } : old;
    });
    await ipc.approve(requestId, allow).catch(() => {});
  }

  /** Newest permission still waiting for a decision. */
  const pendingPermission = [...messages]
    .reverse()
    .find((m) => m.who === "permission" && !m.decision);

  /** A finished task that receives work again comes back to "doing". */
  async function reactivateIfDone(title: string) {
    const task = (overview?.board ?? []).find((t) => t.title === title);
    if (task?.status === "done") {
      await ipc.boardMove(title, "doing").catch(() => {});
      push({ who: "sys", text: `task "${title}" reativada (done → doing)`, task: title });
      refresh();
    }
  }

  /**
   * Focus a task INSIDE the chat: pull the tail of its real history into
   * the transcript (free, straight from the log file).
   */
  async function focusTask(title: string, sessionId: string, note?: string) {
    setDraftChat(null);
    setFocusedTask({ title, sessionId, note });
    if (loadedTasks.current.has(title)) return; // thread already built once
    loadedTasks.current.add(title);
    try {
      const out = await ipc.readTranscript(sessionId, 12).catch(async (err) => {
        // Linked session vanished from disk (os error 2): fall back to the
        // best topic match instead of a dead end.
        const hit = await ipc.findSession(title).catch(() => null);
        if (!hit || hit.session_id === sessionId) throw err;
        setFocusedTask({ title, sessionId: hit.session_id, note });
        push({
          who: "sys",
          text: `sessão vinculada sumiu; usando "${hit.title ?? hit.session_id.slice(0, 8)}"`,
          task: title,
        });
        return ipc.readTranscript(hit.session_id, 12);
      });
      for (const e of out.entries) {
        if (e.role === "user") push({ who: "user", text: e.text, task: title });
        else if (e.role === "assistant") push({ who: "vox", text: e.text, task: title });
        else if (e.role === "tool_use")
          push({ who: "tool", name: e.tool ?? "tool", input: e.text, task: title });
      }
      push({
        who: "sys",
        text: `contexto de "${out.session_title ?? title}" carregado; mensagens continuam esta task`,
        task: title,
      });
    } catch (err) {
      push({ who: "sys", text: `não consegui ler o histórico: ${err}`, task: title });
    }
  }

  /** Send a message into a task: live worker if any, else a fresh resume. */
  async function sendToFocusedTaskWith(
    title: string,
    sessionId: string,
    text: string,
    img: string | null,
  ) {
    await reactivateIfDone(title);
    const liveEntry = Object.entries(liveWorkers).find(([, w]) => w.label === title);
    push({ who: "user", text, image: img ?? undefined, task: title });
    if (liveEntry) {
      await ipc
        .workerSend(
          liveEntry[0],
          text,
          img ? img.split(",")[1] : null,
          img ? img.slice(5, img.indexOf(";")) : null,
        )
        .then((directives) =>
          setLiveWorkers((old) => ({
            ...old,
            [liveEntry[0]]: { ...old[liveEntry[0]], directives },
          })),
        )
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    await runDispatch(text, sessionId);
  }

  function startDraftChat(project: Project) {
    setDraftChat(project);
    setFocusedTask(null);
    setFocused(null);
    push({
      who: "sys",
      text: `novo chat em ${project.name} (${project.path}): a próxima mensagem abre a sessão`,
    });
  }

  async function openTaskFromSidebar(t: BoardTask) {
    let session = t.session_ids.at(-1);
    if (!session) {
      const hit = await ipc.findSession(t.title).catch(() => null);
      session = hit?.session_id ?? undefined;
    }
    if (!session) {
      push({ who: "sys", text: `nenhuma sessão encontrada para "${t.title}"` });
      return;
    }
    await focusTask(t.title, session, t.note);
  }

  const visibleMessages = messages.filter((m) => {
    const thread = "task" in m ? (m.task ?? null) : null;
    return thread === (focusedTask?.title ?? null);
  });

  // Typed windows, shared by the tiled layout and the expanded mode.
  const frameArquivo = () => (
    <PanelFrame
      title="Arquivo"
      expanded={expanded === "arquivo"}
      onToggleExpand={() => setExpanded((e) => (e === "arquivo" ? null : "arquivo"))}
      onClose={() => {
        setOpenFiles([]);
        setDirtyPaths(new Set());
        setExpanded((e) => (e === "arquivo" ? null : e));
      }}
    >
      <FilesEditor
        files={openFiles}
        active={activeFile}
        dirty={dirtyPaths}
        onActivate={(i) => {
          setActiveFile(i);
          const f = openFiles[i];
          if (f) lastFocus.current.set(f.abs, Date.now());
        }}
        onCloseTab={closeFileTab}
        onDirty={onFileDirty}
      />
    </PanelFrame>
  );
  const frameTerminal = () => (
    <PanelFrame
      title="Terminal"
      expanded={expanded === "terminal"}
      onToggleExpand={() => setExpanded((e) => (e === "terminal" ? null : "terminal"))}
      onClose={() => {
        setTermOpen(false);
        setExpanded((e) => (e === "terminal" ? null : e));
      }}
    >
      <TerminalPane rawLog={rawLog} focusedLabel={focusedTask?.title} />
    </PanelFrame>
  );
  const frameArquivos = () => (
    <PanelFrame
      title="Arquivos"
      expanded={expanded === "arquivos"}
      onToggleExpand={() => setExpanded((e) => (e === "arquivos" ? null : "arquivos"))}
      onClose={() => {
        setFilesOpen(false);
        setExpanded((e) => (e === "arquivos" ? null : e));
      }}
    >
      <FilesPanel
        projects={projects}
        initialProject={filesInitialProject}
        onOpen={openFile}
      />
    </PanelFrame>
  );

  const placeholder = draftChat
    ? `primeira mensagem do novo chat em ${draftChat.name}…`
    : focused
      ? `→ ${labelFor(focused)} (perguntas ainda vão pro vox)`
      : focusedTask
        ? `→ ${focusedTask.title} (mensagem retoma a task; perguntas vão pro vox)`
        : 'pergunte ("pendências de hoje?"), mande trabalho, @arquivo, Cmd+P abre arquivos';

  return (
    <div className="app">
      <div className="topbar">
        <span className="title">VOX</span>
        <nav className="tabs">
          <button className={tab === "code" ? "active" : ""} onClick={() => setTab("code")}>
            code
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
          <button
            className="dim"
            title="custos globais ficam na janela mãe"
            onClick={() => ipc.focusMain("custos")}
          >
            custos ↗
          </button>
        </nav>
        <button
          className="scope"
          title="escopo atual (clique: stats da sessão e gastos)"
          onClick={() => setScopeInfo((s) => !s)}
        >
          <FolderOpen size={12} /> {activeProject?.name ?? "todos"}
        </button>
        <button
          className={`scope ${termOpen ? "on" : ""}`}
          title="janela Terminal (feeds brutos dos workers)"
          onClick={() => setTermOpen((t) => !t)}
        >
          <SquareTerminal size={12} />
        </button>
        <button
          className={`scope ${speak ? "on" : ""}`}
          title={speak ? "voz ligada (Esc corta a fala)" : "voz desligada"}
          onClick={() => setSpeak((s) => !s)}
        >
          {speak ? <Volume2 size={12} /> : <VolumeX size={12} />}
        </button>
        {rateLimit && (
          <span
            className={`scope ratelimit ${rateLimit.status}`}
            title={`janela ${rateLimit.limit_kind ?? "?"} · status ${rateLimit.status}`}
          >
            ⏳ {rateLimit.status === "allowed" ? "ok" : rateLimit.status === "allowed_warning" ? "quase no limite" : "limite atingido"}
            {rateLimit.resets_at
              ? ` · reseta ${new Date(rateLimit.resets_at * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}`
              : ""}
          </span>
        )}
        <VoiceOrb
          mode={
            (recording
              ? "listening"
              : speaking
                ? "speaking"
                : busy
                  ? "busy"
                  : "idle") as OrbMode
          }
        />
        <span className={`state ${busy ? "busy" : ""}`}>
          {busy ?? (recording ? "ouvindo…" : "pronto")}
        </span>
      </div>

      {tab === "code" && expanded ? (
        <div className="workarea expanded-area">
          {expanded === "arquivo"
            ? frameArquivo()
            : expanded === "terminal"
              ? frameTerminal()
              : frameArquivos()}
        </div>
      ) : tab === "code" ? (
        <PanelGroup direction="horizontal" autoSaveId="vox-code" className="workarea">
          <Panel
            ref={sidebarRef}
            collapsible
            defaultSize={18}
            minSize={10}
            maxSize={34}
            className="pane"
          >
            <Sidebar
              projects={projects}
              tasks={board}
              activeTitle={focusedTask?.title}
              liveTitles={Object.values(liveWorkers).map((w) => w.label)}
              onOpen={openTaskFromSidebar}
              onRename={(t, newTitle) =>
                ipc.boardRename(t.title, newTitle).then(refresh)
              }
              onPin={(t) => ipc.boardPin(t.title).then(refresh)}
              onArchive={(t) => ipc.boardArchive(t.title).then(refresh)}
              onResume={(t) =>
                setPending({
                  kind: "resume-task",
                  title: t.title,
                  sessionId: t.session_ids.at(-1),
                  instruction: `Continua a tarefa: ${t.title}.${t.note ? ` Contexto: ${t.note}.` : ""}`,
                })
              }
              onNewChat={startDraftChat}
              onAddProject={(path) =>
                ipc
                  .projectAdd(path)
                  .then((p) => {
                    push({ who: "sys", text: `projeto ${p.name} adicionado (${p.path})` });
                    refresh();
                  })
                  .catch((err) => push({ who: "sys", text: `projeto: ${err}` }))
              }
              onRemoveProject={(p) =>
                ipc.projectRemove(p.path).then(() => {
                  push({ who: "sys", text: `projeto ${p.name} removido da lista` });
                  refresh();
                })
              }
              onOpenFiles={(p) => {
                setFilesInitialProject(p.path);
                setFilesOpen(true);
              }}
            />
          </Panel>
          <PanelResizeHandle className="rhandle" />
          <Panel minSize={30} className="pane">
            <div className="maincol">
              <Transcript
                messages={visibleMessages}
                directivesFor={directivesFor}
                onAnswerPermission={answerPermission}
                onOpenPath={openAbsolutePath}
              />
              <Composer
                disabled={!!busy}
                recording={recording}
                placeholder={placeholder}
                projects={projects}
                activeProject={activeProject}
                pendingPermissionId={
                  pendingPermission?.who === "permission" ? pendingPermission.requestId : undefined
                }
                onSubmit={submit}
                onMic={onMic}
                onAnswerPermission={answerPermission}
              >
                <WorkerChips
                  liveWorkers={liveWorkers}
                  focused={focused}
                  focusedTaskTitle={focusedTask?.title}
                  onToggleFocus={(taskId) => {
                    if (focused === taskId) {
                      setFocused(null);
                      setFocusedTask(null);
                    } else {
                      setFocused(taskId);
                      const session = overview?.workers.find(
                        (x) => x.task_id === taskId,
                      )?.session_id;
                      const label = labelFor(taskId);
                      setFocusedTask({ title: label, sessionId: session ?? "" });
                      loadedTasks.current.add(label);
                    }
                  }}
                  onReleaseFocusedTask={() => setFocusedTask(null)}
                  onInfo={(taskId) => {
                    const session = overview?.workers.find(
                      (w) => w.task_id === taskId,
                    )?.session_id;
                    if (session) {
                      setTab("board");
                      setReading({ sessionId: session, title: labelFor(taskId) });
                    } else {
                      setPending({ kind: "task-summary", taskId });
                    }
                  }}
                  onStop={stopWorker}
                />
              </Composer>
            </div>
          </Panel>
          {openFiles.length > 0 && (
            <>
              <PanelResizeHandle className="rhandle" />
              <Panel defaultSize={42} minSize={20} className="pane">
                {frameArquivo()}
              </Panel>
            </>
          )}
          {termOpen && (
            <>
              <PanelResizeHandle className="rhandle" />
              <Panel defaultSize={34} minSize={18} className="pane">
                {frameTerminal()}
              </Panel>
            </>
          )}
          {filesOpen && (
            <>
              <PanelResizeHandle className="rhandle" />
              <Panel defaultSize={26} minSize={16} className="pane">
                {frameArquivos()}
              </Panel>
            </>
          )}
        </PanelGroup>
      ) : reading ? (
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
      ) : (
        <Board
          tasks={board}
          onMove={(title, status) => ipc.boardMove(title, status).then(refresh).catch(() => {})}
        />
      )}

      {scopeInfo && (
        <SessionInfo
          taskTitle={focusedTask?.title}
          sessionId={focusedTask?.sessionId || undefined}
          projectName={activeProject?.name}
          workspace={forcedProject?.path}
          costs={costs}
          onClose={() => setScopeInfo(false)}
        />
      )}

      {quickOpen && (
        <QuickOpen
          projects={projects}
          onPick={(file) => {
            openFile(file);
            setQuickOpen(false);
            setTab("code");
          }}
          onClose={() => setQuickOpen(false)}
        />
      )}

      <Modals
        pending={pending}
        setPending={setPending}
        focusedTaskTitle={focusedTask?.title}
        liveWorkers={liveWorkers}
        messages={messages}
        onDispatch={(instruction, sessionId, taskTitle) => {
          setTab("code");
          const title = taskTitle ?? focusedTask?.title;
          if (title) reactivateIfDone(title);
          runDispatch(instruction, sessionId);
        }}
        onFocusWorker={(taskId) => setFocused(taskId)}
      />
    </div>
  );
}
