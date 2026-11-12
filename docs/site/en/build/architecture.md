---
title: Project layout and architecture
description: Hexagonal, with a functional core and an imperative shell; where each thing lives.
sidebar:
  order: 2
---

Hark is hexagonal (ports and adapters), with a functional core and an imperative shell:

```
crates/hark-core           all logic, no UI
  src/domain/              pure functions and immutable types, no I/O
  src/ports/               traits: Stt, Tts, AgentRunner, SessionStore, SpendLedger…
  src/adapters/            thin shells: sqlite, whisper, cpal, say, pty
crates/hark-agent          the plugin contract: an event vocabulary + a capability sheet
crates/hark-plugin-claude  the Claude Code backend
crates/hark-plugin-acp     every Agent Client Protocol backend
crates/hark-cli            headless driver: hark ask / dispatch / spend
src-tauri + src/           the desktop app (Tauri v2 + React)
```

Domain code is test-driven and runs without a microphone, a database or a network. The voice pipeline is local: whisper.cpp (Metal on Apple Silicon, CPU elsewhere) for speech to text, the system voice for text to speech.

## Three answer layers

Deterministic questions (the board, running workers, today's spend) are answered locally for zero tokens. Phrasing-only questions get a light model with about 1k of context. Only real reasoning pays full price. A cheap evaluator gates a dispatch before a worker burns dollars on the wrong session.

## Local-first everything

Session history lives in a SQLite index built incrementally from the CLI's own logs. Recalling a chat, searching sessions, opening files and reading transcripts never touch a model. Every turn runs under a dollar ceiling held by Hark itself.

## Where the notes for agents live

The repository keeps the notes written for agents and for maintainers: `CLAUDE.md`, the artifact skill Hark installs, and `docs/FINDINGS*.md`, the measured findings behind the architecture (costs per path, what each CLI reports). The pages you are reading come from `docs/site/` in the same repository.
