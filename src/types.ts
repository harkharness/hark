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

export type HarkEvent =
  | { kind: "tool"; name: string; input: string }
  | { kind: "worker"; task_id: string; name: string; input: string }
  | { kind: "assistant_text"; task_id: string; text: string }
  | { kind: "tool_result"; task_id: string; content: string; is_error: boolean }
  | {
      kind: "worker_turn";
      task_id: string;
      /** Human name of the task (session title), for announcements. */
      label?: string;
      /** The session this turn wrote to — the butler offer's target. */
      session_id?: string | null;
      /** Registry id of the backend that ran the turn ("claude", "gemini"). */
      agent?: string;
      /** How one logs in to THAT agent — the auth card's runnable fix. */
      login_hint?: string | null;
      text: string;
      cost_usd?: number;
      model?: string;
      is_error: boolean;
      /** The user stopped this turn; the CLI's own error flag is not one. */
      stopped?: boolean;
      /** Failure class of an error turn ("agent_auth"): fatal — never
       *  auto-resent, and the window offers the remedy. */
      error_code?: string | null;
      usage?: TurnUsage;
      context_pct?: number | null;
      /** The window the model said it had — lets the reading be checked. */
      context_window?: number | null;
    }
  | {
      kind: "rate_limit";
      status: "allowed" | "allowed_warning" | "rejected";
      resets_at?: number | null;
      limit_kind?: string | null;
    }
  /** What a worker is doing while it is quiet, straight from the CLI's
   *  own status line and its partial-message deltas. */
  | { kind: "phase"; task_id: string; phase: "requesting" | "thinking" | "writing" }
  | { kind: "worker_exit"; task_id: string; exit_code?: number | null; reason?: string | null }
  | { kind: "session_started"; task_id: string; session_id: string }
  | { kind: "speaking"; on: boolean }
  /** What hear_once is doing right now. The promise alone can't say: on
   *  CPU the transcription grinds long after the capture ended, and a UI
   *  stuck on "listening" the whole way read as frozen. */
  | { kind: "mic"; phase: "capturing" | "transcribing" | "idle"; owner: string }
  | { kind: "hotkey_mic" }
  | { kind: "main_tab"; tab: string }
  | { kind: "focus_task"; title: string; session_id?: string | null }
  | { kind: "hud_listen" }
  /** A permission was decided somewhere: every window resolves its card. */
  | { kind: "permission_decided"; request_id: string; allow: boolean }
  /** config.toml changed via the settings UI: re-read what you show. */
  | { kind: "config_changed" }
  | { kind: "voice_action"; utterance: string; target?: string | null; status?: string }
  /** Spoken "sempre pode": the owning window records the standing rule. */
  | { kind: "allow_rule"; label: string; tool: string }
  /** A spoken turn handled by the HUD, echoed into the mother's thread:
   *  the question always; the reply too when the lean ask answered it. */
  | {
      kind: "chat_echo";
      question: string;
      reply?: { fala: string; detalhes?: string; itens?: string[]; cost_usd?: number; model?: string };
      work: boolean;
    }
  /** A message the outbox was holding just went out as its own turn. */
  | { kind: "queued_sent"; task_id: string; id: string }
  /** ...or never will: the thread died holding it, or delivery failed. */
  | { kind: "queued_failed"; task_id: string; id: string; why?: string }
  | { kind: "status"; text: string }
  | { kind: "error"; text: string };

export type PermissionAsk = {
  request_id: string;
  task_id: string;
  /** Human name of the task asking. */
  label?: string;
  tool_name: string;
  input: string;
  /** Set when the command touches production (domain::prodgate): the ask
   *  must reach a human — no standing rule may answer it. */
  prod_risk?: string | null;
};

export type Msg =
  | {
      who: "user";
      text: string;
      images?: string[];
      task?: string;
      ts?: number;
      /** Shared with the backend's outbox, so a bubble on screen and a
       *  message waiting in the queue are the same thing. */
      msgId?: string;
      /** Typed while the agent was still working, so it has not run yet:
       *  "waiting" can be pushed ahead of the running turn or taken back,
       *  "failed" is a message the thread died holding. */
      queued?: { state: "waiting" | "failed"; why?: string };
    }
  | { who: "usage"; report: UsageReport; task?: string; ts?: number }
  | {
      who: "hark";
      text: string;
      detalhes?: string;
      itens?: string[];
      cost?: number;
      model?: string;
      /** Registry id of the agent that answered; the footer signs and
       *  explains a missing price with it. */
      agent?: string;
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
      /** Production-gate reason: card turns red, "always" disappears. */
      prodRisk?: string | null;
      /** Human name of the asker (assistant/session title) for the card. */
      label?: string;
      /** The worker died before anyone answered: the buttons would talk to
       *  a process that no longer exists. */
      expired?: boolean;
      task?: string;
    }
  /** The CLI's compaction summary: history, not conversation — folded. */
  | { who: "compact"; text: string; task?: string }
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
  /** How full the session's context window is (0..1), from the last turn. */
  context_pct?: number | null;
  /** The window the model reported for that turn. */
  context_window?: number | null;
  /** Model that produced the last turn (the pill shows the truth). */
  model?: string | null;
  /** Registry id of the agent running it ("claude", "gemini", …). */
  agent?: string | null;
};

