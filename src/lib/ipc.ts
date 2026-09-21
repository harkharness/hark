// Typed wrappers around every Tauri command. Components never call
// `invoke` directly: this file is the single seam between the React UI
// and the Rust shell, so command names/payloads live in one place.

import { invoke } from "@tauri-apps/api/core";
import type {
  DispatchOutcome,
  Directives,
  GateOut,
  Overview,
  Project,
  Reply,
  RepoState,
  TaskCommandResult,
  TranscriptEntry,
} from "../types";

/** Git state for a set of task workspaces, in one call. Cached in Rust
 *  for a few seconds, so asking on a render is cheap. */
export const repoStates = (paths: string[]) =>
  invoke<Record<string, RepoState>>("repo_states", { paths });

export const overview = () => invoke<Overview>("overview");
export const useContext = (name: string) => invoke("use_context", { name });
export const routeText = (text: string) => invoke<string>("route_text", { text });
export const speak = (text: string) => invoke("speak", { text });
export const speakStop = () => invoke("speak_stop");
/** One mic, one owner: a busy mic rejects with "mic_busy:<owner>". */
export const hearOnce = (owner?: string) =>
  invoke<string>("hear_once", { owner: owner ?? null });
/** Esc while recording: cut the capture and transcribe what was said. */
export const hearStop = () => invoke("hear_stop");
/** Esc once the words no longer matter: kill the in-flight hear_once —
 *  capturing or transcribing — and reject it with "mic_aborted". */
export const hearAbort = () => invoke("hear_abort");

/** Spoken verdict on whatever is pending — the domain grammar decides. */
export type VerdictOut =
  | { kind: "confirm"; always: boolean }
  | { kind: "deny" }
  | { kind: "pick"; index: number }
  | { kind: "action"; id: string }
  | { kind: "instruction"; text: string }
  | { kind: "unknown" };
export const interpretVerdict = (
  utterance: string,
  options?: string[],
  actions?: [string, string[]][],
) =>
  invoke<VerdictOut>("interpret_verdict", {
    utterance,
    options: options ?? null,
    actions: actions ?? null,
  });

/** Ask with any pasted screenshots as ordered (media, base64) pairs. */
export const askText = (
  question: string,
  images: [string, string][] = [],
  project?: Project,
) =>
  invoke<Reply>("ask_text", {
    question,
    images: images.length > 0 ? images : null,
    projectName: project?.name ?? null,
    projectPath: project?.path ?? null,
  });

export const workerStart = (
  instruction: string,
  sessionId: string | null,
  mode?: string,
  /** Fork: a NEW session seeded with sessionId's history (--fork-session)
   *  — parallel work while a terminal holds the original. */
  fork?: boolean,
  /** Window's model selector (spoken directives still win). */
  model?: string,
  /** Window's effort selector; "" or undefined means no --effort at all. */
  effort?: string,
) =>
  invoke<DispatchOutcome>("worker_start", {
    instruction,
    sessionId,
    mode: mode ?? null,
    model: model ?? null,
    effort: effort || null,
    fork: fork ?? null,
  });

/** `queued` names the message the worker parked because it was busy — the
 *  same id the window pushed, so the bubble and the queue entry are one
 *  thing. Absent means it went out immediately. */
export type WorkerSendOut = { directives: Directives; queued: string | null };

export const workerSend = (
  taskId: string,
  text: string,
  images: [string, string][] = [],
  msgId?: string,
) =>
  invoke<WorkerSendOut>("worker_send", {
    taskId,
    text,
    images: images.length > 0 ? images : null,
    msgId: msgId ?? null,
  });

/** Push a waiting message ahead of the running turn: it jumps the queue
 *  and the turn is cut so it can run now. */
export const workerQueuedNow = (taskId: string, id: string) =>
  invoke<void>("worker_queued_now", { taskId, id });

/** Take a waiting message back before it ever runs. */
export const workerQueuedDrop = (taskId: string, id: string) =>
  invoke<void>("worker_queued_drop", { taskId, id });

export const workerStop = (taskId: string) => invoke("worker_stop", { taskId });

/** Heavy session → fresh one on the same task, context rebuilt from a
 *  LOCAL brief (zero tokens). */
export const workerRestartLight = (taskId: string) =>
  invoke("worker_restart_light", { taskId });
