---
title: Plugins and switching agents
description: The catalog in Settings, which agent opens new chats, and moving a thread between agents.
sidebar:
  order: 3
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Settings › **Plugins** is the catalog: every agent Hark knows, whether its binary is on this machine and the install line when it is not, what each plugin can do (resume, permissions, cost reporting, history, slash commands…), what the agent calls each model tier, which build you have against what the ACP registry publishes, and which one opens new chats.

## One cockpit, several agents

Every chat belongs to the agent that opened it, and the pills beside the composer (permission mode, model tier, effort) speak to that agent in its own vocabulary. A knob an agent does not offer stays on screen, disabled, with the reason: hidden would read as "Hark cannot".

Two spoken commands move between agents:

- *"open a chat on gemini in webhook"*: a new chat in that project, on that agent.
- *"switch this chat to gemini"*: the thread you are in moves. A fresh session on the other agent opens with a local, zero-token brief of the conversation so far, on the same card. The ⓘ popover of a live thread has a button per available agent for the same move.

## Connectors

The session bar's *Connectors* section lists the MCP servers the session opened, with their state. Reconnecting is in Settings › Plugins.
