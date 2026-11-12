---
title: Home and the chat
description: Início is Hark's own chat over every project; Code is where the work lives.
sidebar:
  order: 1
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

One window holds every project, behind two doors at the top of the sidebar, as in the Claude Code app. **Início** is Hark's own chat, one conversation over every project (*Hoje*), with the list of the day beside it. **Code** is where the work lives: **Criar** starts a new chat (the project comes first; a folder not yet registered becomes a project), then **Procurar**, the artifacts, the board shortcut and the projects with their chats, newest first, from the local index. Each project owns a colour; it runs down the sidebar group, across the top of the chat and through the composer's scope chip, which says where the next message lands.

## Hoje is Hark itself

Início › Hoje is not an agent session: one chat over every project, kept in a local journal with the days before it listed in the sidebar. Questions it can answer from the local index (what is on the board, which workers are running, what you spent today) never reach a model. Questions that only need rephrasing go to a light model with a small context. Only real reasoning pays full price. A sentence that is work ("continue the webhook migration") is resolved against every project and confirmed before it runs; work with no chat to land in gets a hint, not a session.

With no chat yet, Início greets you with one line of counts, four ways in (resume the latest, speak some work, find a chat, open the board) and the composer. Once the day has a chat, Hoje is that chat.

## Demandas: the list of the day

What asks for you, from five sources: the chats of the last 24 hours, a board card stopped at a permission or waiting for you, one stalled for days, a page an agent wrote in the last day, and your own requests ("remind me to call the bank at 2pm", typed or spoken). The list is grouped by what it asks (need you, to review, reminders, scheduled, in progress, stalled); *go* opens the chat or the page, *later* sinks a row for the day, *done* closes a request. "What do I have today?" in Hoje answers from this list, zero tokens. `hark demands` prints the same list in the terminal.

## Foco

"Focus on billing" (typed in Hoje, or *focus* on a demand) pins Início's composer to that chat without leaving Início: a chip at the top says which, what you write goes there, and a condensed mirror shows its tools as one line each. "Back" or "pause" releases it.

## The bell

Hark never interrupts a turn you are in. What finishes, asks or gets ready while you are elsewhere becomes a row behind the bell at the top: a chat's turn ended, a permission asked, a page an agent wrote. The sidebar shows the chat in strong type with a red dot; opening the chat reads its rows.

## Two keys

`Cmd+P` opens the files window with its filter ready. `Cmd+K` opens **Procurar**, one palette over everything the window knows (chats, pages, projects, the Claude Code history, a few actions) with a tab per kind. Local index, zero tokens.

## A new chat gets a name

The first message of a chat, when it reads as a sentence, is handed to the light model for a short title (a few cents, once). Renaming is the chat's: "rename this chat to dns fix", `/rename`, or the row's menu. The name is written into the session's own history, so the CLI shows it too.
