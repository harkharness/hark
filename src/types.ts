export type Reply = {
  fala: string;
  detalhes: string;
  itens: string[];
  cost_usd?: number;
  model?: string;
};

export type Project = { name: string; path: string };

export type VoxEvent =
  | { kind: "tool"; name: string; input: string }
  | { kind: "worker"; task_id: string; name: string; input: string }
  | { kind: "assistant_text"; task_id: string; text: string }
  | { kind: "tool_result"; task_id: string; content: string; is_error: boolean }
  | {
      kind: "worker_turn";
      task_id: string;
      text: string;
      cost_usd?: number;
      model?: string;
      is_error: boolean;
    }
  | { kind: "worker_exit"; task_id: string }
  | { kind: "session_started"; task_id: string; session_id: string }
  | { kind: "speaking"; on: boolean }
  | { kind: "status"; text: string }
  | { kind: "error"; text: string };

export type PermissionAsk = {
  request_id: string;
  task_id: string;
  tool_name: string;
  input: string;
};

export type Msg =
  | { who: "user"; text: string; image?: string; task?: string }
  | {
      who: "vox";
      text: string;
      detalhes?: string;
      itens?: string[];
      cost?: number;
      model?: string;
      task?: string;
    }
  | { who: "tool"; name: string; input: string; task?: string }
  | { who: "output"; content: string; error: boolean; task?: string }
  | {
      who: "permission";
      requestId: string;
      tool: string;
      input: string;
      decision?: "allow" | "deny";
      task?: string;
    }
  | { who: "sys"; text: string; task?: string };

export type Directives = {
  mode?: "manual" | "acceptEdits" | "plan" | "auto" | "bypass";
  effort?: "low" | "medium" | "high" | "xhigh" | "max";
  model?: string;
};

export type LiveWorker = {
  label: string;
  status: "running" | "turn_done" | "awaiting";
  directives: Directives;
};

export type DispatchOutcome =
  | { status: "started"; task_id: string; directives: Directives }
  | { status: "done"; task_id: string; summary: string; cost_usd?: number }
  | { status: "failed"; task_id: string; summary: string }
  | { status: "choice"; candidates: { session_id: string; title: string; last_ts: string }[] }
  | { status: "busy"; session_id: string }
  | { status: "no_match" };

export type TranscriptEntry = {
  ts: string;
  role: "user" | "assistant" | "tool_use" | "tool_result";
  text: string;
  tool?: string;
  is_error: boolean;
};

export type BoardTask = {
  title: string;
  status: "backlog" | "doing" | "waiting" | "done";
  note?: string;
  updated_at: string;
  session_ids: string[];
  pinned: boolean;
  workspace?: string | null;
};

export type GateOut = {
  acao: "meta_vox" | "continuar_task" | "trocar_task" | "nova_task" | "pergunta";
  confianca: number;
  motivo: string;
  aviso?: string;
  task_alvo?: string;
  needs_confirmation: boolean;
  cost_usd?: number;
};

export type TaskCommandResult =
  | { kind: "open"; title: string; session_id?: string }
  | {
      kind: "switch";
      title: string;
      session_id?: string;
      note?: string;
      instruction?: string;
    }
  | { kind: "renamed"; title: string }
  | { kind: "pinned"; title: string }
  | { kind: "archived"; title: string }
  | { kind: "open_file"; query: string; project?: string | null }
  | { kind: "project_added"; title: string; path: string }
  | { kind: "project_error"; title: string }
  | { kind: "new_chat"; title: string; path: string }
  | { kind: "not_found"; query: string };

export type Overview = {
  contexts: string[];
  active: string;
  workers: {
    task_id: string;
    status: string;
    workspace: string;
    summary: string;
    session_id: string;
  }[];
  board: BoardTask[];
  projects: Project[];
};

/** A file open in the local viewer. */
export type OpenFile = {
  /** Absolute path (what the backend reads/saves). */
  abs: string;
  /** Path relative to the project root (what the UI shows). */
  rel: string;
  project: Project;
};
