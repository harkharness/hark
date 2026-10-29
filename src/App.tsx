import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { keepIfSame } from "./lib/settle";
import {
  Panel,
  PanelGroup,
  PanelResizeHandle,
  type ImperativePanelHandle,
} from "react-resizable-panels";
import { Compass, Copy, Download, ExternalLink, FileText, FolderTree, GitCompare, PanelLeft, Share2, SquareKanban, SquareTerminal, Volume2, VolumeX, Wallet } from "lucide-react";
import { colourIndex, projColour, scopeOf } from "./lib/scope";
import VoiceOrb, { type OrbMode } from "./components/VoiceOrb";
import { setLang, setSpeechLang, st, t } from "./lib/i18n";
import Artifacts, { ArtifactLinks, type ArtSel } from "./components/Artifacts";
import Board, { type CardAsk } from "./components/Board";
import SharePopover, { type ShareRow } from "./components/SharePopover";
import VoiceRound from "./components/VoiceRound";
import { useVoiceRound } from "./hooks/useVoiceRound";
import DiffPanel from "./components/DiffPanel";
import BrowserPanel from "./components/BrowserPanel";
import { transcriptMarkdown } from "./lib/export";
import Composer from "./components/Composer";
import { toImagePair, type Attachment } from "./lib/composerText";
import { FileTabs } from "./components/FilesEditor";
import FilesWindow, { TreeToggle } from "./components/FilesWindow";
import { useFilesTree } from "./hooks/useFilesTree";
import Modals, { type Pending } from "./components/Modals";
import ReportDialog from "./components/ReportDialog";
import ModeSelect from "./components/ModeSelect";
import ModelSelect from "./components/ModelSelect";
import EffortSelect from "./components/EffortSelect";
import PanelFrame from "./components/PanelFrame";
import QuickOpen from "./components/QuickOpen";
import ChatPalette from "./components/ChatPalette";
import Reader from "./components/Reader";
import SessionPanel from "./components/SessionPanel";
import { collectOutputs } from "./lib/outputs";
import Sidebar from "./components/Sidebar";
import TerminalPane, { TerminalResume, TerminalTabs } from "./components/TerminalPane";
import Transcript from "./components/Transcript";
import EmptyProject from "./components/EmptyProject";
import WorkerChips from "./components/WorkerChips";
import TurnStatus, { type TurnState } from "./components/TurnStatus";
import { useWorkerDefaults } from "./hooks/useWorkerDefaults";
import { useAgentCatalog } from "./hooks/useAgentCatalog";
import { bypassFloorFor, modelPillFor, terminalResumeFor, unsupported } from "./lib/support";
import { useHarkEvents } from "./hooks/useHarkEvents";
import { useRepoStates } from "./hooks/useRepoStates";
import RepoRuler from "./components/RepoRuler";
import { ContextRing } from "./components/Meter";
import { agentError, askReplyMsg, firstSentence, isAuthError } from "./lib/format";
import Settings, { type Section as SettingsSection } from "./components/Settings";
import Onboarding from "./components/Onboarding";
import { checkForUpdate, restartIntoUpdate } from "./lib/updater";
import * as ipc from "./lib/ipc";
import { terminalPayload } from "./lib/terminalPayload";
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
  ArtifactPage,
  ArtifactTree,
} from "./types";

