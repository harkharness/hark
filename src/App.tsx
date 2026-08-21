import { useCallback, useEffect, useRef, useState } from "react";
import {
  Panel,
  PanelGroup,
  PanelResizeHandle,
  type ImperativePanelHandle,
} from "react-resizable-panels";
import { PanelLeft, SquareTerminal, Volume2, VolumeX, Wallet } from "lucide-react";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import { setLang, t } from "./lib/i18n";
import Board from "./components/Board";
import Composer, { toImagePair, type Attachment } from "./components/Composer";
import FilesEditor, { FileTabs } from "./components/FilesEditor";
import FilesPanel from "./components/FilesPanel";
import Modals, { type Pending } from "./components/Modals";
import ModeSelect from "./components/ModeSelect";
import PanelFrame from "./components/PanelFrame";
import QuickOpen from "./components/QuickOpen";
import Reader from "./components/Reader";
import SessionInfo from "./components/SessionInfo";
import Sidebar from "./components/Sidebar";
import TerminalPane, { TerminalTabs } from "./components/TerminalPane";
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
  SessionHit,
} from "./types";

type RailItem = "arquivo" | "terminal" | "arquivos" | "board";

export default function App({
  forcedProject,
  initialTask,
}: {
  forcedProject?: Project;
  /** Task to open on mount (a card click on the mother's global board). */
  initialTask?: { title: string; sessionId?: string };
}) {
  const [messages, setMessages] = useState<Msg[]>([]);
  const [overview, setOverview] = useState<Overview | null>(null);
  // Every Claude Code session of this project (index), newest first.
  const [chats, setChats] = useState<SessionHit[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  // The global Esc handler must see the live value (no stale closure).
  const recordingRef = useRef(false);
  const [pending, setPending] = useState<Pending>(null);
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
  const [filesInitialProject, setFilesInitialProject] = useState<string | undefined>();
  /** Typed window taking the whole work area (menu stays). */
  const [expanded, setExpanded] = useState<RailItem | null>(null);
  const [quickOpen, setQuickOpen] = useState(false);
  // Standing "sempre permitir" rules: task label → tools auto-approved.
  // Window-scoped by design: closing the window forgets every rule.
  const allowAlways = useRef<Map<string, Set<string>>>(new Map());
  // Mode chosen on the selector for NEW tasks (null = config default).
  const [modeDefault, setModeDefault] = useState<string | null>(null);
  // Per-thread raw worker feed (the task's "terminal") and window spend.
  const [rawLog, setRawLog] = useState<Record<string, string[]>>({});
  const [costs, setCosts] = useState<Record<string, number>>({});
  /**
   * THE RAIL: chat is the anchor; every typed window lives stacked on the
   * right, collapsible to its title bar, draggable to reorder. Opening a
   * third expanded panel collapses the oldest — nothing ever dies, it
   * shrinks. Order in the array = order on screen.
   */
  const [rail, setRail] = useState<{ id: RailItem; collapsed: boolean }[]>([]);
  const dragRail = useRef<RailItem | null>(null);
  // Real shell tabs (PTY ids). Owned here so hiding the pane keeps them.
  const [shells, setShells] = useState<string[]>([]);
  const [termTab, setTermTab] = useState<string>("feed");
  const shellSeq = useRef(1);
  const [scopeInfo, setScopeInfo] = useState(false);
  // Mirrors the sidebar panel's collapsed state (topbar toggle icon).
  const [sidebarOpen, setSidebarOpen] = useState(true);
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
  // Same trick for the board→chat jump (mount effect and event listener).
  const openTaskRef = useRef<(title: string, sessionId?: string, note?: string) => void>(
    () => {},
  );
  openTaskRef.current = openTaskByTitle;
  const speakRef = useRef(speak);
  speakRef.current = speak;
  const sidebarRef = useRef<ImperativePanelHandle>(null);
  // Tasks whose history was already injected once (avoid re-loading on refocus).
  const loadedTasks = useRef(new Set<string>());

  const push = useCallback(
    (m: Msg) =>
      setMessages((old) => [
        ...old,
        (m.who === "user" || m.who === "vox") && m.ts == null
          ? { ...m, ts: Date.now() }
          : m,
      ]),
    [],
  );
  /** Thread key for worker events: the task LABEL, stable and readable. */
  const labelFor = useCallback(
    (taskId: string) => workersRef.current[taskId]?.label ?? taskId,
    [],
  );
  const refresh = useCallback(() => {
    ipc
      .overview()
      .then((o) => {
        // Language first: setOverview re-renders with t() already right.
        setLang(o.ui_language);
        setOverview(o);
        // Code-surface theme (chat blocks, editor, terminal) via CSS vars.
        document.documentElement.dataset.theme = o.theme;
      })
      .catch(() => {});
    // Full Claude Code history of this project (local index): the sidebar
    // shows every chat, not only the ones the board already knows.
    if (forcedProject) {
      ipc.projectSessions(forcedProject.path).then(setChats).catch(() => {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  // This window was opened by a card click: the chat of that task is the
  // landing screen, history already loaded, ready to keep working.
  useEffect(() => {
    if (initialTask) openTaskRef.current(initialTask.title, initialTask.sessionId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Tell the voice layer what this window is looking at: an unaddressed
  // spoken sentence lands HERE while this window is the focused one.
  useEffect(() => {
    const report = () =>
      ipc
        .setActiveContext({
          project_path: forcedProject?.path ?? null,
          project_name: forcedProject?.name ?? null,
          task_title: focusedTask?.title ?? null,
          session_id: focusedTask?.sessionId || null,
        })
        .catch(() => {});
    report();
    window.addEventListener("focus", report);
    return () => window.removeEventListener("focus", report);
  }, [focusedTask, forcedProject]);

  useVoxEvents({
    labelFor,
    push,
    setMessages,
    setLiveWorkers,
    pushRaw,
    addCost,
    onSpeaking: setSpeaking,
    onRateLimit: setRateLimit,
    autoAllow: useCallback(
      (ask: import("./types").PermissionAsk) => {
        const label = workersRef.current[ask.task_id]?.label ?? ask.label ?? ask.task_id;
        return allowAlways.current.get(label)?.has(ask.tool_name) ?? false;
      },
      [],
    ),
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
    onFocusTask: useCallback((title: string, sessionId?: string | null) => {
      openTaskRef.current(title, sessionId ?? undefined);
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
      // App settings live on the mother: Cmd+, from here redirects.
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        ipc.focusMain("settings").catch(() => {});
      }
      // Permission pending and not typing anywhere: y / n / a decide it
      // from wherever the focus is — no mouse trip required.
      const target = e.target as HTMLElement | null;
      const typing =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        !!target?.isContentEditable;
      if (!typing && !e.metaKey && !e.ctrlKey && pendingPermRef.current?.who === "permission") {
        if (e.key === "y" || e.key === "n" || e.key === "a") {
          e.preventDefault();
          answerPermissionRef.current(
            pendingPermRef.current.requestId,
            e.key !== "n",
            e.key === "a",
          );
          return;
        }
      }
      // Dispatch confirm open: Enter fires it, Esc cancels — from
      // anywhere (the textarea handles its own keys).
      if (!typing && pendingRef.current?.kind === "confirm-dispatch") {
        if (e.key === "Enter") {
          e.preventDefault();
          confirmDispatchRef.current();
          return;
        }
        if (e.key === "Escape") {
          setPending(null);
          return;
        }
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

  async function runAsk(question: string, images: Attachment[]) {
    setBusy("perguntando…");
    try {
      // The focused project scopes the snapshot: clicking a chat or
      // starting one IS the context selection (no manual picker).
      const reply = await ipc.askText(question, images.map(toImagePair), activeProject);
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
      const out = await ipc.workerStart(instruction, sessionId ?? null, modeDefault ?? undefined);
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
      const out = await ipc.chatStart(project.path, instruction, modeDefault ?? undefined);
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
    ensureRail("arquivo");
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
      if (next.length === 0) removeRail("arquivo");
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

  /** Formats the editor can't render — the OS opens these. */
  const OS_ONLY = /\.(html?|pdf|png|jpe?g|gif|svg|webp)$/i;

  /**
   * The single router for every clicked path (markdown links, tool call
   * chips): resolves relative paths against the project, sends browser
   * formats to the OS, everything else to the local editor — including
   * files outside registered projects (e.g. ~/.claude/plans) via a
   * synthetic root, so a proposed plan opens as rendered markdown.
   */
  function openAbsolutePath(raw: string) {
    let path = raw.trim();
    if (/^https?:/i.test(path)) {
      ipc.openExternal(path).catch(() => {});
      return;
    }
    if (!path.startsWith("/") && !path.startsWith("~")) {
      const base = activeProject?.path;
      if (!base) {
        push({ who: "sys", text: `${path}: caminho relativo sem projeto ativo` });
        return;
      }
      path = `${base}/${path.replace(/^\.\//, "")}`;
    }
    if (OS_ONLY.test(path)) {
      ipc.openExternal(path).catch(() => {});
      return;
    }
    // Compare in ~-form so "~/Projects/x" and "/Users/me/Projects/x" meet.
    const norm = (p: string) => p.replace(/^\/Users\/[^/]+\//, "~/");
    const np = norm(path);
    const project = projects.find(
      (p) => np === norm(p.path) || np.startsWith(`${norm(p.path)}/`),
    );
    if (project) {
      openFile({ abs: path, rel: np.slice(norm(project.path).length + 1), project });
      return;
    }
    const parts = path.split("/");
    openFile({
      abs: path,
      rel: parts[parts.length - 1] ?? path,
      project: { name: parts[parts.length - 2] ?? "", path: parts.slice(0, -1).join("/") },
    });
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
        say(`Abrindo ${hits[0].split("/").pop()}.`);
        return;
      }
    }
    push({ who: "sys", text: `nenhum arquivo bate com "${query}"` });
    say("Não achei esse arquivo.");
  }

  const QUESTION_START =
    /^(quais|qual|como|o que|onde|quando|por que|porque|quem|quanto|lista|resumo|status)\b/i;

  /** "/modo <palavra>" → CLI --permission-mode flag. */
  const MODE_WORDS: Record<string, string> = {
    manual: "manual",
    "edições": "acceptEdits",
    edicoes: "acceptEdits",
    plano: "plan",
    plan: "plan",
    auto: "auto",
    "automático": "auto",
    automatico: "auto",
    ignorar: "bypassPermissions",
    bypass: "bypassPermissions",
  };

  /** Deliver a "/comando" verbatim to the focused chat's session, skipping
   *  the evaluator (typed slash = explicit intent, nothing to gate). */
  async function sendSlash(text: string) {
    if (focused) {
      push({ who: "user", text, task: labelFor(focused) });
      await ipc
        .workerSend(focused, text, [])
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    if (focusedTask) {
      await sendToFocusedTaskWith(focusedTask.title, focusedTask.sessionId, text);
      return;
    }
    push({ who: "sys", text: "comandos / precisam de um chat focado" });
  }

  async function submit(text: string, images: Attachment[]) {
    if (!text.trim() || busy) return;

    // A permission waiting + a clean typed verdict = the answer. ONE
    // grammar decides (domain::verdict) — "sempre pode" records the
    // standing rule, "assim que der" is NOT a yes.
    if (pendingPermission?.who === "permission") {
      const verdict = await ipc.interpretVerdict(text).catch(() => null);
      if (verdict?.kind === "confirm") {
        push({ who: "user", text, task: pendingPermission.task });
        await answerPermission(pendingPermission.requestId, true, verdict.always);
        say(verdict.always ? "Permitido, e não pergunto mais nesta task." : "Permitido.");
        return;
      }
      if (verdict?.kind === "deny") {
        push({ who: "user", text, task: pendingPermission.task });
        await answerPermission(pendingPermission.requestId, false);
        say("Negado.");
        return;
      }
    }

    // "/" = explicit command: NEVER the evaluator/gate. Vox-native ones
    // run here; anything else is delivered verbatim to the focused session
    // (the CLI expands its own slash commands — /compact, customs, plugins).
    if (text.startsWith("/")) {
      const [name = "", ...restWords] = text.slice(1).trim().split(/\s+/);
      const args = restWords.join(" ");
      if (name === "modo") {
        const flag = MODE_WORDS[args.toLowerCase()];
        push({ who: "user", text });
        if (flag) selectMode(flag);
        else push({ who: "sys", text: "modos: manual · edições · plano · auto · ignorar" });
        return;
      }
      if (name === "board") {
        railHas("board") ? removeRail("board") : ensureRail("board");
        return;
      }
      if (name === "rename" && args) {
        // Reuse the spoken path: "renomeia para X" renames the focused chat.
        text = `renomeia para ${args}`;
      } else {
        await sendSlash(text);
        return;
      }
    }

    // Local commands first ("abre o arquivo X", "vai pra task Y", "novo
    // chat no projeto Z"): zero tokens, resolved on this machine.
    const cmd = await ipc.taskCommand(text, focusedTask?.title).catch(() => null);
    if (cmd) {
      push({ who: "user", text });
      if (cmd.kind === "open" && cmd.session_id) {
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
          await sendToFocusedTaskWith(cmd.title, session, cmd.instruction);
        }
      } else if (cmd.kind === "open_file") {
        await openFileByQuery(cmd.query, cmd.project);
      } else if (cmd.kind === "project_added") {
        push({ who: "sys", text: `projeto ${cmd.title} adicionado (${cmd.path})` });
        say(`Projeto ${cmd.title} adicionado. Abrindo.`);
        refresh();
        // "abre um novo projeto": registering IS half the intent — the
        // window is the other half.
        await ipc.openProjectWindow(cmd.title, cmd.path).catch(() => {});
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
          ensureRail("board");
          say("Quadro na tela.");
        } else {
          // Costs are global: they live on the mother window.
          await ipc.focusMain("custos").catch(() => {});
          say("Custos na janela mãe.");
        }
      } else if (cmd.kind === "session_candidates") {
        // Recovering a session is a local lookup: the index already knows
        // every session on this machine. No agent, no digging, no tokens.
        if (cmd.candidates.length === 0) {
          push({ who: "sys", text: `nenhuma sessão fala sobre "${cmd.query}"` });
          say("Não achei sessão sobre isso.");
        } else if (cmd.candidates.length === 1) {
          await recoverSession(cmd.candidates[0]);
        } else {
          setPending({ kind: "pick-session", query: cmd.query, candidates: cmd.candidates });
          say(`Achei ${cmd.candidates.length} sessões. Qual delas?`);
        }
      } else if (cmd.kind === "task_candidates") {
        // Too close to call: options on screen, never a silent guess.
        setPending({ kind: "pick-task", query: cmd.query, candidates: cmd.candidates });
        say(`Achei ${cmd.candidates.length} tasks. Qual delas?`);
      } else if (cmd.kind === "open_settings") {
        // App settings live on the mother window.
        await ipc.focusMain("settings").catch(() => {});
        say("Configurações na janela mãe.");
      } else if (cmd.kind === "compact") {
        // Spoken "compacta o contexto": /compact on the focused session.
        await sendSlash("/compact");
        say("Compactando o contexto.");
      } else if (cmd.kind === "set_mode") {
        selectMode(cmd.mode);
        say("Modo trocado.");
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
      push({ who: "user", text, images: images.map((i) => i.dataUrl), task: focused && labelFor(focused) });
      await ipc
        .workerSend(focused, text, images.map(toImagePair))
        .then((directives) =>
          setLiveWorkers((old) =>
            old[focused] ? { ...old, [focused]: { ...old[focused], directives } } : old,
          ),
        )
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    if (!isQuestion && focusedTask) {
      // THE GATE (mismatch detection, LLM) + local PRECHECKS (size and
      // context numbers from THIS machine) run together: the gate can no
      // longer invent cost warnings, the prechecks can't be wrong.
      setBusy("avaliando…");
      const [gate, warnings] = await Promise.all([
        ipc.evaluate(text, focusedTask.title, focusedTask.sessionId).catch(() => null),
        ipc.dispatchPrechecks(focusedTask.sessionId).catch(() => []),
      ]);
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
          runAsk(text, images);
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
      }
      if (gate?.needs_confirmation || warnings.length > 0) {
        // The modal announces itself out loud and listens for the
        // verdict (the voice loop above) — no extra say() here.
        setPending({
          kind: "confirm-dispatch",
          instruction: text,
          sessionId: focusedTask.sessionId,
          warning: gate?.aviso ?? undefined,
          warnings,
        });
        return;
      }
      await sendToFocusedTaskWith(focusedTask.title, focusedTask.sessionId, text, images);
      return;
    }
    const route = await ipc.routeText(text);
    if (route === "dispatch") {
      push({ who: "user", text });
      setPending({ kind: "confirm-dispatch", instruction: text });
      return;
    }
    push({ who: "user", text, images: images.map((i) => i.dataUrl) });
    runAsk(text, images);
  }

  /** Mode the pill shows: the focused live task's, else the window/config default. */
  const currentMode =
    (focused ? liveWorkers[focused]?.directives.mode : undefined) ??
    modeDefault ??
    (overview?.default_mode || "manual");

  /**
   * Selector change: a focused live worker switches mode NOW (process
   * restart on the same session — flags are per-process, no text is
   * sent); with nothing focused it becomes this window's default.
   */
  function selectMode(flag: string) {
    if (focused && liveWorkers[focused]) {
      ipc
        .workerSetMode(focused, flag)
        .then((directives) =>
          setLiveWorkers((old) =>
            old[focused] ? { ...old, [focused]: { ...old[focused], directives } } : old,
          ),
        )
        .catch((err) => push({ who: "sys", text: `modo: ${err}` }));
      return;
    }
    setModeDefault(flag);
    push({ who: "sys", text: `novas tasks desta janela nascem no modo ${flag}` });
  }

  const railHas = (id: RailItem) => rail.some((s) => s.id === id);

  /** Open (or re-expand) a rail panel; a 3rd expanded one collapses the
   *  oldest other expanded panel instead of killing anything. */
  function ensureRail(id: RailItem) {
    setRail((old) => {
      let next = old.some((s) => s.id === id)
        ? old.map((s) => (s.id === id ? { ...s, collapsed: false } : s))
        : [...old, { id, collapsed: false }];
      const expanded = next.filter((s) => !s.collapsed);
      if (expanded.length > 2) {
        const victim = expanded.find((s) => s.id !== id);
        if (victim) {
          next = next.map((s) => (s.id === victim.id ? { ...s, collapsed: true } : s));
        }
      }
      return next;
    });
  }

  function removeRail(id: RailItem) {
    setRail((old) => old.filter((s) => s.id !== id));
    setExpanded((e) => (e === id ? null : e));
  }

  function toggleRailCollapse(id: RailItem) {
    setRail((old) =>
      old.map((s) => (s.id === id ? { ...s, collapsed: !s.collapsed } : s)),
    );
  }

  /** Drop the dragged panel at the target's position. */
  function dropRail(target: RailItem) {
    const from = dragRail.current;
    dragRail.current = null;
    if (!from || from === target) return;
    setRail((old) => {
      const moving = old.find((s) => s.id === from);
      if (!moving) return old;
      const without = old.filter((s) => s.id !== from);
      const at = without.findIndex((s) => s.id === target);
      return [...without.slice(0, at), moving, ...without.slice(at)];
    });
  }

  function addShell(): string {
    const id = `sh-${shellSeq.current++}-${Date.now() % 1e6}`;
    setShells((old) => [...old, id]);
    setTermTab(id);
    return id;
  }

  function closeShell(id: string) {
    setShells((old) => old.filter((s) => s !== id));
    setTermTab((t) => (t === id ? "feed" : t));
  }

  /**
   * The ▶ / >_ buttons on command blocks in the chat: open the Terminal
   * window and drop the command into a real shell — running it (execute)
   * or just leaving it typed for the user to review and hit Enter.
   */
  function runInTerminal(cmd: string, execute: boolean) {
    ensureRail("terminal");
    const existing = shells.includes(termTab) ? termTab : shells[0];
    const target = existing ?? addShell();
    if (existing) setTermTab(existing);
    const payload = cmd.replace(/\s+$/, "") + (execute ? "\r" : "");
    // A fresh shell needs a beat to spawn and print its prompt.
    setTimeout(() => ipc.termWrite(target, payload).catch(() => {}), existing ? 120 : 700);
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

  /** ONE voice surface: every mic button opens the global HUD. It knows
   *  this window is focused (the ledger) — speech lands here anyway, and
   *  the confirm chip + candidates come along for free. */
  async function onMic() {
    await ipc.hudShow().catch((err) => push({ who: "sys", text: `voz: ${err}` }));
  }

  /**
   * Answer an inline permission card and mark it decided in place.
   * `always` records a standing rule: this tool, this task, no more asks
   * (window-scoped — it dies with the window, never touches settings).
   */
  async function answerPermission(requestId: string, allow: boolean, always = false) {
    const card = messages.find((m) => m.who === "permission" && m.requestId === requestId);
    if (always && allow && card?.who === "permission") {
      const key = card.task ?? "";
      const set = allowAlways.current.get(key) ?? new Set<string>();
      set.add(card.tool);
      allowAlways.current.set(key, set);
      push({
        who: "sys",
        text: `sempre permitir ${card.tool} nesta task (vale até fechar a janela)`,
        task: card.task,
      });
    }
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
  const pendingPermRef = useRef<typeof pendingPermission>(undefined);
  pendingPermRef.current = pendingPermission;
  const answerPermissionRef = useRef(answerPermission);
  answerPermissionRef.current = answerPermission;
  const pendingRef = useRef<Pending>(null);
  pendingRef.current = pending;

  /** Confirm the open dispatch modal (Enter, voice, or the button). */
  function confirmPendingDispatch() {
    const p = pendingRef.current;
    if (p?.kind !== "confirm-dispatch") return;
    setPending(null);
    if (focusedTask) reactivateIfDone(focusedTask.title);
    runDispatch(p.instruction, p.sessionId);
  }
  const confirmDispatchRef = useRef(confirmPendingDispatch);
  confirmDispatchRef.current = confirmPendingDispatch;

  /** "compactar antes": /compact runs as its own turn, the message queues
   *  right behind it — the heavy-session warning's remedy, one click/word. */
  async function dispatchCompactFirst(instruction: string, sessionId?: string) {
    push({ who: "sys", text: "compactando o contexto antes de despachar…" });
    const liveEntry =
      focusedTask && sessionId === focusedTask.sessionId
        ? Object.entries(liveWorkers).find(([, w]) => w.label === focusedTask.title)
        : undefined;
    if (liveEntry) {
      await ipc.workerSend(liveEntry[0], "/compact", []).catch(() => {});
      await ipc
        .workerSend(liveEntry[0], instruction, [])
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    // Dead session: the resume opens on "/compact" (a resume inherits the
    // session's title — the card is never named "/compact"), then the
    // real work queues on the fresh worker's stdin.
    const out = await ipc
      .workerStart("/compact", sessionId ?? null, modeDefault ?? undefined)
      .catch(() => null);
    if (out?.status === "started") {
      const label =
        focusedTask && sessionId === focusedTask.sessionId
          ? focusedTask.title
          : instruction.split(/\s+/).slice(0, 5).join(" ");
      adoptWorker(out.task_id, label, out.directives, sessionId ?? "");
      await ipc
        .workerSend(out.task_id, instruction, [])
        .catch((err) => push({ who: "sys", text: `worker: ${err}` }));
      return;
    }
    push({ who: "sys", text: "não consegui compactar antes; despachando direto" });
    runDispatch(instruction, sessionId);
  }
  const compactFirstRef = useRef(dispatchCompactFirst);
  compactFirstRef.current = dispatchCompactFirst;

  // Voice-answerable modals (FASE 6): the app SPEAKS what it needs and
  // hears the verdict — "sim", "não", "a segunda", or a full rephrase
  // that REPLACES the message. Buttons and keys stay alive throughout.
  useEffect(() => {
    const p = pending;
    if (
      !p ||
      (p.kind !== "confirm-dispatch" &&
        p.kind !== "pick-session" &&
        p.kind !== "pick-task" &&
        p.kind !== "choice")
    ) {
      return;
    }
    let dead = false;
    let capturing = false;
    (async () => {
      const labels =
        p.kind === "confirm-dispatch"
          ? []
          : p.candidates.map((c) => ("title" in c && c.title ? c.title : ""));
      const announce =
        p.kind === "confirm-dispatch"
          ? `Mostrei sua mensagem na tela${
              p.sessionId && focusedTask ? ` para ${focusedTask.title}` : ""
            }. Você confirma?`
          : `Achei ${labels.length} opções. Qual delas?`;
      // HARD RULE: never arm the mic while speaking — whisper would
      // transcribe our own voice (speak resolves when `say` exits).
      await ipc.speak(announce).catch(() => {});
      if (dead) return;
      await new Promise((r) => setTimeout(r, 150));
      for (let round = 0; round < 4 && !dead; round++) {
        let heard = "";
        try {
          capturing = true;
          heard = (await ipc.hearOnce("modal")).trim();
        } catch {
          return; // mic busy (HUD took it) or absent: keys/click remain
        } finally {
          capturing = false;
        }
        if (dead || !heard) return;
        // Warning actions ("compacta antes") are part of the grammar
        // whenever the modal shows them.
        const hasCompact =
          p.kind === "confirm-dispatch" &&
          (pendingRef.current?.kind === "confirm-dispatch"
            ? (pendingRef.current.warnings ?? []).some((w) =>
                w.actions.includes("compact_first"),
              )
            : false);
        const actions: [string, string[]][] = hasCompact
          ? [["compact_first", ["compacta antes", "compactar antes", "compacta primeiro"]]]
          : [];
        const verdict = await ipc.interpretVerdict(heard, labels, actions).catch(() => null);
        const cur = pendingRef.current;
        if (!verdict || dead || !cur || cur.kind !== p.kind) return;
        if (verdict.kind === "unknown") continue;
        if (verdict.kind === "action" && verdict.id === "compact_first") {
          if (cur.kind === "confirm-dispatch") {
            const { instruction, sessionId } = cur;
            setPending(null);
            say("Compactando antes.");
            compactFirstRef.current(instruction, sessionId);
          }
          return;
        }
        if (verdict.kind === "deny") {
          setPending(null);
          say("Cancelado.");
          return;
        }
        if (cur.kind === "confirm-dispatch") {
          if (verdict.kind === "confirm") {
            push({ who: "user", text: heard });
            confirmDispatchRef.current();
            say("Despachando.");
            return;
          }
          if (verdict.kind === "instruction") {
            // The user rephrased (STT got words wrong): swap the message
            // and ask again — the textarea shows the new text.
            setPending({ ...cur, instruction: verdict.text });
            await ipc.speak("Troquei. Confirma?").catch(() => {});
            continue;
          }
        } else if (verdict.kind === "pick") {
          if (cur.kind === "pick-session") {
            const hit = cur.candidates[verdict.index];
            if (!hit) continue;
            setPending(null);
            recoverSession(hit);
            return;
          }
          if (cur.kind === "pick-task") {
            const cand = cur.candidates[verdict.index];
            if (!cand) continue;
            setPending(null);
            openTaskByTitle(cand.title, cand.session_id ?? undefined);
            return;
          }
          if (cur.kind === "choice") {
            const cand = cur.candidates[verdict.index];
            if (!cand) continue;
            setPending(null);
            runDispatch(cur.instruction, cand.session_id);
            return;
          }
        }
      }
    })();
    return () => {
      dead = true;
      // Cut only OUR orphan capture — never a capture the HUD owns.
      if (capturing) ipc.hearStop().catch(() => {});
    };
    // Re-arm on modal KIND changes only: edits to the same modal (voice
    // rephrase, typing in the textarea) must not restart the announce.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending?.kind]);

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
    images: Attachment[] = [],
  ) {
    await reactivateIfDone(title);
    const liveEntry = Object.entries(liveWorkers).find(([, w]) => w.label === title);
    push({ who: "user", text, images: images.map((i) => i.dataUrl), task: title });
    if (liveEntry) {
      await ipc
        .workerSend(liveEntry[0], text, images.map(toImagePair))
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

  /**
   * Open a task's chat: resolve its session (board link first, then best
   * topic match) and focus it. The single door used by the sidebar, the
   * board cards and the global board of the mother window.
   */
  async function openTaskByTitle(title: string, sessionId?: string, note?: string) {
    setReading(null);
    let session = sessionId;
    if (!session) {
      const hit = await ipc.findSession(title).catch(() => null);
      session = hit?.session_id ?? undefined;
    }
    if (!session) {
      push({ who: "sys", text: `nenhuma sessão encontrada para "${title}"` });
      return;
    }
    await focusTask(title, session, note);
  }

  /** Sidebar click: open the task — or release it when already focused
   *  (the chip that used to do this was redundant with the sidebar). */
  const openTaskFromSidebar = (t: BoardTask) => {
    if (focusedTask?.title === t.title) {
      setFocusedTask(null);
      setFocused(null);
      return Promise.resolve();
    }
    return openTaskByTitle(t.title, t.session_ids.at(-1), t.note);
  };

  /**
   * Recover an existing session: it becomes a board task named after the
   * SESSION (not after the sentence that found it) and its chat opens with
   * the history loaded. Sessions living in another project open there.
   */
  async function recoverSession(hit: SessionHit) {
    let task: { title: string; workspace?: string | null };
    try {
      task = await ipc.taskFromSession(hit.session_id);
    } catch (err) {
      push({ who: "sys", text: `não consegui registrar a sessão: ${err}` });
      return;
    }
    const ws = task.workspace;
    const elsewhere =
      ws && forcedProject && ws !== forcedProject.path && !ws.startsWith(`${forcedProject.path}/`);
    if (elsewhere) {
      const owner = allProjects.find((p) => ws === p.path || ws.startsWith(`${p.path}/`));
      if (owner) {
        await ipc
          .openProjectWindow(owner.name, owner.path, task.title, hit.session_id)
          .catch((err) => push({ who: "sys", text: `janela: ${err}` }));
        push({ who: "sys", text: `"${task.title}" é do projeto ${owner.name}: abri lá` });
        say(`Essa sessão é do projeto ${owner.name}. Abri a janela dele.`);
        refresh();
        return;
      }
    }
    await openTaskByTitle(task.title, hit.session_id);
    refresh();
    say(`Retomando ${task.title}.`);
  }

  const visibleMessages = messages.filter((m) => {
    const thread = "task" in m ? (m.task ?? null) : null;
    return thread === (focusedTask?.title ?? null);
  });

  // Typed windows, shared by the tiled layout and the expanded mode.
  /** Drag-to-reorder wiring for a rail panel's header. */
  const railDragProps = (id: RailItem): React.HTMLAttributes<HTMLDivElement> => ({
    draggable: true,
    onDragStart: () => {
      dragRail.current = id;
    },
    onDragOver: (e) => e.preventDefault(),
    onDrop: () => dropRail(id),
  });

  const frameArquivo = (slot?: { collapsed: boolean }) => (
    <PanelFrame
      title={t("frame_file")}
      tabs={
        <FileTabs
          files={openFiles}
          active={activeFile}
          dirty={dirtyPaths}
          onActivate={(i) => {
            setActiveFile(i);
            const f = openFiles[i];
            if (f) lastFocus.current.set(f.abs, Date.now());
          }}
          onCloseTab={closeFileTab}
        />
      }
      expanded={expanded === "arquivo"}
      collapsed={slot?.collapsed ?? false}
      onToggleExpand={() => setExpanded((e) => (e === "arquivo" ? null : "arquivo"))}
      onToggleCollapse={slot ? () => toggleRailCollapse("arquivo") : undefined}
      dragProps={slot ? railDragProps("arquivo") : undefined}
      onClose={() => {
        setOpenFiles([]);
        setDirtyPaths(new Set());
        removeRail("arquivo");
      }}
    >
      <FilesEditor files={openFiles} active={activeFile} onDirty={onFileDirty} />
    </PanelFrame>
  );
  const frameTerminal = (slot?: { collapsed: boolean }) => (
    <PanelFrame
      title={t("frame_terminal")}
      tabs={
        <TerminalTabs
          shells={shells}
          active={termTab}
          onActivate={setTermTab}
          onAddShell={addShell}
          onCloseShell={closeShell}
        />
      }
      expanded={expanded === "terminal"}
      collapsed={slot?.collapsed ?? false}
      onToggleExpand={() => setExpanded((e) => (e === "terminal" ? null : "terminal"))}
      onToggleCollapse={slot ? () => toggleRailCollapse("terminal") : undefined}
      dragProps={slot ? railDragProps("terminal") : undefined}
      onClose={() => removeRail("terminal")}
    >
      <TerminalPane
        rawLog={rawLog}
        focusedLabel={focusedTask?.title}
        shells={shells}
        active={termTab}
        cwd={forcedProject?.path}
        onAddShell={addShell}
        onCloseShell={closeShell}
        onActivate={setTermTab}
      />
    </PanelFrame>
  );
  const frameBoard = (slot?: { collapsed: boolean }) => (
    <PanelFrame
      title={t("frame_board")}
      expanded={expanded === "board"}
      collapsed={slot?.collapsed ?? false}
      onToggleExpand={() => setExpanded((e) => (e === "board" ? null : "board"))}
      onToggleCollapse={slot ? () => toggleRailCollapse("board") : undefined}
      dragProps={slot ? railDragProps("board") : undefined}
      onClose={() => removeRail("board")}
    >
      <Board
        tasks={board}
        onMove={(title, status) => ipc.boardMove(title, status).then(refresh).catch(() => {})}
        onOpen={openTaskFromSidebar}
      />
    </PanelFrame>
  );
  const frameArquivos = (slot?: { collapsed: boolean }) => (
    <PanelFrame
      title={t("frame_files")}
      expanded={expanded === "arquivos"}
      collapsed={slot?.collapsed ?? false}
      onToggleExpand={() => setExpanded((e) => (e === "arquivos" ? null : "arquivos"))}
      onToggleCollapse={slot ? () => toggleRailCollapse("arquivos") : undefined}
      dragProps={slot ? railDragProps("arquivos") : undefined}
      onClose={() => removeRail("arquivos")}
    >
      <FilesPanel
        projects={projects}
        initialProject={filesInitialProject}
        onOpen={openFile}
      />
    </PanelFrame>
  );

  const placeholder = draftChat
    ? t("composer_draft", { name: draftChat.name })
    : focused
      ? t("composer_worker", { name: labelFor(focused) })
      : focusedTask
        ? t("composer_task", { name: focusedTask.title })
        : t("composer_idle");

  /** Window controls docked on the composer row — no topbar, no wasted strip. */
  const trailingControls = (
    <>
      {rateLimit && rateLimit.status !== "allowed" && (
        <span
          className={`scope ratelimit ${rateLimit.status}`}
          title={`janela ${rateLimit.limit_kind ?? "?"} · status ${rateLimit.status}`}
        >
          ⏳ {rateLimit.status === "allowed_warning" ? "quase no limite" : "limite atingido"}
        </span>
      )}
      <div className="scope-anchor">
        <button
          className={`scope ${scopeInfo ? "on" : ""}`}
          title={t("costs_btn")}
          onClick={() => setScopeInfo((s) => !s)}
        >
          <Wallet size={13} />
        </button>
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
      </div>
      <button
        className={`scope ${railHas("terminal") ? "on" : ""}`}
        title={t("terminal_btn")}
        onClick={() =>
          railHas("terminal") ? removeRail("terminal") : ensureRail("terminal")
        }
      >
        <SquareTerminal size={13} />
      </button>
      <button
        className={`scope ${speak ? "on" : ""}`}
        title={speak ? "voz ligada (Esc corta a fala)" : "voz desligada"}
        onClick={() => setSpeak((s) => !s)}
      >
        {speak ? <Volume2 size={13} /> : <VolumeX size={13} />}
      </button>
      <span className="composer-orb" title={busy ?? (recording ? "ouvindo…" : "pronto")}>
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
      </span>
    </>
  );

  return (
    <div className="app">
      {/* Sidebar toggle floats where the topbar used to be — the topbar
          itself is gone: that strip was pure wasted height. */}
      <button
        className={`side-toggle-float ${sidebarOpen ? "" : "closed"}`}
        title={sidebarOpen ? t("side_hide") : t("side_show")}
        onClick={() => {
          const panel = sidebarRef.current;
          if (panel) panel.isCollapsed() ? panel.expand() : panel.collapse();
        }}
      >
        <PanelLeft size={14} />
      </button>

      {expanded ? (
        <div className="workarea expanded-area">
          {expanded === "arquivo"
            ? frameArquivo()
            : expanded === "terminal"
              ? frameTerminal()
              : expanded === "board"
                ? frameBoard()
                : frameArquivos()}
        </div>
      ) : (
        <PanelGroup direction="horizontal" autoSaveId="vox-code" className="workarea">
          <Panel
            ref={sidebarRef}
            collapsible
            defaultSize={18}
            minSize={10}
            maxSize={34}
            className="pane"
            onCollapse={() => setSidebarOpen(false)}
            onExpand={() => setSidebarOpen(true)}
          >
            <Sidebar
              projects={projects}
              tasks={board}
              chats={chats.filter(
                (c) =>
                  !(overview?.board ?? []).some((t) => t.session_ids.includes(c.session_id)),
              )}
              activeTitle={focusedTask?.title}
              liveTitles={Object.values(liveWorkers).map((w) => w.label)}
              onOpen={openTaskFromSidebar}
              onOpenChat={recoverSession}
              boardOpen={railHas("board")}
              onToggleBoard={() =>
                railHas("board") ? removeRail("board") : ensureRail("board")
              }
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
                ensureRail("arquivos");
              }}
            />
          </Panel>
          <PanelResizeHandle className="rhandle" />
          <Panel minSize={30} className="pane">
            <div className="maincol">
              {reading ? (
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
                <Transcript
                  messages={visibleMessages}
                  directivesFor={directivesFor}
                  onAnswerPermission={answerPermission}
                  onOpenPath={openAbsolutePath}
                  onRunCommand={runInTerminal}
                />
              )}
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
                trailing={trailingControls}
              >
                <ModeSelect
                  value={currentMode}
                  appliesTo={focused ? labelFor(focused) : undefined}
                  onSelect={selectMode}
                />
                <WorkerChips
                  liveWorkers={liveWorkers}
                  focused={focused}
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
                  onInfo={(taskId) => {
                    const session = overview?.workers.find(
                      (w) => w.task_id === taskId,
                    )?.session_id;
                    if (session) {
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
          {rail.length > 0 && (
            <>
              <PanelResizeHandle className="rhandle" />
              <Panel defaultSize={42} minSize={20} className="pane">
                {/* The rail: panels stack vertically; expanded ones split
                    the height, collapsed ones cost a title bar. */}
                <div className="rail">
                  {rail.map((slot) => (
                    <div
                      key={slot.id}
                      className={`rail-slot ${slot.collapsed ? "collapsed" : ""}`}
                    >
                      {slot.id === "arquivo"
                        ? frameArquivo(slot)
                        : slot.id === "terminal"
                          ? frameTerminal(slot)
                          : slot.id === "board"
                            ? frameBoard(slot)
                            : frameArquivos(slot)}
                    </div>
                  ))}
                </div>
              </Panel>
            </>
          )}
        </PanelGroup>
      )}

      {quickOpen && (
        <QuickOpen
          projects={projects}
          onPick={(file) => {
            openFile(file);
            setQuickOpen(false);
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
          setReading(null);
          const title = taskTitle ?? focusedTask?.title;
          if (title) reactivateIfDone(title);
          runDispatch(instruction, sessionId);
        }}
        onFocusWorker={(taskId) => setFocused(taskId)}
        onPickSession={recoverSession}
        onPickTask={(title, sessionId) => openTaskByTitle(title, sessionId)}
        onCompactFirst={dispatchCompactFirst}
      />
    </div>
  );
}
