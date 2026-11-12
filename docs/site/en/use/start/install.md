---
title: Install
description: One command installs the app and the CLI; or grab a .dmg from the latest release.
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

One command picks the right build for your Mac, installs the `hark` CLI into `~/.local/bin`, drops `hark.app` into `/Applications` and clears the quarantine bit (the app is not code-signed yet, so macOS would otherwise refuse to open it):

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

## Prefer to click?

Grab a `.dmg` from [the latest release](https://github.com/harkharness/hark/releases/latest):

| File | For |
|---|---|
| `hark-app-aarch64-apple-darwin.dmg` | macOS, Apple Silicon (M1 and newer) |
| `hark-app-x86_64-apple-darwin.dmg` | macOS, Intel |
| `hark-cli-<target>.tar.gz` | the headless `hark` CLI on its own |

If you download the `.dmg` by hand, run this once before opening the app:

```bash
xattr -cr /Applications/hark.app
```

## Build from source

You need the Xcode command line tools, Rust (stable; CI pins 1.98.0), Node 22 and npm.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev                     # the desktop app, with hot reload
cargo build --release -p hark-cli     # the headless CLI
```

The [contributor docs](/docs/en/build/contributing/) cover the rest: tests, the project layout and how a release is cut.

## Updates

When a new release lands, the app downloads it in the background, verifies its signature and shows one quiet pill in the main window: **restart to update**. Click it and the app reopens on the new version; ignore it and nothing happens. The check is a single GET against the GitHub releases, the only network call Hark makes on its own. Turn it off with `auto_update = false` and update by hand with the same install command. The `hark` CLI is updated by that script; the in-app updater replaces only the app bundle.
