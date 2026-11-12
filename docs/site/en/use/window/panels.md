---
title: Panels beside a chat
description: The terminal, the files window, the browser, the artifact pane and a detached window.
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Each chat keeps its own side panels, stacked on the rail: the session bar, the terminal, the files window, the browser and the artifact pane open one over the other; each folds to its bar, and any one can take the whole rail. They belong to the chat they were opened in: come back to a chat and its shell, its open file or its page is still there, and another chat has its own.

## The chat

Transcripts in real markdown: headings, tables, code with syntax highlighting, tool calls folded, bursts of shell commands grouped. Deliverables surface as file cards; CSVs open as tables. Every privileged tool call renders as an inline [permission card](/docs/en/use/window/permissions/).

## The terminal

A real PTY terminal, with a tab per shell. A code block in a reply offers "run in terminal" and "insert in terminal" (without running).

## The files window

The project tree on the left, tabs for open files, an editor with save (`Cmd+S`). A path in a reply opens there, at its line. `Cmd+P` opens it with the filter ready.

## The browser

A real page beside the chat, in a webview of its own: its own origin, cookies and history, and no way to reach Hark. The bar is Hark's (back, forward, reload, the address, open outside); the page is whatever you point it at. One page per window, remembered per chat.

## The artifact pane

A page the agent wrote, clicked in the session bar's outputs or through its `hark://` link in a reply, opens beside the chat, never instead of it: the page alone, with its source, comment and "take to" controls. The pane's expand button opens the full [artifacts view](/docs/en/use/artifacts/kinds/) with a "back to chat" button; each chat remembers the page its pane showed.

## A window of its own

A chat can be detached into a window of its own (the ↗ button in the title bar). A detached window keeps receiving its project's tasks, spoken or clicked, until it is closed, and shows the board filtered to that project.
