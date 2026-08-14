# Vox

A local, voice-first companion for [Claude Code](https://code.claude.com). Talk to it
like JARVIS: ask what you were working on, hear the answer out loud, and (eventually)
dispatch real work to Claude Code sessions.

Vox never calls the Anthropic API directly. It drives the `claude` CLI you already
have installed and authenticated, so usage draws from your existing subscription.

> Status: early development. The headless CLI (M1) works; voice and the
> desktop window are next. See the roadmap below.

## Quickstart (headless CLI)

```bash
cargo build --release -p vox-cli

# Index your local Claude Code history (incremental, fast after first run)
./target/release/vox index

# List recent sessions
./target/release/vox sessions

# Ask about your work (spawns `claude`, costs ~$0.04 of your plan usage)
./target/release/vox ask "what did I leave pending today?"

# Debug: print the exact prompt that would be sent, without calling Claude
./target/release/vox prompt "what did I leave pending today?"

# Contexts (kubectl-style focus per workspace; see Configuration)
./target/release/vox contexts
./target/release/vox use myproject

# Dispatch real work INTO an existing session (resumes it, full settings,
# every privileged tool call asks you y/N in the terminal)
./target/release/vox dispatch "continue the webhook migration, open the DNS PR"
./target/release/vox dispatch --session <id> "…"   # explicit target
./target/release/vox ps                             # machine-wide worker registry
```

Optional: `cargo install --path crates/vox-cli` puts `vox` on your PATH.

Vox keeps a `.vox/` directory in each workspace it touches (Q&A journal,
dispatch state and briefs). Add `.vox/` to your global gitignore, or commit it
if you want the trail visible to your team.

## What it will do

- **Ask about your work by voice or text**: "what's pending today?" Vox indexes your
  local Claude Code session history (`~/.claude/projects/**/*.jsonl`) into SQLite,
  builds a compact context snapshot, and asks Claude with a lean, cheap prompt.
- **Speak the answer**: responses are constrained by a JSON schema into
  `{ speech, details, items[] }`. Text-to-speech reads `speech`; the UI shows `details`.
- **Show everything**: a terminal-styled desktop window (Tauri + React) streams every
  event from the Claude CLI, including tool payloads.
- **Ask before acting**: in deep mode, tool permission requests surface in the UI and
  wait for your explicit approval (click first; voice approval later).

## Architecture

Hexagonal (ports & adapters) with a functional core and an imperative shell:

```
crates/vox-core      all logic
  src/domain/        pure functions, immutable types, no I/O (unit-tested)
  src/ports/         traits: Stt, Tts, AudioIn, AgentRunner, SessionStore, ...
  src/adapters/      thin imperative shells: claude CLI, sqlite, whisper, cpal, say
  src/app/           use cases wiring domain + ports
crates/vox-cli       headless driver: `vox ask "..."` (debugging, scripting, CI)
src-tauri + src/     desktop window driver (Tauri v2 + React), added in M2
spikes/              throwaway experiments that de-risked the design (kept as docs)
```

The state machine at the center is `fn update(State, Msg) -> (State, Vec<Effect>)`.
Shells interpret effects (record, transcribe, spawn claude, speak, notify UI); the
core stays pure and testable without a microphone, database, or network.

## Roadmap

- **M0**: scaffold + spikes (Claude CLI control protocol, whisper pt-BR quality)
- **M1**: headless brain: session index + snapshot + `vox ask "..."`
- **M2**: Tauri window (text in, streamed events, payload pane)
- **M3**: voice: push-to-talk hotkey, VAD, whisper STT, `say` TTS
- **M4**: interactive sessions, permission approvals, image paste
- **Later**: dispatching real project sessions, MCP tools, voice approvals, wake word

## Requirements

- macOS (Apple Silicon tested); other platforms untested for now
- [Claude Code](https://code.claude.com) installed and logged in (`claude` on PATH)
- Rust toolchain

## Configuration

`~/.config/vox/config.toml` (all paths are examples, nothing is hardcoded):

```toml
model = "sonnet"
language = "pt"
voice = "Luciana"
whisper_model = "~/.local/share/vox/models/ggml-large-v3-turbo.bin"
projects_dir = "~/.claude/projects"
repos = ["~/Projects/my-repo"]
hotkey = "Cmd+Shift+Space"
deep_triggers = ["investigate", "dig deeper"]
```

## Privacy

- The local index (`index.db`) contains fragments of your Claude Code prompts and
  session titles. It lives in your user data directory and never leaves your machine.
- Vox makes no network calls of its own. The only external process it talks to is
  the `claude` CLI, under your existing account.
- No telemetry.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
