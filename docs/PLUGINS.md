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

| Plugin | Status |
|---|---|
| **Claude Code** (`claude` CLI) | shipping — the default, selectable in the wizard and in Settings → Plugins |
| **Gemini CLI** | in development |

The catalog in the app shows each plugin's capability sheet, whether the binary
is detected on your machine, and how to install it if it is not. Selecting one
writes a single line to your config; nothing else changes.

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
`live_list` · `slash_commands` · `memory_file` · `shell_tools`

A backend without `cost_reporting` simply has no dollars in its panel — tokens
still show. Without `permissions`, the approval cards disappear rather than
lying about being in control. `shell_tools` is what feeds the production floor:
it tells Hark which tool inputs are shell commands, so `terraform apply` can
never be auto-approved regardless of which agent is running it.

Notice what is *not* in the contract: no prompt format, no model names, no
vendor concepts. A plugin translates its CLI's dialect at the edge, and the
cockpit stays neutral.

## Where this is going

1. **The contract and the Claude Code plugin become public repositories** under
   [@harkharness](https://github.com/harkharness). They are already a separate
   compilation unit inside Hark with no dependency on the private core, which is
   what makes the split mechanical rather than a rewrite.
2. **Out-of-process plugins.** The same events, serialized as newline-delimited
   JSON over stdio, so a plugin can be written in any language and shipped as
   its own binary — declared in a small manifest under
   `~/.config/hark/plugins/`. In-process today, over a pipe tomorrow, with the
   same vocabulary either way.
3. **A community catalog.** Once plugins are separate binaries, the catalog in
   the app can list what other people published, not just what Hark bundles.

There is no timeline promise here. The order is deliberate, though: the contract
goes public before the cockpit does, because the contract is the part other
people need in order to build on it.

## Want a backend for the CLI your company allows?

That is exactly the case this design exists for. Open an
[issue](https://github.com/harkharness/hark/issues) describing the CLI — how it
streams, whether it resumes sessions, whether it reports cost — and it goes on
the list. The contract was shaped by exactly one backend so far, which means the
second one will teach it something.
