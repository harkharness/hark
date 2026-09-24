# Contributing to Hark

Thanks for helping. Hark has a clear shape — a cockpit over the agent CLIs
people already pay for, local-first, voice-first, honest about cost — and the
best contributions keep that shape.

## Good first contributions

- **An agent that speaks ACP.** Most new agents are one entry in
  `builtins()` (`crates/hark-core/src/domain/agents.rs`), with its `cmd`,
  `args`, memory file and model tiers, plus a line in
  [docs/PLUGINS.md](docs/PLUGINS.md). Anyone can also add one locally with an
  `[agents.<id>]` table in `~/.hark/config.toml` — see
  [docs/CONFIG.md](docs/CONFIG.md).
- **A backend for a CLI that does not speak ACP.** Start at
  [docs/PLUGIN-CONTRACT.md](docs/PLUGIN-CONTRACT.md).
- **Translations.** The interface speaks Portuguese and English
  (`src/lib/i18n.ts`); a new language is a new table.
- **Voice phrasings.** `crates/hark-core/tests/corpus/voice_corpus.jsonl` keeps
  the voice classifier honest: a phrase, the catalog it was said against, and
  where it should land.
- **Bug reports** with the Hark version, macOS version and architecture, and
  the agent CLI with its version. The issue form asks for all of it.

For anything bigger than a fix, open an issue first so we can agree on the
shape before you spend a weekend on it.

## Setup

You need macOS 12+, the Xcode command line tools, Rust (stable; CI pins
1.98.0), Node 22 and npm, and at least one agent CLI logged in — Claude Code
gives the richest experience.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev
```

The first run downloads the speech model (~500MB) and asks for the microphone.

## Checks

Everything CI runs, runnable locally:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
npx tsc --noEmit
npm test
npm run build
```

## How code is written here

- **Functional core, imperative shell.** `crates/hark-core/src/domain` is pure:
  no I/O, immutable types, tested without a microphone, a database or a
  network. Adapters and the Tauri layer stay thin.
- **Tests first in the domain.** Write the test, watch it fail for the reason
  you expect, then make it pass.
- **Agent output is untrusted.** A repository can prompt-inject an agent into
  printing anything. Code that turns agent text into an action — opening a
  file, writing to disk, the terminal, spawning a process — goes through a
  guard with a test. [SECURITY.md](SECURITY.md) has the model.
- **Sample data is invented.** Fixtures, mocks, screenshots and examples never
  carry real company, customer, ticket or person names.
- **Commits** are in English, `type(scope): summary` in lowercase (`fix`,
  `feat`, `chore`, `ci`, `docs`), with a body that explains why.

## Releases

Maintainers cut releases from a private pipeline that holds the signing
key. It builds exactly the tagged source in this repository and publishes to
its Releases; installed apps only accept updates signed by that key. One
feature release a week at most; fixes ship as patch releases when they are
ready.

## License

By contributing you agree that your contribution is dual licensed under
[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), as the rest of Hark, without
any additional terms or conditions.
