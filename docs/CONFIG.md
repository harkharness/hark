# Configuration

[← back to the README](../README.md)

Everything lives in `~/.config/hark/config.toml`. Every key is optional — the
file below is the full set with its defaults. Edits made through Settings
preserve your comments and formatting.

```toml
# --- the agent ------------------------------------------------------------
claude_bin       = "claude"      # binary name, or an absolute path
model            = "sonnet"      # what `--model` gets by default
projects_dir     = "~/.claude/projects"   # where the CLI keeps session logs

[agent]
plugin = "claude"                # which backend drives sessions

[models]                         # router tiers; unset falls back to `model`
light    = "haiku"
standard = "sonnet"
heavy    = "sonnet"
max      = "opus"

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
floor — commands that touch live infrastructure always ask.

**`prompt_budget_chars`.** The ceiling on how much context a cheap question is
allowed to carry. Lower is cheaper and blunter; the pruning keeps whatever is
topical to your question first.

**`[contexts]`.** Focus areas keyed by cwd prefix. `hark use <name>` switches
the active one, and questions get filtered to those sessions and repos. Mention
another context in a question and it applies just for that turn.

**`[assist]`.** Hark can pass per-process environment variables to community
token-savers so their behaviour is set **per worker**, never in your global
settings. The defaults are evidence-based, not enthusiasm-based: enabled where a
reproducible benchmark shows a win, off where it does not.

## The assistant's memory file

Separate from the config: the mother chat keeps its own personality and
accumulated learnings in a markdown file in the data directory
(`~/Library/Application Support/hark/CLAUDE.md` — named after whatever the
active plugin calls its memory file). It is plain text, the model edits it
itself through the normal permission flow, and Settings has a button to open it.
