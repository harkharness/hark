---
title: Gemini CLI, Codex and any ACP agent
description: "The ACP plugin: one protocol, every agent that speaks it."
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

The [Agent Client Protocol](https://agentclientprotocol.com) is the open editor-to-agent standard (JSON-RPC over stdio, a capability handshake, permissions on the wire) that Zed, JetBrains and Google ship against. Hark speaks it once, and every agent that speaks it plugs in: Gemini CLI (`gemini --acp`), Codex (`codex-acp`), Claude Code over ACP (`claude-agent-acp`, ships off), DeepSeek, Kiro, Google Antigravity, and any other ACP agent with one `[agents.<id>]` entry in config. Each built-in follows the [ACP agent registry](https://github.com/agentclientprotocol/registry) for how it is distributed and invoked.

## What the same cockpit gives every ACP agent

- **Permissions, always.** The same inline card as Claude Code, answered by click, keyboard or voice. Allow means once; standing rules are Hark's own.
- **Directives that cross.** The mode, model and effort pills speak to the agent in its own ids, for whatever it offered at session start. The model pill speaks in tiers, and each agent's registry line says what it calls them (`haiku` to Claude, `gemini-2.5-flash-lite` to Gemini). A knob an agent does not offer stays on screen, disabled, with the reason.
- **Costs where they exist.** The Claude adapter prices its turns; Codex reports tokens; Gemini reports nothing. A turn nobody priced is `$ –` with the reason on hover, never `$0.00`.
- **A history Hark writes.** ACP agents keep no session file, so Hark records each session it opens as the turns happen: searchable, readable, resumable. The honest limit: a session opened outside Hark stays invisible.
- **The production floor.** `kubectl apply`, `terraform apply`, force pushes and friends never auto-approve, whichever agent runs them and whatever it names its shell tool.
- **A ceiling.** The per-turn dollar ceiling is Hark's on every agent.
- **Handoff.** "Switch this chat to Gemini" moves the thread: a fresh session on the other agent, opened with a local zero-token brief, same card.

## Adding an agent that is not built in

```toml
[agents.myagent]
cmd  = "myagent"
args = ["--acp"]
```

It is in the catalog on the next launch. If the CLI your company allows does not speak ACP, [open an issue](https://github.com/harkharness/hark/issues) describing how it streams, whether it resumes sessions and whether it reports cost, or [write the plugin](/docs/en/build/plugin-contract/).

## Keeping agents current

Detected is not current. Settings › Plugins compares the version your binary reports with the one the registry publishes and shows the update line. An old Gemini CLI, for one, answers every new session with "This client is no longer supported"; that is the update, not a login problem.
