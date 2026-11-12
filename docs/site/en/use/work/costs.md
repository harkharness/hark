---
title: Costs and limits
description: A local ledger of every turn, honest about what it does not know.
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Every turn lands in a local SQLite ledger: kind, project, task, model, tokens, cost, whether it failed. Settings › Costs breaks it down by day or week, by kind, project, task and model, shows your cache hit ratio, what the architecture avoided spending, and lists the most expensive sessions.

## Two rules keep the numbers honest

- **USD comes only from the agent's own reporting.** There is no price table anywhere in Hark. Claude Code and the Claude ACP adapter price their turns; Codex reports tokens only; Gemini reports nothing. A turn nobody priced shows as `$ –`, never as `$0.00`.
- **Token counts and dollars are never summed together.** They come from different sources, the live stream and the session logs, and the logs cover all of your agent usage, not just what went through Hark.

## The ceiling

Every turn runs under a dollar ceiling (`worker_budget_usd`, default 5) held by Hark itself: it prices the turn's calls as the stream reports them and cuts the turn between calls past the ceiling, then says how much, and offers to lift it for that chat, restart light, or switch model. On every agent.

## The cheap path is the default

Short, whole questions about the running workers, the board, the spend and the list of the day are answered from the local index for zero tokens. Phrasing-only questions get a light model with a small context. Only real reasoning pays full price. Say "think harder" to route up or "quick one" to route down, per question.

## Subscription windows

The 5h and 7d window percentages come from an opt-in bridge around the Claude Code status line (Settings › Costs), or from the Claude ACP adapter, which reports them on every turn.
