---
title: Frontend and visual grammar
description: React 18 + TypeScript + Vite; one orchestrator, pure components, a demo mode for screenshots.
sidebar:
  order: 4
---

React 18 + TypeScript + Vite, functional components only. The layout mirrors the Rust side's philosophy: pure rendering at the edges, state and side effects concentrated in one orchestrator (`src/App.tsx`). `src/lib/ipc.ts` is the only place a Tauri command is invoked, `src/lib/events.ts` the only place events are listened to, `src/lib/*.ts` the pure rules (scope, search, diff, outputs, artifacts), `src/components/` the presentation with state coming in through props.

## Rules

1. **Components never call `invoke`.** Everything goes through `lib/ipc.ts`, so command names and payloads have one home.
2. **State lives in App.** Components receive data and callbacks; transient UI state (an open menu, a draft) may stay local, anything another component reads lives in App.
3. **Threads are keyed by task label**, not task id, because the transcript survives worker restarts. A line read back from the session file is dropped when the thread already shows it.
4. **Backend events arrive only via `useHarkEvents`.** A new event kind gets a branch there and a variant in `types.ts`.
5. **Zero-token features** (file viewer, quick open, board actions) never spawn an agent process. If a feature costs money it goes through the gate and router flows in App.
6. **The window runs under a CSP.** Scripts only from the bundle; artifact HTML renders in an `<iframe sandbox>` that adds no origin.

## Demo mode

`npm run dev`, then `http://localhost:1420/?demo=<scene>` in any browser: every Tauri command is answered from `lib/demo.ts` and every event comes from the scene's script, so the window runs with no backend. The scenes are `home`, `inicio`, `task`, `board`, `docs` and `costs`. The README's and the site's screenshots are taken here, at 1440×900 and 2×; nothing on them is a real project, session or name. `demo.test.ts` fails when a command the seam knows has no answer in the demo.

## Adding a Tauri command

1. Implement it in `src-tauri/src/lib.rs` and register it in `invoke_handler`.
2. Add a typed wrapper in `src/lib/ipc.ts`.
3. Add or extend the payload types in `src/types.ts` (field names stay snake_case, as serialized).

## The visual grammar

`docs/CHROME.md` in the repository is the visual grammar: measure, do not look; tokens on `:root` (a type scale in six sizes, body at 14px; colours; hairline borders); separate by what the content does, not by drawing a boundary; dividers are hairlines with a grab area; one column, one inset; every window has the overlay title bar; two doors, one sidebar; panel headers; and the cascade traps that made several of these look like design decisions when they were not. Titles, headings, menu items, buttons, placeholders and tooltips start with a capital letter.
