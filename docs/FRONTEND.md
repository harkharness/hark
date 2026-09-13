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

See `docs/CHROME.md` for the visual grammar — tokens, elevation,
dividers, panel headers, the overlay title bar, and the cascade traps
that made several of them look like design decisions when they were not.

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

## Rendering cost: polls and the transcript

Measured 13/09 with the app idle and unfocused: WebContent CPU jumping
`0.0 → 14.7 → 0.1 → 13.6` in a 2.5s cycle, and a long thread freezing for
seconds on hover. Two causes, both structural:

1. **A poll that says nothing new must not wake React.** `setOwners(list)`
   with a fresh array from IPC is a state change every tick, and App is
   the root of the window — the whole tree re-rendered every 2.5s. Every
   poll sets state through `keepIfSame` (`lib/settle.ts`), which returns
   the OLD reference when the payload is structurally equal, so React
   bails out. A new poll follows the same rule, no exceptions.
2. **The transcript re-renders rows, not the thread.** Each row is a
   `memo` component keyed on the message's identity (`push` appends;
   `setMessages(old.map(...))` keeps untouched messages `===`), and
   `Markdown` is `memo` too — a parse plus a highlight per message per
   render was the cost. App hands the transcript inline lambdas and plain
   function declarations (a new identity per render), so Transcript wraps
   them in `useLatest` before they reach a row. `Transcript.test.tsx`
   pins both: a parent re-render with the same thread runs zero markdown;
   appending a reply renders exactly one.

The Rust side of the same bug: `session_owners` cached the `claude agents
--json` listing for 1.2s against a 2.5s poll, so every tick missed and
spawned a process (0.4s of wall clock each). The TTL now equals one poll
period, and the poll is 3s.
