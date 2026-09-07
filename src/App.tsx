import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import {
  Panel,
  PanelGroup,
  PanelResizeHandle,
  type ImperativePanelHandle,
} from "react-resizable-panels";
import { PanelLeft, SquareTerminal, Volume2, VolumeX, Wallet } from "lucide-react";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import { setLang, setSpeechLang, st, t } from "./lib/i18n";
import Board from "./components/Board";
import Composer from "./components/Composer";
import { toImagePair, type Attachment } from "./lib/composerText";
import FilesEditor, { FileTabs } from "./components/FilesEditor";
import FilesPanel from "./components/FilesPanel";
import Modals, { type Pending } from "./components/Modals";
import ModeSelect from "./components/ModeSelect";
import ModelSelect, { type ModelTiers } from "./components/ModelSelect";
import PanelFrame from "./components/PanelFrame";
import QuickOpen from "./components/QuickOpen";
import Reader from "./components/Reader";
import SessionInfo from "./components/SessionInfo";
import Sidebar from "./components/Sidebar";
import TerminalPane, { TerminalTabs } from "./components/TerminalPane";
import Transcript from "./components/Transcript";
import EmptyProject from "./components/EmptyProject";
import WorkerChips from "./components/WorkerChips";
import TurnStatus, { type TurnState } from "./components/TurnStatus";
import { useHarkEvents } from "./hooks/useHarkEvents";
import { useRepoStates } from "./hooks/useRepoStates";
import RepoRuler from "./components/RepoRuler";
import { ContextRing } from "./components/Meter";
import { agentError, askReplyMsg, isAuthError } from "./lib/format";
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
  SessionOwner,
  TranscriptEntry,
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
  // Key for the window's general chat, which has no task title.
  const GENERAL = "";
  // What each running turn is doing, keyed by thread — the status line
  // belongs to ONE chat, not to the window. A single global one showed
  // "1m 35s · quase terminando de pensar…" above every chat you opened,
  // and the first worker to finish wiped the clock of every other.
  const [turns, setTurns] = useState<Record<string, TurnState>>({});
  const [recording, setRecording] = useState(false);
  /** Capture ended, whisper still grinding — seconds of it without Metal. */
  const [transcribing, setTranscribing] = useState(false);
  // The global Esc handler must see the live value (no stale closure).
  const recordingRef = useRef(false);
  const transcribingRef = useRef(false);
  const [pending, setPending] = useState<Pending>(null);
  // Read-only thread being viewed (board tab), never executes anything.
  /** Threads whose log has no more history above what is loaded. */
  const [historyDone, setHistoryDone] = useState<Record<string, boolean>>({});
  /** Sessions a human is holding at a terminal — ours or any other app's.
   *  While one is held, Hark reads its log and writes nothing. */
  const [owners, setOwners] = useState<SessionOwner[]>([]);
  /** Where each thread's mirror has read up to, in bytes of its log. */
  const mirror = useRef<Map<string, number>>(new Map());
  /** Armed by the held banner: the next message forks the held session
   *  into a NEW one and works there — the owner's transcript untouched. */
  const [forkDraft, setForkDraft] = useState<{ title: string; sessionId: string } | null>(null);
  /** Tasks auto-compacted on this climb of the context window. */
  const autoCompacted = useRef<Set<string>>(new Set());
  /** The login nudge fired already — expired OAuth fails every call, and
   *  one typed `claude` is help; five are harassment. */
  const loginNudged = useRef(false);
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
  /** Stand-in for "every tool" in a task's standing allow rules. */
  const ANY_TOOL = "*";
  /** How much of a chat comes back when you open it. Twelve entries — and
   *  a tool call plus its result are two — meant four real messages: a
   *  reopened chat showed almost nothing of the work it contained. Reading
   *  the log is local and free; only the DOM pays. */
  const HISTORY_TAIL = 150;
  /** How much further back one click reaches. */
  const HISTORY_PAGE = 200;
  /** What each thread has loaded so far, and where its window starts. */
  const history = useRef<Map<string, { count: number; firstKey: string }>>(new Map());

  // Mode chosen on the selector for NEW tasks (null = config default).
  const [modeDefault, setModeDefault] = useState<string | null>(null);
  /** Window's model choice ("" = auto/router); focused live tasks switch
   *  the process, otherwise it seeds new chats born here. */
  const [modelDefault, setModelDefault] = useState<string>("");
  const [tiers, setTiers] = useState<ModelTiers>({
    light: "haiku", standard: "sonnet", heavy: "opus", max: "fable",
  });
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
  // Vertical split between expanded rail panels (flex units, default 1).
  const [railFlex, setRailFlex] = useState<Record<string, number>>({});
  const railRef = useRef<HTMLDivElement>(null);

  /** Drag the divider between two expanded rail panels. */
  function startRailResize(e: React.MouseEvent, above: RailItem, below: RailItem) {
    e.preventDefault();
    e.stopPropagation();
    const startY = e.clientY;
    const fa = railFlex[above] ?? 1;
    const fb = railFlex[below] ?? 1;
    const sum = fa + fb;
    const expanded = rail.filter((s) => !s.collapsed);
    const totalFlex = expanded.reduce((a, s) => a + (railFlex[s.id] ?? 1), 0) || 1;
    const totalPx = railRef.current?.getBoundingClientRect().height ?? 600;
    const unit = Math.max(40, totalPx / totalFlex);
    const onMove = (ev: MouseEvent) => {
      const d = (ev.clientY - startY) / unit;
      const a = Math.min(Math.max(fa + d, 0.25), sum - 0.25);
      setRailFlex((old) => ({ ...old, [above]: a, [below]: sum - a }));
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }
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

  /** The thread on screen right now — the label of the focused worker, or
   *  of the focused task, or null for the window's general chat. */
  const currentThread = useCallback((): string | null => {
    const live = focusedRef.current;
    if (live) return workersRef.current[live]?.label ?? live;
    return focusedTaskRef.current?.title ?? null;
  }, []);
  /** Threads whose next agent turn IS a compaction (we sent "/compact").
   *  The summary is pages long and the CLI writes it as ordinary prose. */
  const compacting = useRef<Set<string>>(new Set());
  /** The CLI's own opening line for a compaction, in case one arrives
   *  without us having asked (a session that compacted itself). */
  const COMPACTION_HEAD = /^This session is being continued from a previous conversation/;
  /** An auth failure anywhere: open the terminal with `claude` typed so
   *  /login is one Enter away. Detection is the health code (agent_auth),
   *  never a re-parse of wording here. */
  function nudgeLogin(err: unknown) {
    if (!isAuthError(err) || loginNudged.current) return;
    loginNudged.current = true;
    // The remedy as a message: a runnable block — its ▶ opens the
    // terminal pane beside and executes (user-asked shape, 26/08).
    const fence = "```";
    push({ who: "hark", text: `${t("auth_fix")}\n\n${fence}bash\nclaude /login\n${fence}` });
  }
  const nudgeRef = useRef(nudgeLogin);
  nudgeRef.current = nudgeLogin;
  const runInTerminalRef = useRef<(cmd: string, execute: boolean) => void>(() => {});

  const push = useCallback(
    (m: Msg) =>
      setMessages((old) => [
        ...old,
        {
          ...asCompaction(m),
          // An untagged message belongs to the general chat, so with a task
          // focused it is pushed straight out of view. Whatever is added
          // while a chat is open belongs to that chat unless the caller
          // named another thread.
          ...(m.task == null ? { task: currentThread() ?? undefined } : {}),
          ...((m.who === "user" || m.who === "hark") && m.ts == null
            ? { ts: Date.now() }
            : {}),
        },
      ]),
    [currentThread],
  );
  /** Turn an agent reply that is really a compaction summary into the
   *  folded kind. Live, the only tell is that we asked for it; a session
   *  that compacted itself is caught by the CLI's fixed opening line. */
  const asCompaction = useCallback((m: Msg): Msg => {
    if (m.who !== "hark") return m;
    const thread = m.task ?? currentThread() ?? "";
    const asked = compacting.current.delete(thread);
    if (!asked && !COMPACTION_HEAD.test(m.text)) return m;
    return { who: "compact", text: m.text, task: m.task };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
        setSpeechLang(o.language);
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

  // Who is holding a session right now. One question, asked on a timer,
  // answered from the CLI's own listing — so a terminal opened OUTSIDE
  // Hark (iTerm, tmux, another machine's app) counts exactly the same.
  useEffect(() => {
    let alive = true;
    const tick = () =>
      ipc
        .sessionOwners()
        .then((list) => {
          if (alive) setOwners(list);
        })
        .catch(() => {});
    tick();
    const id = setInterval(tick, 2500);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  /** The terminal session holding the chat on screen, if any. */
  const heldBy = focusedTask
    ? owners.find((o) => o.session_id === focusedTask.sessionId)
    : undefined;
  const heldByRef = useRef(heldBy);
  const previouslyHeld = heldByRef.current;
  heldByRef.current = heldBy;

  // Mirror: while the terminal owns the session, follow its log. The file
  // is append-only and `--resume` writes to the SAME one, so reading what
  // grew IS the mirror — no process, no tokens, no cooperation needed from
  // whoever is typing over there.
  useEffect(() => {
    const task = focusedTask;
    if (!task || !heldBy) return;
    let alive = true;
    const tick = () => {
      const at = mirror.current.get(task.title) ?? 0;
      ipc
        .transcriptSince(task.sessionId, at)
        .then((out) => {
          if (!alive) return;
          mirror.current.set(task.title, out.offset);
          for (const e of out.entries) {
            const msg = entryToMsg(e, task.title);
            if (msg) push(msg);
          }
        })
        .catch(() => {});
    };
    const id = setInterval(tick, 1500);
    tick();
    return () => {
      alive = false;
      clearInterval(id);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusedTask?.title, focusedTask?.sessionId, heldBy?.pid]);

  // The boundary, said out loud in the thread: the wheel changing hands is
  // the one thing that must never be silent.
  useEffect(() => {
    const title = focusedTask?.title;
    if (!title) return;
    if (heldBy && !previouslyHeld) {
      push({ who: "sys", text: `o volante passou pro terminal (${heldBy.name})`, task: title });
    }
    if (!heldBy && previouslyHeld) {
      // One last read: the final turns land before the chat unlocks.
      const at = mirror.current.get(title) ?? 0;
      ipc
        .transcriptSince(previouslyHeld.session_id, at)
        .then((out) => {
          mirror.current.set(title, out.offset);
          for (const e of out.entries) {
            const msg = entryToMsg(e, title);
            if (msg) push(msg);
          }
          push({
            who: "sys",
            text: `volante de volta — ${out.entries.length} entradas novas do terminal`,
            task: title,
          });
        })
        .catch(() => push({ who: "sys", text: "volante de volta", task: title }));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [heldBy?.session_id, heldBy?.pid]);

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

  useHarkEvents({
    labelFor,
    push,
    setMessages,
    setLiveWorkers,
    pushRaw,
    addCost,
    // The turn landed: that chat's clock stops, and only that one. The
    // event's own label and the worker registry's can disagree (a renamed
    // card), so both keys are cleared.
    onWorkerTurn: useCallback(
      (
        label: string,
        _err: boolean,
        taskId?: string,
        contextPct?: number | null,
        _cost?: number,
        _session?: string | null,
        errorCode?: string | null,
      ) => {
        endTurn(label, taskId);
        // Auth death is fatal and identical on retry: the remedy is the
        // terminal, so open it with `claude` typed (once per window).
        if (errorCode === "agent_auth") nudgeRef.current("agent_auth:turn");
        // FASE 8.6: act on context pressure instead of only warning. At
        // 85% the next turns degrade and a forced summary is coming
        // anyway; compacting NOW is the cheap version of that. Once per
        // climb — the flag re-arms when the window drops back.
        if (taskId && contextPct != null) {
          if (contextPct >= 0.85 && !autoCompacted.current.has(taskId)) {
            autoCompacted.current.add(taskId);
            compacting.current.add(label);
            push({
              who: "sys",
              text: `contexto em ${Math.round(contextPct * 100)}% — compactando sozinho`,
              task: label,
            });
            beginTurn(label);
            ipc.workerSend(taskId, "/compact", []).catch(() => endTurn(label));
          } else if (contextPct < 0.7) {
            autoCompacted.current.delete(taskId);
          }
        }
      },
      // eslint-disable-next-line react-hooks/exhaustive-deps
      [],
    ),
    onSpeaking: setSpeaking,
    onMicPhase: useCallback((phase: "capturing" | "transcribing" | "idle") => {
      const live = phase === "capturing";
      recordingRef.current = live;
      transcribingRef.current = phase === "transcribing";
      setRecording(live);
      setTranscribing(phase === "transcribing");
    }, []),
    // Real events drive the status; nothing here is inferred from timers.
    // Each event names the thread it belongs to, so a busy worker never
    // moves the clock of the chat you are reading.
    onTurnActivity: useCallback(
      (
        thread: string,
        phase: "thinking" | "writing" | "tool",
        detail?: string,
        chars?: number,
      ) =>
        setTurns((old) => {
          // A turn this window did not start — dispatched by voice, or by
          // another window — still deserves a status the moment it speaks.
          // Its clock counts from the first event seen HERE; no earlier
          // timestamp exists on this side.
          const running = old[thread] ?? {
            startedAt: Date.now(),
            phase: { kind: "thinking" as const },
            chars: 0,
          };
          return {
            ...old,
            [thread]: {
              ...running,
              phase:
                phase === "tool" ? { kind: "tool", name: detail ?? "tool" } : { kind: phase },
              chars: running.chars + (chars ?? 0),
            },
          };
        }),
      [],
    ),
    onRateLimit: setRateLimit,
    autoAllow: useCallback(
      (ask: import("./types").PermissionAsk) => {
        const label = workersRef.current[ask.task_id]?.label ?? ask.label ?? ask.task_id;
        const rules = allowAlways.current.get(label);
        // ANY_TOOL is what relaxing the mode mid-turn installs: the CLI
        // keeps its old flags until the process reopens, so the window
        // answers in its place. The production gate still overrides it —
        // that check lives before autoAllow is ever consulted.
        return !!rules && (rules.has(ask.tool_name) || rules.has(ANY_TOOL));
      },
      [],
    ),
    // Spoken "sempre pode" (HUD) reaches THIS window's standing rules too.
    onAllowRule: useCallback((label: string, tool: string) => {
      const set = allowAlways.current.get(label) ?? new Set<string>();
      set.add(tool);
      allowAlways.current.set(label, set);
    }, []),
    speakRef,
    refresh,
    onWorkerExit: useCallback((taskId: string) => {
      endTurn(null, taskId);
      // Its open asks can never be answered now: approving would reach a
      // process that is gone. They say so instead of offering buttons.
      const label = labelFor(taskId);
      setMessages((old) =>
        old.map((m) =>
          m.who === "permission" && !m.decision && !m.expired && m.task === label
            ? { ...m, expired: true }
            : m,
        ),
      );
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
        // Already transcribing? The words are not coming — kill the turn
        // rather than make the user wait out a whisper pass on the CPU.
        if (transcribingRef.current) {
          ipc.hearAbort().catch(() => {});
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

  // Git state for the work on screen: the focused task's repository (its
  // workspace is often a subdirectory — the Rust side resolves the root)
  // plus this window's project as the fallback.
  const focusedWorkspace =
    board.find((t) => t.title === focusedTask?.title)?.workspace || activeProject?.path || "";
  // One repository, one readout. Tasks in a project overwhelmingly share
  // the SAME working tree, so a per-row chip repeated the identical number
  // down the whole sidebar — noise, not news. The ruler says it once.
  const repoStates = useRepoStates(focusedWorkspace ? [focusedWorkspace] : []);
  const focusedRepo = repoStates[focusedWorkspace];

  const directivesFor = useCallback(
    (taskLabel?: string): Directives | undefined =>
      taskLabel
        ? Object.values(workersRef.current).find((w) => w.label === taskLabel)?.directives
        : undefined,
    [],
  );

  async function runAsk(question: string, images: Attachment[]) {
    setBusy("perguntando…");
    const thread = currentThread() ?? GENERAL;
    beginTurn();
    try {
      // The focused project scopes the snapshot: clicking a chat or
      // starting one IS the context selection (no manual picker).
      const reply = await ipc.askText(question, images.map(toImagePair), activeProject);
      push({ who: "hark", ...askReplyMsg(reply) });
      if (reply.cost_usd) addCost("hark (perguntas)", reply.cost_usd);
      say(reply.fala);
    } catch (err) {
      nudgeRef.current(err);
      push({ who: "sys", text: `erro: ${agentError(err)}` });
    } finally {
      endTurn(thread);
      setBusy(null);
      refresh();
    }
  }

  /** Register a started worker in the UI and focus its thread. */
  function adoptWorker(taskId: string, label: string, directives: Directives, sessionId: string) {
    // The dispatch's clock was started before this task had a name.
    rekeyTurn(currentThread() ?? GENERAL, label);
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
    const from = currentThread() ?? GENERAL;
    push({ who: "sys", text: `dispatch: ${instruction}` });
    // Spawning a worker takes seconds before the first event: the clock
    // starts with the dispatch, not with the agent's first word.
    beginTurn(from);
    // Only a live worker keeps the clock: every other outcome ends here,
    // and a clock nobody stops ticks forever.
    let live = false;
    try {
      const out = await ipc.workerStart(instruction, sessionId ?? null, modeDefault ?? undefined, undefined, modelDefault || undefined);
      if (out.status === "started") {
        const label =
          focusedTask && sessionId === focusedTask.sessionId
            ? focusedTask.title
            : instruction.split(/\s+/).slice(0, 5).join(" ");
        adoptWorker(out.task_id, label, out.directives, sessionId ?? "");
        live = true;
      } else if (out.status === "done") {
        push({ who: "hark", text: out.summary, cost: out.cost_usd });
        say("Tarefa concluída.");
      } else if (out.status === "failed") {
        push({ who: "sys", text: `worker falhou: ${out.summary}` });
        say(st("sp_task_failed"));
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
      nudgeRef.current(err);
      push({ who: "sys", text: `erro: ${agentError(err)}` });
    } finally {
      if (!live) endTurn(from);
      setBusy(null);
      refresh();
    }
  }

  /** First message of a "+" draft: opens a brand-new session in the project. */
  async function runNewChat(project: Project, instruction: string) {
    setBusy("abrindo sessão nova…");
    const from = currentThread() ?? GENERAL;
    push({ who: "sys", text: `novo chat em ${project.name}` });
    beginTurn(from);
    let live = false;
    try {
      const out = await ipc.chatStart(project.path, instruction, modeDefault ?? undefined, modelDefault || undefined);
      if (out.status === "started") {
        const label = instruction.split(/\s+/).slice(0, 5).join(" ");
        push({ who: "user", text: instruction, task: label });
        adoptWorker(out.task_id, label, out.directives, "");
        live = true;
      } else {
        push({ who: "sys", text: `não abriu: ${JSON.stringify(out)}` });
      }
    } catch (err) {
      nudgeRef.current(err);
      push({ who: "sys", text: `erro: ${agentError(err)}` });
    } finally {
      if (!live) endTurn(from);
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
        // Same file, new line: the tab is reused and jumps.
        return file.line && old[existing].line !== file.line
          ? old.map((f, i) => (i === existing ? { ...f, line: file.line } : f))
          : old;
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
    // "src/lib.rs:38" is how every tool prints a location: the number
    // travels to the viewer, which scrolls there.
    const at = /:(\d+)(?::\d+)?$/.exec(path);
    const line = at ? Number(at[1]) : undefined;
    if (at) path = path.slice(0, at.index);
    if (!path.startsWith("/") && !path.startsWith("~")) {
      // A relative path is relative to where the AGENT runs, which is the
      // task's workspace — often a repository inside the project, not the
      // project root. The registered project is the fallback.
      const base =
        board.find((t) => t.title === focusedTask?.title)?.workspace || activeProject?.path;
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
      openFile({ abs: path, rel: np.slice(norm(project.path).length + 1), project, line });
      return;
    }
    const parts = path.split("/");
    openFile({
      abs: path,
      rel: parts[parts.length - 1] ?? path,
      project: { name: parts[parts.length - 2] ?? "", path: parts.slice(0, -1).join("/") },
      line,
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
        say(st("sp_opening_file", { f: hits[0].split("/").pop() ?? "" }));
        return;
      }
    }
    push({ who: "sys", text: `nenhum arquivo bate com "${query}"` });
    say("Não achei esse arquivo.");
  }

  /** "hark, quanto gastei hoje?" — a vocative at the front is how you talk
   *  to hark from inside a chat. It takes a comma (or an interjection)
   *  precisely because this project is NAMED hark: "hark precisa de um fix
   *  no composer" is a message about the code, not a summons. "vox" still
   *  answers — the product was renamed, the habit was not. */
  const TO_HARK = /^(?:(?:ei|oi|opa|olha)\s+(?:hark|vox)\b[\s,:.!?]*|(?:hark|vox)\s*[,:]\s*)/i;

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
    // The reply to "/compact" is the summary itself: mark the thread so it
    // arrives folded instead of pasted into the conversation.
    if (/^\/compact\b/.test(text)) {
      const thread = focused ? labelFor(focused) : focusedTask?.title;
      if (thread) compacting.current.add(thread);
    }
    if (focused) {
      push({ who: "user", text, task: labelFor(focused) });
      // "/compact" is a turn like any other: it thinks, it costs, it takes
      // a minute. It showed no clock because this path never started one.
      beginTurn(labelFor(focused));
      await ipc.workerSend(focused, text, []).catch((err) => {
        endTurn(labelFor(focused));
        nudgeRef.current(err);
          push({ who: "sys", text: `worker: ${agentError(err)}` });
      });
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
        say(verdict.always ? "Permitido, e não pergunto mais nesta task." : st("sp_allowed"));
        return;
      }
      if (verdict?.kind === "deny") {
        push({ who: "user", text, task: pendingPermission.task });
        await answerPermission(pendingPermission.requestId, false);
        say(st("sp_denied"));
        return;
      }
    }

    // "/" = explicit command: NEVER the evaluator/gate. Hark-native ones
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
      if (name === "usage") {
        // Local /usage: ledger + statusline bridge, zero tokens — the
        // card lands in the focused thread like the CLI draws its own.
        const report = await ipc
          .usageReport(focusedTask?.sessionId ?? undefined)
          .catch(() => null);
        if (report) push({ who: "usage", report, task: focused ? labelFor(focused) : focusedTask?.title });
        else push({ who: "sys", text: "uso indisponível (ledger vazio?)" });
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
        say(st("sp_opening", { t: cmd.title }));
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
        say(st("sp_on_task", { t: cmd.title }));
        if (cmd.instruction) {
          await sendToFocusedTaskWith(cmd.title, session, cmd.instruction);
        }
      } else if (cmd.kind === "open_file") {
        await openFileByQuery(cmd.query, cmd.project);
      } else if (cmd.kind === "project_added") {
        push({ who: "sys", text: `projeto ${cmd.title} adicionado (${cmd.path})` });
        say(st("sp_project_added", { t: cmd.title }));
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
          say(st("sp_new_chat_first", { t: cmd.title }));
        }
      } else if (cmd.kind === "open_project") {
        await ipc.openProjectWindow(cmd.title, cmd.path).catch((err) =>
          push({ who: "sys", text: `janela: ${err}` }),
        );
        if (cmd.instruction) {
          await ipc.chatStart(cmd.path, cmd.instruction).catch((err) =>
            push({ who: "sys", text: `chat: ${err}` }),
          );
          say(st("sp_opening_and_work", { t: cmd.title }));
        } else {
          say(st("sp_opening_project", { t: cmd.title }));
        }
      } else if (cmd.kind === "open_hq") {
        if (cmd.tab === "board") {
          ensureRail("board");
          say(st("sp_board_screen"));
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
        say(st("sp_found_tasks", { n: cmd.candidates.length }));
      } else if (cmd.kind === "open_settings") {
        // App settings live on the mother window.
        await ipc.focusMain("settings").catch(() => {});
        say("Configurações na janela mãe.");
      } else if (cmd.kind === "compact") {
        // Spoken "compacta o contexto": /compact on the focused session.
        await sendSlash("/compact");
        say(st("sp_compacting"));
      } else if (cmd.kind === "set_mode") {
        selectMode(cmd.mode);
        say(st("sp_mode_changed"));
      } else if (cmd.kind === "project_offer") {
        setPending({ kind: "project-offer", query: cmd.query, candidates: cmd.candidates });
        say(st("sp_found_dirs", { n: cmd.candidates.length }));
      } else if (cmd.kind === "not_found") {
        push({ who: "sys", text: `nada bate com "${cmd.query}"` });
        say("Não achei isso no quadro.");
      } else {
        push({ who: "sys", text: `${cmd.kind}: ${cmd.title}` });
        refresh();
      }
      return;
    }

    // Calling hark by name is the ONE exit from an open chat. Everything
    // else typed into a chat belongs to that chat: you chose the target by
    // opening it. The rule used to be "anything shaped like a question goes
    // to hark", and a real message — "temos muitas coisas para commitar, vc
    // pode verificar e organizar em commits semânticos?" — left the thread
    // because of its final "?": it vanished from the chat on screen and was
    // answered by the global ask, which has no shell and could only offer
    // to dispatch the work back.
    const aside = TO_HARK.exec(text);
    if (aside) text = text.slice(aside[0].length).trim() || text;
    const toHark = !!aside;

    // A fork draft armed by the held banner: this message starts a NEW
    // session seeded with the held one's history and works there.
    if (forkDraft && !toHark) {
      const draft = forkDraft;
      setForkDraft(null);
      push({ who: "user", text, task: draft.title });
      beginTurn(`${draft.title} (fork)`);
      const out = await ipc
        .workerStart(text, draft.sessionId, modeDefault ?? undefined, true, modelDefault || undefined)
        .catch((err) => {
          nudgeRef.current(err);
          push({ who: "sys", text: `fork: ${agentError(err)}` });
          return null;
        });
      if (out?.status === "started") {
        adoptWorker(out.task_id, `${draft.title} (fork)`, out.directives, "");
      } else {
        endTurn(`${draft.title} (fork)`);
        if (out) push({ who: "sys", text: `fork não abriu: ${JSON.stringify(out)}` });
      }
      return;
    }

    // A pending "+" draft: this message opens the new session.
    if (draftChat && !toHark) {
      await runNewChat(draftChat, text);
      return;
    }

    if (focused && !toHark) {
      push({ who: "user", text, images: images.map((i) => i.dataUrl), task: focused && labelFor(focused) });
      beginTurn();
      text = await withCrossref(text, labelFor(focused));
      await ipc
        .workerSend(focused, text, images.map(toImagePair))
        .then((directives) =>
          setLiveWorkers((old) =>
            old[focused] ? { ...old, [focused]: { ...old[focused], directives } } : old,
          ),
        )
        .catch((err) => {
          endTurn(labelFor(focused));
          nudgeRef.current(err);
          push({ who: "sys", text: `worker: ${agentError(err)}` });
        });
      return;
    }
    if (focusedTask && !toHark) {
      // Echo FIRST. Everything below can take a round trip, and a chat
      // that swallows what you typed until the backend answers reads as
      // broken — you cannot even tell whether Enter registered.
      push({
        who: "user",
        text,
        images: images.map((i) => i.dataUrl),
        task: focusedTask.title,
      });
      // NO evaluator here any more. Its job is catching a dispatch that
      // landed in the wrong session — which only exists when the target
      // was INFERRED. Typing into a chat you opened is not a guess: you
      // picked it. Running it here cost real money per message (one turn
      // billed $0.14), delayed every send behind "avaliando…", and read
      // a pasted ArgoCD error as a "meta-complaint about the rename MCP",
      // hijacking the message into the global ask.
      //
      // Worse, its mismatch rule fired on a legitimate, PERMANENT state:
      // a board card the user renamed never matches the session's own
      // auto-generated title, so it would have nagged — and charged —
      // on every message forever.
      //
      // The local prechecks stay: their numbers come from this machine,
      // they cannot hallucinate, and they cost nothing.
      const warnings = await ipc.dispatchPrechecks(focusedTask.sessionId).catch(() => []);
      if (warnings.length > 0) {
        // The modal announces itself out loud and listens for the
        // verdict (the voice loop above) — no extra say() here.
        setPending({
          kind: "confirm-dispatch",
          instruction: text,
          sessionId: focusedTask.sessionId,
          warnings,
        });
        return;
      }
      await sendToFocusedTaskWith(focusedTask.title, focusedTask.sessionId, text, images, false);
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

  /** Model the pill answers for: the focused task's directive, else the
   *  window default. What it SHOWS prefers the last turn's real model. */
  const currentModel =
    (focused ? liveWorkers[focused]?.directives.model : undefined) ?? modelDefault;
  const liveModel = focused ? liveWorkers[focused]?.model : undefined;

  useEffect(() => {
    ipc
      .configRead()
      .then((snap) => {
        const v = snap.values as { models?: Partial<ModelTiers>; model?: string };
        const m = v.models ?? {};
        setTiers({
          light: m.light || "haiku",
          standard: m.standard || v.model || "sonnet",
          heavy: m.heavy || "opus",
          max: m.max || "fable",
        });
      })
      .catch(() => {});
  }, []);

  function selectModel(model: string) {
    const taskId = focused;
    if (taskId && liveWorkers[taskId]) {
      ipc
        .workerSetModel(taskId, model)
        .then((out) => {
          setLiveWorkers((old) =>
            old[taskId] ? { ...old, [taskId]: { ...old[taskId], directives: out.directives } } : old,
          );
        })
        .catch((err) => push({ who: "sys", text: `modelo: ${err}`, task: labelFor(taskId) }));
      return;
    }
    setModelDefault(model);
  }

  /** Mode the pill shows: the focused live task's, else the window/config default. */
  const currentMode =
    (focused ? liveWorkers[focused]?.directives.mode : undefined) ??
    modeDefault ??
    (overview?.default_mode || "manual");

  /** Modes that mean "stop asking me": relaxing one mid-turn is answered
   *  by the window itself, since the process keeps its flags until it
   *  reopens. Restrictive modes only ever wait for the reopen. */
  const RELAXED = new Set(["auto", "bypassPermissions"]);

  /**
   * Selector change: a focused live worker switches mode (process restart
   * on the same session — flags are per-process, no text is sent); with
   * nothing focused it becomes this window's default.
   *
   * A turn in flight is never interrupted for it. Switching to "auto"
   * mid-turn used to shut the process down under a running tool call and
   * take the whole turn — seven tool calls — with it.
   */
  function selectMode(flag: string) {
    const taskId = focused;
    if (taskId && liveWorkers[taskId]) {
      const label = labelFor(taskId);
      ipc
        .workerSetMode(taskId, flag)
        .then((out) => {
          setLiveWorkers((old) =>
            old[taskId] ? { ...old, [taskId]: { ...old[taskId], directives: out.directives } } : old,
          );
          if (out.restarted) {
            allowAlways.current.get(label)?.delete(ANY_TOOL);
            return;
          }
          // The turn keeps running with the old flags, and it stays that
          // way: forcing a restart when the turn lands would re-cache the
          // whole context for nothing. The window answering in the CLI's
          // place is free, and identical from where the user sits.
          const rules = allowAlways.current.get(label) ?? new Set<string>();
          if (RELAXED.has(flag)) rules.add(ANY_TOOL);
          else rules.delete(ANY_TOOL);
          allowAlways.current.set(label, rules);
          if (RELAXED.has(flag)) {
            const answered = approveOpenAsks(label);
            push({
              who: "sys",
              text:
                `modo ${flag}: as permissões desta task passam sem perguntar` +
                (answered ? ` (${answered} liberada${answered > 1 ? "s" : ""})` : "") +
                " — produção continua pedindo confirmação",
              task: label,
            });
          } else {
            push({
              who: "sys",
              text: `modo ${flag} vale a partir da próxima abertura da thread`,
              task: label,
            });
          }
        })
        .catch((err) => push({ who: "sys", text: `modo: ${err}` }));
      return;
    }
    setModeDefault(flag);
    push({ who: "sys", text: `novas tasks desta janela nascem no modo ${flag}` });
  }

  /** Answer every permission still open in a thread. Returns how many. */
  function approveOpenAsks(label: string): number {
    const open = messages.filter(
      (m) => m.who === "permission" && !m.decision && m.task === label && !m.prodRisk,
    );
    for (const ask of open) {
      if (ask.who === "permission") void answerPermission(ask.requestId, true);
    }
    return open.length;
  }

  const railHas = (id: RailItem) => rail.some((s) => s.id === id);

  /** Open (or re-expand) a rail panel; a 3rd expanded one collapses the
   *  oldest other expanded panel instead of killing anything. */
  function ensureRail(id: RailItem) {
    // The terminal opens on a REAL shell. It used to land on the feed,
    // which is a log of the agent's tool calls — and on a fresh thread
    // that is "(sem eventos ainda)": a dead panel where the user asked
    // for a prompt. The feed stays a tab, it just stops being the door.
    if (id === "terminal") {
      if (shells.length === 0) addShell();
      else setTermTab((tab) => (tab === "feed" ? shells[0] : tab));
    }
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
  /** A turn starts: its clock and phase start with it. Defaults to the
   *  thread on screen; voice and follow-ups name another one. */
  function beginTurn(thread: string | null = currentThread()) {
    const key = thread ?? GENERAL;
    setTurns((old) => ({
      ...old,
      [key]: { startedAt: Date.now(), phase: { kind: "thinking" }, chars: 0 },
    }));
  }

  /** Same turn, new thread: a dispatch from the general chat creates the
   *  task it runs in, so the clock moves over keeping its start time. */
  function rekeyTurn(from: string, to: string) {
    if (from === to) return;
    setTurns((old) => {
      const running = old[from];
      if (!running || old[to]) return old;
      const next = { ...old };
      delete next[from];
      next[to] = running;
      return next;
    });
  }

  /** A turn ended: drop its clock. Called with whatever names it — the
   *  event's label, the worker's registered label, or both. */
  function endTurn(label: string | null, taskId?: string) {
    const keys = [label, taskId ? labelFor(taskId) : null].filter(
      (k): k is string => k != null,
    );
    if (keys.length === 0) return;
    setTurns((old) => {
      const next = { ...old };
      let hit = false;
      for (const k of keys) {
        if (k in next) {
          delete next[k];
          hit = true;
        }
      }
      return hit ? next : old;
    });
  }

  /**
   * Put the focused chat's resume command on a shell prompt — and stop
   * there. No Enter: the line is editable, and until it runs nothing is
   * held, so there is no state to unwind if you change your mind. What
   * locks the chat is the session EXISTING, which Hark detects; this
   * button is convenience, never the source of truth.
   */
  function pasteResume() {
    const session = focusedTask?.sessionId;
    if (!session) return;
    runInTerminal(`claude --resume ${session}`, false);
  }

  runInTerminalRef.current = runInTerminal;
  function runInTerminal(cmd: string, execute: boolean) {
    ensureRail("terminal");
    const existing = shells.includes(termTab) ? termTab : shells[0];
    const target = existing ?? addShell();
    if (existing) setTermTab(existing);
    const payload = cmd.replace(/\s+$/, "") + (execute ? "\r" : "");
    // A fresh shell needs a beat to spawn and print its prompt — and a
    // swallowed write is a phantom click, so failure retries once and
    // then says so instead of pretending.
    const write = (attempt: number) => {
      ipc.termWrite(target, payload).catch((err) => {
        if (attempt === 0) setTimeout(() => write(1), 600);
        else push({ who: "sys", text: `terminal: ${err}` });
      });
    };
    setTimeout(() => write(0), existing ? 120 : 700);
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
    if (heldBy) {
      push({
        who: "sys",
        text: `a voz mandaria mensagem pra esta sessão, e ela está no terminal (${heldBy.name})`,
        task: focusedTask?.title,
      });
      say("Essa sessão está aberta no terminal. Fecha lá ou abre um fork.");
      return;
    }
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
    // TWO turns run from here — the compaction and then the message —
    // and this was the last path with no clock at all: the remedy the
    // heavy-session warning offers looked exactly like a dead window.
    const thread = currentThread() ?? GENERAL;
    beginTurn(thread);
    compacting.current.add(thread);
    const liveEntry =
      focusedTask && sessionId === focusedTask.sessionId
        ? Object.entries(liveWorkers).find(([, w]) => w.label === focusedTask.title)
        : undefined;
    if (liveEntry) {
      await ipc.workerSend(liveEntry[0], "/compact", []).catch(() => {});
      await ipc.workerSend(liveEntry[0], instruction, []).catch((err) => {
        endTurn(thread);
        nudgeRef.current(err);
          push({ who: "sys", text: `worker: ${agentError(err)}` });
      });
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
      await ipc.workerSend(out.task_id, instruction, []).catch((err) => {
        endTurn(label);
        nudgeRef.current(err);
          push({ who: "sys", text: `worker: ${agentError(err)}` });
      });
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
          ? p.sessionId && focusedTask
            ? st("sp_shown_confirm", { t: focusedTask.title })
            : st("sp_shown_confirm_bare")
          : st("sp_found_options", { n: labels.length });
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
            say(st("sp_compact_first"));
            compactFirstRef.current(instruction, sessionId);
          }
          return;
        }
        if (verdict.kind === "deny") {
          setPending(null);
          say(st("sp_cancelled"));
          return;
        }
        if (cur.kind === "confirm-dispatch") {
          if (verdict.kind === "confirm") {
            push({ who: "user", text: heard });
            confirmDispatchRef.current();
            say(st("sp_dispatching"));
            return;
          }
          if (verdict.kind === "instruction") {
            // The user rephrased (STT got words wrong): swap the message
            // and ask again — the textarea shows the new text.
            setPending({ ...cur, instruction: verdict.text });
            await ipc.speak(st("sp_swapped_confirm")).catch(() => {});
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
      // Abort only OUR orphan turn — never one the HUD owns. Abort, not
      // stop: nobody reads this transcription, and on CPU it would hold
      // the mic lease for many seconds after the modal died.
      if (capturing) ipc.hearAbort().catch(() => {});
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
  /** "há 6 min" from an epoch-ms timestamp; blank when unreported. */
  function sinceLabel(startedAt?: number | null): string {
    if (!startedAt) return "";
    const mins = Math.max(0, Math.round((Date.now() - startedAt) / 60000));
    if (mins < 1) return "agora";
    if (mins < 60) return `há ${mins} min`;
    return `há ${Math.floor(mins / 60)}h${String(mins % 60).padStart(2, "0")}`;
  }

  /** A log entry as a chat message. The two loaders share it so a paged
   *  page looks exactly like the first one. */
  function entryToMsg(e: TranscriptEntry, title: string): Msg | null {
    if (e.role === "user") return { who: "user", text: e.text, task: title };
    if (e.role === "assistant") return { who: "hark", text: e.text, task: title };
    if (e.role === "tool_use")
      return { who: "tool", name: e.tool ?? "tool", input: e.text, task: title };
    if (e.role === "tool_result")
      return { who: "output", content: e.text, error: e.is_error, task: title };
    if (e.role === "compaction") return { who: "compact", text: e.text, task: title };
    return null;
  }

  /** Identity of a log entry, for finding where the loaded window starts
   *  again in a longer read (the file may have grown meanwhile). */
  const entryKey = (e: TranscriptEntry) => `${e.ts}|${e.role}|${e.text.slice(0, 80)}`;

  /**
   * Older history, on demand: re-read a longer tail and splice in only
   * what is new, ABOVE the thread's first message. Live messages of other
   * threads sit elsewhere in the array and never move.
   */
  async function loadOlderHistory() {
    const task = focusedTask;
    const state = task ? history.current.get(task.title) : undefined;
    if (!task || !state || historyDone[task.title]) return;
    const want = state.count + HISTORY_PAGE;
    let out: { entries: TranscriptEntry[] };
    try {
      out = await ipc.readTranscript(task.sessionId, want);
    } catch (err) {
      push({ who: "sys", text: `histórico: ${err}`, task: task.title });
      return;
    }
    const at = out.entries.findIndex((e) => entryKey(e) === state.firstKey);
    const older = at > 0 ? out.entries.slice(0, at) : [];
    if (older.length === 0) {
      setHistoryDone((old) => ({ ...old, [task.title]: true }));
      push({ who: "sys", text: "começo do histórico", task: task.title });
      return;
    }
    const msgs = older.map((e) => entryToMsg(e, task.title)).filter(Boolean) as Msg[];
    setMessages((old) => {
      const first = old.findIndex((m) => (m.task ?? null) === task.title);
      return first < 0 ? [...msgs, ...old] : [...old.slice(0, first), ...msgs, ...old.slice(first)];
    });
    history.current.set(task.title, {
      count: out.entries.length,
      firstKey: entryKey(out.entries[0]),
    });
    if (out.entries.length < want) {
      setHistoryDone((old) => ({ ...old, [task.title]: true }));
    }
  }

  async function focusTask(title: string, sessionId: string, note?: string) {
    setDraftChat(null);
    setFocusedTask({ title, sessionId, note });
    if (loadedTasks.current.has(title)) {
      // Built once already — but the session may have moved on since, in a
      // terminal or in another window. Catch up on whatever the log grew.
      const at = mirror.current.get(title);
      if (at != null) {
        ipc
          .transcriptSince(sessionId, at)
          .then((out) => {
            mirror.current.set(title, out.offset);
            for (const e of out.entries) {
              const msg = entryToMsg(e, title);
              if (msg) push(msg);
            }
          })
          .catch(() => {});
      }
      return;
    }
    loadedTasks.current.add(title);
    try {
      const out = await ipc.readTranscript(sessionId, HISTORY_TAIL).catch(async (err) => {
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
        return ipc.readTranscript(hit.session_id, HISTORY_TAIL);
      });
      // Results used to be dropped, so a reopened chat lost every ✓/✗ and
      // read differently from the same chat live.
      for (const e of out.entries) {
        const msg = entryToMsg(e, title);
        if (msg) push(msg);
      }
      mirror.current.set(title, out.offset);
      if (out.entries[0]) {
        history.current.set(title, {
          count: out.entries.length,
          firstKey: entryKey(out.entries[0]),
        });
        if (out.entries.length < HISTORY_TAIL) {
          setHistoryDone((old) => ({ ...old, [title]: true }));
        }
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

  /** "…o que decidimos no chat do X": pull the cited chat's lines from the
   *  local log and ride them inside the message — visible, attributed,
   *  zero tokens. Returns the message to actually send. */
  async function withCrossref(text: string, thread?: string): Promise<string> {
    const ref = await ipc.crossrefContext(text).catch(() => null);
    if (!ref) return text;
    push({
      who: "sys",
      text: `puxei ${ref.lines} linha${ref.lines > 1 ? "s" : ""} do chat "${ref.title}"`,
      task: thread,
    });
    return `${text}\n\n${ref.block}`;
  }

  /** Send a message into a task: live worker if any, else a fresh resume. */
  async function sendToFocusedTaskWith(
    title: string,
    sessionId: string,
    text: string,
    images: Attachment[] = [],
    /** The submit path echoes before it starts, so it opts out here. */
    echo = true,
  ) {
    // Slash commands, spoken follow-ups and compact-then-send all land
    // here. "/compact" showed no clock at all because only the two typed
    // paths above started one.
    beginTurn(title);
    await reactivateIfDone(title);
    const liveEntry = Object.entries(liveWorkers).find(([, w]) => w.label === title);
    if (echo) push({ who: "user", text, images: images.map((i) => i.dataUrl), task: title });
    text = await withCrossref(text, title);
    if (liveEntry) {
      await ipc
        .workerSend(liveEntry[0], text, images.map(toImagePair))
        .then((directives) =>
          setLiveWorkers((old) => ({
            ...old,
            [liveEntry[0]]: { ...old[liveEntry[0]], directives },
          })),
        )
        .catch((err) => {
          endTurn(title);
          nudgeRef.current(err);
          push({ who: "sys", text: `worker: ${agentError(err)}` });
        });
      return;
    }
    await runDispatch(text, sessionId);
  }

  function startDraftChat(project: Project) {
    setDraftChat(project);
    leaveChat();
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

  /** Sidebar click: open the task. Always — clicking the chat you are
   *  already in never takes you somewhere else. */
  const openTaskFromSidebar = (t: BoardTask) =>
    focusedTask?.title === t.title
      ? Promise.resolve()
      : openTaskByTitle(t.title, t.session_ids.at(-1), t.note);

  /** Back to the window's general chat with hark (the sidebar's own row).
   *  Refs first: state arrives a render later and push() reads the refs. */
  function leaveChat() {
    setForkDraft(null);
    focusedRef.current = null;
    focusedTaskRef.current = null;
    setFocusedTask(null);
    setFocused(null);
  }

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
    say(st("sp_resuming", { t: task.title }));
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
          onResumeSession={focusedTask?.sessionId ? pasteResume : undefined}
          resumeSpent={!!heldBy}
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
        onSubtask={(title, index, done) => ipc.boardSubtaskToggle(title, index, done).then(refresh).catch(() => {})}
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

  const placeholder = forkDraft
    ? t("composer_fork", { name: forkDraft.title })
    : heldBy
      ? t("composer_held")
      : draftChat
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
            onDetail={() => {
              void ipc
                .usageReport(focusedTask?.sessionId ?? undefined)
                .then((report) =>
                  push({
                    who: "usage",
                    report,
                    task: focused ? labelFor(focused) : focusedTask?.title,
                  }),
                )
                .catch(() => {});
            }}
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
      <span
        className="composer-orb"
        title={
          busy ??
          (recording ? "ouvindo…" : transcribing ? "transcrevendo… (Esc cancela)" : "pronto")
        }
      >
        <VoiceOrb
          mode={
            (recording
              ? "listening"
              : speaking
                ? "speaking"
                : transcribing || busy
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
        <PanelGroup direction="horizontal" autoSaveId="hark-code" className="workarea">
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
              onOpenGeneral={leaveChat}
              generalActive={!focusedTask && !draftChat}
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
              {/* The thread says what it is: a slim title over the prose,
                  like the Claude Code app — controls stay on the composer. */}
              {!reading && focusedTask && visibleMessages.length > 0 && (
                <div className="chat-title-head">
                  <span className="chat-title">{focusedTask.title}</span>
                  <span className="chat-turns">
                    {t("n_turns_of", { n: visibleMessages.filter((m) => m.who === "user").length })}
                  </span>
                  <span className="chat-head-gap" />
                  {/* Both numbers are already in memory — no query per render:
                      the cost this window watched accumulate, and the context
                      the last turn reported. */}
                  {(costs[focusedTask.title] ?? 0) > 0 && (
                    <span className="chat-spend">${costs[focusedTask.title].toFixed(2)}</span>
                  )}
                  {focused && liveWorkers[focused]?.context_pct != null && (
                    <ContextRing used={liveWorkers[focused]!.context_pct!} />
                  )}
                </div>
              )}
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
              ) : visibleMessages.length === 0 ? (
                <EmptyProject
                  project={activeProject}
                  board={board}
                  onResume={openTaskFromSidebar}
                  onSpeak={onMic}
                  onSearch={() => sidebarRef.current?.expand()}
                  onBoard={() => ensureRail("board")}
                />
              ) : (
                <Transcript
                  messages={visibleMessages}
                  directivesFor={directivesFor}
                  onAnswerPermission={answerPermission}
                  onOpenPath={openAbsolutePath}
                  onRunCommand={runInTerminal}
                  onLoadOlder={
                    focusedTask &&
                    history.current.has(focusedTask.title) &&
                    !historyDone[focusedTask.title]
                      ? loadOlderHistory
                      : undefined
                  }
                />
              )}
              {turns[focusedTask?.title ?? GENERAL] && (
                <TurnStatus state={turns[focusedTask?.title ?? GENERAL]} />
              )}
              {heldBy && (
                <div className="held">
                  <div className="held-head">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="3" y="11" width="18" height="11" rx="2" />
                      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                    </svg>
                    <div>
                      <b>{t("held_title")}</b>
                      <div className="held-why">
                        {t("held_why", {
                          name: heldBy.name,
                          pid: String(heldBy.pid ?? "?"),
                          since: sinceLabel(heldBy.started_at),
                        })}
                      </div>
                    </div>
                  </div>
                  <div className="held-actions">
                    <button
                      onClick={() => {
                        ensureRail("terminal");
                        if (shells[0]) setTermTab(shells[0]);
                        else addShell();
                      }}
                    >
                      {t("held_goto")}
                    </button>
                    <button
                      onClick={() => {
                        setForkDraft({ title: focusedTask?.title ?? "", sessionId: heldBy.session_id });
                        push({ who: "sys", text: t("fork_armed"), task: focusedTask?.title });
                      }}
                    >
                      {t("held_fork")}
                    </button>
                  </div>
                </div>
              )}
              <Composer
                disabled={!!busy || (!!heldBy && !forkDraft)}
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
                banner={focusedRepo ? <RepoRuler state={focusedRepo} /> : undefined}
              >
                <ModeSelect
                  value={currentMode}
                  appliesTo={focused ? labelFor(focused) : undefined}
                  onSelect={selectMode}
                />
                <ModelSelect
                  value={currentModel ?? ""}
                  liveModel={liveModel}
                  tiers={tiers}
                  appliesTo={focused ? labelFor(focused) : undefined}
                  onSelect={selectModel}
                />
                <WorkerChips
                  liveWorkers={liveWorkers}
                  focused={focused}
                  onToggleFocus={(taskId) => {
                    if (focused === taskId) {
                      leaveChat();
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
                  onRestartLight={(taskId) =>
                    ipc
                      .workerRestartLight(taskId)
                      .catch((err) => push({ who: "sys", text: `recomeço leve: ${err}` }))
                  }
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
                <div className="rail" ref={railRef}>
                  {rail.map((slot, i) => {
                    const prev = rail
                      .slice(0, i)
                      .reverse()
                      .find((s) => !s.collapsed);
                    return (
                      <Fragment key={slot.id}>
                        {i > 0 && !slot.collapsed && prev && (
                          <div
                            className="rail-vhandle"
                            onMouseDown={(e) => startRailResize(e, prev.id, slot.id)}
                          />
                        )}
                        <div
                          className={`rail-slot ${slot.collapsed ? "collapsed" : ""}`}
                          style={
                            slot.collapsed
                              ? undefined
                              : { flexGrow: railFlex[slot.id] ?? 1 }
                          }
                        >
                          {slot.id === "arquivo"
                            ? frameArquivo(slot)
                            : slot.id === "terminal"
                              ? frameTerminal(slot)
                              : slot.id === "board"
                                ? frameBoard(slot)
                                : frameArquivos(slot)}
                        </div>
                      </Fragment>
                    );
                  })}
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
        onPickProject={async (path) => {
          const entry = await ipc.projectAdd(path).catch(() => null);
          if (!entry) {
            push({ who: "sys", text: `não consegui registrar ${path}` });
            return;
          }
          push({ who: "sys", text: `projeto ${entry.name} registrado (${entry.path})` });
          say(st("sp_project_added", { t: entry.name }));
          refresh();
          await ipc.openProjectWindow(entry.name, entry.path).catch(() => {});
        }}
        onCompactFirst={dispatchCompactFirst}
      />
    </div>
  );
}
