# Frontend architecture

React 18 + TypeScript + Vite, functional components only. The layout
mirrors the Rust side's philosophy: pure rendering at the edges, state and
side effects concentrated in one orchestrator.

```
src/
  main.tsx            entry point: the main window (every project), a
                      detached project window (?project=), or the HUD
  Hud.tsx             the floating voice HUD window: the face of one voice
                      round (useVoiceRound) when the main window is not
                      in front
  App.tsx             THE orchestrator: all state, all flows, the layout;
                      the same component is the main window and a
                      detached project window (`forcedProject`)
  types.ts            shared types (mirror of the Rust command payloads)
  styles.css          single stylesheet, CSS variables for the palette
  lib/
    ipc.ts            typed wrappers for every Tauri command (the ONLY
                      place `invoke` is called); the seam routes to the
                      demo's fixtures under `?demo=`
    events.ts         the ONLY place `listen`/`emit` come from: Tauri, or
                      the demo's local bus under `?demo=`
    demo.ts           demo mode: invented projects, chats, pages, spend
                      and a script per scene, for the screenshots
    format.ts         pure formatting helpers (model names, directive chips)
    scope.ts          the scope chip (all / project / chat / page), pure
    artifacts.ts      the documentation tree flattened for painting, pure
    outputs.ts        the files a thread produced, from its tool calls
    diff.ts           the diff panel's rules: seen per file, totals, paths
    search.ts         "Procurar": the index of everything, sections, kinds
    board.ts          the board's derived state (columns, idle, attention)
    export.ts         a chat as markdown, a page as one html file (share)
    voice.ts          a voice round's words: confirm speed, target label
    transcript.ts     whether a log line read back is one the thread shows
    settle.ts         keepIfSame: a poll that says nothing new returns the
                      old reference, so React does not render
    filesTree.ts      the files window's tree, open tabs and filter, pure
    highlight.ts      highlight.js core + grammars for the file viewer
  hooks/
    useHarkEvents.ts   the single subscription to backend events
    useVoiceRound.ts   one voice round (hear → plan → confirm → execute),
                       the HUD's state machine lifted so the composer runs
                       it in place; no rendering
  components/         pure presentation, state comes in via props
    Sidebar.tsx       chats grouped by project (+ add/remove projects)
    Transcript.tsx    the visible thread (markdown, tool calls, permissions)
    Composer.tsx      textarea, @file autocomplete, paste, y/n shortcuts
    WorkerChips.tsx   live worker chips row
    FileViewer.tsx    local file view/edit panel (zero tokens)
    FilesWindow.tsx   the files panel of the rail: tree, tabs, editor
    FilesPanel.tsx    (FilesPanel/FilesEditor/FileTree are its parts)
    ShellTerminal.tsx the PTY terminal (xterm) and TerminalPane, its frame
    PanelFrame.tsx    one rail panel: title bar, fold, expand, close
    TurnStatus.tsx    the clock under the composer while a turn runs
    Meter.tsx         the subscription rings and the ledger pill
    SearchPalette.tsx "Procurar" (Cmd+K): chats, pages, projects, history
                      and actions, with a tab per kind; lib/search.ts rules
    Board.tsx         kanban tab
    VoiceRound.tsx    the rows of a voice round (stages), shared by the
                      HUD window and the composer's voice strip
    SharePopover.tsx  share on a chat or a page: local rows, honest footer
    DiffPanel.tsx     the workspace diff: changed files, one open, seen ticks
    BrowserPanel.tsx  the browser's bar; the page is a child webview the
                      panel keeps glued to its body (browser_* commands)
    Artifacts.tsx     the documentation tree, a space's home, the open page
                      (html in a sandboxed frame) and the Links rail panel
    SessionPanel.tsx  the rail's session panel: environment, context,
                      outputs, costs
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
   SearchPalette import ipc for file/page search, which is still ipc.)
2. **State lives in App.** Components receive data + callbacks. If a
   component needs its own transient UI state (open menu, draft text),
   that's fine — anything another component reads must live in App.
3. **Threads are keyed by task LABEL** (`Msg.task`), not task_id, because
   the transcript survives worker restarts and the label is stable. The
   worker's label and the chat's title can differ (a chat titled after
   its first prompt), so a turn's end also clears the clock of every chat
   on that session, found by session id. A line read back from the
   session file (the mirror, a catch-up) goes through `push(msg,
   {replay: true})`, which drops it when the thread already shows it
   (`lib/transcript.ts`): the live events and the file describe the same
   turn, and it must appear once.
4. **Backend events arrive only via `useHarkEvents`.** New event kinds get a
   branch there and a variant in `types.ts` (`HarkEvent`). `listen` and
   `emit` are imported from `lib/events.ts`, never from the Tauri API.
5. **Panels**: `react-resizable-panels` v2 (`PanelGroup/Panel/
   PanelResizeHandle`), layout persisted via `autoSaveId`. Sidebar
   collapses with Cmd+B; Cmd+P opens the files window with its filter
   focused; Cmd+K opens "Procurar".
6. **Zero-token features** (file viewer, quick open, board actions, task
   commands) must never spawn a `claude` process. If a feature costs money
   it goes through the gate/router flows in App.
7. **The window runs under a CSP** (`src-tauri/tauri.conf.json`): scripts
   only from the bundle, styles inline allowed (the components set them),
   images from the bundle, `data:`/`blob:` and https, connections to the
   Tauri IPC and the Vite dev server only. Anything that needs more must
   earn its directive there, not relax the policy. Artifact HTML renders
   in an `<iframe sandbox>` that inherits this policy and adds no origin.

## Demo mode: the real window, invented data

`npm run dev`, then `http://localhost:1420/?demo=<scene>` in any browser:
every Tauri command is answered from `lib/demo.ts` and every event comes
from the scene's script, so the window runs with no backend at all. The
scenes are `home` (Hark's chat with a voice round over the composer),
`inicio` (the same day with no chat yet: Hoje opens with the brief),
`task` (a chat with a permission card and the rail), `board`, `docs` (a
page open in the artifacts view) and `costs`. The README's and
the site's screenshots are taken here, at 1440×900 and 2× — nothing on
them is a real project, session or name. `demo.test.ts` fails when a
command the seam knows has no answer, so a new command lands in the demo
the day it lands in `ipc.ts`.

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