export type DispatchOutcome =
  | { status: "started"; task_id: string; directives: Directives }
  | { status: "done"; task_id: string; summary: string; cost_usd?: number }
  | { status: "failed"; task_id: string; summary: string; cost_usd?: number }
  | { status: "choice"; candidates: { session_id: string; title: string; last_ts: string }[] }
  | { status: "busy"; session_id: string }
  | { status: "no_match" };

/** A session held by a human at a terminal — nobody else may write it. */
export type SessionOwner = {
  session_id: string;
  /** The CLI's own name for the session ("workspace-fabrica-c7"). */
  name: string;
  cwd: string;
  pid?: number | null;
  started_at?: number | null;
};

/** One remembered ask turn from the global journal (mother restore). */
export type JournalTurn = {
  ts: string;
  question: string;
  fala: string;
  body: string;
};

export type TranscriptEntry = {
  ts: string;
  role: "user" | "assistant" | "tool_use" | "tool_result" | "compaction";
  text: string;
  tool?: string;
  is_error: boolean;
};

/** Git state of the repository a task works in (zero tokens, from `git`). */
export type RepoState = {
  /** Repository ROOT — often an ancestor of the task's own workspace. */
  root: string;
  branch: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  /** Files changed but not committed: staged, unstaged and untracked. */
  dirty: number;
  added: number;
  removed: number;
};

export type BoardTask = {
  title: string;
  status: "backlog" | "doing" | "waiting" | "done";
  /** The approved plan as a checklist (user-toggled). */
  subtasks?: { text: string; done: boolean }[];
  note?: string;
  updated_at: string;
  session_ids: string[];
  pinned: boolean;
  workspace?: string | null;
};

export type GateOut = {
  acao: "meta_hark" | "continuar_task" | "trocar_task" | "nova_task" | "pergunta";
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
  | { kind: "done"; title: string }
  | { kind: "open_file"; query: string; project?: string | null }
  | { kind: "project_added"; title: string; path: string }
  | { kind: "project_error"; title: string }
  | { kind: "new_chat"; title: string; path: string; instruction?: string | null; agent?: string | null }
  | { kind: "open_project"; title: string; path: string; instruction?: string | null }
  /** "troca esse chat pro gemini": the focused thread moves to `agent`. */
  | { kind: "handoff"; agent: string; name: string }
  | { kind: "agent_error"; title: string }
  | { kind: "open_hq"; tab: "board" | "custos" }
  | { kind: "session_candidates"; query: string; candidates: SessionHit[] }
  /** Board tasks too close to call: the user picks, never a silent guess. */
  /** Spoken project not registered, but directories on disk match: offer
   *  them — registering is one pick away, never a dead end. */
  | { kind: "project_offer"; query: string; candidates: string[] }
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
  /** Code color scheme from config.toml ("hark" | "dracula" | custom). */
  theme: string;
  /** Config default permission mode for new workers ("" = CLI default). */
  default_mode: string;
  default_model: string;
  default_effort: string;
  /** UI language ("pt" | "en") — separate from the spoken one. */
  ui_language: string;
  /** SPOKEN language (STT/TTS) — drives the speech dictionary. */
  language: string;
  /** The assistant's own name (chat header, feed, announcements). */
  assistant_name: string;
  /** The human's first name (from $USER), for the greeting. */
  user_name: string;
};

/** One aggregated bucket of the persistent spend ledger. */
export type SpendAgg = {
  key: string;
  /** Sum over the turns that carry a price — a floor when `priced_turns < turns`. */
  cost_usd: number;
  /** How many of `turns` reported USD at all. */
  priced_turns: number;
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
/** One model's line in the /usage breakdown (domain::usage::ModelLine). */
export type UsageModelLine = {
  model: string;
  input: number;
  output: number;
  cache_read: number;
  cache_created: number;
  cost_usd: number;
  turns: number;
};

export type UsageAgg = {
  key: string;
  cost_usd: number;
  input: number;
  output: number;
  cache_read: number;
  cache_created: number;
  turns: number;
  errors: number;
};

/** The /usage card: session breakdown + machine 24h + subscription. */
export type UsageReport = {
  session: {
    models: UsageModelLine[];
    turns: number;
    cost_usd: number;
    duration_ms: number;
    cache_hit: number | null;
    context: { total: number; window: number | null; pct: number | null };
  } | null;
  day: { total_usd: number; week_usd: number; kinds: UsageAgg[]; top: UsageAgg[] };
  /** Every agent session on this box, from the jsonl index — exact tokens. */
  machine: { models: UsageModelLine[]; turns: number; workspaces: UsageAgg[] };
  limits: StatusLine | null;
};

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
  /** 1-based line to scroll to, when the path carried one ("foo.rs:38"). */
  line?: number;
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
