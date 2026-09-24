# Plugins: the agent is a backend, not the product

[← back to the README](../README.md)

Hark is a cockpit. The thing that actually talks to a model is a **plugin**, and
the cockpit only knows it through a small contract. That is the whole design:
the voice loop, the windows, the board, the permission floor and the cost ledger
belong to you and travel with you; which agent CLI sits underneath is a
configuration choice.

Think of it the way an editor treats language servers. The editor does not
reimplement every compiler — it speaks one protocol, and every language gets the
same experience. Hark does that for coding agents.

## What ships today

Two plugins speak for every agent.

| Plugin | Agents | Status |
|---|---|---|
| **Claude Code**, native | the `claude` CLI | shipping — the default when it is installed |
| **ACP** — the [Agent Client Protocol](https://agentclientprotocol.com) | Gemini CLI (`gemini --acp`), Codex (`codex-acp`), Claude Code over ACP (`claude-agent-acp`, ships off), DeepSeek (`dsh --profile acp`), Kiro (`kiro-cli acp`), Google Antigravity — and any other ACP agent, one `[agents.<id>]` line in config | shipping |

ACP is the open editor-to-agent standard (JSON-RPC over stdio, a capability
handshake, permissions on the wire) that Zed, JetBrains and Google ship
against. Hark speaks it once and every agent that speaks it plugs in — no
per-vendor plugin, no waiting on anyone. Each built-in follows the
[ACP agent registry](https://github.com/agentclientprotocol/registry) for how
it is distributed and invoked.

The catalog in the app (Settings → Plugins) shows each agent: whether its
binary is on your machine and the install line when it is not; the version you
have against the one the registry publishes, with the update command when yours
is behind (an old Gemini CLI, for one, refuses every new session with a
deprecation notice that only an update fixes); what each plugin can do; and
what the agent calls each model tier. Selecting one decides which agent opens
*new* chats — an existing chat always resumes on the agent that created it.

### What the same cockpit gives every ACP agent

- **Permissions**, always: the same inline card as Claude Code, answered by
  click, keyboard or voice. Allow means *once*; standing rules are Hark's own.
- **Directives that cross.** The mode, model and effort pills speak to the
  agent in its own ids, for whatever it offered at session start — live, with
  no reopen. The model pill speaks in tiers, and each agent's registry line says
  what it calls them (`haiku` to Claude, `gemini-2.5-flash-lite` to Gemini). A
  knob an agent does not offer stays on screen, disabled, with the reason.
- **Costs where they exist.** The Claude adapter prices its turns; Codex
  reports tokens; Gemini reports nothing. A turn nobody priced is `$ –` with the
  reason on hover — never `$0.00`. USD only ever comes from the agent.
- **A history Hark writes.** ACP agents keep no session file, so Hark records
  each session it opens as the turns happen: searchable, readable, resumable,
  briefable — the honest limit being that a session opened outside Hark stays
  invisible.
- **The production floor.** `kubectl apply`, `terraform apply`, force pushes
  and friends never auto-approve, whichever agent runs them and whatever it
  names its shell tool. Bypass mode is Claude Code's (its deny list is a Claude
  Code flag): elsewhere a bypass pick opens accept-edits and says so.
- **A ceiling.** The per-worker dollar budget is a Claude Code flag; on ACP
  Hark itself cancels the turn past the budget and says by how much.
- **Handoff.** "Switch this chat to Gemini" moves the thread: a fresh session on
  the other agent, opened with a local zero-token brief, same card, lineage
  kept.
- **Forks, resumes, slash commands, the subscription meter** — each where the
  agent announces it: `session/fork` on the Claude adapter and Codex,
  `session/load` where the agent says so, the agent's own command palette, and
  the 5h/7d meter riding the Claude adapter's updates with no bridge needed.
- **The cheap lane, cheap.** Over the Claude adapter the voice ask runs with
  Hark's own system prompt, no settings and no tools: about $0.002 a question
  against $0.057 for a chat-style opening.

## Why a CLI and not a model API

Because the CLI is where everything you already paid for lives. Driving
`claude -p` instead of an HTTP endpoint means Hark inherits, for free:

- **Your subscription.** Tokens draw from the plan you already have. Hark has no
  API key and cannot open a second bill.
- **Your setup.** MCP servers, memory files, skills, permission settings, hooks
  — the CLI loads them exactly as it does in your terminal.
- **Your history.** Sessions are on disk in a documented format. Hark indexes
  them, so a chat you started in the terminal three weeks ago shows up in the
  sidebar, resumable.
- **Your company's approval.** The binary your security team already signed off
  on is the binary Hark runs. Nothing new goes over the network.

Harnesses that call model APIs directly have to rebuild the agent loop, the tool
sandbox, the session store and the billing story. Hark sits one layer above
them: a cockpit over harnesses, not another harness.

## The contract

A plugin implements two things.

**An event vocabulary.** Whatever the backend prints, the plugin translates into
a fixed set of events the cockpit understands:

| Event | Meaning |
|---|---|
| `SessionStarted` | which session this process writes to, and its slash commands |
| `AssistantText` | prose from the agent, streamed |
| `ToolUse` | a tool was called (this is what gets narrated out loud) |
| `ToolResult` | it finished, and whether it failed |
| `PermissionRequest` | the backend is asking whether a tool may run |
| `Result` | the turn ended: token usage per model, cost, error state |
| `RateLimit` | quota window status, when the backend reports one |
| `Ignored` | everything else, kept so the stream stays auditable |

**A capability sheet.** What the backend can actually do — the UI degrades
against it instead of pretending:

`resume` · `permissions` · `structured_output` · `cost_reporting` · `history` ·
`live_list` · `slash_commands` · `memory_file` · `shell_tools` · `fork` ·
`directive_mode` · `directive_model` · `directive_effort`

The rule is **unsupported is shown, never hidden**. A feature the plugin lacks
stays on screen, disabled, with the reason as its hover ("not supported by the
Gemini CLI plugin: effort") — hidden would read as "Hark cannot", and a control
that takes the click and does nothing is worse than either. A backend without
`cost_reporting` has `$ –` where the dollars would be, tokens still shown.
`shell_tools` names the shell tool for the production floor (Claude's `Bash`,
an ACP agent's `execute`), and under it any tool input carrying a command is
read as one, so `terraform apply` can never be auto-approved whichever agent
runs it. An ACP agent negotiates its sheet at every handshake, and Hark keeps
what it learned so the catalog shows the truth without spawning anything.

Notice what is *not* in the contract: no prompt format, no model names, no
vendor concepts. A plugin translates its CLI's dialect at the edge, and the
cockpit stays neutral.

## Where this is going

1. **The contract becomes a published crate.** It is already public, like the
   rest of Hark: `crates/hark-agent`, walked through in
   [PLUGIN-CONTRACT.md](PLUGIN-CONTRACT.md). Publishing it on crates.io lets a
   backend live outside this repository and pin a contract version.
2. **Out-of-process plugins are ACP.** An earlier note here described a
   Hark-specific stdio protocol. It died the day the Agent Client Protocol was
   read properly: ACP *is* that — JSON-RPC over stdio, a capability handshake,
   permissions on the wire — and the agents already speak it. A community
   plugin is an ACP agent, in any language, plus one registry line.
3. **The community catalog exists**: it is the ACP registry. Hark's built-ins
   follow it, and the version check reads it.

## Want a backend for the CLI your company allows?

If it speaks ACP, add it — `[agents.<id>]` with its `cmd` and `args` in
[config](CONFIG.md) — and it is in the catalog. If it does not, open an
[issue](https://github.com/harkharness/hark/issues) describing the CLI — how it
streams, whether it resumes sessions, whether it reports cost — and it goes on
the list. Or write the plugin yourself: the contract is in `crates/hark-agent`
and [PLUGIN-CONTRACT.md](PLUGIN-CONTRACT.md) walks through it. The contract was shaped by Claude Code first and by six ACP agents
since; the next one will still teach it something.