/** Move a live task to another agent: fresh session there, opened with a
 *  local brief of this one (zero tokens); same card, lineage kept. */
export const workerHandoff = (taskId: string, agent: string) =>
  invoke("worker_handoff", { taskId, agent });

/** The savings meter: what the architecture avoided spending, with the
 *  formula of every counter in `methodology`. */
export type SavingsOut = {
  avoided_gate_usd: number;
  avoided_local_usd: number;
  avoided_cache_usd: number;
  total_usd: number;
  methodology: string[];
};
export const savingsSummary = (since: string | null) =>
  invoke<SavingsOut>("savings_summary", { since });

/** Community eco-tools on this machine and how workers run them. */
export type EcoOut = {
  status: { rtk: boolean; ponytail: boolean; caveman: boolean; tokensave: boolean };
  envs: [string, string][];
  fingerprint: string;
};
export const ecoStatus = () => invoke<EcoOut>("eco_status");

/** Tick/untick one step of a task's plan checklist. */
export const boardSubtaskToggle = (title: string, index: number, done: boolean) =>
  invoke("board_subtask_toggle", { title, index, done });

/** Which surface a message to global hark belongs to (zero tokens). */
export const askLane = (text: string) => invoke<"lean" | "work">("ask_lane", { text });

/** One spoken sentence placed by the light model, with the local catalog
 *  in the prompt. The fallback that used to be a prose answer. */
export type SpokenIntent = {
  kind: "open_project" | "open_session" | "dispatch" | "status" | "question" | "clarify";
  project?: string | null;
  session_id?: string | null;
  session_title?: string | null;
  instruction?: string | null;
  question?: string | null;
  options: { session_id: string; title: string; project?: string | null }[];
  cost_usd?: number | null;
};
export const classifyUtterance = (utterance: string, recent: string[]) =>
  invoke<SpokenIntent>("classify_utterance", { utterance, recent });

/** What a reply to "terminei lá — quer que eu faça algo?" meant. */
export const interpretFollowup = (utterance: string, label: string) =>
  invoke<{ kind: "go" | "stay" | "do" | "unrelated"; instruction?: string }>(
    "interpret_followup",
    { utterance, label },
  );

/** Send to the mother's persistent work chat (full settings + MCP);
 *  spawns/resumes the chat worker as needed. Events arrive with
 *  task_id "hark-chat". */
export const harkChatSend = (
  text: string,
  images: [string, string][] = [],
  msgId?: string,
) =>
  invoke<{ task_id: string; resumed: boolean; queued: string | null }>("hark_chat_send", {
    text,
    images: images.length > 0 ? images : null,
    msgId: msgId ?? null,
  });

export const harkChatStatus = () =>
  invoke<{ alive: boolean; session_id: string | null }>("hark_chat_status");

/** Brand-new Claude Code session inside a project directory. */
export const chatStart = (
  projectPath: string,
  instruction: string,
  mode?: string,
  model?: string,
  effort?: string,
  /** Registry id to open the chat on ("abre um chat gemini…"); default when absent. */
  agent?: string,
) =>
  invoke<DispatchOutcome>("chat_start", {
    projectPath,
    instruction,
    mode: mode ?? null,
    model: model ?? null,
    effort: effort || null,
    agent: agent ?? null,
  });

/** Sessions a human is holding at a terminal (ours or any other app). */
export const sessionOwners = () =>
  invoke<import("../types").SessionOwner[]>("session_owners");

/** Follow a session's log from a byte offset — the terminal mirror. */
export const transcriptSince = (sessionId: string, offset: number) =>
  invoke<{ entries: import("../types").TranscriptEntry[]; offset: number }>(
    "transcript_since",
    { sessionId, offset },
  );

/** The /usage card: session breakdown + machine 24h + limits. Local. */
export const usageReport = (sessionId?: string) =>
  invoke<import("../types").UsageReport>("usage_report", { sessionId: sessionId ?? null });

/** Newest ask turns from the global journal — restores the mother thread. */
export const journalRecent = (n?: number) =>
  invoke<import("../types").JournalTurn[]>("journal_recent", { n: n ?? null });

/** A sentence citing another chat: the local excerpt to travel with it. */
export const crossrefContext = (utterance: string) =>
  invoke<{ title: string; session_id: string; block: string; lines: number } | null>(
    "crossref_context",
    { utterance },
  );

