# Using Hark

[← back to the README](../README.md)

## First run

Open `hark.app`. A wizard runs once and refuses to leave you with a half-working
install:

1. **Language, name and hotkey.** What the microphone expects to hear, what the
   screen shows, what you want to call your assistant, and the global key that
   opens the mic from anywhere. All of it is editable later in Settings.
2. **Agent plugin.** Hark checks which agent CLIs are installed and logged in.
   Claude Code plugs in natively; Gemini CLI, Codex, Claude Code over ACP and
   any other Agent Client Protocol agent through the ACP plugin. The catalog
   shows each one, its install line, and whether your build is the one the ACP
   registry publishes. Nothing runs until at least one is detected.
3. **Speech model.** Downloads a whisper model (`small`, ~466MB, the default;
   `large-v3-turbo` if you want more accuracy for the price of latency). The
   download is checksummed and resumable.
4. **Microphone test.** Records two seconds and shows you the transcription —
   which is also what triggers the macOS permission prompt, with context, before
   you need it in the middle of something.

Everything the wizard writes lands in `~/.hark/config.toml`, in plain text,
with your comments preserved on later edits.

## Two languages, at the same time

Hark's spoken grammar accepts **Portuguese and English simultaneously** — you
never pick one. `abre a board` and `open the board` both switch tabs. `sim` and
`yes` both confirm; `não` and `no` both refuse. `a primeira` and `the first one`
both pick the first candidate. `capricha` and `take your time` both raise the
effort of the turn.

Words that mean different things in each language are disambiguated rather than
dropped. A bare "no" is a refusal; "no projeto webhook" addresses a target and
is never read as one.

Two separate settings control the rest:

| Setting | What it decides |
|---|---|
| `language` | what the speech model expects to **hear** — a fixed code, or `auto` to detect each utterance |
| `ui_language` | what Hark **writes and says** — interface, local answers, warnings, and the language the agent is told to answer in |

They are separate on purpose: plenty of people speak Portuguese to an English
interface, or the other way around. Recognition does not care either way.

## The voice loop

Press the hotkey (`cmd+shift+space` by default) anywhere, in any app. A floating
HUD opens, listens, and stops on its own after a beat of silence.

Before anything runs, Hark shows you two things: **what it heard** and **where
it is going**. That second part is the whole point — a voice command that lands
in the wrong session is worse than no voice command at all. Confirmation comes
at one of two speeds:

- **Target is the chat you are already in** → a chip appears and 1.6 seconds of
  silence executes it. No ceremony for the common case.
- **Hark had to resolve the target** (by search, or the match was ambiguous, or
  it is a brand new session) → Hark *says* the destination out loud and waits
  for a verdict. Say `sim` to go, `não` to cancel, or simply say a different
  instruction and it replaces the text and asks again. Ambiguity is never
  guessed away: you get numbered candidates and pick one ("a primeira").

Answers are spoken back through the system voice while the details render on
screen.

## The mother window

Three tabs: **voice**, **board**, **costs**.

The voice tab is a persistent chat backed by one long-lived agent session
carrying your full settings — MCP servers, tools, skills, memory file. It has a
name, a personality, and a memory file it maintains itself: tell it "remember
that I prefer short answers" and it writes that down, durably, through the
normal permission flow.

That session is expensive, so Hark protects it. Questions it can answer from the
local index — what is on the board, which workers are running, what you spent
today — never reach a model at all. Questions that only need rephrasing go to a
light model with a small context. Only real reasoning pays full price.

Project chips at the bottom show every workspace Hark knows about, which ones
have live sessions, and what they have cost.

## Project windows

One window per project, and they read like the terminal you already use:

- **Session history** in the sidebar, indexed from the agent CLI's own logs — a
  chat you started in the terminal three weeks ago is right there. An agent
  that keeps no logs (the ACP agents) gets its history written by Hark as the
  turns happen, so its chats are searchable, readable and resumable too; the
  honest limit is that a session opened outside Hark stays invisible.
- **Transcripts** in real markdown: headings, tables, code with syntax
  highlighting, tool calls folded, bursts of shell commands grouped.
- **Deliverables** surface as file cards; CSVs open as tables; there is a real
  PTY terminal and a local file editor in the same window.
- **The board, filtered** to that project, plus what the project has spent.

## Dispatching work

Say — or type — what you want done: *"continua a migração do webhook"*. Hark
resolves it against the sessions on your disk by whole-word matching (never
substrings, so "core" does not match "hark-core"), resumes the winner **in its
own workspace with your full settings**, and streams every event back into the
window. A rare term outweighs a common one; a close call surfaces as candidates
instead of a guess.

Before an expensive worker starts, a cheap evaluator checks the instruction
against the session it is about to land in — a few cents to avoid dollars spent
in the wrong conversation. Heavy sessions get an honest warning first
("histórico de 3.8 MB") with the option to compact before sending.

Workers run in parallel across projects. Every turn — including the ones that
fail — lands in the ledger.

## Permissions

Every privileged tool call renders as an inline card in the thread: the tool,
the payload, and allow / always / deny. Answer by click, by keyboard, or by
voice from any window. Nothing blocks the rest of the interface — a permission
waiting in one thread never freezes another.

