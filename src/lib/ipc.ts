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
export const hearOnce = () => invoke<string>("hear_once");
/** Esc while recording: cut the capture and transcribe what was said. */
export const hearStop = () => invoke("hear_stop");

export const askText = (
  question: string,
  imageB64: string | null,
  mediaType: string | null,
  project?: Project,
) =>
  invoke<Reply>("ask_text", {
    question,
    imageB64,
    mediaType,
    projectName: project?.name ?? null,
    projectPath: project?.path ?? null,
  });

export const workerStart = (instruction: string, sessionId: string | null) =>
  invoke<DispatchOutcome>("worker_start", { instruction, sessionId });

export const workerSend = (
  taskId: string,
  text: string,
  imageB64: string | null = null,
  mediaType: string | null = null,
) => invoke<Directives>("worker_send", { taskId, text, imageB64, mediaType });

export const workerStop = (taskId: string) => invoke("worker_stop", { taskId });

/** Brand-new Claude Code session inside a project directory. */
export const chatStart = (projectPath: string, instruction: string) =>
  invoke<DispatchOutcome>("chat_start", { projectPath, instruction });

export const readTranscript = (sessionId: string, limit?: number) =>
  invoke<{ session_title: string | null; entries: TranscriptEntry[] }>("read_transcript", {
    sessionId,
    limit,
  });

export const findSession = (query: string) =>
  invoke<{ session_id: string; title?: string } | null>("find_session", { query });

/** Ledger aggregation. USD lives in source="live"; token history in
 * source="jsonl". Never sum across sources. */
export const spendSummary = (
  since: string | null,
  group: "kind" | "model" | "label" | "workspace" | "day" | "session",
  source: "live" | "jsonl",
) => invoke<import("../types").SpendAgg[]>("spend_summary", { since, group, source });

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

export const taskCommand = (text: string) =>
  invoke<TaskCommandResult | null>("task_command", { text });

export const evaluate = (message: string, focusedTask: string, focusedSession: string) =>
  invoke<GateOut>("evaluate", { message, focusedTask, focusedSession });

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
export const openProjectWindow = (name: string, path: string) =>
  invoke("open_project_window", { name, path });

/** Open (or focus) the global HQ window (board + costs across projects). */
export const openHqWindow = (tab?: "board" | "custos") =>
  invoke("open_hq_window", { tab: tab ?? null });

/** Fuzzy file search inside one project (relative paths). */
export const projectFiles = (path: string, query: string, limit?: number) =>
  invoke<string[]>("project_files", { path, query, limit });

export const fileRead = (path: string) =>
  invoke<{ content: string; truncated: boolean }>("file_read", { path });
export const fileSave = (path: string, content: string) =>
  invoke("file_save", { path, content });
