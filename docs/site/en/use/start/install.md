---
title: Install
description: One command installs the app and the CLI; or grab a .dmg from the latest release.
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

One command picks the right build for your machine and installs the `hark` CLI into `~/.local/bin` and the app where your OS keeps apps:

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

On macOS that is `hark.app` in `/Applications`, with the quarantine bit cleared (the app is not code-signed yet, so macOS would otherwise refuse to open it). On Ubuntu, Debian and their relatives it is the `.deb`, installed through `apt` so the WebKit, GTK and Speech Dispatcher packages come with it (sudo asks for your password once). On any other Linux it is the AppImage in `~/.local/opt/hark`, with a launcher entry.

## Prefer to click?

Grab a package from [the latest release](https://github.com/harkharness/hark/releases/latest):

| File | For |
|---|---|
| `hark-app-aarch64-apple-darwin.dmg` | macOS, Apple Silicon (M1 and newer) |
| `hark-app-x86_64-apple-darwin.dmg` | macOS, Intel |
| `hark-app-x86_64-unknown-linux-gnu.deb` | Ubuntu 22.04+, Debian 12+ and relatives, x86_64 |
| `hark-app-aarch64-unknown-linux-gnu.deb` | the same, on arm64 |
| `hark-app-<target>.AppImage` | any other Linux (needs FUSE 2: `libfuse2`) |
| `hark-cli-<target>.tar.gz` | the headless `hark` CLI on its own |

If you download the `.dmg` by hand, run this once before opening the app:

```bash
xattr -cr /Applications/hark.app
```

A `.deb` installs with `sudo apt install ./hark-app-<target>.deb`; an AppImage only needs the executable bit.

## Build from source

You need Rust (stable; CI pins 1.98.0), Node 22 and npm. On macOS, the Xcode command-line tools; on Linux, the WebKitGTK, GTK, ALSA and Speech Dispatcher development packages: `scripts/dev-linux.sh` installs every one of them on Ubuntu and runs the app.

```bash
git clone https://github.com/harkharness/hark && cd hark
npm install
npm run tauri dev                     # the desktop app, with hot reload
cargo build --release -p hark-cli     # the headless CLI
```

The [contributor docs](/docs/en/build/contributing/) cover the rest: tests, the shape of the project, and how a release is cut.

## Updates

When a new release is out, the app downloads it in the background, verifies the signature and shows a quiet pill in the main window: **restart to update**. Click it and the app reopens on the new version; ignore it and nothing happens. The check is one GET against the GitHub releases, the only network call Hark makes on its own. Turn it off with `auto_update = false` and update by hand with the same install command. A `.deb` install belongs to `apt`: the app never replaces itself there, so the install command is how it updates. The `hark` CLI is updated by that script; the app's updater replaces only the app bundle.
