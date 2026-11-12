---
title: Editing by hand
description: Every page is yours to change in place; what the agent changed is marked until you accept it.
sidebar:
  order: 2
banner:
  content: "<b>Hark is in development.</b> Most testing so far was done with the Claude Code CLI on macOS. Running another agent or system? <a href=\"/docs/en/build/testing-other-agents-and-oss/\">Help us test it.</a>"
---

Every page is the person's to edit, and every edit of yours is a version the page accepts on the spot: your own edits are never something to review.

## Versions and marked changes

A page keeps the versions you saw: the first opening takes the baseline, "accept" takes the current text, going back keeps the current one first. Thirty live hidden beside the page (`.history/`). What an agent changed since you last accepted is marked (green cells in a sheet), with *accept* and *undo all* above.

## A spreadsheet

Hark's own grid: tabs, a formula bar, the values computed here with a small engine (`SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNTIF`, `SUMIF`, `IF`, `ROUND`, `TODAY`, and their Portuguese names), zero tokens. An error the engine cannot resolve is named in the cell (`#NOME?`, `#CICLO!`, `#DIV/0!`). Double-click edits a cell and writes the file. A column's letter opens its menu (insert, remove, a format: money, percent, integer, date, text); dragging the letter moves the column, its edge resizes it; a row number inserts or removes rows; a tab renames on a double click. Formulas follow their cells when rows and columns move; a removed cell leaves `#REF!`. The open tab copies as csv with values.

## A whiteboard

Notes on a canvas, groups around them and a timeline. Drag a note (into a group joins it), double-click to edit, add notes and groups, change colours. A selected note offers "link": the next click names the target and an arrow is drawn (click it to label or remove it). A group renames on a double click and its corner resizes it; the timeline opens a small form to add, edit or remove a milestone. A lasso on the background selects notes, and "ask about the selection" takes them to a chat as text.

## A dashboard

Panels over data on this machine: a csv or json file of the project, a sqlite file (read-only, SELECT only) or a command of the project that prints csv or json. The numbers are cached beside the page with the instant they were taken; "refresh now" runs every source, and that is the only time a command runs. A query you **certify** is yours: the agent uses it and never rewrites it. "Sources and queries" shows each query with its preview and lets you edit it in place, add and remove queries and sources. A pencil on a panel opens its form (title, query, chart, columns), with move and remove; the cadence is a select in the header. "Data story" asks the light model for one paragraph over the panels, which becomes a child page or goes to a chat after you read it.

## Words edited in place

With scripts on, a double click on a text in a deck, a design or an html document makes it editable right there (Esc cancels; Enter closes a heading). Hark places the change in the file by the element's old text, and refuses rather than guess when that text is not in the file exactly once. A markdown document edits with the source on the left and the rendered page on the right.