export const readTranscript = (sessionId: string, limit?: number) =>
  invoke<{ session_title: string | null; entries: TranscriptEntry[]; offset: number }>("read_transcript", {
    sessionId,
    limit,
  });

/** Sessions matching a topic, from the local index — zero tokens, no agent. */
export const sessionCandidates = (query: string, limit?: number) =>
  invoke<import("../types").SessionHit[]>("session_candidates", {
    query,
    limit: limit ?? null,
  });

/** Every Claude Code session of one project (local index, zero tokens). */
export const projectSessions = (path: string) =>
  invoke<import("../types").SessionHit[]>("project_sessions", { path });

/** Bind a recovered session to a board task named after the SESSION. */
export const taskFromSession = (sessionId: string) =>
  invoke<{ title: string; workspace?: string | null; session_id: string }>(
    "task_from_session",
    { sessionId },
  );

export const findSession = (query: string) =>
  invoke<{ session_id: string; title?: string; cwd?: string | null } | null>("find_session", {
    query,
  });

/** Ledger aggregation. USD lives in source="live"; token history in
 * source="jsonl". Never sum across sources. */
export const spendSummary = (
  since: string | null,
  group: "kind" | "model" | "label" | "workspace" | "day" | "session" | "task",
  source: "live" | "jsonl",
  /** Project root: scopes every number to that directory and below. */
  workspace?: string | null,
) =>
  invoke<import("../types").SpendAgg[]>("spend_summary", {
    since,
    group,
    source,
    workspace: workspace ?? null,
  });

/** Subscription windows (5h/weekly). null until the bridge is installed. */
export const subscriptionLimits = () =>
  invoke<import("../types").StatusLine | null>("subscription_limits");

export const statuslineBridgeStatus = () =>
  invoke<import("../types").BridgeStatus>("statusline_bridge_status");
/** Edits ~/.claude/settings.json (with a backup) — click only. */
export const statuslineBridgeInstall = () =>
  invoke<string | null>("statusline_bridge_install");
export const statuslineBridgeUninstall = () => invoke("statusline_bridge_uninstall");

export const spendTopSessions = (since: string, limit: number) =>
  invoke<import("../types").SpendAgg[]>("spend_top_sessions", { since, limit });

export const sessionContextWeight = (sessionId: string) =>
  invoke<import("../types").ContextWeight>("session_context_weight", { sessionId });

/** Local, zero-token stats of one session (size = resume weight). */
export const sessionStats = (sessionId: string) =>
  invoke<{ title: string | null; size_mb: number; entries: number; last_ts: string | null }>(
    "session_stats",
    { sessionId },
  );

/** Local spoken commands. `focused` names the task the window has open,
 *  so "renomeia esse chat para X" knows what "esse" is. */
export const taskCommand = (text: string, focused?: string) =>
  invoke<TaskCommandResult | null>("task_command", { text, focused: focused ?? null });

export const evaluate = (message: string, focusedTask: string, focusedSession: string) =>
  invoke<GateOut>("evaluate", { message, focusedTask, focusedSession });

/** Slash commands the workspace's sessions accept (from the CLI's init). */
export const slashCommands = (workspace?: string) =>
  invoke<string[]>("slash_commands", { workspace: workspace ?? null });

export const boardMove = (title: string, status: string) =>
  invoke("board_move", { title, status });
export const boardRename = (title: string, newTitle: string) =>
  invoke("board_rename", { title, newTitle });
export const boardPin = (title: string) => invoke("board_pin", { title });
export const boardArchive = (title: string) => invoke("board_archive", { title });

export const approve = (requestId: string, allow: boolean) =>
  invoke("approve", { requestId, allow });

export const projectAdd = (path: string) => invoke<Project>("project_add", { path });
export const projectRemove = (key: string) => invoke("project_remove", { key });

/** Open (or focus) a project's own window — the VSCode model. */
/** Open (or focus) a project window. With `task`, that task's chat is the
 *  landing screen — a card click on the global board resumes the work. */
export const openProjectWindow = (
  name: string,
  path: string,
  task?: string,
  session?: string,
) =>
  invoke("open_project_window", {
    name,
    path,
    task: task ?? null,
    session: session ?? null,
  });

