# Frontend architecture

React 18 + TypeScript + Vite, functional components only. The layout
mirrors the Rust side's philosophy: pure rendering at the edges, state and
side effects concentrated in one orchestrator.

```
src/
  main.tsx            entry point
  App.tsx             THE orchestrator: all state, all flows, the layout
  types.ts            shared types (mirror of the Rust command payloads)
  styles.css          single stylesheet, CSS variables for the palette
  lib/
    ipc.ts            typed wrappers for every Tauri command (the ONLY
                      place `invoke` is called)
    format.ts         pure formatting helpers (model names, directive chips)
    highlight.ts      highlight.js core + grammars for the file viewer
  hooks/
    useHarkEvents.ts   the single subscription to backend events
  components/         pure presentation, state comes in via props
    Sidebar.tsx       chats grouped by project (+ add/remove projects)
    Transcript.tsx    the visible thread (markdown, tool calls, permissions)
    Composer.tsx      textarea, @file autocomplete, paste, y/n shortcuts
    WorkerChips.tsx   live worker chips row
    FileViewer.tsx    local file view/edit panel (zero tokens)
    QuickOpen.tsx     Cmd+P fuzzy file search
    Board.tsx         kanban tab
    Reader.tsx        read-only session history
    Modals.tsx        confirm/choice/resume/summary dialogs
    Markdown.tsx      react-markdown + remark-gfm + rehype-highlight
    ToolCall.tsx      per-tool payload rendering (Bash → shell block, …)
```

## Rules

1. **Components never call `invoke`.** Everything goes through `lib/ipc.ts`,
   so command names/payloads have one home. (Exceptions: none. Composer and
   QuickOpen import ipc for file search, which is still ipc.)
2. **State lives in App.** Components receive data + callbacks. If a
   component needs its own transient UI state (open menu, draft text),
   that's fine — anything another component reads must live in App.
3. **Threads are keyed by task LABEL** (`Msg.task`), not task_id, because
   the transcript survives worker restarts and the label is stable.
4. **Backend events arrive only via `useHarkEvents`.** New event kinds get a
   branch there and a variant in `types.ts` (`HarkEvent`).
5. **Panels**: `react-resizable-panels` v2 (`PanelGroup/Panel/
   PanelResizeHandle`), layout persisted via `autoSaveId`. Sidebar
   collapses with Cmd+B; quick-open on Cmd+P.
6. **Zero-token features** (file viewer, quick open, board actions, task
   commands) must never spawn a `claude` process. If a feature costs money
   it goes through the gate/router flows in App.

## Adding a Tauri command

1. Implement in `src-tauri/src/lib.rs`, register in `invoke_handler`.
2. Add a typed wrapper in `src/lib/ipc.ts`.
3. Add/extend payload types in `src/types.ts` (keep field names snake_case
   as serialized by serde).
