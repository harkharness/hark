---
title: Troubleshooting
description: Gatekeeper, the microphone, the hotkey, a missing agent, the speech model, history, costs, uninstalling.
sidebar:
  order: 3
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

## "hark.app is damaged and can't be opened"

macOS quarantines anything downloaded from the internet, and Hark is not code-signed yet. The installer clears the flag; if you downloaded the `.dmg` by hand:

```bash
xattr -cr /Applications/hark.app
```

If Gatekeeper still refuses, right-click the app, Open once, and macOS remembers the exception.

## The microphone does nothing

The first-run wizard records two seconds on purpose, which makes macOS ask for permission with context. If you skipped it or denied the prompt, grant it in System Settings › Privacy & Security › Microphone, then restart Hark. Nothing is recorded unless you press the hotkey, and audio never leaves the machine.

## The hotkey does not open anything

Another app owns the same shortcut. Change it in Settings, or in the config:

```toml
hotkey = "cmd+shift+h"
```

## "agent plugin not found"

Hark needs at least one agent CLI installed and logged in; it does not ship one and cannot authenticate for you. Check that it works on its own first:

```bash
claude --version
gemini --version
codex-acp --version
```

If the binary lives somewhere unusual, point Hark at it (`claude_bin`, or the `cmd` of the agent's entry), then Settings › Plugins › recheck.

## The catalog says my agent is behind

Settings › Plugins compares the version your binary reports with the one the ACP registry publishes and shows the update line. An old Gemini CLI answers every new session with "This client is no longer supported"; that is the update, not a login problem.

## The agent says it is not logged in

The card carries that agent's own login command. Log in once in your terminal and send the message again; Hark never types credentials for you.

## The speech model failed to download

It is about 466 MB over `curl` and checksummed on arrival. Retry from Settings, or from the terminal:

```bash
hark setup
```

## No session history in the sidebar

Hark indexes the agent CLI's own logs. If the directory is somewhere else, set `projects_dir` and rebuild the index:

```bash
hark index
```

## The costs panel shows tokens but no dollars

USD only ever comes from the agent's own reporting; Hark has no price table. A turn nobody priced shows `$ –` with the reason on hover.

## Uninstalling

```bash
rm -rf /Applications/hark.app
rm -f  ~/.local/bin/hark
rm -rf ~/.hark
```

The last one deletes your configuration, index, ledger, board, the assistant's memory file and Hark's own session records. If you installed the status-line bridge, uninstall it from Settings first so your agent CLI settings are restored.

## On Linux

The `.deb` belongs to `apt`: run the install command again to update, and `sudo apt remove hark` to uninstall. The AppImage updates itself like the Mac app does; it needs FUSE 2 to start on most distros (`libfuse2` on Ubuntu and Debian, `fuse2` elsewhere), or run it with `--appimage-extract-and-run`. The voice speaks through Speech Dispatcher: if Hark is silent, check that the service answers on its own, and pick a voice from its list in Settings:

```bash
spd-say -l pt-BR "olá"
spd-say -L
```

The microphone comes through PipeWire or PulseAudio; a desktop session has one of them. Under a virtual machine, a blank window usually means the WebKitGTK compositor and the virtual GPU disagree: start Hark with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Something else

[Open an issue](https://github.com/harkharness/hark/issues) with your macOS version, your architecture and the release you installed. The app's "Report a problem" (sidebar, Settings or the menu) fills it in for you, with scrubbed diagnostics if you tick the box: the issue is public, read the preview.
