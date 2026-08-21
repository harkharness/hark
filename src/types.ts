export type Reply = {
  fala: string;
  detalhes: string;
  itens: string[];
  cost_usd?: number;
  model?: string;
};

export type Project = { name: string; path: string };

export type TurnUsage = {
  input: number;
  output: number;
  cache_read: number;
  cache_created: number;
};

export type VoxEvent =
  | { kind: "tool"; name: string; input: string }
  | { kind: "worker"; task_id: string; name: string; input: string }
  | { kind: "assistant_text"; task_id: string; text: string }
  | { kind: "tool_result"; task_id: string; content: string; is_error: boolean }
  | {
      kind: "worker_turn";
      task_id: string;
      /** Human name of the task (session title), for announcements. */
      label?: string;
      text: string;
      cost_usd?: number;
      model?: string;
      is_error: boolean;
      usage?: TurnUsage;
      context_pct?: number | null;
    }
  | {
      kind: "rate_limit";
      status: "allowed" | "allowed_warning" | "rejected";
      resets_at?: number | null;
      limit_kind?: string | null;
    }
  | { kind: "worker_exit"; task_id: string }
  | { kind: "session_started"; task_id: string; session_id: string }
  | { kind: "speaking"; on: boolean }
  | { kind: "hotkey_mic" }
  | { kind: "main_tab"; tab: string }
  | { kind: "focus_task"; title: string; session_id?: string | null }
  | { kind: "hud_listen" }
  /** A permission was decided somewhere: every window resolves its card. */
  | { kind: "permission_decided"; request_id: string; allow: boolean }
  /** config.toml changed via the settings UI: re-read what you show. */
  | { kind: "config_changed" }
  | { kind: "voice_action"; utterance: string; target?: string | null; status?: string }
  | { kind: "status"; text: string }
  | { kind: "error"; text: string };

export type PermissionAsk = {
  request_id: string;
  task_id: string;
  /** Human name of the task asking. */
  label?: string;
  tool_name: string;
  input: string;
};

export type Msg =
  | { who: "user"; text: string; images?: string[]; task?: string; ts?: number }
  | {
      who: "vox";
      text: string;
      detalhes?: string;
      itens?: string[];
      cost?: number;
      model?: string;
      usage?: TurnUsage;
      task?: string;
      /** Epoch ms, stamped by push() — feeds the hover "há N min". */
      ts?: number;
    }
  | { who: "tool"; name: string; input: string; task?: string }
  | { who: "output"; content: string; error: boolean; task?: string }
  | {
      who: "permission";
      requestId: string;
      tool: string;
      input: string;
      decision?: "allow" | "deny";
      /** Decided by a standing "sempre permitir" rule, not a click. */
      auto?: boolean;
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
  | { status: "failed"; task_id: string; summary: string; cost_usd?: number }
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
  | { kind: "new_chat"; title: string; path: string; instruction?: string | null }
  | { kind: "open_project"; title: string; path: string; instruction?: string | null }
  | { kind: "open_hq"; tab: "board" | "custos" }
  | { kind: "session_candidates"; query: string; candidates: SessionHit[] }
  /** Board tasks too close to call: the user picks, never a silent guess. */
  | {
      kind: "task_candidates";
      query: string;
      candidates: { title: string; session_id?: string | null; workspace?: string | null }[];
    }
  | { kind: "not_found"; query: string }
  /** "compacta o contexto": deliver "/compact" to the focused session. */
  | { kind: "compact" }
  /** "muda o modo pra X": switch the focused worker's permission mode. */
  | { kind: "set_mode"; mode: string }
  /** "abre as configurações": the settings modal on the mother window. */
  | { kind: "open_settings" };

/** One indexed session offered when recovering work by topic. */
export type SessionHit = {
  session_id: string;
  /** Never empty: real title, opening prompt, or the id. */
  title: string;
  cwd?: string | null;
  last_ts?: string | null;
  last_prompt?: string | null;
};

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
  /** Code color scheme from config.toml ("vox" | "dracula" | custom). */
  theme: string;
  /** Config default permission mode for new workers ("" = CLI default). */
  default_mode: string;
  /** UI language ("pt" | "en") — separate from the spoken one. */
  ui_language: string;
  /** The assistant's own name (chat header, feed, announcements). */
  assistant_name: string;
};

/** One aggregated bucket of the persistent spend ledger. */
export type SpendAgg = {
  key: string;
  cost_usd: number;
  input: number;
  output: number;
  cache_read: number;
  cache_created: number;
  turns: number;
  errors: number;
};

export type ContextWeight = {
  last_total_tokens: number;
  /** The total split into the parts that occupy the window. */
  cache_read: number;
  cache_created: number;
  input: number;
  output: number;
  context_window: number | null;
  pct: number | null;
};

/** One subscription usage window (5h, weekly, per-model weekly). */
export type LimitWindow = {
  key: string;
  /** 0..1 */
  used: number;
  resets_at?: string | null;
};

/** Last statusLine payload — the only source of subscription percentages. */
export type StatusLine = {
  context_used: number | null;
  context_tokens: number | null;
  context_window: number | null;
  model: string | null;
  session_id: string | null;
  limits: LimitWindow[];
};

export type BridgeStatus = {
  installed: boolean;
  age_secs: number | null;
  payload_path: string;
};

export type RateLimitState = {
  status: "allowed" | "allowed_warning" | "rejected";
  resets_at?: number | null;
  limit_kind?: string | null;
};

/** A file open in the local viewer. */
export type OpenFile = {
  /** Absolute path (what the backend reads/saves). */
  abs: string;
  /** Path relative to the project root (what the UI shows). */
  rel: string;
  project: Project;
};

/** A local, deterministic dispatch warning with its offered actions. */
export type DispatchWarning = {
  kind: "big_history" | "full_context";
  text: string;
  actions: ("compact_first" | "proceed")[];
};

/** One possible destination offered on the HUD. */
export type VoiceCandidate = {
  title: string;
  session_id?: string | null;
  workspace?: string | null;
  project_name?: string | null;
};

/** Where a spoken utterance will land (shown on the HUD before running). */
export type VoicePlan =
  | { kind: "command"; command: TaskCommandResult }
  | {
      kind: "work";
      instruction: string;
      task_title?: string | null;
      session_id?: string | null;
      workspace?: string | null;
      project_name?: string | null;
      new_task: boolean;
      /** "high" = silence confirms (chat on screen / unique address);
       *  "low" = search-resolved: an explicit verdict is required. */
      confidence: "high" | "low";
      /** Local precheck texts: any warning downgrades to explicit verdict
       *  and offers "compacta antes" / "segue". */
      warnings: string[];
    }
  | { kind: "candidates"; instruction: string; options: VoiceCandidate[] }
  | { kind: "question"; question: string }
  | { kind: "no_target"; instruction: string }
  /** A clean yes/no while a permission card waits anywhere. */
  | {
      kind: "permission_answer";
      request_id: string;
      label: string;
      tool: string;
      allow: boolean;
      always: boolean;
    };
