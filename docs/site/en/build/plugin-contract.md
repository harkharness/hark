---
title: The plugin contract
description: An event vocabulary and a capability sheet; unsupported is shown, never hidden.
sidebar:
  order: 3
---

Hark is a cockpit. The thing that actually talks to a model is a plugin, and the cockpit only knows it through a small contract in `crates/hark-agent`. Two plugins speak for every agent today: Claude Code, natively, and the ACP plugin for any agent that speaks the Agent Client Protocol.

## The event vocabulary

Whatever the backend prints, the plugin translates into a fixed set of events:

| Event | Meaning |
|---|---|
| `SessionStarted` | which session this process writes to, its slash commands, its MCP servers |
| `AssistantText` | prose from the agent, streamed |
| `ToolUse` | a tool was called |
| `ToolResult` | it finished, and whether it failed |
| `PermissionRequest` | the backend is asking whether a tool may run |
| `Result` | the turn ended: token usage per model, cost, error state |
| `RateLimit` | quota window status, when the backend reports one |
| `BackgroundDone` | a command the agent left running finished |
| `Ignored` | everything else, kept so the stream stays auditable |

## The capability sheet

What the backend can actually do: `resume`, `permissions`, `structured_output`, `cost_reporting`, `history`, `live_list`, `slash_commands`, `memory_file`, `shell_tools`, `fork`, `directive_mode`, `directive_model`, `directive_effort`. The rule is **unsupported is shown, never hidden**: a feature the plugin lacks stays on screen, disabled, with the reason as its hover. `shell_tools` names the shell tool for the production floor, so `terraform apply` can never be auto-approved whichever agent runs it. An ACP agent negotiates its sheet at every handshake.

Notice what is not in the contract: no prompt format, no model names, no vendor concepts.

## Out-of-process plugins are ACP

A community plugin is an ACP agent, in any language, plus one registry line. The built-ins follow the [ACP registry](https://github.com/agentclientprotocol/registry) for how each is distributed and invoked, and the version check reads it.

## The full walkthrough

`docs/PLUGIN-CONTRACT.md` in the repository walks through the contract in depth: the registry line, tiers, gateways, what ACP gives and what it does not, handoff between agents, and the plan to publish the contract as a crate.
