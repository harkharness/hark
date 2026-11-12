---
title: The session bar
description: Environment, context, connectors, outputs, changes, background tasks, costs and limits.
sidebar:
  order: 3
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

The session bar reads like the one in the Claude Code app: sections that fold and remember it, each with its summary while folded.

| Section | What it shows |
|---|---|
| **Environment** | this machine, the project and its folder, the branch, the agent and its knobs |
| **Context** | the window's share, and what fills it |
| **Connectors** | the MCP servers the session opened, with their state; reconnecting is in Settings › Plugins |
| **Outputs** | what the session wrote: a page under `~/.hark/artifacts` by its title and kind, opening in the artifact pane; a markdown of the workspace as a doc, any other file as a file, both opening in the files window |
| **Changes** | the git of the session: every file changed since the last commit, the lines of the one you open, a tick per file you have read, and "commit the changes", which hands the ask to the chat. Hark commits nothing itself |
| **Background tasks** | commands the agent left running, and how many finished |
| **Costs** and **Limits** | what this session cost, and the subscription windows |

The diff lives here and nowhere else: it is per session.