/** Bring the mother window to the front, optionally on a specific tab
 * (board/custos are global and live there). */
export const focusMain = (tab?: "board" | "custos" | "settings") =>
  invoke("focus_main", { tab: tab ?? null });

/** Fuzzy file search inside one project (relative paths). */
export const projectFiles = (path: string, query: string, limit?: number) =>
  invoke<string[]>("project_files", { path, query, limit });

export const fileRead = (path: string) =>
  invoke<{ content: string; truncated: boolean }>("file_read", { path });
export const fileSave = (path: string, content: string) =>
  invoke("file_save", { path, content });

/* ---- real terminals (PTY per tab) ---- */
export const termOpen = (id: string, cwd?: string, cols?: number, rows?: number) =>
  invoke("term_open", { id, cwd: cwd ?? null, cols: cols ?? null, rows: rows ?? null });
export const termWrite = (id: string, data: string) => invoke("term_write", { id, data });
export const termResize = (id: string, cols: number, rows: number) =>
  invoke("term_resize", { id, cols, rows });
export const termClose = (id: string) => invoke("term_close", { id });

/** Switch a live worker's permission mode (restarts it, sends no text). */
export const workerSetMode = (taskId: string, mode: string) =>
  invoke<{ directives: Directives; restarted: boolean }>("worker_set_mode", { taskId, mode });
export const workerSetModel = (taskId: string, model: string) =>
  invoke<{ directives: Directives; restarted: boolean }>("worker_set_model", { taskId, model });
/** Stop the turn in flight; the session stays and answers the next message. */
export const workerInterrupt = (taskId: string) => invoke<void>("worker_interrupt", { taskId });
/** Reasoning effort; "" clears the directive (back to the CLI's default). */
export const workerSetEffort = (taskId: string, effort: string) =>
  invoke<{ directives: Directives; restarted: boolean }>("worker_set_effort", { taskId, effort });

/* ---- global voice (HUD) ---- */
export const planUtterance = (text: string) =>
  invoke<import("../types").VoicePlan>("plan_utterance", { text });
export const voiceExecute = (
  plan: {
    instruction: string;
    session_id?: string | null;
    workspace?: string | null;
    project_name?: string | null;
    task_title?: string | null;
    new_task: boolean;
  },
  /** "compacta antes": /compact runs as its own turn, then the message. */
  compactFirst = false,
) =>
  invoke("voice_execute", {
    instruction: plan.instruction,
    sessionId: plan.session_id ?? null,
    workspace: plan.workspace ?? null,
    projectName: plan.project_name ?? null,
    taskTitle: plan.task_title ?? null,
    newTask: plan.new_task,
    compactFirst,
  });

/** Local dispatch warnings for a session (size, context) — zero tokens. */
export const dispatchPrechecks = (sessionId: string) =>
  invoke<import("../types").DispatchWarning[]>("dispatch_prechecks", { sessionId });

/** The user's config.toml as the settings UI sees it. */
export type ConfigSnapshot = {
  values: {
    claude_bin: string;
    model: string;
    projects_dir: string;
    language: string;
    ui_language: string;
    voice: string;
    whisper_model: string;
    stt?: string;
    theme: string;
    prompt_budget_chars: number;
    hotkey: string;
    worker_budget_usd: number;
    worker_max_turns: number;
    worker_mode: string;
    worker_model: string;
    worker_effort: string;
    assistant_name: string;
    auto_update: boolean;
    models: { light?: string | null; standard?: string | null; heavy?: string | null; max?: string | null };
  };
  path: string;
  claude_bin_resolved: string;
  whisper_model_resolved: string;
  data_dir: string;
};
export const configRead = () => invoke<ConfigSnapshot>("config_read");
/** Surgical patch into config.toml (comments survive); hot-applies. */
export const configWrite = (patch: Record<string, string | number | boolean>) =>
  invoke("config_write", { patch });
/** config.toml as text, for the editor inside Settings. */
export const configRawRead = () => invoke<{ text: string; path: string }>("config_raw_read");
/** Write config.toml whole; the backend validates it before writing. */
export const configRawWrite = (text: string) => invoke("config_raw_write", { text });
export const ttsVoices = () => invoke<[string, string][]>("tts_voices");

