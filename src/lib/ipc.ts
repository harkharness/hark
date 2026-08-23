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
  TaskCommandResult,
  TranscriptEntry,
} from "../types";

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
) => invoke<DispatchOutcome>("worker_start", { instruction, sessionId, mode: mode ?? null });

export const workerSend = (
  taskId: string,
  text: string,
  images: [string, string][] = [],
) => invoke<Directives>("worker_send", { taskId, text, images: images.length > 0 ? images : null });

export const workerStop = (taskId: string) => invoke("worker_stop", { taskId });

/** Heavy session → fresh one on the same task, context rebuilt from a
 *  LOCAL brief (zero tokens). */
export const workerRestartLight = (taskId: string) =>
  invoke("worker_restart_light", { taskId });

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

/** Which surface a message to global vox belongs to (zero tokens). */
export const askLane = (text: string) => invoke<"lean" | "work">("ask_lane", { text });

/** Send to the mother's persistent work chat (full settings + MCP);
 *  spawns/resumes the chat worker as needed. Events arrive with
 *  task_id "vox-chat". */
export const voxChatSend = (text: string, images: [string, string][] = []) =>
  invoke<{ task_id: string; resumed: boolean }>("vox_chat_send", {
    text,
    images: images.length > 0 ? images : null,
  });

export const voxChatStatus = () =>
  invoke<{ alive: boolean; session_id: string | null }>("vox_chat_status");

/** Brand-new Claude Code session inside a project directory. */
export const chatStart = (projectPath: string, instruction: string, mode?: string) =>
  invoke<DispatchOutcome>("chat_start", { projectPath, instruction, mode: mode ?? null });

export const readTranscript = (sessionId: string, limit?: number) =>
  invoke<{ session_title: string | null; entries: TranscriptEntry[] }>("read_transcript", {
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
  invoke<Directives>("worker_set_mode", { taskId, mode });

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
    theme: string;
    prompt_budget_chars: number;
    hotkey: string;
    worker_budget_usd: number;
    worker_max_turns: number;
    worker_mode: string;
    assistant_name: string;
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
