---
title: Claude Code
description: "The native plugin: the claude CLI with your settings, MCP servers, memory file and session history."
sidebar:
  order: 1
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Claude Code is Hark's native plugin: the `claude` CLI with your full settings, MCP servers, skills, memory file and session history on disk. It is the default agent when it is installed, and the one most of Hark was tested with.

## What you get

- **Your history.** Hark indexes the sessions the CLI already writes under `~/.claude/projects`, so a chat you started in the terminal three weeks ago is in the sidebar, resumable. `history = "all"` (the default) also offers folders the CLI worked in that are no project yet, one click each in Criar.
- **Your setup.** MCP servers, memory file, skills, permission settings and hooks load exactly as they do in your terminal. Close Hark and your terminal workflow is where you left it.
- **Costs.** Claude Code prices its turns, so the ledger shows dollars, not only tokens.
- **The subscription meter.** The 5h and 7d window percentages come from an opt-in bridge: a one-click wrapper around the Claude Code status line that tees its JSON to a file Hark reads. It backs up your settings first, changes nothing until you click, and uninstalls cleanly (Settings › Costs).
- **Bypass mode** is Claude Code's own: in it the production CLIs are blocked outright. See [Permissions](/docs/en/use/window/permissions/).
- **A chat's name is the chat's.** Renaming in Hark writes the same `custom-title` line the CLI's `/rename` writes, so `claude --resume`, Hark's rows and every other reader show one name.

## A company gateway

A second `[agents.<id>]` entry with `plugin = "claude"` and `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN` in its `env` routes the whole native experience through a gateway. Values in `env` are never logged. See [Configuration](/docs/en/use/reference/configuration/).

## If the binary lives somewhere unusual

```toml
claude_bin = "/opt/homebrew/bin/claude"
```