/** Open a URL/file with the OS (default browser/app) — never in-webview. */
export const openExternal = (target: string) => invoke("open_external", { target });
export const setActiveContext = (ctx: {
  project_path?: string | null;
  project_name?: string | null;
  task_title?: string | null;
  session_id?: string | null;
}) => invoke("set_active_context", { ctx });
export const hudHide = () => invoke("hud_hide");
/** Open the global voice HUD — every mic button funnels here now. */
export const hudShow = () => invoke("hud_show");

// --- first-run wizard -------------------------------------------------
export type SetupStatus = {
  onboarded: boolean;
  config_exists: boolean;
  whisper_ok: boolean;
  whisper_path: string;
  claude_bin: string;
  claude_ok: boolean;
  /** Some enabled, detected backend can drive Hark — claude or any ACP agent. */
  agent_ok: boolean;
  projects_dir_ok: boolean;
  models: { key: string; filename: string; size_label: string; recommended: boolean }[];
  /** Why the recommended model is the small one, when it is ("no_metal_cpu"). */
  stt_reco_reason: string | null;
  language: string;
  assistant_name: string;
  hotkey: string;
};
export const setupStatus = () => invoke<SetupStatus>("setup_status");
/** Background download; progress arrives as `hark-setup` events. */
export const setupDownloadModel = (key: string) =>
  invoke("setup_download_model", { key });
export const setupMarkDone = () => invoke("setup_mark_done");

// --- agent plugins (the backend catalog) ------------------------------
export type AgentCapabilities = {
  resume: boolean;
  permissions: boolean;
  structured_output: boolean;
  cost_reporting: boolean;
  history: boolean;
  live_list: boolean;
  slash_commands: boolean;
  memory_file: string | null;
  shell_tools: string[];
  fork: boolean;
  /** Each directive pill works for this agent: a CLI flag, or an ACP
   *  mode / config option the agent offered at session/new. */
  directive_mode: boolean;
  directive_model: boolean;
  directive_effort: boolean;
};
export type AgentPlugin = {
  id: string;
  name: string;
  /** Which plugin speaks to it: "claude" (native) or "acp". */
  plugin: string;
  /** Binary Hark looks for and spawns. */
  cmd: string;
  vendor: string;
  status: "available" | "planned";
  /** The binary is on this machine. */
  detected: boolean;
  /** The user left this backend switched on in config. */
  enabled: boolean;
  detail: string;
  install: string;
  selected: boolean;
  memory_file: string | null;
  login_hint: string | null;
  /** Extra args the registry entry passes to the binary. */
  args: string[];
  /** NAMES of the env vars set on this agent's processes — never the values. */
  env_keys: string[];
  /** What this agent calls each tier (light/standard/heavy/max) — the
   *  registry line with the user's overrides merged. Empty = no table:
   *  the model pill says so rather than offering claude's names. */
  models: Record<string, string>;
  /** Installed against what the ACP registry publishes. */
  version: AgentVersion;
  /** Only the native plugin declares one today; ACP negotiates at
   *  handshake, so its sheet is null until the runtime lands. */
  capabilities: AgentCapabilities | null;
};
export type AgentVersion = {
  /** From `<cmd> --version`, when the binary is here and answers. */
  installed: string | null;
  /** From the last registry reading, when the agent is listed there. */
  current: string | null;
  freshness:
    | { state: "unknown" }
    | { state: "current" }
    | { state: "behind"; installed: string; current: string };
  /** The one line that brings the package to `current` (npm); null for archives. */
  update: string | null;
  /** When Hark last read the registry; null = never. */
  checked_at: string | null;
};
export const agentPlugins = () => invoke<AgentPlugin[]>("agent_plugins");
/** Read the ACP registry now; answers the refreshed catalog. */
export const agentRegistryRefresh = () => invoke<AgentPlugin[]>("agent_registry_refresh");
export const agentPluginSelect = (id: string) => invoke("agent_plugin_select", { id });
/** `[agents.<id>] enabled` — the catalog's on/off switch, persisted in config. */
export const agentPluginEnable = (id: string, enabled: boolean) =>
  invoke("agent_plugin_enable", { id, enabled });
/** The agent that runs (or would run) a session — "" for a new chat. */
export const agentForSession = (sessionId: string) =>
  invoke<string>("agent_for_session", { sessionId });
