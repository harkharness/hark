---
title: Six kinds of page
description: Documents, decks, designs, spreadsheets, whiteboards and dashboards, as plain files under ~/.hark/artifacts.
sidebar:
  order: 1
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

**Artifacts** is the documentation of this machine: pages you and the agents write, kept as a tree the way a wiki keeps them, one *space* per project plus a personal one. A page is a file under `~/.hark/artifacts/<space>/`; the tree is an `index.json` beside them, and a file dropped there by hand is adopted at the root on the next read. Nothing here costs tokens to browse.

## The kinds

| Kind | File | What it is |
|---|---|---|
| Document | `.md` | markdown, rendered as a reading column |
| Deck | `.html` | a run of slides on a 1280×720 stage, with Hark's pager, filmstrip, arrow keys and present mode |
| Design | `.html` | a canvas at its own size on a board Hark zooms and pans |
| Spreadsheet | `.sheet.json` | tabs of cells with values and formulas, computed by Hark |
| Whiteboard | `.board.json` | notes, groups, arrows and a timeline; the agent writes no coordinates |
| Dashboard | `.dash.json` | panels over data on this machine: a csv, a sqlite file, a command |

An html page says what it is with `<meta name="hark-kind" content="slides">`; without the tag a run of `class="slide"` sections reads as a deck. HTML renders in an isolated frame: a document with scripts off, a deck or a design with its own scripts on; either way the frame has no access to Hark.

## The tree, the gallery, the page

- **The tree column** lists every space and page. Click a page to read it, drag one onto another to move it under it, `+` on a row makes a child, `⋯` renames, changes the kind or removes (removing deletes the file and its children).
- **The home of a space** is a gallery: a tab per kind, tiles to create a page of each, and a card per page with a thumbnail drawn from the file itself.
- **A page** opens with its breadcrumb, kind, last touch and the chat it was born in (the chip opens that chat). **Links**, the panel beside the page, lists the origin chat, child pages and citations.

## How agents write pages

Agents write pages through `HARK_ARTIFACTS_DIR`, set on every worker, and a skill Hark installs under that folder says which space to pick, how to name the file and what never to do. A `Write` into the folder is caught as the page's origin, so the tree knows which chat produced which page. A spreadsheet, a whiteboard or a dashboard is json the agent can write without spending a token on layout.

## Taking a page to a chat

"Take to…" on a page offers Hark, the chat it was born in, recent chats or a new one, and puts the page's link in that chat's field; dragging a page onto a chat in the sidebar does the same. A page also reaches a chat by being named in a message ("…as in ADR 004"), by its `hark://artifacts/<space>/<page>` link, or through the session bar's outputs.

## Sharing

The share button on a page (and on a chat) delivers something on this machine: a local link that reopens Hark right there, the page as markdown or as one html file in `~/.hark/exports`, the file in the Finder. Nothing leaves this machine; shared spaces are a later phase.
