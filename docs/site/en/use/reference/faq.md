---
title: FAQ
description: Short answers first.
sidebar:
  order: 4
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

## Does Hark use my subscription?

Yes, that is the point. Hark shells out to the agent binaries you already authenticated: `claude`, `gemini`, `codex-acp` and so on. It has no API key and cannot bill you separately.

## Do I need to change how I work?

No. Hark reads the sessions the CLI already writes to disk, honours your settings, memory file, MCP servers and skills, and hands the same sessions back. Close Hark and your terminal workflow is where you left it. Agents that keep no session file get one written by Hark.

## Is it free? Is it open source?

Yes and yes. Hark is MIT or Apache-2.0 at your option, on [GitHub](https://github.com/harkharness/hark), and costs nothing beyond the agent subscriptions you already have.

## Where do the pages the agent writes go?

Into `~/.hark/artifacts/<space>/`, as markdown, html or json files you can open with anything. Hark keeps an `index.json` beside them and reconciles it with the disk on every read.

## Does anything leave my machine?

One call of its own: an update check against the GitHub releases, which you can turn off. Speech runs locally. The index and the ledger live in your user folder. No telemetry, no account.

## Windows or Linux?

macOS 12 or newer today, Apple Silicon and Intel. Linux and Windows are the next releases; [help us test them](/docs/en/build/testing-other-agents-and-oss/).

## Is Hark affiliated with Anthropic, Google or OpenAI?

No. Hark is an independent project. Claude Code, Gemini CLI and Codex are trademarks of their respective owners.
