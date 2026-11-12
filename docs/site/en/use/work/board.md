---
title: The board and dispatching work
description: A kanban where every card is a session; work resolved against the sessions on your disk.
sidebar:
  order: 1
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

## Dispatching work

Say, or type, what you want done: *"continue the webhook migration"*. Hark resolves it against the sessions on your disk by whole-word matching (never substrings, so "core" does not match "hark-core"), resumes the winner in its own workspace with your full settings, and streams every event back into the window. A rare term outweighs a common one; a close call surfaces as candidates instead of a guess.

Before an expensive worker starts, a cheap evaluator checks the instruction against the session it is about to land in: a few cents to avoid dollars spent in the wrong conversation. Heavy sessions get an honest warning first ("history of 3.8 MB") with the option to compact before sending. Workers run in parallel across projects, and every turn, including the ones that fail, lands in the ledger.

## The board

A kanban of tasks, in Início next to Hoje and Demandas (the Code door keeps a shortcut that opens it as a panel beside the chat). A **task is a deliberate record**: nothing creates one for you. It is born when you ask for it ("create task fix the dns"), when you accept the chat's own offer (after a few turns in a chat no card has, one line above the composer offers to register it, once), or on the board itself. A task is linked to the chat it was asked in; one chat may carry several tasks, and a task may gather several chats. Moving a card tracks the conversations doing the work, so resuming means resuming, not starting over. An approved plan becomes the card's checklist; finished work sinks to done on its own.

## Project windows

The board is global; a detached project window shows its slice, plus what the project has spent.
