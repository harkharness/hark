---
title: What Hark is
description: An open-source desktop for the coding-agent CLIs you already pay for, with voice when you want it.
sidebar:
  order: 1
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Hark is a desktop app for the coding-agent CLIs you already pay for: Claude Code natively; Gemini CLI, Codex and any [Agent Client Protocol](https://agentclientprotocol.com) agent through one plugin. One window holds every project. Each chat opens next to a terminal, the project's files, a browser and the session panel; the agents produce documents, spreadsheets, whiteboards and dashboards you edit, comment on and send back; a board tracks every task, each card a session; the cost of every chat stays in view. Ask out loud and hear the answer when you want to, type when you don't.

## What it never does

Hark never calls a model API behind your back. It drives the agent CLIs you already have installed and authenticated, so every token draws from the subscription you already pay for. No API key, no second bill, no telemetry, no account. The speech model runs locally; audio never leaves the machine.

## What you need

- macOS 12 or newer, Apple Silicon or Intel. Linux and Windows are the next releases.
- At least one supported agent CLI installed and logged in. The [Claude Code CLI](https://code.claude.com) gets the richest experience; [Gemini CLI](https://github.com/google-gemini/gemini-cli), Codex (`@agentclientprotocol/codex-acp`) or any other ACP agent works through the ACP plugin.
- About 500 MB of disk for the speech model.

## Where to go next

- [Install](/docs/en/use/start/install/) and [First run](/docs/en/use/start/first-run/).
- [The window, panel by panel](/docs/en/use/window/home-and-chat/).
- [Artifacts: six kinds of page](/docs/en/use/artifacts/kinds/).
- Something broke? [Troubleshooting](/docs/en/use/reference/troubleshooting/).