type RailItem = "sessao" | "terminal" | "arquivos" | "board" | "artefatos" | "diff" | "browser";

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
  // Mode/model/effort defaults for this window — seeded from config and
  // written back on every pick. Shared with the mother's chat.
  const defaults = useWorkerDefaults(overview ?? null);
  // "abre um chat gemini no hark" with no first task yet: the agent waits
  // with the draft until the first message opens the chat.
  const draftAgent = useRef<string | undefined>(undefined);
  const modeDefault = defaults.mode;
  const modelDefault = defaults.model;
  const effortDefault = defaults.effort;
  // Every Claude Code session of this project (index), newest first.
  const [chats, setChats] = useState<SessionHit[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  // Key for the window's general chat, which has no task title.
  const GENERAL = "";
  /** The persistent general chat's task id on the Rust side. Its events
   *  land in the GENERAL thread of the main window. */
  const HARK_CHAT = "hark-chat";
  /** No forced project = the main window: every project, the general chat
   *  backed by the persistent worker, board/costs/settings as views. */
  const isMain = !forcedProject;
  type View = "chat" | "board" | "ajustes" | "artefatos";
  const [view, setView] = useState<View>("chat");
  // The artifacts view: the space and page on screen, the page's data
  // (for the composer's scope and the rail) and a tick that re-reads the
  // tree when an agent writes a page.
  const [artSel, setArtSel] = useState<ArtSel | null>(null);
  const [artPage, setArtPage] = useState<ArtifactPage | null>(null);
  const [artTree, setArtTree] = useState<ArtifactTree | null>(null);
  const [artTick, setArtTick] = useState(0);
  const [artCount, setArtCount] = useState(0);
  const onArtPage = useCallback((page: ArtifactPage | null, tree: ArtifactTree | null) => {
    setArtPage(page);
    setArtTree(tree);
  }, []);
  useEffect(() => {
    if (!isMain) return;
    ipc
      .artifactsSpaces()
      .then((spaces) => setArtCount(spaces.reduce((a, s) => a + s.pages, 0)))
      .catch(() => {});
  }, [isMain, artTick, overview]);
  const [settingsSection, setSettingsSection] = useState<SettingsSection>("geral");
  /** Settings on a given section (costs is one of them). */
  const openSettings = (section: SettingsSection = "geral") => {
    setSettingsSection(section);
    setView("ajustes");
  };
  const [setup, setSetup] = useState<ipc.SetupStatus | null>(null);
  const [updateReady, setUpdateReady] = useState<string | null>(null);
  /** The last typed message in flight on the general chat, for the one
   *  resend after a worker crash. */
  const chatInFlightRef = useRef<{ text: string; retried: boolean } | null>(null);
  /** A finished chat elsewhere offers "faz algo lá": the NEXT sentence
   *  may answer it. */
  const followUpRef = useRef<{ label: string; sessionId: string } | null>(null);
  // What each running turn is doing, keyed by thread — the status line
  // belongs to ONE chat, not to the window. A single global one showed
  // "1m 35s · quase terminando de pensar…" above every chat you opened,
  // and the first worker to finish wiped the clock of every other.
  const [turns, setTurns] = useState<Record<string, TurnState>>({});
  const [recording, setRecording] = useState(false);
  /** Sidebar rows pulsing after a voice dispatch landed on them. */
  const [pulsing, setPulsing] = useState<string[]>([]);
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
  // Sessions whose heavy-session warning was answered "go ahead": the
  // user said no to compacting, and a compaction they refused is not
  // for the window to run anyway (25/09).
  const declinedCompact = useRef<Set<string>>(new Set());
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
  // Which agent answers the focused task: the owner of its session, or the
  // selected default for a chat that has no session yet. Asked of the
  // driver on focus change, corrected by the turn stream once the worker
  // speaks. Every control that needs a capability asks the sheet first,
  // so a feature this plugin lacks shows as disabled-with-a-reason.
  const { catalog, selected: selectedAgent } = useAgentCatalog();
  // The turn handler below is a stable callback: it reads the catalog
  // through a ref so a chat that opened before the sheet arrived still
  // sees the sheet when its context fills up.
  const catalogRef = useRef(catalog);
  useEffect(() => {
    catalogRef.current = catalog;
  }, [catalog]);
  const [agentBySession, setAgentBySession] = useState<Record<string, string>>({});
  const focusedSession = focusedTask?.sessionId ?? "";
  useEffect(() => {
    let live = true;
    ipc
      .agentForSession(focusedSession)
      .then((id) => {
        if (!live) return;
        setAgentBySession((old) => (old[focusedSession] === id ? old : { ...old, [focusedSession]: id }));
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [focusedSession]);
  const focusedAgent =
    (focused ? liveWorkers[focused]?.agent ?? undefined : undefined) ??
    agentBySession[focusedSession] ??
    selectedAgent;
  const modeOff = unsupported(catalog, focusedAgent, "directive_mode", t("cap_mode"));
  const modelOff = unsupported(catalog, focusedAgent, "directive_model", t("cap_model"));
  // What THIS chat's agent calls each tier; the pill speaks tiers, the
  // driver translates. An ACP agent with no table gets the pill disabled
  // with the config knob as the reason, never claude's names.
  const modelPill = modelPillFor(catalog, focusedAgent, defaults.tiers);
  const effortOff = unsupported(catalog, focusedAgent, "directive_effort", t("cap_effort"));
  const forkOff = unsupported(catalog, focusedAgent, "fork", t("held_fork"));
  const ringOff = unsupported(catalog, focusedAgent, "cost_reporting", t("cap_context"));
  const [speak, setSpeak] = useState(true);
  // "Arquivos" window: up to 5 tabs, LRU-evicted, dirty tabs protected.
  const [openFiles, setOpenFiles] = useState<OpenFile[]>([]);
  const [activeFile, setActiveFile] = useState(0);
  const [dirtyPaths, setDirtyPaths] = useState<Set<string>>(new Set());
  const lastFocus = useRef(new Map<string, number>());
  // ...and its tree column (project trees + filter), open or folded.
  const [filesInitialProject, setFilesInitialProject] = useState<string | undefined>();
  const filesTree = useFilesTree();
  /** Typed window taking the whole work area (menu stays). */
  const [expanded, setExpanded] = useState<RailItem | null>(null);
  const [quickOpen, setQuickOpen] = useState(false);
  /** The history palette (Cmd+K, or the magnifier in the sidebar head). */
  const [chatPalette, setChatPalette] = useState(false);
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

  /** Window's model choice ("" = auto/router); focused live tasks switch
   *  the process, otherwise it seeds new chats born here. */


  // Per-thread raw worker feed (the task's "terminal") and window spend.
  /** The worker's raw event lines. Its only reader was the terminal's
   *  feed tab, which is gone (the transcript already shows tool calls),
   *  so nothing accumulates here — the events hook still reports and the
   *  sink stays for a future log viewer, if one is ever wanted. */
  const [costs, setCosts] = useState<Record<string, number>>({});
  /**
   * THE RAIL: chat is the anchor; every typed window lives stacked on the
   * right, collapsible to its title bar, draggable to reorder. Opening a
   * third expanded panel collapses the oldest — nothing ever dies, it
   * shrinks. Order in the array = order on screen.
   */
  // ONE panel at a time (phase 10.2): the rail shows the panel named
  // here or nothing at all, and the choice survives a restart.
  const RAIL_KEY = "hark.rail";
  const [railOn, setRailOn] = useState<RailItem | null>(() => {
    try {
      const saved = window.localStorage.getItem(RAIL_KEY);
      return saved === "sessao" || saved === "arquivos" || saved === "board" || saved === "artefatos" || saved === "diff" || saved === "browser"
        ? saved
        : null;
    } catch {
      return null;
    }
  });
  useEffect(() => {
    try {
      // A terminal is a process, not a view: it is never reopened blind.
      if (railOn && railOn !== "terminal") window.localStorage.setItem(RAIL_KEY, railOn);
      else window.localStorage.removeItem(RAIL_KEY);
    } catch {
      /* private window */
    }
  }, [railOn]);

  // Real shell tabs (PTY ids). Owned here so hiding the pane keeps them.
  const [shells, setShells] = useState<string[]>([]);
  const [termTab, setTermTab] = useState<string>("");
  const shellSeq = useRef(1);
  // Mirrors the sidebar panel's collapsed state (topbar toggle icon).
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [speaking, setSpeaking] = useState(false);
  const [rateLimit, setRateLimit] = useState<import("./types").RateLimitState | null>(null);
  /** Timestamp of the last Esc, for the double-Esc worker abort. */

  const workersRef = useRef(liveWorkers);
  workersRef.current = liveWorkers;
  const focusedTaskRef = useRef(focusedTask);
  focusedTaskRef.current = focusedTask;
  const focusedRef = useRef(focused);
  focusedRef.current = focused;
  // stopWorker is declared below (hoisted); the ref keeps the Esc handler fresh.
  const stopWorkerRef = useRef<(taskId: string) => void>(() => {});
  stopWorkerRef.current = stopWorker;
  const interruptRef = useRef<(taskId: string) => void>(() => {});
  interruptRef.current = interruptWorker;
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
    (m: Msg) => {
      // ONCE: asCompaction consumes the "we asked for it" flag, so calling
      // it a second time would see a compaction as ordinary prose.
      const folded = asCompaction(m);
      // A compaction makes every context reading obsolete. The turn that
      // DID the compacting carries the whole pre-compaction prompt — the
      // largest the session ever was — so leaving it on the ring shows a
      // session at its fullest at the exact moment it got emptied. The
      // new size is unknown until the next turn reports one, and showing
      // nothing is the honest version of not knowing.
      if (folded.who === "compact") {
        setLedgerCtx(null);
        const thread = m.task ?? currentThread() ?? "";
        setLiveWorkers((old) => {
          const id = Object.keys(old).find((k) => old[k].label === thread);
          return id ? { ...old, [id]: { ...old[id], context_pct: null } } : old;
        });
      }
      return setMessages((old) => [
        ...old,
        {
          ...folded,
          // An untagged message belongs to the general chat, so with a task
          // focused it is pushed straight out of view. Whatever is added
          // while a chat is open belongs to that chat unless the caller
          // named another thread.
          ...(m.task == null ? { task: currentThread() ?? undefined } : {}),
          ...((m.who === "user" || m.who === "hark") && m.ts == null
            ? { ts: Date.now() }
            : {}),
        },
      ]);
    },
    [currentThread],
  );
  /** The bubble and the backend's outbox entry share an id, so a message
   *  can say on screen that it has not run yet. `undefined` clears it:
   *  the queue let it go and it is an ordinary message again. */
  const markQueued = useCallback(
    (msgId: string, queued?: { state: "waiting" | "failed"; why?: string }) => {
      setMessages((old) =>
        old.map((m) => (m.who === "user" && m.msgId === msgId ? { ...m, queued } : m)),
      );
    },
    [],
  );

  /** Send to a live worker. Busy workers park the message instead of
   *  queueing it invisibly on stdin — the bubble shows it waiting, and
   *  `held` is how we learn that happened. */
  const sendQueued = useCallback(
    async (
      taskId: string,
      text: string,
      msgId: string,
      images: [string, string][] = [],
    ) => {
      const out = await ipc.workerSend(taskId, text, images, msgId);
      if (out.queued) markQueued(msgId, { state: "waiting" });
      setLiveWorkers((old) =>
        old[taskId] ? { ...old, [taskId]: { ...old[taskId], directives: out.directives } } : old,
      );
    },
    [markQueued],
  );

  /** The worker holding a message is the one whose label the bubble
   *  carries; an untagged bubble belongs to the thread in view. */
  const workerFor = useCallback(
    (taskLabel?: string) => {
      const want = taskLabel ?? currentThread();
      return Object.entries(liveWorkers).find(([, w]) => w.label === want)?.[0];
    },
    [liveWorkers, currentThread],
  );

  const queuedNow = useCallback(
    (msgId: string, taskLabel?: string) => {
      const taskId = workerFor(taskLabel);
      if (!taskId) return;
      ipc.workerQueuedNow(taskId, msgId).catch((err) => {
        push({ who: "sys", text: t("sys_queue_err", { e: agentError(err) }) });
      });
    },
    [workerFor, push],
  );

  const queuedDrop = useCallback(
    (msgId: string, taskLabel?: string) => {
      const taskId = workerFor(taskLabel);
      if (!taskId) return;
      ipc
        .workerQueuedDrop(taskId, msgId)
        // It never ran and never will: the bubble goes with it.
        .then(() =>
          setMessages((old) =>
            old.filter((m) => !(m.who === "user" && m.msgId === msgId)),
          ),
        )
        .catch((err) => push({ who: "sys", text: t("sys_queue_err", { e: agentError(err) }) }));
    },
    [workerFor, push],
  );

  /** Turn an agent reply that is really a compaction summary into the
   *  folded kind. Live, the only tell is that we asked for it; a session
   *  that compacted itself is caught by the CLI's fixed opening line. */
  const asCompaction = useCallback((m: Msg): Msg => {
    if (m.who !== "hark") return m;
    const thread = m.task ?? currentThread() ?? "";
    const asked = compacting.current.delete(thread);
    const isSummary = COMPACTION_HEAD.test(m.text);
    if (!asked && !isSummary) return m;
    // We asked and what came back is not a summary: the agent declined.
    // Folded as a "compaction" it became a bare English sentence sitting
    // where a summary should be ("Not enough messages to compact."), which
    // reads like the compaction happened and produced that. It is a
    // refusal, and it says so as one.
    if (asked && !isSummary && m.text.trim().length < 200) {
      return { who: "sys", text: t("sys_compact_fail", { e: m.text.trim() }), task: m.task };
    }
    return { who: "compact", text: m.text, task: m.task };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Thread key for worker events: the task LABEL, stable and readable. */
  const labelFor = useCallback(
    (taskId: string) => (taskId === HARK_CHAT ? GENERAL : (workersRef.current[taskId]?.label ?? taskId)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
  // The main window lists every project's history: one query per
  // registered project, merged. Refreshed with the project list.
  const projectPaths = (overview?.projects ?? []).map((p) => p.path).join("\n");
  useEffect(() => {
    if (!isMain || !projectPaths) return;
    let alive = true;
    Promise.all(projectPaths.split("\n").map((path) => ipc.projectSessions(path).catch(() => [] as SessionHit[])))
      .then((lists) => {
        if (alive) setChats(lists.flat());
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectPaths]);

  // Main window only: the first-run gate, the update poll and the general
  // chat's stored record (the mother's three boot duties).
  useEffect(() => {
    if (!isMain) return;
    ipc
      .setupStatus()
      .then((st) => {
        // Any usable backend counts: a machine with only gemini or codex
        // must not be sent to a wizard asking for claude.
        if (!st.onboarded && (!st.config_exists || !st.whisper_ok || !st.agent_ok)) setSetup(st);
      })
      .catch(() => {});
    let alive = true;
    const poll = async () => {
      const cfg = await ipc.configRead().catch(() => null);
      if (!cfg?.values.auto_update) return;
      const up = await checkForUpdate();
      if (alive && up.kind === "ready") setUpdateReady(up.version);
    };
    void poll();
    const id = setInterval(poll, 6 * 60 * 60 * 1000);
    // The general chat is CONTINUOUS across restarts, visually too: the
    // journal (ask turns) and the hark-chat transcript (work turns), one
    // timeline. Local, zero tokens.
    void (async () => {
      const [stt, journal] = await Promise.all([
        ipc.harkChatStatus().catch(() => null),
        ipc.journalRecent(12).catch(() => [] as import("./types").JournalTurn[]),
      ]);
      const hist: (Msg & { ts?: number })[] = [];
      for (const turn of journal) {
        const ts = Date.parse(turn.ts) || undefined;
        hist.push({ who: "user", text: turn.question, ts });
        hist.push({ who: "hark", text: turn.fala, detalhes: turn.body || undefined, ts });
      }
      if (stt?.session_id) {
        const tr = await ipc.readTranscript(stt.session_id, 30).catch(() => null);
        for (const e of tr?.entries ?? []) {
          if (e.role !== "user" && e.role !== "assistant") continue;
          hist.push({ who: e.role === "user" ? "user" : "hark", text: e.text, ts: Date.parse(e.ts) || undefined });
        }
      }
      hist.sort((x, y) => (x.ts ?? 0) - (y.ts ?? 0));
      if (hist.length && alive) setMessages((old) => [...hist, ...old].slice(-80));
    })();
    return () => {
      alive = false;
      clearInterval(id);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const say = useCallback((text: string) => {
    if (speakRef.current) ipc.speak(text).catch(() => {});
  }, []);
  const pushRaw = useCallback((_label: string, _line: string) => {}, []);
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
          // Same answer → same reference → no render (lib/settle.ts).
          if (alive) setOwners(keepIfSame(list));
        })
        .catch(() => {});
    tick();
    // One period of the Rust side's cache (session_owners): every window
    // asking within it shares ONE `claude agents` spawn.
    const id = setInterval(tick, 3000);
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
      push({ who: "sys", text: t("sys_wheel_to_terminal", { name: heldBy.name }), task: title });
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
            text: t("sys_wheel_back_n", { n: out.entries.length }),
            task: title,
          });
        })
        .catch(() => push({ who: "sys", text: t("sys_wheel_back"), task: title }));
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

  // "Reportar um problema": the sidebar's bug button and the native menu
  // both land here.
  const [reportOpen, setReportOpen] = useState(false);

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
        session?: string | null,
        errorCode?: string | null,
        _model?: string | null,
        agent?: string | null,
      ) => {
        endTurn(label, taskId);
        // Auth death is fatal and identical on retry: the remedy is the
        // terminal, so open it with `claude` typed (once per window).
        if (errorCode === "agent_auth") nudgeRef.current("agent_auth:turn");
        if (taskId === HARK_CHAT) {
          // The reply landed (or failed as an answer): nothing in flight.
          chatInFlightRef.current = null;
          return;
        }
        // A chat finished elsewhere: the next sentence in the general chat
        // may answer the offer. Failures announce but never offer.
        if (isMain && !_err && session) followUpRef.current = { label, sessionId: session };
        // FASE 8.6: act on context pressure instead of only warning. At
        // 85% the next turns degrade and a forced summary is coming
        // anyway; compacting NOW is the cheap version of that. Once per
        // climb — the flag re-arms when the window drops back.
        // Never on a reading past 100%: that is a window we were told
        // wrong, not a full one, and a 1M-token compaction is the most
        // expensive turn there is (30491% set one off, 25/09). Never
        // either on a session whose user already said no to it.
        const refused = !!session && declinedCompact.current.has(session);
        if (taskId && contextPct != null && contextPct <= 1 && !refused) {
          if (contextPct >= 0.85 && !autoCompacted.current.has(taskId)) {
            autoCompacted.current.add(taskId);
            // "/compact" is claude's; an ACP agent has it only when it
            // announced it. Sent elsewhere it is a paid, useless turn.
            const pct = Math.round(contextPct * 100);
            void (async () => {
              const sheet = agent ? catalogRef.current[agent] : undefined;
              if (sheet && sheet.plugin !== "claude") {
                const cmds = await ipc.slashCommands(forcedProject?.path).catch(() => [] as string[]);
                if (!cmds.some((c) => c.replace(/^\//, "") === "compact")) {
                  push({ who: "sys", text: t("no_compact_here", { name: sheet.name }), task: label });
                  return;
                }
              }
              compacting.current.add(label);
              push({ who: "sys", text: t("sys_auto_compact", { pct }), task: label });
              beginTurn(label);
              ipc.workerSend(taskId, "/compact", []).catch(() => endTurn(label));
            })();
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
    // The meter ticking under the status line: what this turn has cost so
    // far, as the backend prices its calls. Null dollars = unpriced.
    onTurnSpend: useCallback(
      (thread: string, usd: number | null) =>
        setTurns((old) => {
          const running = old[thread] ?? {
            startedAt: Date.now(),
            phase: { kind: "thinking" as const },
            chars: 0,
          };
          return { ...old, [thread]: { ...running, usd } };
        }),
      [],
    ),
    onRateLimit: setRateLimit,
    onOpenReport: useCallback(() => setReportOpen(true), []),
    // The main window is the one that SPEAKS; project windows stay quiet.
    announce: isMain,
    // Other windows and the HUD redirect here for the global views.
    onMainTab: useCallback((tab: string) => {
      if (tab === "board") setView("board");
      if (tab === "custos") openSettings("custos");
      if (tab === "settings") openSettings();
    }, []),
    // The general chat speaks its ACTUAL reply (first sentence); another
    // chat finishing gets the butler's offer, through the same TTS queue.
    turnSpeech: useCallback((taskId: string, text: string, isError: boolean, label: string) => {
      if (!isMain || isError) return undefined;
      if (taskId === HARK_CHAT) return firstSentence(text) || undefined;
      return st("sp_fu_offer", { t: label });
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []),
    // Spoken turns handled by the HUD land in the general thread too.
    onChatEcho: useCallback(
      (
        question: string,
        reply: { fala: string; detalhes?: string; itens?: string[]; cost_usd?: number; model?: string } | undefined,
        work: boolean,
      ) => {
        if (!isMain) return;
        push({ who: "user", text: question });
        if (work) chatInFlightRef.current = { text: question, retried: false };
        if (reply) push({ who: "hark", ...askReplyMsg(reply) });
      },
      // eslint-disable-next-line react-hooks/exhaustive-deps
      [],
    ),
    // The voice feed of the mother, as one quiet line in the general thread.
    onVoiceAction: useCallback((utterance: string, target?: string | null, status?: string) => {
      if (!isMain) return;
      // The narration carries a way back to the chat the work landed in,
      // and that chat's row pulses for a beat: voice never switched the
      // scope, so the eye has to be told where it went.
      push({
        who: "sys",
        text: `${t("hud_voice_line")} “${utterance}” → ${target ?? "?"}${status ? ` · ${status}` : ""}`,
        goto: target ? { title: target } : undefined,
      });
      if (target) {
        setPulsing((p) => [...p.filter((x) => x !== target), target]);
        window.setTimeout(() => setPulsing((p) => p.filter((x) => x !== target)), 4000);
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []),
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
    onQueued: markQueued,
    speakRef,
    refresh,
    onWorkerExit: useCallback((taskId: string, reason?: string | null) => {
      endTurn(null, taskId);
      if (taskId === HARK_CHAT) {
        // One retry, announced: hark_chat_send respawns the worker on the
        // same session, so the conversation's memory survives the crash.
        // An unauthenticated death would die again: no resend.
        const inflight = chatInFlightRef.current;
        if (inflight && !inflight.retried && !(reason ?? "").includes("agent_auth")) {
          chatInFlightRef.current = { ...inflight, retried: true };
          push({ who: "sys", text: t("chat_crash_resend", { why: reason ? ` (${reason})` : "" }) });
          ipc.harkChatSend(inflight.text).catch((err) => push({ who: "sys", text: t("sys_chat_err", { e: String(err) }) }));
        } else {
          chatInFlightRef.current = null;
        }
        return;
      }
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
      setView("chat");
      openTaskRef.current(title, sessionId ?? undefined);
    }, []),
    onFocusProject: useCallback((project: Project) => {
      setView("chat");
      startDraftRef.current(project);
    }, []),
    onArtifactWritten: useCallback(() => setArtTick((n) => n + 1), []),
    onOpenPage: useCallback((space: string, id: string) => {
      setView("artefatos");
      setArtSel({ space, id });
    }, []),
    onDeepLinkUnknown: useCallback((url: string) => {
      push({ who: "sys", text: t("share_link_unknown", { url }) });
    }, []),
    // The hotkey with this window in front: the composer listens, no HUD.
    onHudListen: useCallback(() => voiceStartRef.current(), []),
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
      // Cmd+P is files; Cmd+K is this project's chat history.
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setChatPalette((q) => !q);
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "b") {
        e.preventDefault();
        const panel = sidebarRef.current;
        if (panel) panel.isCollapsed() ? panel.expand() : panel.collapse();
      }
      // App settings live on the mother: Cmd+, from here redirects.
      if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        if (isMain) openSettings();
        else ipc.focusMain("settings").catch(() => {});
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
        // Esc stops the TURN and keeps the conversation, which is what a
        // stop key means next to running work. Ending the session is a
        // louder, separate act — it does not belong on a keystroke people
        // press to dismiss things.
        const target = focusedRef.current;
        if (target) interruptRef.current(target);
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

  /** Where a message lands, as ONE value: the chip on the composer and the
   *  breadcrumb over the chat print it. It is derived from the three
   *  states above for now; it replaces them in the single-window cut. */
  const scope = scopeOf({
    forcedProject,
    draftChat,
    focusedTask,
    taskProject: focusedTask ? projectOf(board.find((t) => t.title === focusedTask.title)) : undefined,
    page:
      isMain && view === "artefatos" && artPage && artSel?.id
        ? { title: artPage.node.title, path: artPage.node.path, space: artSel.space }
        : null,
  });
  /** Colour by REGISTRY order, not by this window's filtered list, so a
   *  project is the same colour in every window and in the mother. */
  const colourOf = useCallback((path?: string) => projColour(colourIndex(path, allProjects)), [allProjects]);
  const scopeColour = colourOf(scope.kind === "chat" ? scope.project?.path : scope.kind === "project" ? scope.project.path : undefined);

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

  async function runAsk(question: string, images: Attachment[], noProject = false) {
    setBusy("perguntando…");
    const thread = currentThread() ?? GENERAL;
    beginTurn();
    try {
      // The focused project scopes the snapshot: clicking a chat or
      // starting one IS the context selection (no manual picker). The
      // general chat of the main window asks with no project at all.
      const reply = await ipc.askText(question, images.map(toImagePair), noProject ? undefined : activeProject);
      push({ who: "hark", ...askReplyMsg(reply) });
      if (reply.cost_usd) addCost("hark (perguntas)", reply.cost_usd);
      say(reply.fala);
    } catch (err) {
      nudgeRef.current(err);
      push({ who: "sys", text: t("sys_error", { e: agentError(err) }) });
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
    // Nothing is announced here. The focus moving is real, but the
    // composer's placeholder already names the task every message will
    // go to, and it keeps saying so — a one-off line under the message
    // repeated what the input field says permanently.
  }

  /**
   * `alreadyShown` is for the composer: what was typed is sitting right
   * above as the user's own message, and echoing it back as a system line
   * said the same sentence twice. Every other caller (voice, the confirm
   * modal, the candidate picker) dispatches text the thread has NOT shown,
   * and there the echo is the only record of what was sent.
   */
  async function runDispatch(instruction: string, sessionId?: string, alreadyShown = false) {
    setBusy("despachando…");
    const from = currentThread() ?? GENERAL;
    if (!alreadyShown) {
      push({ who: "sys", text: t("sys_dispatch", { i: instruction }) });
    }
    // Spawning a worker takes seconds before the first event: the clock
    // starts with the dispatch, not with the agent's first word.
    beginTurn(from);
    // Only a live worker keeps the clock: every other outcome ends here,
    // and a clock nobody stops ticks forever.
    let live = false;
    try {
      const out = await ipc.workerStart(
        instruction,
        sessionId ?? null,
        modeDefault ?? undefined,
        undefined,
        modelDefault || undefined,
        effortDefault || undefined,
      );
      if (out.status === "started") {
        const label =
          focusedTask && sessionId === focusedTask.sessionId
            ? focusedTask.title
            : instruction.split(/\s+/).slice(0, 5).join(" ");
        adoptWorker(out.task_id, label, out.directives, sessionId ?? "");
        live = true;
      } else if (out.status === "done") {
        push({ who: "hark", text: out.summary, cost: out.cost_usd });
        say(st("sp_task_done"));
      } else if (out.status === "failed") {
        push({ who: "sys", text: t("sys_worker_failed", { e: out.summary }) });
        say(st("sp_task_failed"));
      } else if (out.status === "choice") {
        setPending({ kind: "choice", instruction, candidates: out.candidates });
      } else if (out.status === "busy") {
        push({
          who: "sys",
          text: t("sys_session_held", { s: out.session_id }),
        });
        say(st("sp_target_held"));
      } else {
        push({ who: "sys", text: t("sys_no_session_match") });
        say(st("sp_no_session_for_that"));
      }
    } catch (err) {
      nudgeRef.current(err);
      push({ who: "sys", text: t("sys_error", { e: agentError(err) }) });
    } finally {
      if (!live) endTurn(from);
      setBusy(null);
      refresh();
    }
  }

  /** First message of a "+" draft: opens a brand-new session in the project. */
  async function runNewChat(project: Project, instruction: string, agent?: string) {
    setBusy("abrindo sessão nova…");
    const from = currentThread() ?? GENERAL;
    push({ who: "sys", text: t("sys_new_chat_in", { name: project.name }) });
    beginTurn(from);
    let live = false;
    try {
      const out = await ipc.chatStart(
        project.path,
        instruction,
        modeDefault ?? undefined,
        modelDefault || undefined,
        effortDefault || undefined,
        agent,
      );
      if (out.status === "started") {
        const label = instruction.split(/\s+/).slice(0, 5).join(" ");
        push({ who: "user", text: instruction, task: label });
        adoptWorker(out.task_id, label, out.directives, "");
        live = true;
      } else {
        push({ who: "sys", text: t("sys_not_opened", { e: JSON.stringify(out) }) });
      }
    } catch (err) {
      nudgeRef.current(err);
      push({ who: "sys", text: t("sys_error", { e: agentError(err) }) });
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
    ensureRail("arquivos");
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
    push({ who: "sys", text: t("sys_no_file_match", { q: query }) });
    say(st("sp_no_file"));
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
          push({ who: "sys", text: t("sys_worker_err", { e: agentError(err) }) });
      });
      return;
    }
    if (focusedTask) {
      await sendToFocusedTaskWith(focusedTask.title, focusedTask.sessionId, text);
      return;
    }
    push({ who: "sys", text: t("sys_slash_needs_chat") });
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
        else push({ who: "sys", text: t("sys_modes_list") });
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
        else push({ who: "sys", text: t("sys_usage_unavailable") });
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
          push({ who: "sys", text: t("sys_no_session_for", { t: cmd.title }) });
          say(st("sp_no_task_session"));
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
        push({ who: "sys", text: t("sys_project_added", { name: cmd.title, path: cmd.path }) });
        say(st("sp_project_added", { t: cmd.title }));
        refresh();
        // "abre um novo projeto": registering IS half the intent — the
        // window is the other half.
        await ipc.openTask(cmd.title, cmd.path).catch(() => {});
      } else if (cmd.kind === "project_error") {
        push({ who: "sys", text: cmd.title });
        say(st("sp_project_add_fail"));
      } else if (cmd.kind === "new_chat") {
        if (cmd.instruction) {
          await runNewChat({ name: cmd.title, path: cmd.path }, cmd.instruction, cmd.agent ?? undefined);
        } else {
          draftAgent.current = cmd.agent ?? undefined;
          startDraftChat({ name: cmd.title, path: cmd.path });
          say(st("sp_new_chat_first", { t: cmd.title }));
        }
      } else if (cmd.kind === "handoff") {
        // The focused LIVE thread moves; nothing else could.
        if (focused && liveWorkers[focused]) {
          push({ who: "sys", text: st("sp_handoff_go", { a: cmd.name }), task: labelFor(focused) });
          await ipc.workerHandoff(focused, cmd.agent).catch((err) => push({ who: "sys", text: t("sys_handoff_err", { e: String(err) }) }));
        } else {
          say(st("sp_handoff_needs_thread"));
        }
      } else if (cmd.kind === "agent_error") {
        push({ who: "sys", text: cmd.title });
        say(cmd.title);
      } else if (cmd.kind === "open_project") {
        await ipc.openTask(cmd.title, cmd.path).catch((err) =>
          push({ who: "sys", text: t("sys_window_err", { e: String(err) }) }),
        );
        if (cmd.instruction) {
          await ipc.chatStart(cmd.path, cmd.instruction).catch((err) =>
            push({ who: "sys", text: t("sys_chat_open_err", { e: String(err) }) }),
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
          // Costs are a section of the settings, in the main window.
          if (isMain) openSettings("custos");
          else await ipc.focusMain("custos").catch(() => {});
          say(st(isMain ? "sp_costs_screen" : "sp_costs_main"));
        }
      } else if (cmd.kind === "session_candidates") {
        // Recovering a session is a local lookup: the index already knows
        // every session on this machine. No agent, no digging, no tokens.
        if (cmd.candidates.length === 0) {
          push({ who: "sys", text: t("sys_no_session_about", { q: cmd.query }) });
          say(st("sp_no_session_about"));
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
        // App settings live in the main window.
        if (isMain) openSettings();
        else await ipc.focusMain("settings").catch(() => {});
        say(st(isMain ? "sp_settings_screen" : "sp_settings_main"));
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
      } else if (cmd.kind === "done") {
        // Closing the task you are in, from the chat. Before this the
        // words went to the agent as work: "fechamos essa task?" spent
        // real money asking a torn-down cluster whether it was gone.
        push({ who: "sys", text: t("sys_task_done", { t: cmd.title }), task: cmd.title });
        say(st("sp_task_done"));
        refresh();
      } else if (cmd.kind === "not_found") {
        push({ who: "sys", text: t("sys_nothing_matches", { q: cmd.query }) });
        say(st("sp_not_on_board"));
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

    // A page in scope: the message goes to the general chat with the page
    // along (whole while it fits, else its head — domain::scope). The
    // bubble shows what was typed; the page rides in the prompt only.
    if (scope.kind === "page" && artSel?.id) {
      const brief = await ipc.artifactBrief(artSel.space, artSel.id).catch(() => "");
      const msgId = crypto.randomUUID();
      push({ who: "user", text, images: images.map((i) => i.dataUrl), msgId });
      push({ who: "sys", text: t("art_sent_about", { t: scope.title }) });
      leaveChat();
      setView("chat");
      await sendToChat(brief ? `${brief}\n\n---\n\n${text}` : text, images, msgId);
      return;
    }

    // A fork draft armed by the held banner: this message starts a NEW
    // session seeded with the held one's history and works there.
    if (forkDraft && !toHark) {
      const draft = forkDraft;
      setForkDraft(null);
      push({ who: "user", text, task: draft.title });
      beginTurn(`${draft.title} (fork)`);
      const out = await ipc
        .workerStart(
          text,
          draft.sessionId,
          modeDefault ?? undefined,
          true,
          modelDefault || undefined,
          effortDefault || undefined,
        )
        .catch((err) => {
          nudgeRef.current(err);
          push({ who: "sys", text: t("sys_fork_err", { e: agentError(err) }) });
          return null;
        });
      if (out?.status === "started") {
        adoptWorker(out.task_id, `${draft.title} (fork)`, out.directives, "");
      } else {
        endTurn(`${draft.title} (fork)`);
        if (out) push({ who: "sys", text: t("sys_fork_not_opened", { e: JSON.stringify(out) }) });
      }
      return;
    }

    // A pending "+" draft: this message opens the new session.
    if (draftChat && !toHark) {
      await runNewChat(draftChat, text, draftAgent.current);
      draftAgent.current = undefined;
      return;
    }

    if (focused && !toHark) {
      const msgId = crypto.randomUUID();
      push({
        who: "user",
        text,
        images: images.map((i) => i.dataUrl),
        task: focused && labelFor(focused),
        msgId,
      });
      beginTurn();
      text = await withCrossref(text, labelFor(focused));
      await sendQueued(focused, text, msgId, images.map(toImagePair)).catch((err) => {
        endTurn(labelFor(focused));
        nudgeRef.current(err);
        push({ who: "sys", text: t("sys_worker_err", { e: agentError(err) }) });
      });
      return;
    }
    if (focusedTask && !toHark) {
      // Echo FIRST. Everything below can take a round trip, and a chat
      // that swallows what you typed until the backend answers reads as
      // broken — you cannot even tell whether Enter registered.
      const typedId = crypto.randomUUID();
      push({
        who: "user",
        text,
        images: images.map((i) => i.dataUrl),
        task: focusedTask.title,
        msgId: typedId,
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
      // A genuinely heavy session is worth stopping for — compacting
      // before an expensive turn is the cheap version of what happens
      // anyway. What was NOT worth stopping for was the old shape: the
      // message reprinted in a textarea under "Despachar tarefa?", asking
      // someone to confirm words they had just typed. `typed` drops the
      // textarea and leaves the only open question, which is the
      // warning's: compact first, or send as is.
      const warnings = await ipc.dispatchPrechecks(focusedTask.sessionId).catch(() => []);
      if (warnings.length > 0) {
        setPending({
          kind: "confirm-dispatch",
          instruction: text,
          sessionId: focusedTask.sessionId,
          warnings,
          typed: true,
        });
        return;
      }
      await sendToFocusedTaskWith(
        focusedTask.title,
        focusedTask.sessionId,
        text,
        images,
        false,
        typedId,
      );
      return;
    }
    if (isMain) {
      await submitGeneral(text, images);
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

  /**
   * The general chat of the main window, scope "all projects". Order:
   * the pending butler offer; action verbs are PLANNED against every
   * project (board, index) and confirmed, never guessed; work with no
   * target goes to the persistent chat (full settings + MCP); a sentence
   * the grammar cannot place is placed by the light classifier before
   * the cheap bare ask answers the wrong thing.
   */
  async function submitGeneral(text: string, images: Attachment[]) {
    const offer = followUpRef.current;
    if (offer) {
      followUpRef.current = null;
      const fu = await ipc.interpretFollowup(text, offer.label).catch(() => null);
      if (fu?.kind === "go") {
        push({ who: "user", text });
        say(st("sp_fu_going", { t: offer.label }));
        openTaskByTitle(offer.label, offer.sessionId);
        return;
      }
      if (fu?.kind === "do" && fu.instruction) {
        push({ who: "user", text });
        await ipc
          .voiceExecute({ instruction: fu.instruction, session_id: offer.sessionId, new_task: false })
          .catch((err) => push({ who: "sys", text: t("sys_dispatch_err", { e: String(err) }) }));
        say(st("sp_fu_doing", { t: offer.label }));
        return;
      }
      if (fu?.kind === "stay") {
        push({ who: "user", text });
        say(st("sp_fu_stay"));
        return;
      }
    }
    const route = await ipc.routeText(text).catch(() => "ask");
    const lane = await ipc.askLane(text).catch(() => "lean");
    const msgId = crypto.randomUUID();
    push({ who: "user", text, images: images.map((i) => i.dataUrl), msgId });
    if (route === "dispatch") {
      const plan = await ipc.planUtterance(text).catch(() => null);
      if (plan?.kind === "work") {
        // Same modal as a typed dispatch in a project window; the session
        // the funnel found rides along so the worker resumes it.
        setPending({ kind: "confirm-dispatch", instruction: plan.instruction ?? text, sessionId: plan.session_id ?? undefined, typed: true });
        return;
      }
      if (plan?.kind === "candidates") {
        setPending({
          kind: "pick-session",
          query: text,
          candidates: plan.options.map((o) => ({ session_id: o.session_id ?? "", title: o.title, cwd: o.workspace })),
        });
        say(st("sp_found_dests", { n: plan.options.length }));
        return;
      }
      if (lane === "work") {
        await sendToChat(text, images, msgId);
        return;
      }
      if (await tryIntent(text)) return;
      push({ who: "sys", text: t("no_target_hint") });
      say(st("sp_no_target"));
      return;
    }
    if (lane === "work") {
      await sendToChat(text, images, msgId);
      return;
    }
    if (await tryIntent(text)) return;
    runAsk(text, images, true);
  }

  /** Hand real work to the persistent general chat; its turns stream back
   *  into the GENERAL thread as events (task id "hark-chat"). */
  async function sendToChat(text: string, images: Attachment[], msgId?: string) {
    beginTurn(GENERAL);
    try {
      chatInFlightRef.current = { text, retried: false };
      const out = await ipc.harkChatSend(text, images.map(toImagePair), msgId);
      if (out.queued && msgId) markQueued(msgId, { state: "waiting" });
    } catch (err) {
      chatInFlightRef.current = null;
      endTurn(GENERAL);
      nudgeRef.current(err);
      push({ who: "sys", text: t("sys_chat_err", { e: agentError(err) }) });
    }
  }

  /**
   * The light model PLACES a sentence against the real catalog of
   * projects and sessions, then the window acts. Returns true when it did.
   */
  async function tryIntent(text: string): Promise<boolean> {
    const recent = messages
      .filter((m) => !("task" in m && m.task))
      .slice(-6)
      .map((m) => `${m.who}: ${("text" in m ? m.text : "").slice(0, 200)}`);
    const intent = await ipc.classifyUtterance(text, recent).catch(() => null);
    if (!intent || intent.kind === "question") return false;
    if (intent.kind === "clarify" && intent.options.length > 0) {
      setPending({
        kind: "pick-session",
        query: text,
        candidates: intent.options.map((o) => ({ session_id: o.session_id, title: o.title, cwd: o.project ?? undefined })),
      });
      const question = intent.question ?? st("sp_found_dests", { n: intent.options.length });
      push({ who: "hark", text: question, cost: intent.cost_usd ?? undefined });
      say(question);
      return true;
    }
    if (intent.kind === "status") {
      const workers = overview?.workers ?? [];
      const running = workers.filter((w) => w.status === "running");
      const line = (() => {
        if (intent.session_id) {
          const w = workers.find((x) => x.session_id === intent.session_id);
          return w
            ? st("sp_status_one", { t: w.summary, s: w.status === "running" ? st("sp_status_running") : st("sp_status_done") })
            : st("sp_status_none");
        }
        if (running.length === 0) return st("sp_status_none");
        if (running.length === 1) return st("sp_status_one", { t: running[0].summary, s: st("sp_status_running") });
        return st("sp_status_many", { n: running.length, list: running.map((w) => w.summary).slice(0, 3).join("; ") });
      })();
      push({ who: "hark", text: line, cost: intent.cost_usd ?? undefined });
      say(line);
      return true;
    }
    if (intent.kind === "open_project" && intent.project) {
      const project = allProjects.find((p) => p.name.toLowerCase() === intent.project!.toLowerCase());
      if (!project) return false;
      if (intent.instruction) {
        try {
          await ipc.chatStart(project.path, intent.instruction);
          say(st("sp_opening_and_work", { t: project.name }));
        } catch (err) {
          push({ who: "sys", text: t("sys_chat_open_err", { e: String(err) }) });
        }
      } else {
        startDraftChat(project);
        say(st("sp_opening_project", { t: project.name }));
      }
      return true;
    }
    if (intent.kind === "open_session" && intent.session_id) {
      await recoverSession({ session_id: intent.session_id, title: intent.session_title ?? text });
      if (intent.instruction) {
        await ipc
          .voiceExecute({ instruction: intent.instruction, session_id: intent.session_id, new_task: false })
          .catch((err) => push({ who: "sys", text: t("sys_dispatch_err", { e: String(err) }) }));
      }
      return true;
    }
    if (intent.kind === "dispatch" && intent.instruction && intent.session_id) {
      await ipc
        .voiceExecute({ instruction: intent.instruction, session_id: intent.session_id, new_task: false })
        .catch((err) => push({ who: "sys", text: t("sys_dispatch_err", { e: String(err) }) }));
      say(st("sp_confirm_to", { target: intent.session_title ?? st("sp_focused_session") }));
      return true;
    }
    return false;
  }

  /** Model the pill answers for: the focused task's directive, else the
   *  window default. What it SHOWS prefers the last turn's real model. */
  const currentModel =
    (focused ? liveWorkers[focused]?.directives.model : undefined) ?? modelDefault;
  const liveModel = focused ? liveWorkers[focused]?.model : undefined;

  // THE authoritative context reading: the CLI's own, teed by the
  // statusLine bridge. Everything below it is inference from billing
  // tokens, which was wrong in five different ways before this was
  // noticed — it read 232% for a session the CLI itself called 10%,
  // because the window it divides by is only ever a guess. The bridge is
  // opt-in, so the inference stays as the fallback, but it is no longer
  // the plan.
  const [truth, setTruth] = useState<import("./types").StatusLine | null>(null);
  useEffect(() => {
    const read = () => {
      ipc
        .subscriptionLimits()
        .then((v) => setTruth(keepIfSame(v)))
        .catch(() => {});
    };
    read();
    const id = window.setInterval(read, 15_000);
    return () => window.clearInterval(id);
  }, []);

  // The ring used to appear only once a live worker had finished a turn
  // in THIS window. Reopening a heavy chat, it was simply absent — and
  // then a warning quoted a context percentage the user had no way of
  // seeing anywhere. The ledger knows it: read the focused session's last
  // turn and show that until a live turn overwrites it.
  const [ledgerCtx, setLedgerCtx] = useState<
    { pct: number; window?: number | null; tokens: number } | null
  >(null);
  useEffect(() => {
    const session = focusedTask?.sessionId;
    if (!session) {
      setLedgerCtx(null);
      return;
    }
    ipc
      .sessionContextWeight(session)
      .then((w) => {
        const window = w.context_window ?? 0;
        setLedgerCtx(
          window > 0 && w.last_total_tokens > 0
            ? { pct: w.last_total_tokens / window, window, tokens: w.last_total_tokens }
            : null,
        );
      })
      .catch(() => setLedgerCtx(null));
  }, [focusedTask?.sessionId, liveWorkers[focused ?? ""]?.context_pct]);

  /** What the ring shows, best source first: the CLI's own reading for
   *  THIS session, then the live turn, then the ledger. */
  const shownContext =
    (truth?.context_used != null &&
    truth.session_id &&
    focusedTask?.sessionId === truth.session_id
      ? {
          pct: truth.context_used,
          window: truth.context_window,
          tokens: truth.context_tokens ?? 0,
        }
      : null) ??
    (focused && liveWorkers[focused]?.context_pct != null
      ? {
          pct: liveWorkers[focused]!.context_pct!,
          window: liveWorkers[focused]!.context_window,
          // The prompt behind the ratio, so the ring can fall back to a
          // fact when the ratio itself cannot be trusted.
          tokens: Math.round(
            liveWorkers[focused]!.context_pct! * (liveWorkers[focused]!.context_window ?? 0),
          ),
        }
      : null) ?? ledgerCtx;


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
        .catch((err) => push({ who: "sys", text: t("sys_model_err", { e: String(err) }), task: labelFor(taskId) }));
      return;
    }
    defaults.setModelDefault(model);
  }

  /** Effort the pill shows: the focused live task's, else this window's. */
  const currentEffort =
    (focused ? liveWorkers[focused]?.directives.effort : undefined) ?? effortDefault;

  /** Same contract as the model pill: a live task reopens on the same
   *  session, nothing focused sets the window default. "" clears the
   *  directive instead of picking a level. */
  function selectEffort(effort: string) {
    const taskId = focused;
    if (taskId && liveWorkers[taskId]) {
      ipc
        .workerSetEffort(taskId, effort)
        .then((out) => {
          setLiveWorkers((old) =>
            old[taskId] ? { ...old, [taskId]: { ...old[taskId], directives: out.directives } } : old,
          );
        })
        .catch((err) => push({ who: "sys", text: t("sys_effort_err", { e: String(err) }), task: labelFor(taskId) }));
      return;
    }
    defaults.setEffortDefault(effort);
  }

  /** Mode the pill shows: the focused live task's, else the window/config default. */
  const currentMode =
    (focused ? liveWorkers[focused]?.directives.mode : undefined) ?? defaults.modeInForce;

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
              text: t("sys_mode_next_open", { m: flag }),
              task: label,
            });
          }
        })
        .catch((err) => push({ who: "sys", text: t("sys_mode_err", { e: String(err) }) }));
      return;
    }
    defaults.setModeDefault(flag);
    push({ who: "sys", text: t("sys_mode_default", { m: flag }) });
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

  const railHas = (id: RailItem) => railOn === id;

  // The task's share popover: link, resume command, transcript, brief as
  // a page. Every row is local; the footer says so.
  const [shareOpen, setShareOpen] = useState(false);
  const shareRows: ShareRow[] = (() => {
    if (!focusedTask?.sessionId) return [];
    const session = focusedTask.sessionId;
    const title = focusedTask.title;
    const rows: ShareRow[] = [
      {
        icon: <Copy size={13} />,
        label: t("share_copy_link"),
        hint: `hark://chat/${session.slice(0, 8)}…`,
        kbd: "⌘C",
        run: () => navigator.clipboard.writeText(`hark://chat/${encodeURIComponent(session)}`).then(() => t("share_link_copied")),
      },
    ];
    if (!terminalResumeFor(catalog, focusedAgent)) {
      rows.push({
        icon: <SquareTerminal size={13} />,
        label: t("share_copy_resume"),
        hint: `claude --resume ${session.slice(0, 8)}…`,
        run: () => navigator.clipboard.writeText(`claude --resume ${session}`).then(() => t("share_copied")),
      });
    }
    rows.push({
      icon: <Download size={13} />,
      label: t("share_export_transcript"),
      hint: t("share_export_transcript_hint"),
      run: async () => {
        const out = await ipc.readTranscript(session, 5000);
        const abs = await ipc.exportFile(`${title}.md`, transcriptMarkdown(out.session_title ?? title, out.entries));
        await ipc.revealPath(abs).catch(() => {});
        return t("share_exported");
      },
    });
    const space = scope.kind === "chat" && scope.project ? scope.project.name : "pessoal";
    rows.push({
      icon: <FileText size={13} />,
      label: t("share_save_brief"),
      hint: t("share_save_brief_hint", { space }),
      run: async () => {
        const node = await ipc.artifactFromBrief(session, title, space, title);
        setArtTick((n) => n + 1);
        push({ who: "sys", text: t("share_brief_saved", { t: node.title, space }), task: title });
        return t("share_brief_saved_short");
      },
    });
    return rows;
  })();

  /** The permission each task is waiting on right now: the board's cards
   *  answer it in place. One per task — the newest open one. */
  const askingByTask: Record<string, CardAsk> = {};
  for (const m of messages) {
    if (m.who === "permission" && !m.decision && !m.expired && m.task) {
      askingByTask[m.task] = { requestId: m.requestId, tool: m.tool, prodRisk: m.prodRisk };
    }
  }
  const liveTitles = Object.values(liveWorkers).map((w) => w.label);
  /** The project the scope points at, for the board's filter. */
  const scopeProject = scope.kind === "chat" ? scope.project ?? null : scope.kind === "project" ? scope.project : null;

  /** Show a rail panel; whatever was there steps aside. */
  function ensureRail(id: RailItem) {
    // The terminal opens on a REAL shell. It used to land on the feed,
    // which is a log of the agent's tool calls — and on a fresh thread
    // that is "(sem eventos ainda)": a dead panel where the user asked
    // for a prompt. The feed stays a tab, it just stops being the door.
    if (id === "terminal") {
      if (shells.length === 0) addShell();
      else if (!shells.includes(termTab)) setTermTab(shells[0]);
    }
    setRailOn(id);
  }

  function removeRail(id: RailItem) {
    setRailOn((on) => (on === id ? null : on));
    setExpanded((e) => (e === id ? null : e));
  }

  /** Toggle from a header icon: open it, or close it if it is the one open. */
  const toggleRail = (id: RailItem) => (railHas(id) ? removeRail(id) : ensureRail(id));

  function addShell(): string {
    const id = `sh-${shellSeq.current++}-${Date.now() % 1e6}`;
    setShells((old) => [...old, id]);
    setTermTab(id);
    return id;
  }

  function closeShell(id: string) {
    setShells((old) => old.filter((s) => s !== id));
    // Closing the active shell falls back to whichever remains.
    setTermTab((t) => (t === id ? (shells.find((s) => s !== id) ?? "") : t));
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
    const payload = terminalPayload(cmd, execute);
    // A fresh shell needs a beat to spawn and print its prompt — and a
    // swallowed write is a phantom click, so failure retries once and
    // then says so instead of pretending.
    const write = (attempt: number) => {
      ipc.termWrite(target, payload).catch((err) => {
        if (attempt === 0) setTimeout(() => write(1), 600);
        else push({ who: "sys", text: t("sys_terminal_err", { e: String(err) }) });
      });
    };
    setTimeout(() => write(0), existing ? 120 : 700);
  }

  /** Cut the turn in flight; the conversation stays open. */
  function interruptWorker(taskId: string) {
    if (!liveWorkers[taskId]) return;
    ipc
      .workerInterrupt(taskId)
      .catch((err) => push({ who: "sys", text: t("sys_stop_err", { e: String(err) }), task: labelFor(taskId) }));
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
  // The voice round in the composer (phase 10.6): the same ear the HUD
  // has, drawn over the text while the main window is in front. A
  // detached project window still floats the HUD — it has no general
  // chat to narrate into.
  const voice = useVoiceRound({ owner: "main", onHide: () => {}, keys: true });
  const voiceStartRef = useRef(voice.start);
  voiceStartRef.current = voice.start;

  async function onMic() {
    if (heldBy) {
      push({
        who: "sys",
        text: t("sys_voice_held", { name: heldBy.name }),
        task: focusedTask?.title,
      });
      say(st("sp_held_fork"));
      return;
    }
    if (isMain) {
      voice.start();
      return;
    }
    await ipc.hudShow().catch((err) => push({ who: "sys", text: t("sys_voice_err", { e: String(err) }) }));
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
        text: t("sys_allow_always", { tool: card.tool }),
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
  const startDraftRef = useRef(startDraftChat);
  startDraftRef.current = startDraftChat;
  const pendingRef = useRef<Pending>(null);
  pendingRef.current = pending;

  /** Confirm the open dispatch modal (Enter, voice, or the button). */
  function confirmPendingDispatch() {
    const p = pendingRef.current;
    if (p?.kind !== "confirm-dispatch") return;
    setPending(null);
    // Confirming past a "compact first" offer is the answer to it.
    if (p.sessionId && (p.warnings ?? []).some((w) => w.actions.includes("compact_first"))) {
      declinedCompact.current.add(p.sessionId);
    }
    if (focusedTask) reactivateIfDone(focusedTask.title);
    runDispatch(p.instruction, p.sessionId);
  }
  const confirmDispatchRef = useRef(confirmPendingDispatch);
  confirmDispatchRef.current = confirmPendingDispatch;

  /** "compactar antes": /compact runs as its own turn, the message queues
   *  right behind it — the heavy-session warning's remedy, one click/word. */
  async function dispatchCompactFirst(instruction: string, sessionId?: string) {
    push({ who: "sys", text: t("sys_compacting_first") });
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
          push({ who: "sys", text: t("sys_worker_err", { e: agentError(err) }) });
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
          push({ who: "sys", text: t("sys_worker_err", { e: agentError(err) }) });
      });
      return;
    }
    push({ who: "sys", text: t("sys_compact_skipped") });
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
      push({ who: "sys", text: t("sys_task_reopened", { t: title }), task: title });
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
    // The log's OWN time, not the moment we read the file. Without it
    // push() stamps now, and a thread reopened after a week came back as
    // a wall of "agora" — every message claiming to have just happened.
    const ts = Date.parse(e.ts) || undefined;
    if (e.role === "user") return { who: "user", text: e.text, task: title, ts };
    if (e.role === "assistant") return { who: "hark", text: e.text, task: title, ts };
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
      push({ who: "sys", text: t("sys_history_err", { e: String(err) }), task: task.title });
      return;
    }
    const at = out.entries.findIndex((e) => entryKey(e) === state.firstKey);
    const older = at > 0 ? out.entries.slice(0, at) : [];
    if (older.length === 0) {
      setHistoryDone((old) => ({ ...old, [task.title]: true }));
      push({ who: "sys", text: t("sys_history_start"), task: task.title });
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
          text: t("sys_session_replaced", { t: hit.title ?? hit.session_id.slice(0, 8) }),
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
        text: t("sys_context_loaded", { t: out.session_title ?? title }),
        task: title,
      });
    } catch (err) {
      push({ who: "sys", text: t("sys_history_read_err", { e: String(err) }), task: title });
    }
  }

  /** "…o que decidimos no chat do X": pull the cited chat's lines from the
   *  local log and ride them inside the message — visible, attributed,
   *  zero tokens. Returns the message to actually send. */
  async function withCrossref(text: string, thread?: string): Promise<string> {
    const [ref, pages] = await Promise.all([
      ipc.crossrefContext(text).catch(() => null),
      ipc.artifactCites(text).catch(() => null),
    ]);
    let out = text;
    if (ref) {
      push({
        who: "sys",
        text: t("sys_crossref_pulled", { n: ref.lines, t: ref.title }),
        task: thread,
      });
      out = `${out}\n\n${ref.block}`;
    }
    // A page named by its title rides along too: a decision page is the
    // context the instruction assumes.
    if (pages) {
      push({ who: "sys", text: t("art_cited", { n: pages.titles.length, t: pages.titles.join(", ") }), task: thread });
      out = `${out}\n\n${pages.block}`;
    }
    return out;
  }

  /** Send a message into a task: live worker if any, else a fresh resume. */
  async function sendToFocusedTaskWith(
    title: string,
    sessionId: string,
    text: string,
    images: Attachment[] = [],
    /** The submit path echoes before it starts, so it opts out here. */
    echo = true,
    /** ...and hands over the id of the bubble it already pushed, so a
     *  parked message still finds its own bubble. */
    msgId = crypto.randomUUID(),
  ) {
    // Slash commands, spoken follow-ups and compact-then-send all land
    // here. "/compact" showed no clock at all because only the two typed
    // paths above started one.
    beginTurn(title);
    await reactivateIfDone(title);
    const liveEntry = Object.entries(liveWorkers).find(([, w]) => w.label === title);
    if (echo)
      push({ who: "user", text, images: images.map((i) => i.dataUrl), task: title, msgId });
    text = await withCrossref(text, title);
    if (liveEntry) {
      await sendQueued(liveEntry[0], text, msgId, images.map(toImagePair)).catch((err) => {
        endTurn(title);
        nudgeRef.current(err);
        push({ who: "sys", text: t("sys_worker_err", { e: agentError(err) }) });
      });
      return;
    }
    await runDispatch(text, sessionId, true);
  }

  function startDraftChat(project: Project) {
    setDraftChat(project);
    leaveChat();
    push({
      who: "sys",
      text: t("sys_draft_chat", { name: project.name, path: project.path }),
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
      push({ who: "sys", text: t("sys_no_session_for", { t: title }) });
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
      push({ who: "sys", text: t("sys_session_register_err", { e: String(err) }) });
      return;
    }
    const ws = task.workspace;
    const elsewhere =
      ws && forcedProject && ws !== forcedProject.path && !ws.startsWith(`${forcedProject.path}/`);
    if (elsewhere) {
      const owner = allProjects.find((p) => ws === p.path || ws.startsWith(`${p.path}/`));
      if (owner) {
        await ipc
          .openTask(owner.name, owner.path, task.title, hit.session_id)
          .catch((err) => push({ who: "sys", text: t("sys_window_err", { e: String(err) }) }));
        push({ who: "sys", text: t("sys_elsewhere", { t: task.title, p: owner.name }) });
        say(st("sp_elsewhere", { p: owner.name }));
        refresh();
        return;
      }
    }
    await openTaskByTitle(task.title, hit.session_id);
    refresh();
    say(st("sp_resuming", { t: task.title }));
  }

  // Memoised: a fresh filter() every render is O(n) for nothing and hands
  // the transcript a new array, re-running its scroll effect each time.
  const focusedTitle = focusedTask?.title ?? null;
  /** What this thread produced: the session panel's "Saídas". */
  const threadOutputs = useMemo(() => collectOutputs(messages, focusedTitle), [messages, focusedTitle]);
  const visibleMessages = useMemo(
    () =>
      messages.filter((m) => {
        // The general thread is the empty label: the persistent general
        // chat's events arrive tagged with it, typed asks with none.
        const thread = "task" in m && m.task ? m.task : null;
        return thread === focusedTitle;
      }),
    [messages, focusedTitle],
  );

  // Typed windows, shared by the tiled layout and the expanded mode.
  /** Drag-to-reorder wiring for a rail panel's header. */
  /** The session panel: environment, context, outputs, costs of the
   *  thread on screen (or of the window, in the "all projects" scope). */
  const frameSessao = () => (
    <PanelFrame
      title={t("frame_session")}
      expanded={expanded === "sessao"}
      onToggleExpand={() => setExpanded((e) => (e === "sessao" ? null : "sessao"))}
      onClose={() => removeRail("sessao")}
    >
      <SessionPanel
        scope={scope}
        projectsCount={allProjects.length}
        taskTitle={focusedTask?.title}
        sessionId={focusedTask?.sessionId || undefined}
        workspace={activeProject?.path}
        repo={focusedRepo}
        agentName={focusedAgent ? (catalog[focusedAgent]?.name ?? focusedAgent) : undefined}
        mode={currentMode ?? undefined}
        model={liveModel ?? currentModel ?? undefined}
        effort={currentEffort || undefined}
        context={shownContext}
        turnUsd={turns[focusedTask?.title ?? GENERAL]?.usd ?? null}
        costs={costs}
        outputs={threadOutputs}
        onOpenPath={openAbsolutePath}
        onDetail={() => {
          void ipc
            .usageReport(focusedTask?.sessionId ?? undefined)
            .then((report) =>
              push({ who: "usage", report, task: focused ? labelFor(focused) : focusedTask?.title }),
            )
            .catch(() => {});
        }}
      />
    </PanelFrame>
  );
  const frameTerminal = () => (
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
      actions={
        focusedTask?.sessionId ? (
          <TerminalResume
            onResumeSession={pasteResume}
            spent={!!heldBy}
            disabled={terminalResumeFor(catalog, focusedAgent)}
          />
        ) : undefined
      }
      expanded={expanded === "terminal"}
      onToggleExpand={() => setExpanded((e) => (e === "terminal" ? null : "terminal"))}
      onClose={() => removeRail("terminal")}
    >
      <TerminalPane
        shells={shells}
        active={termTab}
        cwd={forcedProject?.path}
        onAddShell={addShell}
        onCloseShell={closeShell}
        onActivate={setTermTab}
      />
    </PanelFrame>
  );
  const frameBoard = () => (
    <PanelFrame
      title={t("frame_board")}
      expanded={expanded === "board"}
      onToggleExpand={() => setExpanded((e) => (e === "board" ? null : "board"))}
      onClose={() => removeRail("board")}
    >
      <Board
        tasks={board}
        projects={isMain ? allProjects : undefined}
        colourOf={colourOf}
        filter={isMain ? scopeProject : undefined}
        liveTitles={liveTitles}
        asking={askingByTask}
        costs={costs}
        onAnswerPermission={(id, allow) => void answerPermission(id, allow)}
        onMove={(title, status) => ipc.boardMove(title, status).then(refresh).catch(() => {})}
        onOpen={openTaskFromSidebar}
        onSubtask={(title, index, done) => ipc.boardSubtaskToggle(title, index, done).then(refresh).catch(() => {})}
      />
    </PanelFrame>
  );
  /** The links of the artifact page on screen: what it is, where it came
   *  from, what it ties to. */
  const frameArtefatos = () => (
    <PanelFrame
      title={t("frame_artifacts")}
      expanded={expanded === "artefatos"}
      onToggleExpand={() => setExpanded((e) => (e === "artefatos" ? null : "artefatos"))}
      onClose={() => removeRail("artefatos")}
    >
      <ArtifactLinks
        page={view === "artefatos" ? artPage : null}
        tree={artTree}
        onOpen={(id) => artSel && setArtSel({ space: artSel.space, id })}
        onOpenTask={(title, sessionId) => {
          setView("chat");
          void openTaskByTitle(title, sessionId);
        }}
      />
    </PanelFrame>
  );
  /** A real page beside the chat (a child webview the panel steers). */
  const frameBrowser = () => (
    <PanelFrame
      title={t("frame_browser")}
      expanded={expanded === "browser"}
      onToggleExpand={() => setExpanded((e) => (e === "browser" ? null : "browser"))}
      onClose={() => removeRail("browser")}
    >
      <BrowserPanel />
    </PanelFrame>
  );
  /** The diff of the workspace on screen: changed files, one at a time. */
  const frameDiff = () => (
    <PanelFrame
      title={t("frame_diff")}
      expanded={expanded === "diff"}
      onToggleExpand={() => setExpanded((e) => (e === "diff" ? null : "diff"))}
      onClose={() => removeRail("diff")}
    >
      {focusedWorkspace ? (
        <DiffPanel
          workspace={focusedWorkspace}
          repo={focusedRepo}
          tick={threadOutputs.reduce((n, o) => n + o.touches, 0)}
          onOpenPath={openAbsolutePath}
        />
      ) : (
        <div className="diff-empty">{t("diff_no_repo")}</div>
      )}
    </PanelFrame>
  );
  /** The files window: the tree column, the tabs in the header, the viewer. */
  const frameArquivos = () => (
    <PanelFrame
      title={t("frame_files")}
      lead={<TreeToggle open={filesTree.open} onToggle={filesTree.toggle} />}
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
      expanded={expanded === "arquivos"}
      onToggleExpand={() => setExpanded((e) => (e === "arquivos" ? null : "arquivos"))}
      onClose={() => {
        setOpenFiles([]);
        setDirtyPaths(new Set());
        removeRail("arquivos");
      }}
    >
      <FilesWindow
        projects={projects}
        files={openFiles}
        active={activeFile}
        initialProject={filesInitialProject}
        treeOpen={filesTree.open}
        onOpen={openFile}
        onDirty={onFileDirty}
        onWidth={filesTree.onWidth}
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
    <div className={`app ${sidebarOpen ? "" : "side-closed"}`}>
      {/* The title bar is an Overlay: the chat header and the sidebar's top
          slot drag the window (docs/CHROME.md — a full-width strip here
          covered the rail's headers and killed their ×); the toggle sits
          beside the traffic lights. */}
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
      {setup && <Onboarding status={setup} onClose={() => setSetup(null)} />}
      {updateReady && (
        <button
          className="update-pill"
          title={t("up_restart")}
          onClick={() => restartIntoUpdate().catch(() => setUpdateReady(null))}
        >
          {t("up_ready", { v: updateReady })} · {t("up_restart")}
        </button>
      )}

      {expanded ? (
        <div className="workarea expanded-area">
          {expanded === "sessao"
            ? frameSessao()
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
              colourOf={colourOf}
              tasks={board}
              // A history row steps aside only for a card THIS window
              // lists: a session whose card lives in another project used
              // to drop out of both lists (25/09).
              chats={chats.filter(
                (c) => !board.some((t) => t.session_ids.includes(c.session_id)),
              )}
              activeTitle={focusedTask?.title}
              liveTitles={Object.values(liveWorkers).map((w) => w.label)}
              pulsing={pulsing}
              onOpen={openTaskFromSidebar}
              onOpenChat={recoverSession}
              workspaceName={forcedProject?.name}
              onSearchChats={() => setChatPalette(true)}
              onOpenGeneral={() => {
                leaveChat();
                setView("chat");
              }}
              generalActive={!focusedTask && !draftChat && view === "chat"}
              artifactsOpen={isMain && view === "artefatos"}
              artifactsCount={artCount}
              onToggleArtifacts={isMain ? () => setView((v) => (v === "artefatos" ? "chat" : "artefatos")) : undefined}
              onOpenSettings={isMain ? () => (view === "ajustes" ? setView("chat") : openSettings()) : undefined}
              settingsOpen={view === "ajustes"}
              boardOpen={isMain ? view === "board" : railHas("board")}
              onToggleBoard={() =>
                isMain
                  ? setView((v) => (v === "board" ? "chat" : "board"))
                  : railHas("board")
                    ? removeRail("board")
                    : ensureRail("board")
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
              onReport={() => setReportOpen(true)}
              onAddProject={(path) =>
                ipc
                  .projectAdd(path)
                  .then((p) => {
                    push({ who: "sys", text: t("sys_project_added", { name: p.name, path: p.path }) });
                    refresh();
                  })
                  .catch((err) => push({ who: "sys", text: t("sys_project_err", { e: String(err) }) }))
              }
              onRemoveProject={(p) =>
                ipc.projectRemove(p.path).then(() => {
                  push({ who: "sys", text: t("sys_project_removed", { name: p.name }) });
                  refresh();
                })
              }
              onOpenFiles={(p) => {
                // The folder icon asks for the tree: open it even in the rail.
                setFilesInitialProject(p.path);
                filesTree.set(true);
                ensureRail("arquivos");
              }}
            />
          </Panel>
          <PanelResizeHandle className="rhandle" />
          <Panel minSize={30} className="pane">
            <div className="maincol" style={scopeColour ? ({ "--proj": scopeColour } as React.CSSProperties) : undefined}>
              {/* The main window's global views take the column whole:
                  the board of every project, the ledger, the settings. */}
              {isMain && view === "board" ? (
                <div className="view-wide">
                  <Board
                    tasks={board}
                    projects={allProjects}
                    colourOf={colourOf}
                    filter={scopeProject}
                    onFilter={(p) => {
                      // The pill IS the scope: the composer's chip follows it.
                      leaveChat();
                      setDraftChat(p);
                    }}
                    liveTitles={liveTitles}
                    asking={askingByTask}
                    costs={costs}
                    onAnswerPermission={(id, allow) => void answerPermission(id, allow)}
                    onMove={(title, status) => ipc.boardMove(title, status).then(refresh).catch(() => {})}
                    onOpen={(task) => {
                      setView("chat");
                      openTaskFromSidebar(task);
                    }}
                    onSubtask={(title, index, done) => ipc.boardSubtaskToggle(title, index, done).then(refresh).catch(() => {})}
                  />
                </div>
              ) : isMain && view === "ajustes" ? (
                <div className="view-wide">
                  <Settings initialSection={settingsSection} />
                </div>
              ) : (
              <>
              {/* The thread says what it is: a slim title over the prose,
                  like the Claude Code app — controls stay on the composer. */}
              {!reading && view !== "artefatos" && (
                <div className="chat-title-head" data-tauri-drag-region>
                  {focusedTask && (
                    <>
                      {/* Tauri drags only when the mousedown lands ON an
                          element carrying the attribute: the children that
                          cover the header must carry it too, or the title
                          bar cannot move the window. */}
                      {/* The project before the chat, in its colour: the
                          boundary the sidebar and the composer chip share. */}
                      {scope.kind === "chat" && scope.project && (
                        <>
                          <span className="proj-dot" data-tauri-drag-region />
                          <span className="crumb" data-tauri-drag-region>
                            {scope.project.name} ›
                          </span>
                        </>
                      )}
                      <span className="chat-title" data-tauri-drag-region>
                        {focusedTask.title}
                      </span>
                      <span className="chat-turns" data-tauri-drag-region>
                        {t("n_turns_of", {
                          n: visibleMessages.filter((m) => m.who === "user").length,
                        })}
                      </span>
                    </>
                  )}
                  <span className="chat-head-gap" data-tauri-drag-region />
                  {/* This bar owns the thread's STATE and its WINDOWS; the
                      composer row owns the message and the voice. The cost
                      used to be a pill here AND a wallet button down there —
                      same number, two places. It is one control now: the
                      number opens the breakdown. */}
                  <button
                    className={`chat-spend ${railHas("sessao") ? "on" : ""}`}
                    title={t("session_btn")}
                    onClick={() => toggleRail("sessao")}
                  >
                    ${(costs[focusedTask?.title ?? ""] ?? 0).toFixed(2)}
                    {/* The context rides in the same pill; the click opens
                        the session panel in the rail, where both facts
                        about this thread live with its outputs. Always
                        present while a chat is focused: vanishing read as
                        "nothing to worry about", not "not measured". */}
                    {focusedTask && (
                      <ContextRing
                        used={shownContext?.pct ?? null}
                        window={shownContext?.window}
                        tokens={shownContext?.tokens}
                        model={liveModel ?? currentModel ?? undefined}
                        unsupported={ringOff}
                      />
                    )}
                  </button>
                  {focusedTask?.sessionId && (
                    <span className="share-anchor">
                      <button
                        className={`chat-head-btn ${shareOpen ? "on" : ""}`}
                        title={t("share_title")}
                        onClick={() => setShareOpen((o) => !o)}
                      >
                        <Share2 size={13} />
                      </button>
                      {shareOpen && <SharePopover title={focusedTask.title} rows={shareRows} onClose={() => setShareOpen(false)} />}
                    </span>
                  )}
                  {isMain && scope.kind === "chat" && scope.project && (
                    <button
                      className="chat-head-btn"
                      title={t("detach_window")}
                      onClick={() => {
                        const p = scope.project!;
                        void ipc
                          .openProjectWindow(p.name, p.path, scope.title, scope.sessionId)
                          .catch((err) => push({ who: "sys", text: t("sys_window_err", { e: String(err) }) }));
                      }}
                    >
                      <ExternalLink size={13} />
                    </button>
                  )}
                  {/* The rail's doors, one icon each: one panel shows at a
                      time, the lit icon says which. */}
                  <button
                    className={`chat-head-btn ${railHas("terminal") ? "on" : ""}`}
                    title={t("terminal_btn")}
                    onClick={() => toggleRail("terminal")}
                  >
                    <SquareTerminal size={13} />
                  </button>
                  <button
                    className={`chat-head-btn ${railHas("arquivos") ? "on" : ""}`}
                    title={t("frame_files")}
                    onClick={() => {
                      if (railHas("arquivos")) {
                        removeRail("arquivos");
                      } else {
                        filesTree.set(true);
                        ensureRail("arquivos");
                      }
                    }}
                  >
                    <FolderTree size={13} />
                  </button>
                  <button
                    className={`chat-head-btn ${railHas("browser") ? "on" : ""}`}
                    title={t("frame_browser")}
                    onClick={() => toggleRail("browser")}
                  >
                    <Compass size={13} />
                  </button>
                  {focusedRepo && (
                    <button
                      className={`chat-head-btn ${railHas("diff") ? "on" : ""}`}
                      title={t("frame_diff")}
                      onClick={() => toggleRail("diff")}
                    >
                      <GitCompare size={13} />
                    </button>
                  )}
                  {!isMain && (
                    <button
                      className={`chat-head-btn ${railHas("board") ? "on" : ""}`}
                      title={t("frame_board")}
                      onClick={() => toggleRail("board")}
                    >
                      <SquareKanban size={13} />
                    </button>
                  )}
                </div>
              )}
              {isMain && view === "artefatos" ? (
                <Artifacts
                  sel={artSel}
                  onSel={setArtSel}
                  colourOf={colourOf}
                  tick={artTick}
                  onPage={onArtPage}
                  onOpenTask={(title, sessionId) => {
                    setView("chat");
                    void openTaskByTitle(title, sessionId);
                  }}
                  linksOn={railHas("artefatos")}
                  onToggleLinks={() => toggleRail("artefatos")}
                />
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
                  catalog={catalog}
                  directivesFor={directivesFor}
                  onAnswerPermission={answerPermission}
                  onOpenPath={openAbsolutePath}
                  onGoto={(title, sessionId) => {
                    setView("chat");
                    void openTaskByTitle(title, sessionId);
                  }}
                  onRunCommand={runInTerminal}
                  onLoadOlder={
                    focusedTask &&
                    history.current.has(focusedTask.title) &&
                    !historyDone[focusedTask.title]
                      ? loadOlderHistory
                      : undefined
                  }
                  onQueuedNow={queuedNow}
                  onQueuedDrop={queuedDrop}
                  onLiftCap={(taskId) =>
                    ipc
                      .workerLiftCap(taskId)
                      .then(() =>
                        setMessages((old) =>
                          old.map((m) =>
                            m.who === "ceiling" && m.taskId === taskId ? { ...m, lifted: true } : m,
                          ),
                        ),
                      )
                      .catch((err) => push({ who: "sys", text: t("sys_cap_err", { e: String(err) }) }))
                  }
                  onRestartLight={(taskId) =>
                    ipc
                      .workerRestartLight(taskId)
                      .catch((err) => push({ who: "sys", text: t("sys_light_restart_err", { e: String(err) }) }))
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
                      disabled={!!forkOff}
                      title={forkOff}
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
                scope={scope}
                scopeColour={scopeColour}
                pendingPermissionId={
                  pendingPermission?.who === "permission" ? pendingPermission.requestId : undefined
                }
                onSubmit={submit}
                onMic={onMic}
                running={!!turns[focusedTask?.title ?? GENERAL]}
                onStop={focused ? () => interruptWorker(focused) : undefined}
                onAnswerPermission={answerPermission}
                trailing={trailingControls}
                banner={focusedRepo ? <RepoRuler state={focusedRepo} /> : undefined}
                voice={
                  isMain && voice.active ? (
                    <VoiceRound stage={voice.stage} onPick={voice.pick} onTyped={voice.typed} onEscape={voice.escape} />
                  ) : undefined
                }
              >
                <ModeSelect
                  value={currentMode}
                  appliesTo={focused ? labelFor(focused) : undefined}
                  windowDefault={defaults.modeInForce}
                  disabled={modeOff}
                  bypassOff={bypassFloorFor(catalog, focusedAgent)}
                  onSelect={selectMode}
                />
                <ModelSelect
                  value={currentModel ?? ""}
                  liveModel={liveModel}
                  tiers={modelPill.tiers}
                  global={defaults.tiers}
                  appliesTo={focused ? labelFor(focused) : undefined}
                  windowDefault={modelDefault}
                  disabled={modelOff ?? modelPill.disabled}
                  onSelect={selectModel}
                />
                <EffortSelect
                  value={currentEffort ?? ""}
                  appliesTo={focused ? labelFor(focused) : undefined}
                  disabled={effortOff}
                  onSelect={selectEffort}
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
                  onStop={stopWorker}
                  onRestartLight={(taskId) =>
                    ipc
                      .workerRestartLight(taskId)
                      .catch((err) => push({ who: "sys", text: t("sys_light_restart_err", { e: String(err) }) }))
                  }
                />
              </Composer>
              </>
              )}
            </div>
          </Panel>
          {railOn && (
            <>
              <PanelResizeHandle className="rhandle" />
              <Panel defaultSize={42} minSize={20} className="pane">
                {/* The rail: one panel, the whole height. */}
                <div className="rail">
                  <div className="rail-slot">
                    {railOn === "sessao"
                      ? frameSessao()
                      : railOn === "terminal"
                        ? frameTerminal()
                        : railOn === "board"
                          ? frameBoard()
                          : railOn === "artefatos"
                            ? frameArtefatos()
                            : railOn === "diff"
                              ? frameDiff()
                              : railOn === "browser"
                                ? frameBrowser()
                                : frameArquivos()}
                  </div>
                </div>
              </Panel>
            </>
          )}
        </PanelGroup>
      )}

      {chatPalette && (
        <ChatPalette
          chats={chats}
          onPick={(chat) => {
            recoverSession(chat);
            setChatPalette(false);
          }}
          onClose={() => setChatPalette(false)}
        />
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

      <ReportDialog
        open={reportOpen}
        onClose={() => setReportOpen(false)}
        onNote={(text) => push({ who: "sys", text })}
      />
      <Modals
        pending={pending}
        setPending={setPending}
        focusedTaskTitle={focusedTask?.title}
        onDispatch={(instruction, sessionId, taskTitle) => {
          setReading(null);
          const title = taskTitle ?? focusedTask?.title;
          if (title) reactivateIfDone(title);
          runDispatch(instruction, sessionId);
        }}
        onPickSession={recoverSession}
        onPickTask={(title, sessionId) => openTaskByTitle(title, sessionId)}
        onPickProject={async (path) => {
          const entry = await ipc.projectAdd(path).catch(() => null);
          if (!entry) {
            push({ who: "sys", text: t("sys_project_register_fail", { path }) });
            return;
          }
          push({ who: "sys", text: t("sys_project_added", { name: entry.name, path: entry.path }) });
          say(st("sp_project_added", { t: entry.name }));
          refresh();
          await ipc.openTask(entry.name, entry.path).catch(() => {});
        }}
        onCompactFirst={dispatchCompactFirst}
      />
    </div>
  );
}
