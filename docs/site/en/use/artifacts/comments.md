---
title: Comments and @hark
description: A comment beside the page, anchored to a cell, a note, a slide or a mark; @hark turns it into a request.
sidebar:
  order: 3
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

"Comment" opens a column beside the page. Whatever is selected is the anchor (a cell, a note, a panel, a passage you selected in a document or a deck; else the whole page), and the comment is kept beside the page in `<page>.comments.json`, never inside it.

## @hark

A comment that names `@hark` is a request. "Send to hark" checks itself, the chat the page was born in is the default (any recent chat, Hark's chat or a new one can be picked), and the request goes as a message naming the page's link, the anchor and your words. When that chat writes the page, the comment reads "answered by hark" and the page shows the agent's version with accept and undo, as always. Plain comments stay yours and never enter a chat.

## The markup bar

On a deck, a design or an html document the comments column brings a markup bar: select (a click on a text anchors to it), pen, rectangle and text draw over the page, and the mark is the anchor, in page pixels, on the slide it sits on, with the words under it named. Open marks stay drawn over the page, numbered like their comments.
