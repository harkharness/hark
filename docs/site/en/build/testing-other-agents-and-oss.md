---
title: Testing other agents and OSs
description: Most of Hark was tested with Claude Code on macOS. Here is how to help with the rest.
sidebar:
  order: 5
---

Most of the testing so far was done with the Claude Code CLI on macOS, Apple Silicon and Intel. Gemini CLI, Codex and the other ACP agents run, and ship, but have seen far fewer hours. Linux and Windows are the next releases and are not usable yet. This is where help matters most.

## Testing another agent

1. Install the agent's CLI and log in with it in your terminal first.
2. Open Settings › Plugins: the card shows whether Hark detected it, the version against the registry, and what the plugin says it can do.
3. Open a chat on it ("open a chat on gemini in webhook"), send a few turns, trigger a permission, let a turn run a shell command, and look at the ledger.
4. Report what did not match the docs: [open an issue](https://github.com/harkharness/hark/issues) with the agent and its version, your OS and architecture, the Hark release, and the app's diagnostics ("Report a problem" attaches them, scrubbed).

What we most want to know: did permissions arrive as cards, did the model and effort pills speak to the agent correctly, did the ledger show tokens and, where the agent reports it, dollars, and did a resumed chat really resume.

## Testing another OS

The ports are tracked in the repository. What is known to be macOS-only today: text to speech (`say`), the cue sounds, the Metal build of the speech model, opening files in the Finder, the window's overlay title bar, the shortcuts (`Cmd`), and the installer. If you can build from source on Linux or Windows, the most useful report is the first thing that breaks, with the build log.

## Reporting

Every report is public; read what the diagnostics contain before sending. Nothing leaves your machine through Hark itself.
