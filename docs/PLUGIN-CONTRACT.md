# The agent-plugin contract

For people writing or changing a backend. The user-facing overview is
[PLUGINS.md](PLUGINS.md).

Hark's core never talks to a specific agent CLI. A **plugin** does, and it
translates that CLI into the contract defined by the `hark-agent` crate.
The first plugin wraps the Claude Code CLI (`hark-plugin-claude`); a Gemini
(or any other) plugin implements the same contract and the whole product —
transcript, permissions, board, ledger, voice — keeps working.

The design lesson borrowed from other harnesses (notably deepseek-harness's
`ctx.llm` seam): **the contract is an event vocabulary plus a capability
sheet**, not a config format. If you can turn your CLI's output into
`AgentEvent`s, you have a plugin.

## The contract (`crates/hark-agent`)

Types every plugin speaks:

| Type | Role |
|---|---|
| `AgentEvent` | The event stream: `ToolUse`, `AssistantText`, `ToolResult`, `Result(TurnResult)`, `PermissionRequest`, `SessionStarted`, `RateLimit`, `Ignored` |
| `TurnResult` | End of turn: `is_error`, `raw`, `reply` (raw JSON when a schema constrained the answer), `cost_usd: Option`, per-model `usage` |
| `ModelUsage` / `TokenUsage` | The ledger's raw material; USD optional, tokens not |
| `SessionEvent` | Neutral history facts (`UserPrompt`, `AssistantUsage`, `CompactBoundary`, `Title`) the indexer consumes |
| `SpawnLimits` | Hard caps (`max_budget_usd`, `max_turns`) |
| `Capabilities` | What the backend can do; the UI degrades feature by feature |

`Capabilities` fields and their UI meaning:

- `permissions` — approval cards + voice "pode/nega" flow
- `cost_reporting` — USD anywhere (no → tokens only)
- `structured_output` — the voice ask + dispatch gate (schema-constrained
  one-shot)
- `history` — session browser, resume, retroactive token ledger
- `live_list` — "what's running" from sessions started outside Hark
- `slash_commands` — /compact & friends sent as plain turn text
- `memory_file` — file name the backend auto-loads from its cwd (Claude:
  `CLAUDE.md`); Hark writes the assistant's persona there
- `shell_tools` — tool names whose input is a shell command; feeds the
  production gate (kubectl/terraform/etc. never auto-approve)
- `fork` — the held-session banner's "open in parallel"
- `directive_mode` / `directive_model` / `directive_effort` — each pill
  reaches the agent: a CLI flag for claude, or an ACP mode / config
  option the agent offered at `session/new`

### Unsupported is shown, never hidden

Every surface that offers a feature asks the sheet first
(`src/lib/support.ts`: `unsupported(catalog, agent, cap, feature)`), and
a feature the agent's plugin lacks stays ON SCREEN, disabled, with the
reason as its hover: "sem suporte no plugin Gemini CLI: modo, modelo e
esforço". Hidden would read as "Hark cannot"; a control that takes the
click and changes nothing (what the pills did in an ACP chat) is worse
than either. Numbers follow the same rule: a price the plugin never
reported is `$ –` with the reason on hover, in the footer and in the
costs panel, and the ledger keeps the turn with cost NULL. The agent
behind a chat comes from the driver (`agent_for_session`: the session's
owner, else the selected default) and from the turn stream; an agent
with no negotiated sheet is never accused of lacking anything. The
first-run wizard follows the same rule: its gate is "some enabled,
detected backend can drive Hark" (`setup_status.agent_ok`), so a machine
with only gemini or codex is a working machine, not one sent to a wizard
asking for claude.

Seams the core exposes (in `hark-core::ports` today):

- `AgentRunner::ask(TurnRequest)` — schema-constrained one-shot. The voice
  ask AND the dispatch gate both ride it; `TurnRequest` carries prompt,
  images, model, system_prompt, schema, effort.
- `HistoryIndexer::refresh(projects_dir, store)` — parse your history into
  `SessionEvent`s; the core owns the SQLite index and the spend ledger.

