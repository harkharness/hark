# Configuration

[← back to the README](../README.md)

Everything lives in `~/.hark/config.toml`. Every key is optional — the file
below is the full set with its defaults. Edits made through Settings preserve
your comments and formatting.

```toml
# --- the agent ------------------------------------------------------------
claude_bin       = "claude"      # binary name, or an absolute path
model            = "sonnet"      # what `--model` gets by default
projects_dir     = "~/.claude/projects"   # where the CLI keeps session logs

[agent]
plugin = "claude"                # which agent opens NEW chats (existing chats
                                 #   stay with the agent that opened them)
ask    = ""                      # which agent answers the cheap lane (voice
                                 #   ask, dispatch gate); empty follows `plugin`

[models]                         # the four tiers, in the global (Claude) names;
light    = "haiku"               #   each agent says them in its own names below
standard = "sonnet"
heavy    = "sonnet"
max      = "opus"

# --- other agents ------------------------------------------------------------
# Every agent Hark knows is one entry. The built-ins (claude, gemini, codex,
# claude-acp, deepseek, kiro, antigravity) can be overridden field by field;
# a new id adds an agent — anything that speaks the Agent Client Protocol.
[agents.gemini]
enabled     = true               # off = never offered, detected or not
cmd         = "gemini"           # the binary Hark looks for and spawns
args        = ["--acp"]
memory_file = "GEMINI.md"        # the file this agent auto-loads; Hark writes
                                 #   the assistant's persona into it
login_hint  = "gemini"           # what to tell you when it says "not logged in"
registry    = "gemini"           # its id in the ACP registry, for version checks
[agents.gemini.models]           # what THIS agent calls each tier
light    = "gemini-2.5-flash-lite"
standard = "gemini-2.5-flash"
heavy    = "gemini-2.5-pro"
max      = "gemini-2.5-pro"
[agents.gemini.env]              # extra environment for every process of it
# GEMINI_API_KEY = "…"           #   (a gateway URL, a token); never logged

[agents.claude-acp]
enabled = false                  # ships off: same subscription as `claude`

# --- voice ----------------------------------------------------------------
language      = "pt"             # what the MICROPHONE expects to hear;
                                 #   "auto" detects it per utterance
ui_language   = "pt"             # what the SCREEN shows: "pt" | "en"
voice         = "Luciana"        # a macOS `say` voice
whisper_model = ""               # empty = <data dir>/models/ggml-small.bin
hotkey        = "cmd+shift+space"  # global push-to-talk
vocab         = ["webhook", "pull request", "deploy", "branch", "commit"]
                                 # terms biased into the transcription

# --- what a question can see ----------------------------------------------
repos                = []        # git repos collected into the snapshot
hours_back           = 36        # how far back "recent sessions" reaches
prompt_budget_chars  = 12000     # ceiling on ask context (~chars/4 = tokens)

# --- focus areas, kubectl style -------------------------------------------
default_context = "all"          # "all" = no filter

[contexts.side-projects]
match_cwd = ["~/Projects/side"]  # cwd prefixes that belong to this context
repos     = ["~/Projects/side/api"]

# --- workers --------------------------------------------------------------
worker_budget_usd = 2.0          # hard ceiling per worker process; 0 = off
worker_max_turns  = 0            # turn ceiling per worker; 0 = off
worker_mode       = ""           # "" | "manual" | "acceptEdits" | "plan"
                                 #    | "auto" | "bypass"
batch_messages    = false        # queue follow-ups and send them as one turn

# --- updates ----------------------------------------------------------------
auto_update = true               # poll this repo for new builds; a ready
                                 #   update waits for your click to restart

# --- the assistant --------------------------------------------------------
assistant_name = "Hark"          # what the work chat calls itself
theme          = "hark"          # code color scheme: "hark" | "dracula"

# --- community token-savers, per worker process ---------------------------
[assist]                         # env vars for the worker only; your global
ponytail  = "full"               # settings are never touched
caveman   = "off"
tokensave = "off"
```

## The keys worth understanding

**`language = "auto"`.** Lets the speech model detect the language of each
utterance. That is what someone who genuinely switches languages mid-session
wants; a fixed code (`"pt"`, `"en"`) is more accurate on very short ones, so it
stays the default. Either way the spoken command grammar understands both.

**`language` vs `ui_language`.** They are deliberately separate: plenty of
people speak Portuguese to an English interface. `language` tunes the speech
model; `ui_language` picks the language Hark writes and speaks in — including
what the system prompt tells the agent to answer in. Neither one restricts what
Hark *understands*: the spoken grammar accepts Portuguese and English at the
same time.

**`worker_budget_usd`.** The guardrail. Every worker process spawns with a
dollar ceiling and stops itself when it hits it. `0` disables it — do that
knowingly.

**`worker_mode`.** The permission posture new workers start in when the
instruction does not say otherwise. Empty means the CLI's own default: ask about
everything. Note that no mode, including `bypass`, disables the production
floor — commands that touch live infrastructure always ask. `bypass` itself is
Claude Code's (its deny list is a Claude Code flag); on any other agent it opens
the thread in `acceptEdits` and says so.

**`[agent]` and `[agents.<id>]`.** `plugin` picks the agent that opens new
chats; an existing chat always resumes on the agent that created it. `ask`
picks who answers the cheap lane when that should not be the same agent. Each
`[agents.<id>]` entry is one agent: the built-ins ship with their documented
commands and you override any field; a new id adds any ACP agent. `models` is
how the one model pill drives every agent — you pick a tier, each agent hears
its own name for it; an ACP agent with no table runs its own default model and
the pill says so. `env` is where a gateway goes: `ANTHROPIC_BASE_URL` and
`ANTHROPIC_AUTH_TOKEN` on a second `plugin = "claude"` entry route the whole
native experience through a company LiteLLM. Values in `env` are never logged.

**`[models]`.** The global table, in Claude's names — the vocabulary the router
and the spoken cues ("pensa melhor", "quick one") use. Per-agent tables map
those tiers onto each agent's models.

**`prompt_budget_chars`.** The ceiling on how much context a cheap question is
allowed to carry. Lower is cheaper and blunter; the pruning keeps whatever is
topical to your question first.

**`[contexts]`.** Focus areas keyed by cwd prefix. `hark use <name>` switches
the active one, and questions get filtered to those sessions and repos. Mention
another context in a question and it applies just for that turn.

**`auto_update`.** The only network call Hark makes on its own: a GET against
this repository's `latest.json`. A new build downloads in the background and
its minisign signature is verified against a key compiled into the app —
then it waits. Nothing restarts without your click. `false` turns the check
off entirely; `install.sh` always works as the manual path.

**`[assist]`.** Hark can pass per-process environment variables to community
token-savers so their behaviour is set **per worker**, never in your global
settings. The defaults are evidence-based, not enthusiasm-based: enabled where a
reproducible benchmark shows a win, off where it does not.

## The assistant's memory file

Separate from the config: the mother chat keeps its own personality and
accumulated learnings in `~/.hark/HARK.md`, mirrored into the memory file each
enabled agent auto-loads (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`) in the same
folder so every agent reads the same persona. It is plain text, the model edits
it itself through the normal permission flow, and Settings has a button to open
it.
