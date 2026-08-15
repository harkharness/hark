export type Reply = {
  fala: string;
  detalhes: string;
  itens: string[];
  cost_usd?: number;
  model?: string;
};

export type VoxEvent =
  | { kind: "tool"; name: string; input: string }
  | { kind: "worker"; task_id: string; name: string; input: string }
  | { kind: "status"; text: string }
  | { kind: "error"; text: string };

export type PermissionAsk = {
  request_id: string;
  task_id: string;
  tool_name: string;
  input: string;
};

export type Msg =
  | { who: "user"; text: string; image?: string }
  | { who: "vox"; text: string; detalhes?: string; itens?: string[]; cost?: number; model?: string }
  | { who: "sys"; text: string };

export type DispatchOutcome =
  | { status: "done"; task_id: string; summary: string; cost_usd?: number }
  | { status: "failed"; task_id: string; summary: string }
  | { status: "choice"; candidates: { session_id: string; title: string; last_ts: string }[] }
  | { status: "busy"; session_id: string }
  | { status: "no_match" };

export type BoardTask = {
  title: string;
  status: "backlog" | "doing" | "waiting" | "done";
  note?: string;
  updated_at: string;
  session_ids: string[];
};

export type Overview = {
  contexts: string[];
  active: string;
  workers: { task_id: string; status: string; workspace: string; summary: string }[];
  board: BoardTask[];
};