**There is a hard floor.** Commands that touch production — `kubectl apply`,
`terraform apply`, `helm`, force-pushes, destructive SQL, cloud deletes, package
publishes — are never auto-approved, whichever agent runs them. Standing
"always allow" rules are ignored for them, the card turns red, and in bypass
mode those CLIs are blocked outright on Claude Code. That block is a Claude Code
flag with no equivalent elsewhere, so on any other agent a bypass pick opens
the thread in accept-edits instead and says so in the chat — the pill's bypass
row is disabled there with the reason. Hark can be careless with your tokens;
it is not allowed to be careless with your infrastructure.

## Switching agents

Every chat belongs to the agent that opened it, and the pills beside the
composer (permission mode, model tier, effort) speak to that agent in its own
vocabulary — the model tier you pick is said as `haiku` to Claude and as
`gemini-2.5-flash-lite` to Gemini. A knob an agent does not offer stays on
screen, disabled, with the reason.

Two spoken commands move between agents:

- *"abre um chat gemini no webhook"* / *"open a chat on gemini in webhook"* —
  a new chat in that project, on that agent.
- *"troca esse chat pro gemini"* / *"switch this chat to gemini"* — the thread
  you are in moves: a fresh session on the other agent opens with a local,
  zero-token brief of the conversation so far, on the same card. The ⓘ popover
  of a live thread has a button per available agent for the same move.

## The board

A kanban of your demands where the cards *are* the sessions. Moving a card does
not move a ticket somewhere else — it tracks the conversation that is doing the
work, so resuming means resuming, not starting over. The board is global (it
lives in the mother window) and each project window shows its slice.

## Costs

Every turn lands in a local SQLite ledger: kind, project, task, model, tokens,
cost, whether it failed. The costs tab breaks it down by day or week, by kind,
project and model, shows your cache hit ratio, and lists the most expensive and
the heaviest sessions.

Two rules keep the numbers honest, and they are printed at the bottom of the
panel:

- **USD comes only from the agent's own reporting.** There is no hardcoded price
  table anywhere in Hark. Claude Code and the Claude ACP adapter price their
  turns; Codex reports tokens only; Gemini reports nothing — and a turn nobody
  priced shows as `$ –` in the footer and in the panel, never as `$0.00`.
- **Token counts and dollars are never summed together.** They come from
  different sources — the live stream and the session logs — and the logs cover
  all of your agent usage, not just what went through Hark.

The subscription window percentages (5h / 7d) come from an **opt-in** bridge: a
one-click wrapper around the Claude Code status line that tees its JSON to a
file Hark reads. It backs up your settings first, changes nothing until you
click, and uninstalls cleanly. Over the Claude ACP adapter the meter arrives
with every turn and needs no bridge at all.

## Plugins

Settings → **Plugins** is the catalog: every agent Hark knows, whether its
binary is on this machine, what each plugin can do (resume, permissions, cost
reporting, history, slash commands…), what the agent calls each model tier,
which build you have against what the ACP registry publishes (and the update
command when yours is behind), and which one opens new chats. Claude Code and
the ACP agents — Gemini CLI, Codex, Claude Code over ACP, DeepSeek, Kiro,
Antigravity — ship today. See [PLUGINS.md](PLUGINS.md).

## The `hark` CLI

The same core, headless — useful for scripting, CI, or just a terminal habit:

```
hark listen                                   # the voice loop, in the terminal
hark hear                                     # transcribe one utterance
hark setup                                    # download the speech model
hark ask "what's pending today?"              # a cheap, snapshot-fed question, on the agent the app would use
hark dispatch [--session <id>] "<instruction>"
hark ps [--clear]                             # running workers
hark spend [--day|--week|--project] [--rebuild] [--export csv|json]
hark index                                    # (re)index session history
hark sessions                                 # what is on disk
hark use <context> | hark contexts            # kubectl-style focus areas
hark board                                    # the board, as text
```

## Updates

Hark keeps itself current without ever surprising you. When a new release
lands, the app downloads it in the background, verifies its signature, and
shows one quiet pill in the mother window: **restart to update**. Click it and
the app closes and reopens on the new version. Ignore it and nothing happens —
next launch checks again.

The check is a single GET against this repository (the only network call Hark
makes on its own; nothing identifying is sent). Turn it off with
`auto_update = false`, and update by hand whenever you like:

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

The `hark` CLI in `~/.local/bin` is updated by that same script — the in-app
updater replaces only the app bundle.

## Where things live

| Path | What |
|---|---|
| `~/.hark/config.toml` | your configuration ([reference](CONFIG.md)) |
| `~/.hark/index.db` | session index and spend ledger |
| `~/.hark/models/` | speech models |
| `~/.hark/HARK.md` | the assistant's personality and learnings, mirrored into each agent's own memory file name (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`) beside it |
| `~/.hark/state.json` | board, workers, active context, what each agent turned out to offer |
| `~/.hark/sessions/<agent>/<id>.jsonl` | Hark's own record of sessions on agents that keep none |
| `<project>/.hark/` | per-project journal, state and dispatch briefs |

Older installs kept these under `~/.config/hark` and
`~/Library/Application Support/hark`; Hark moves them to `~/.hark` on first
launch. None of it leaves your machine.
