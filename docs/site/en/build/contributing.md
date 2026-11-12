---
title: Contributing
description: Good first contributions, the setup, the checks, and how code is written here.
sidebar:
  order: 1
---

Thanks for helping. Hark has a clear shape, a cockpit over the agent CLIs people already pay for, local-first, honest about cost, and the best contributions keep that shape. For anything bigger than a fix, open an issue first so we can agree on the shape before you spend a weekend on it.

## Good first contributions

- **An agent that speaks ACP.** Most new agents are one entry in `builtins()` (`crates/hark-core/src/domain/agents.rs`), with its `cmd`, `args`, memory file and model tiers, plus a line in the docs.
- **A backend for a CLI that does not speak ACP.** Start at [the plugin contract](/docs/en/build/plugin-contract/).
- **Testing Hark on another agent or system.** The most valuable thing right now: see [Testing other agents and OSs](/docs/en/build/testing-other-agents-and-oss/).
- **Translations**, of the app and of these docs: see [Translations](/docs/en/build/translations/).
- **Voice phrasings.** `crates/hark-core/tests/corpus/voice_corpus.jsonl` keeps the voice classifier honest: a phrase, the catalog it was said against, and where it should land.
- **Bug reports** with the Hark version, OS version and architecture, and the agent CLI with its version. The app's "Report a problem" fills it in for you.

## Setup

You need macOS 12+, the Xcode command line tools, Rust (stable; CI pins 1.98.0), Node 22 and npm, and at least one agent CLI logged in.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev
```

The first run downloads the speech model (about 500 MB) and asks for the microphone. To look at the window without a backend, and to take screenshots: `npm run dev`, then `http://localhost:1420/?demo=home`; see [Frontend](/docs/en/build/frontend/).

## Checks

Everything CI runs, runnable locally:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npx tsc --noEmit
npm test
npm run build
npm run docs:check
```

## How code is written here

- **Functional core, imperative shell.** `crates/hark-core/src/domain` is pure: no I/O, immutable types, tested without a microphone, a database or a network. Adapters and the Tauri layer stay thin.
- **Tests first in the domain.** Write the test, watch it fail for the reason you expect, then make it pass.
- **Agent output is untrusted.** A repository can prompt-inject an agent into printing anything. Code that turns agent text into an action (opening a file, writing to disk, the terminal, spawning a process) goes through a guard with a test. `SECURITY.md` has the model.
- **Sample data is invented.** Fixtures, mocks, screenshots and examples never carry real company, customer, ticket or person names.
- **Commits** are in English, `type(scope): summary` in lowercase (`fix`, `feat`, `chore`, `ci`, `docs`), with a body that explains why.

## License

By contributing you agree that your contribution is dual licensed under MIT or Apache-2.0, as the rest of Hark, without any additional terms or conditions.
