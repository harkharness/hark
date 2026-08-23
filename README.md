# Hark

[![test](https://github.com/jhonmike/hark-harness/actions/workflows/test.yml/badge.svg)](https://github.com/jhonmike/hark-harness/actions/workflows/test.yml)

A local, voice-first cockpit for [Claude Code](https://code.claude.com). Talk to
your machine like JARVIS: ask what you were working on, hear the answer out
loud, and dispatch real work into your existing Claude Code sessions — by voice
or text, across every project on your disk.

Hark never calls the Anthropic API directly. It drives the `claude` CLI you
already have installed and authenticated, so all usage draws from your existing
subscription. No API key, no separate billing, no telemetry.

> Status: a working desktop app in active development (macOS, Apple Silicon
> and Intel; Linux is on the backlog). Expect sharp edges; the voice loop,
> windows, boards, cost ledger and the persistent assistant chat all work today.

## Install (from a release)

Binaries ship from the PUBLIC releases-only repo
([harkharness/hark](https://github.com/harkharness/hark)) — this source repo
stays private and is never exposed through them. Anyone can install with:

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

That downloads the latest public release for your architecture, installs the
`hark` CLI into `~/.local/bin`, drops `hark.app` into `/Applications` and
clears the quarantine bit (the app is not code-signed yet).

After installing: `hark setup` downloads the whisper speech model (~466MB),
and the first mic use asks for microphone permission.

Releases are cut by tagging: `git tag v0.x.y && git push --tags` builds both
macOS targets and publishes `.dmg`, `.app.tar.gz`, the CLI tarball and
checksums to a Release on `harkharness/hark` (version-less asset names, so
`releases/latest/download` links never break). Needs the `RELEASE_TOKEN`
secret — a PAT with write access to that public repo.

## What it does

- **One global voice surface.** A hotkey opens a floating HUD anywhere: speak,
  see the transcription and the resolved *destination* before anything runs,
  confirm by staying silent (when the target is the focused chat) or by voice
  ("yes" / "no" / a replacement instruction) when Hark had to guess.
- **A mother window that is your assistant.** A persistent chat backed by one
  long-lived Claude Code session with your full settings — MCP servers and
  tools included. It has a name, a personality file it maintains itself, and
  it speaks its replies. Cheap questions are routed AWAY from it to a bare,
  snapshot-fed one-shot ask (~cents), so the expensive session only does real
  work.
- **Project windows that read like Claude Code.** Sidebar with full session
  history, markdown transcripts with folded tool calls and grouped command
  bursts, deliverable file cards, CSV-as-table viewer, an embedded terminal
  (real PTY), a local file editor, and a kanban board of your demands.
- **Dispatch into existing sessions.** "Continue the webhook migration" finds
  the right session on disk (word-boundary matching, ambiguity surfaces as
  candidates — never a guess), resumes it in its own workspace with your full
  settings, and streams every event back.
- **Permissions with a hard floor.** Every privileged tool call renders as an
  inline card (allow / always / deny — keyboard, click or voice from any
  window). Commands that touch production (kubectl apply, terraform apply,
  helm, force-push, destructive SQL, cloud deletes, package publishes…) are
  NEVER auto-approved: standing rules are ignored, the card turns red, and in
  bypass mode the infra CLIs are blocked outright.

## Token efficiency by design

Measured on this codebase with `claude` v2.1.x:

| Path | Cost / turn |
|---|---|
| Lean ask (no tools, no settings, pre-cooked snapshot) | ~$0.009 |
| Default `claude -p` (CLAUDE.md + MCP + tools) | $0.23+ |
| Worker turn with tools executing | $0.33+ |
| Resumed worker turn (prompt cache warm) | ~$0.001 |

The architecture assumes tokens are the scarce resource:

- **Three answer layers**: deterministic questions (board, running workers,
  spend) are answered locally for zero tokens; phrasing-only questions get a
  light model with ~1k of context; only real reasoning pays full price.
- **A gate before the expensive path**: a haiku-tier evaluator (~$0.01) checks
  a dispatch against its target before a worker burns dollars on the wrong
  session.
- **Local-first everything**: session history lives in a SQLite index built
  incrementally from `~/.claude/projects`; recalling a chat, searching
  sessions, opening files and reading transcripts never touch a model.
- **Hard ceilings**: every worker spawns with `--max-budget-usd` (default $2)
  so a runaway turn stops itself.
- **An honest ledger**: every turn (including failures) lands in a local spend
  ledger — USD only from the CLI's own numbers, token counts from the session
  logs, the two never summed. The costs panel shows where money went, cache
  hit ratios, and your subscription windows (opt-in status-line bridge).

## Architecture

Hexagonal (ports & adapters) with a functional core and an imperative shell:

```
crates/hark-core      all logic
  src/domain/        pure functions, immutable types, no I/O (219 unit tests)
  src/ports/         traits: Stt, Tts, AgentRunner, SessionStore, SpendLedger…
  src/adapters/      thin shells: claude CLI, sqlite, whisper, cpal, say, pty
crates/hark-cli       headless driver: hark ask/dispatch/spend (scripting, CI)
src-tauri + src/     the desktop app (Tauri v2 + React)
```

Domain code is TDD'd and runs without a microphone, database or network.
The voice pipeline is fully local: whisper.cpp (Metal) for STT, the system
`say` voice for TTS. The only external process ever spawned is `claude`.

## Quickstart

```bash
# Desktop app (dev)
npm install
npm run tauri dev

# Headless CLI
cargo build --release -p hark-cli
./target/release/hark index                      # index your session history
./target/release/hark ask "what's pending today?"
./target/release/hark dispatch "continue the webhook migration"
./target/release/hark spend --week               # the ledger, in your terminal
```

First voice use: `hark setup` downloads the whisper model (~466MB); macOS asks
for microphone permission once.

## Configuration

`~/.config/hark/config.toml` — everything is optional, comments survive edits
made through the settings UI:

```toml
assistant_name = "Hark"        # what the chat calls itself
model = "sonnet"
language = "pt"               # what the mic expects to hear
ui_language = "pt"            # what the screen shows ("pt" | "en")
voice = "Luciana"             # macOS `say` voice
theme = "hark"                 # code color scheme: "hark" | "dracula"
hotkey = "cmd+shift+space"    # global push-to-talk
worker_budget_usd = 2.0       # hard ceiling per worker process
worker_mode = "acceptEdits"   # default permission mode for new workers

[models]                      # router tiers (all optional)
light = "haiku"
```

The assistant's personality and accumulated learnings live in a plain
markdown file in the app's data directory (Settings → open file). The model
maintains it itself through the normal permission flow: tell it "remember I
prefer short answers" and it writes that down — durable across sessions.

## Privacy

- The local index and spend ledger contain fragments of your prompts and
  session titles. They live in your user data directory and never leave the
  machine.
- Hark makes no network calls of its own; the only external process is the
  `claude` CLI under your existing account.
- No telemetry, no analytics, nothing phones home.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
