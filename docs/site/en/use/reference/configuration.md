---
title: Configuration
description: Every key in ~/.hark/config.toml, with its default.
sidebar:
  order: 1
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Everything lives in `~/.hark/config.toml`. Every key is optional; the file below is the full set with its defaults. Edits made through Settings preserve your comments and formatting.

```toml
# --- the agent ------------------------------------------------------------
claude_bin       = "claude"      # binary name, or an absolute path
model            = "sonnet"      # what --model gets by default
projects_dir     = "~/.claude/projects"   # where the CLI keeps session logs

[agent]
plugin = "claude"                # which agent opens NEW chats
ask    = ""                      # which agent answers the cheap lane; empty follows plugin

[models]                         # the four tiers, in the global (Claude) names
light    = "haiku"
standard = "sonnet"
heavy    = "sonnet"
max      = "opus"
# inicio = "sonnet"               # the model behind Início; light if unset

# --- other agents ------------------------------------------------------------
[agents.claude]
history  = "all"                 # "all" | "projects" | "off"

[agents.gemini]
enabled     = true
cmd         = "gemini"
args        = ["--acp"]
memory_file = "GEMINI.md"
login_hint  = "gemini"
registry    = "gemini"
[agents.gemini.models]
light    = "gemini-2.5-flash-lite"
standard = "gemini-2.5-flash"
heavy    = "gemini-2.5-pro"
max      = "gemini-2.5-pro"
[agents.gemini.env]
# GEMINI_API_KEY = "…"           # never logged

[agents.claude-acp]
enabled = false                  # ships off: same subscription as claude

# --- voice ----------------------------------------------------------------
language      = "pt"             # what the MICROPHONE expects; "auto" detects per utterance
ui_language   = "pt"             # what the SCREEN shows: "pt" | "en"
voice         = "Luciana"        # a macOS say voice; on Linux a Speech Dispatcher voice (spd-say -L), empty = the language's default
whisper_model = ""               # empty = <data dir>/models/ggml-small.bin
hotkey        = "cmd+shift+space"  # ctrl+shift+space off the Mac
vocab         = ["webhook", "pull request", "deploy", "branch", "commit"]

# --- what a question can see ----------------------------------------------
repos                = []
hours_back           = 36
prompt_budget_chars  = 12000

# --- focus areas, kubectl style -------------------------------------------
default_context = "all"
[contexts.side-projects]
match_cwd = ["~/Projects/side"]
repos     = ["~/Projects/side/api"]

# --- workers --------------------------------------------------------------
worker_budget_usd = 5.0          # ceiling per TURN, held by Hark; 0 = off
worker_max_turns  = 0
worker_mode       = ""           # "" | "manual" | "acceptEdits" | "plan" | "auto" | "bypass"
batch_messages    = false

# --- updates ----------------------------------------------------------------
auto_update = true

# --- the assistant --------------------------------------------------------
assistant_name = "Hark"
theme          = "hark"          # "hark" | "dracula"

# --- community token-savers, per worker process ---------------------------
[assist]
ponytail  = "full"
caveman   = "off"
tokensave = "off"
```

## The keys worth understanding

**`language = "auto"`** lets the speech model detect the language of each utterance; a fixed code is more accurate on very short ones, so it stays the default.

**`language` vs `ui_language`.** Deliberately separate: `language` tunes the speech model; `ui_language` picks the language Hark writes and speaks in, including what the agent is told to answer in. Neither restricts what Hark understands.

**`worker_budget_usd`.** The guardrail, per turn. Hark prices every call of a running turn as the stream reports it and cuts the turn between calls once the estimate passes the ceiling, so the overshoot is at most one call. It is not Claude Code's own `--max-budget-usd`, which counts the whole process. `0` disables the ceiling.

**`worker_mode`.** The permission posture new workers start in. Empty means manual. No mode, including `bypass`, disables the production floor; `bypass` itself is Claude Code's, elsewhere it opens accept-edits and says so.

**`[agent]` and `[agents.<id>]`.** `plugin` picks the agent that opens new chats; an existing chat resumes on the agent that created it. Each `[agents.<id>]` is one agent: the built-ins ship with their commands and you override any field; a new id adds any ACP agent. `models` maps the tiers onto each agent's names. `env` is where a gateway goes (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` on a second `plugin = "claude"` entry); values in `env` are never logged.

**`[contexts]`.** Focus areas keyed by cwd prefix. `hark use <name>` switches the active one.

**`auto_update`.** The only network call Hark makes on its own: a GET against the GitHub releases. A new build downloads in the background, its minisign signature is verified against a key compiled into the app, then it waits for your click.

**`[assist]`.** Per-process environment variables for community token-savers, so their behaviour is set per worker, never in your global settings.

## The assistant's memory file

The home chat keeps its own personality and learnings in `~/.hark/HARK.md`, mirrored into the memory file each enabled agent auto-loads (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`) beside it. The model edits it itself through the normal permission flow; Settings has a button to open it.