The cheap lane's runner is wrapped once in `hark_core::app::runner::
TieredRunner`, which says the requested tier in the agent's own model
names; the app and `hark ask` (hark-cli) resolve the agent the same way
and share it. Detection (`which`, claude's own binary resolution) lives
in `adapters/agent_detect.rs` for the same reason.

The persistent worker rides the `AgentBackend` / `AgentSession` seam
(`hark-core::ports`): the driver spawns through `backend_for(config,
agent)` and consumes ONE event stream (`EventRx`) whatever the plugin —
the reader loop in `src-tauri/src/lib.rs` does not know which agent is
on the other end.

## What lives in a plugin

Everything that knows the backend. For Claude Code that is:

- `stream.rs` — stream-json → `AgentEvent`, plus the writers
  (`user_message` with image blocks, `permission_response`)
- `worker.rs` — persistent + one-shot processes, CLI flags, `--resume`
- `history.rs` + `log.rs` — `~/.claude/projects/**.jsonl` → `SessionEvent`
- `live.rs` — `claude agents --json`
- `bridge.rs` + `statusline.rs` — the statusline bridge (subscription %)
- `eco.rs` — community eco-tools detection and per-worker env injection

## The registry: one line per agent

`hark-core/src/domain/agents.rs` ships the backends Hark knows about;
`~/.hark/config.toml` overrides any field of a built-in or adds entries.
Two plugins speak for all of them — the native one for Claude Code, the
ACP one for everything that speaks the Agent Client Protocol:

| id | plugin | command | how one logs in |
|---|---|---|---|
| `claude` | native | `claude` | `claude /login` |
| `gemini` | acp | `gemini --acp` | `gemini` |
| `codex` | acp | `codex-acp` | `codex login` |
| `deepseek` | acp | `dsh --profile acp` | `dsh` |
| `kiro` | acp | `kiro-cli acp` | `kiro-cli login` |
| `claude-acp` | acp (off by default) | `claude-agent-acp` | `claude /login` |
| `antigravity` | acp | `agy_acp_server.par` | the agent's own flow |

Detected = the binary answers `which`; usable = detected and enabled.

### Tiers: one pill, every agent's own names

The model pill and the router speak in **tiers** — `light`, `standard`,
`heavy`, `max` — never in one agent's model names. Each registry line
carries a `models` table saying what THIS agent calls each tier
(`gemini`: `light = "gemini-2.5-flash-lite"` … ; `claude` and
`claude-acp`: `haiku`, `sonnet`, `opus`, `fable`), and the user's config
overrides it tier by tier (`[agents.gemini.models] heavy = "gemini-3-pro"`).
The translation happens once, at the spawn boundary
(`agents::model_id`, applied by the driver to every spawn, every live
switch and the cheap lane): a pick travels as its tier (or the global
table's name for it — "haiku" stored as a window default long ago still
means light), the agent hears its own id, an explicit id passes through
untouched. An ACP agent whose line has NO table gets no model at all and
runs its own default — a claude name sent to codex is refused or silently
ignored, and either way the pill would be lying — and in a chat on it the
pill is disabled with the config knob as the reason. The catalog shows
each agent's table. Before this (14/09/2026) the cheap lane asked every
agent for "haiku" and the pill offered claude's names in a gemini chat;
the mismatch was swallowed at the handshake and the agent ran its
default while the pill said otherwise.

### Which package is current

The [ACP agent registry](https://github.com/agentclientprotocol/registry)
is the source of truth for how each agent is distributed and invoked —
one folder per agent, published as
`https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`.
Hark's built-ins follow it; a package that moves gets its entry changed
here, never patched around in config. Checked 13/09/2026:

| agent | package | note |
|---|---|---|
| claude-acp | `@agentclientprotocol/claude-agent-acp` 0.76 | ex `@zed-industries/claude-code-acp` (deprecated, archived); the old one never reported cost |
| codex | `@agentclientprotocol/codex-acp` 1.11 | ex `@zed-industries/codex-acp` (deprecated); bundles `@openai/codex`, same `codex-acp` binary |
| gemini | `@google/gemini-cli` 0.59, `--acp` | `--experimental-acp` is the deprecated spelling |
| deepseek | `@deepseek-ai/dsh` 0.1.5-rc | the `acp` profile is the shipped stdio server |
| kiro | kiro.dev installer | not in the registry; `kiro-cli acp` per the docs |
| antigravity | registry `antigravity-acp` 1.1.1 | a zip per platform (arm64 mac only), no installer |

A deprecated Zed package still runs, so "detected" is no proof the
adapter is current: the catalog's install line is the migration command.
Nor is a current package name proof of a current version: on 14/09/2026
this machine ran gemini 0.46.0 with 0.59.0 published, and 0.46's
`session/new` answered a deprecation notice ("no longer supported for
Gemini Code Assist for individuals") that only an update fixes — Hark
showed it as `agent_failed`, and nobody said "update". So the catalog
compares **installed against published**: each detected binary is asked
its `--version` (bounded, first version-looking token), each registry
line names its id in the ACP registry (`registry` — codex is `codex-acp`
there), and `registry.json` is read into `state.json` (`registry`
snapshot with `checked_at`) by the panel's "check for updates" button,
or by the panel itself when the last reading is a day old — the one
network call of the catalog, a GET of a public file, never on every
open. A row behind shows both numbers and the `npm install -g
<package>@<version>` line the registry implies (archives keep their
install text). `domain/registry.rs` is pure and tested against a trimmed
copy of the real document (`fixtures/acp-registry.trimmed.json`);
`adapters/registry_fetch.rs` is the curl and the probe.
Settings › Plugins is the catalog: it selects the default, and switches
entries on and off (`[agents.<id>] enabled`) — built-ins that ship off,
like claude over ACP, are one click away rather than a config edit. The
selected agent (`[agent] plugin = "<id>"`) opens NEW sessions. An EXISTING session is always resumed by the agent that
created it — the worker registry remembers, and a session it never saw
came from the claude history index and stays claude's. Switching the
default never hands one agent's session to another.

```toml
[agent]
plugin = "gemini"              # new chats open here

[agents.gemini]
args = ["--experimental-acp"]  # override one field, keep the rest

[agents.claude]
env = { ANTHROPIC_BASE_URL = "http://litellm:4000", ANTHROPIC_AUTH_TOKEN = "sk-…" }
```

### Gateways (LiteLLM), and models that are not the agent's own

The claude plugin runs the real Claude Code CLI, and Claude Code takes a
gateway through `ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`. Put those
in the `claude` entry's `env` and EVERY Hark process — workers, the voice
ask, the gate — goes through the company's LiteLLM with the whole native
experience intact. A LiteLLM that aliases model names can answer with a
non-Anthropic model (DeepSeek, say); Hark never knows and never needs to.
A second entry with `plugin = "claude"` and its own `env` keeps both
routes side by side. The cheap lane (voice ask, intent router, dispatch
gate) follows `[agent] ask` when set, else the same agent new chats open
with — so a twin selected as the default also answers the voice. `env`
values are the user's own config and are never logged.

ACP agents that take an OpenAI-compatible base URL (codex, dsh) reach a
gateway through their own config; Hark passes their `env` through too.

### What ACP gives, and what it does not

`hark-plugin-acp` is jsonrpc 2.0 over stdio, framed by hand (`rpc.rs`),
with the message shapes taken from the official v1 schema and from
recorded traffic (`HARK_ACP_TRACE=<file>` records a session both ways —
fixtures come from the wire, not from the spec). Per capability:

- **permissions** — always: `session/request_permission` becomes the same
  card as claude's. Allow picks `allow_once`, never `allow_always` (the
  standing rule is Hark's own, per window). Cancel answers every open ask
  with `cancelled`, as the spec requires.
- **prose** — `agent_message_chunk` is a token-sized delta; Hark coalesces
  a segment (until a tool call, a thought, an ask or the end of the turn)
  into one message, so the transcript stays message-sized like claude's.
- **slash commands** — `available_commands_update` fills the "/" palette
  for that workspace. The palette's fallback list is claude's vocabulary
  and is shown for claude only: `/design` is a Claude command, and an
  agent that does not announce it never shows it.
- **usage** — `usage_update.used/size` is this turn's prompt over the
  window (the context ring); cost is the delta of the session's running
  total. Agents send SEVERAL updates per turn and price it on one of
  them, so the turn keeps the last cost anyone stated while used/size
  follow the latest reading. What each agent actually puts on the wire
  (13/09/2026): claude-agent-acp reports used/size and cumulative USD
  (and rate limits in `_meta["_claude/rateLimit"]`); codex-acp reports
  tokens, no money; gemini reports nothing at all; dsh reports context
  usage. **A turn nobody priced is written down as "custo não
  informado", never as $0.0000** — the subscription paid, the wire just
  did not carry the number — and it still lands in the ledger as one
  row under the agent that ran it, tokens zero, cost NULL, so the turn
  count and the session link survive.
- **resume** — `session/load`, when the agent announces `loadSession`.
- **the production gate** — `prodgate::check` (kubectl apply, terraform
  apply, helm upgrade, force push, DROP TABLE… never auto-approve, a human
  answers) asks the plugin's sheet which tool is the shell: claude's
  `Bash`, and over ACP the `execute` kind. Under the sheet it reads ANY
  input carrying a `command` — a string, or codex-acp's argv array — as a
  shell command whatever the agent titled the tool: a false positive is
  one more human click, a false negative is terraform apply in
  production. Before 15/09/2026 the check hardcoded `Bash` and the sheet's
  `shell_tools` was decorative; over ACP the gate never fired.
- **bypass** — has a deny floor on claude only (`BYPASS_DENY_RULES` as
  `--disallowedTools`: kubectl, terraform and friends cannot run at all).
  Nothing of the kind crosses ACP — in gemini's `yolo` or the Claude
  adapter's `bypassPermissions` the agent asks nothing and the gate never
  sees the command — so on any non-claude plugin a bypass pick opens the
  thread in acceptEdits and SAYS so (a status line in the chat; a live
  pick is refused by name), and the mode pill's bypass row is disabled
  with the reason in a chat on such an agent (`agents::with_floor`,
  `support.bypassFloorFor`).
- **directives** — said in the agent's own ids, for whatever its
  `session/new` answer OFFERS (`directives.rs`): the permission mode
  through `session/set_mode` (exact ids first — the Claude adapter uses
  the CLI's spellings — then aliases: gemini's `autoEdit`/`yolo`; "auto"
  falls back to accepting edits, never to bypass), model and effort
  through `session/set_config_option` (the options categorised `model`
  and `thought_level`; an effort level the agent lacks becomes the
  nearest LOWER one; a model is found by id, label or family — "haiku"
  finds "claude-haiku-4-5"). The opening directives go on the wire
  between `session/new` and the first word; a live change is one call on
  the running session, no reopen. The sheet learns `directive_*` from the
  offer, the driver keeps it per agent (`state.json` → `agent_sheets`)
  so the catalog — which never spawns — shows the truth and the pills
  come back on. A knob the agent does not offer is skipped and the pill
  says so; a model name not on its list is refused by name.

  Recorded from claude-agent-acp 0.76.0 (14/09/2026, one real turn —
  `fixtures/session-new.claude-agent-acp-0.76.0.json` and
  `prompt-response.claude-agent-acp-0.76.0.json`; the ignored test in
  `backend.rs` replays it against the binary): modes `default,
  acceptEdits, plan, auto, bypassPermissions` (the CLI's own, so `auto`
  is auto); config options `mode`, `model` (values `default`, `opus[1m]`,
  `claude-fable-5-1[1m]`, `sonnet`, `haiku` — aliases with a context
  hint, which is why the matcher compares families), `effort` (`default,
  low, medium, high, xhigh, max`), `fast` (on/off), `agent` (subagents).
  The prompt ANSWER carries the turn's own token breakdown
  (`usage.inputTokens/outputTokens/cachedReadTokens/cachedWriteTokens`)
  and, in `_meta.quota.model_usage`, the model that actually ran: the
  ledger row takes both, so a 10-in/72-out turn is not written down as
  28k of input, and the footer signs the real model. One word on haiku
  at low effort cost $0.057: 28.5k tokens of Claude Code system prompt
  written to cache; the lean ask (below) is what cut that.

  **The subscription meter without the bridge** (15/09/2026): the
  adapter puts Claude's rate-limit object on `usage_update` as
  `_meta["_claude/rateLimit"]` — `status`, `resetsAt`, `rateLimitType`,
  `isUsingOverage`, and `unifiedWindows{five_hour, seven_day}{utilization,
  resetsAt}`, the fill of each window. It is the same object the CLI puts
  in `rate_limit_event`, so the contract parses it once
  (`RateLimitInfo::from_claude`, `windows` added to `RateLimitInfo`), the
  ACP plugin emits the `RateLimit` event from it, and the driver keeps the
  last reading ten minutes to answer `subscription_limits` when no bridge
  file is there: the topbar's 5h/7d shows for a claude-acp user who never
  installed the bridge. The bridge file still wins when present (context
  occupancy and per-model windows too). The adapter also sends
  `_auth/status_update` (`authStatus{kind: "account", label, account{plan,
  email, organization}}`) — the account behind the session. Not consumed:
  it names the person, Hark has no surface that needs it, and a fixture of
  it would carry the user's email.
- **structured ask** (voice ask, intent router, dispatch gate) — no
  schema mode, so the schema goes INTO the prompt and the answer is read
  leniently (`reply::extract_lenient`: the object, a fenced block, prose
  around an object). One fresh session per question, closed right after.
  When the agent writes no JSON at all, the gate degrades to "confirm"
  (never a silent dispatch) and the ask reads the prose.
- **fork** — `session/fork` (a new session grown from the old one's
  history: the held-session banner's "open in parallel"), for an agent
  that announces `sessionCapabilities.fork` at initialize — claude-agent-acp
  0.76 and codex-acp 1.11 do (`fixtures/initialize.codex-acp-1.11.0.json`),
  gemini 0.46 does not. The answer is a session/new answer, new id and
  offer included, and the window follows the new id. On an agent that
  does not announce it the plugin REFUSES by name rather than load the
  old session and call it a fork; the sheet keeps the banner off there.
- **history** — RECORDED by Hark (F9.4, 14/09/2026). An ACP agent leaves
  no session file, so the driver writes one: `<data_dir>/sessions/<agent>/
  <session_id>.jsonl`, opened at `SessionStarted` (new, resumed or forked
  alike; a header with agent, cwd and time only when the file is new), the
  user's words written at send time, the agent's prose, tool calls and
  results from the reader loop, and one usage line per model per turn.
  The format is `domain/recorded.rs`: conversation lines are
  `transcript::Entry` written verbatim, so the viewer, the mirror, the
  brief (restart-light) and the crossref read a record with the reader
  they already have (`parse_entry` tells the two dialects apart per line —
  a top-level `role` is ours, claude's lines have none). The index is
  fed by `adapters/recorder.rs::refresh`, the same incremental fold
  claude's files get (offset + mtime, size too), wherever the index is
  refreshed (`BothHistories` in the app and in `hark ask`; `hark index`
  sums both): search, funnel, candidates, session stats, titles, and the
  machine-wide token table (usage lines become `source = jsonl` rows with
  a synthetic request id; the dollars stay on the live row, and the two
  sources are never summed). The sheet's `history` stays false: it means
  "the agent's own files", and the honest limit stands — a session opened
  OUTSIDE Hark is invisible; the popover says so for a session with no
  record rather than "unsupported".
- **live list** — absent: no equivalent of `claude agents --json`, so a
  terminal cannot hold an ACP session and the mirror never engages there.
- **claude's own lines stay claude's** — "resume in the terminal" types
  `claude --resume <id>`; in a chat on another agent the button stays,
  off, with the reason. Auto-compaction at 85% of the window sends
  "/compact", claude's command: an ACP agent gets it only when its
  announced commands include `compact`; otherwise the chat says so and
  points at "restart light", which works everywhere (the brief comes
  from Hark's record).
- `fs/*` and `terminal/*` are declined at initialize: the agent uses its
  own tools; Hark answers `-32601` if asked anyway.

### Claude over ACP: what the adapter would let us cross

`claude-agent-acp` forwards Claude Agent SDK options given in
`session/new` `_meta.claudeCode.options` (everything but cwd,
permissionMode, canUseTool, includePartialMessages, executable — those
are ACP's), and a string `_meta.systemPrompt` REPLACES Claude Code's
system prompt. That is `model`, `effort`, `maxTurns`, `maxBudgetUsd`,
`settingSources`, `disallowedTools`, `outputFormat` (json_schema): the
lean voice ask, the budget ceiling and native structured output are all
reachable for this one agent. Its session config options (`effort`,
`model`) and its modes are what the generic directive path above already
drives, for this adapter as for dsh or gemini.

The adapter signs its `initialize` with `agentCapabilities._meta.claudeCode`
(`Negotiated.claude_code`), and only that signature licenses the `_meta`
Hark puts on `session/new` (`session::claude_meta`):

- **the lean ask** — the cheap lane opens with `Opening::lean`: our
  persona as `_meta.systemPrompt` (a REAL system prompt, replacing Claude
  Code's), `disableBuiltInTools`, and SDK options `settingSources: []`,
  `tools: []`, `maxTurns: 1`; the ask also asks for manual mode (no tools
  to approve, and the adapter otherwise inherits the user's "auto" and
  announces a fallback). Measured on 14/09/2026, haiku at low effort: a
  chat-style opening cost **$0.057** for one word (28.5k tokens of Claude
  Code system prompt written to cache); the lean ask cost **$0.0019** for
  a sentence (875 tokens in, 198 out). Thirty times less. For any other
  agent the persona is prepended to the prompt, as before. Native
  structured output (`outputFormat`) is deliberately NOT used: the schema
  stays in the prompt and the answer is read leniently, which works
  everywhere.
- **the ceiling** — `Opening::limits` (the worker's `SpawnLimits`) rides
  the same `_meta` as `maxBudgetUsd` / `maxTurns`, so the SDK enforces it
  as the CLI's flags would. And for EVERY ACP agent the session is its
  own ceiling: the running cost from `usage_update` past `max_budget_usd`,
  or more tool calls in one turn than `max_turns`, cancels the turn
  (`session/cancel`) and ends it as an error naming both numbers —
  "teto de US$ 2.00 atingido — a sessão já gastou US$ 2.03". Never a
  quiet spend. The money spent stays on the ledger row.

Failures carry the same health codes as the claude plugin
(`agent_missing`, `agent_blocked`, `agent_auth`, `agent_failed`), and an
auth failure carries the registry's `login_hint` so the card's runnable
fix is the agent's own command.

## Handoff: the same task, another agent

"Troca esse chat pro gemini" (`task_command::Handoff`; also "muda/passa/
manda essa thread/conversa pro …", "hand/switch/move this chat to …")
moves the thread in focus to another agent: `worker_handoff(task_id,
agent)` shuts the session down, opens a FRESH session on the target with
a local, zero-token brief of the old transcript as its first message —
session formats do not cross agents, briefs do, and since F9.4 the brief
exists for an ACP session too (Hark's record) — on the SAME task: same
card, same title. The registry record moves first (`memory::hand_off`):
the agent it leaves and the session id go into `lineage`, the new id
lands at `SessionStarted` as for any fresh session. A target that is not
usable here (installed and switched on) is refused by name; so is the
agent the thread already runs on; the mother's own chat follows the
selected agent instead. The session popover (the ⓘ of a focused live
thread) shows the agent behind it and one button per other usable agent.
"Abre um chat gemini no hark" opens a NEW chat on a named agent: the
grammar stays the new-chat grammar (the last "no X" names the project),
the shell reads the agent word from the sentence (`agents::spoken_agent`,
usable agents only) and `chat_start` takes it — refused by name when it
is not usable, never a quiet fallback to the default.

## Out-of-process plugins: ACP is the protocol

An earlier note designed a Hark-specific stdio protocol ("HAP"). It died
the day the Agent Client Protocol was read properly: ACP IS that — jsonrpc
over stdio, a capability handshake, permissions on the wire — and the
agents already speak it. A community plugin is an ACP agent, in any
language, plus one registry line.

## Next: the contract as a published crate

The whole source is public, so nothing has to be extracted to be read. What
is still worth doing is publishing `hark-agent` on crates.io, so a backend
can live outside this repository and pin a contract version.

Today `hark-plugin-claude` compiles against `hark-core`, which a crate
published on its own cannot depend on. The good news is how little is left:
five of its eight modules (`stream`, `log`, `bridge`, `statusline`, `eco`)
already import nothing from the core. The remaining coupling is four
imports' worth:

| Module | Needs from `hark-core` | Fix |
|---|---|---|
| `cli.rs` | `ports::{AgentRunner, TurnRequest}` | move both into `hark-agent` |
| `live.rs` | `ports::LiveSessions`, `domain::prompt::LiveSession` | move both into `hark-agent` |
| `history.rs` | `ports::{SessionStore, SpendLedger, SpendQuery, SpendGroup, HistoryIndexer}`, `domain::spend::{SpendRow, SpendKind, SpendSource}`, `domain::snapshot::SessionSummary` | move the traits + row types into `hark-agent`; the SQLite impl stays in the core |
| `worker.rs` | `domain::directives::{Directives, Mode, Effort}`, `domain::prodgate::BYPASS_DENY_RULES` | `Directives` is contract material (the spawn spec already carries it); the deny rules become a `Capabilities`-adjacent constant owned by the plugin |

`SqliteStore` shows up only in `history.rs` tests — a dev-dependency, not a
real edge.

So the extraction is: **fold the ports into `hark-agent`** (they are the
contract already, they just live in the wrong crate), flip
`hark-plugin-claude`'s dependency to `hark-agent` alone, then publish:

1. `hark-agent` on crates.io (the contract; versioned, semver matters).
2. `hark-plugin-claude` on crates.io, depending on the published
   `hark-agent`.
3. The app keeps both as path dependencies in this workspace.

Until then both crates stay path dependencies here, which is why the
boundary is enforced by the compiler today: the moment a core type sneaks
back into a plugin module, publishing gets harder.
