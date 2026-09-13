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
| `claude-acp` | acp (off by default) | `claude-code-acp` | `claude /login` |

Detected = the binary answers `which`; usable = detected and enabled. The
selected agent (`[agent] plugin = "<id>"`, or Settings › Plugins) opens
NEW sessions. An EXISTING session is always resumed by the agent that
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
routes side by side — only the workers follow a twin; the ask/gate lane
runs the plain entry until F9.5 routes it per agent. `env` values are
the user's own config and are never logged.

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
  total.
- **resume** — `session/load`, when the agent announces `loadSession`.
- **history, live list, structured ask, fork** — absent. The session
  browser, the terminal mirror and the voice ask degrade until F9.4
  (a hark-side recorder) and F9.5 (lenient JSON extraction) give them
  back.
- `fs/*` and `terminal/*` are declined at initialize: the agent uses its
  own tools; Hark answers `-32601` if asked anyway.

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
