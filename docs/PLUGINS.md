# Hark agent plugins

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
with no negotiated sheet is never accused of lacking anything.

Seams the core exposes (in `hark-core::ports` today):

- `AgentRunner::ask(TurnRequest)` — schema-constrained one-shot. The voice
  ask AND the dispatch gate both ride it; `TurnRequest` carries prompt,
  images, model, system_prompt, schema, effort.
- `HistoryIndexer::refresh(projects_dir, store)` — parse your history into
  `SessionEvent`s; the core owns the SQLite index and the spend ledger.

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
- **structured ask** (voice ask, intent router, dispatch gate) — no
  schema mode, so the schema goes INTO the prompt and the answer is read
  leniently (`reply::extract_lenient`: the object, a fenced block, prose
  around an object). One fresh session per question, closed right after.
  When the agent writes no JSON at all, the gate degrades to "confirm"
  (never a silent dispatch) and the ask reads the prose.
- **history, live list, fork** — absent. The session browser and the
  terminal mirror degrade until F9.4 (a hark-side recorder) gives them
  back.
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
drives, for this adapter as for dsh or gemini. Not wired yet: the `_meta`
options (lean ask, budget ceiling, native schema) — the claude-specific
extra — and `limits`, still zeroed at spawn.

Failures carry the same health codes as the claude plugin
(`agent_missing`, `agent_blocked`, `agent_auth`, `agent_failed`), and an
auth failure carries the registry's `login_hint` so the card's runnable
fix is the agent's own command.

## Out-of-process plugins: ACP is the protocol

An earlier note designed a Hark-specific stdio protocol ("HAP"). It died
the day the Agent Client Protocol was read properly: ACP IS that — jsonrpc
over stdio, a capability handshake, permissions on the wire — and the
agents already speak it. A community plugin is an ACP agent, in any
language, plus one registry line.

## TODO: extract the plugins into their own public repos

Decided 23/08/2026: plugins eventually live in a dedicated **Hark org** on
GitHub, public, one repo per plugin (plus the contract). The core app stays
private. Not now — this note is the map for when we do it.

Today `hark-plugin-claude` compiles against `hark-core`, which is exactly
what a public repo cannot depend on. The good news is how little is left:
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
`hark-plugin-claude`'s dependency to `hark-agent` alone, then split:

1. `hark-agent` → public repo (the contract; versioned, semver matters).
2. `hark-plugin-claude` → public repo depending on the published
   `hark-agent`.
3. The core app keeps them as git dependencies (or path deps in a local
   workspace override during development).

Until then both crates stay in this private workspace as path deps, which
is why the boundary is enforced by the compiler today: the moment a core
type sneaks back into a plugin module, the split gets harder.
