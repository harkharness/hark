# Using Hark

[← back to the README](../README.md)

## First run

Open `hark.app`. A wizard runs once and refuses to leave you with a half-working
install:

1. **Language, name and hotkey.** What the microphone expects to hear, what the
   screen shows, what you want to call your assistant, and the global key that
   opens the mic from anywhere. All of it is editable later in Settings.
2. **Agent plugin.** Hark checks which agent CLIs are installed and logged in.
   Claude Code plugs in natively; Gemini CLI, Codex, Claude Code over ACP and
   any other Agent Client Protocol agent through the ACP plugin. The catalog
   shows each one, its install line, and whether your build is the one the ACP
   registry publishes. Nothing runs until at least one is detected.
3. **Speech model.** Downloads a whisper model (`small`, ~466MB, the default;
   `large-v3-turbo` if you want more accuracy for the price of latency). The
   download is checksummed and resumable.
4. **Microphone test.** Records two seconds and shows you the transcription —
   which is also what triggers the macOS permission prompt, with context, before
   you need it in the middle of something.

Everything the wizard writes lands in `~/.hark/config.toml`, in plain text,
with your comments preserved on later edits.

## Two languages, at the same time

Hark's spoken grammar accepts **Portuguese and English simultaneously** — you
never pick one. `abre a board` and `open the board` both switch tabs. `sim` and
`yes` both confirm; `não` and `no` both refuse. `a primeira` and `the first one`
both pick the first candidate. `capricha` and `take your time` both raise the
effort of the turn.

Words that mean different things in each language are disambiguated rather than
dropped. A bare "no" is a refusal; "no projeto webhook" addresses a target and
is never read as one.

Two separate settings control the rest:

| Setting | What it decides |
|---|---|
| `language` | what the speech model expects to **hear** — a fixed code, or `auto` to detect each utterance |
| `ui_language` | what Hark **writes and says** — interface, local answers, warnings, and the language the agent is told to answer in |

They are separate on purpose: plenty of people speak Portuguese to an English
interface, or the other way around. Recognition does not care either way.

## The voice loop

Press the hotkey (`cmd+shift+space` by default) anywhere, in any app. With the
main window in front, the composer itself listens: the round draws over the
text (waveform, what it heard, the target chip) and the window stays where you
were reading. Anywhere else a floating HUD opens instead. Either way it stops
on its own after a beat of silence, and the mic button on the composer starts
the same round.

Before anything runs, Hark shows you two things: **what it heard** and **where
it is going**. That second part is the whole point — a voice command that lands
in the wrong session is worse than no voice command at all. Confirmation comes
at one of two speeds:

- **Target is the chat you are already in** → a chip appears and 1.6 seconds of
  silence executes it. No ceremony for the common case.
- **Hark had to resolve the target** (by search, or the match was ambiguous, or
  it is a brand new session) → Hark *says* the destination out loud and waits
  for a verdict. Say `sim` to go, `não` to cancel, or simply say a different
  instruction and it replaces the text and asks again. Ambiguity is never
  guessed away: you get numbered candidates and pick one ("a primeira").

Answers are spoken back through the system voice while the details render on
screen. Work sent by voice never switches what you are looking at: the chat
it landed in pulses in the sidebar for a beat, and the general thread keeps
one line per dispatch with "ir para o chat" on it.

**Four spoken verbs** move Hark's attention, the whole sentence and nothing
else: "foca no billing" (or "calma, vamos focar nessa") pins Início to
that chat, "volta" or "pausa" releases it. In a focus, a sentence with no
verb goes to the chat in focus; with no focus, Hark finds the target as
above, and work with no home gets a hint instead of a session of its own.

**When Hark speaks on its own.** Never with the window in front: text does
that job. Behind the window, a chat finishing its turn is said aloud as
before, unless you are in a focus — then only the chat in focus ending is
said, and everything else waits behind the bell.

## The main window

One window holds every project, behind two doors at the top of the
sidebar, as in the Claude Code app. **Início** is Hark's own chat, one
conversation over every project (*Hoje*), with the artifacts beside it.
**Code** is where the work lives: **Criar** starts a new chat (the project
comes first; a folder not yet registered becomes a project; folders the
CLI worked in that are no project yet are listed there, one click each),
then **Procurar**, **artifacts**, the **board** shortcut and the projects
with their chats. The rows of a project are its recent chats, from the
local index, newest first; a card decorates the chat it is tied to (its
name, its status dot, a count of the tasks the conversation carries), and
only pinned cards keep a row of their own. A project group folds to one
line (the chevron, remembered)
and its menu holds the rest: new chat, files, every chat of it in the
palette, collapse, remove. Settings (costs inside) sit at the
foot of either door. The door is remembered; opening a chat brings Code
along. The composer carries a scope chip that says where the next
message lands: *all projects* in Início, `project › chat` when a chat is
open. Each project owns a colour; it runs down the sidebar group, across
the top of the chat and through the chip.

**Each chat keeps its own side panels, stacked.** The terminal, the files
window, the diff and the browser open on the rail together, one over the
other; each folds to its bar, and any one can take the whole rail. They
belong to the chat they were opened in: come back to a chat and its shell,
its open file or its page is still there, and another chat has its own. The board and the artifacts are views of their own, with
no chat panels; the artifacts view has the page's links instead.

**Two keys.** `Cmd+P` opens the files window with its filter ready, the
way Claude Code's app does; `Cmd+K` (or the magnifier) opens **Procurar**,
one palette over everything the window knows — chats, artifact pages,
projects, the Claude Code history, a few actions — with a tab per kind.
Before you type it shows what needs you, what moved last and what you can
do. Local index, zero tokens.

**Início › Demandas is the list of the day**: what asks for you, from five
sources — the chats of the last 24 hours (every session in the index, not
only the board's), a board card stopped at a permission or waiting for
you, one stalled for days, a page an agent wrote in the last day, and your
own requests ("me lembra de ligar pro banco às 14h", typed or spoken, kept
in the local index with the time it names). A chat whose last word is the
agent's is waiting for you; one where you spoke last is *working*. A card
tied to a live chat is told by the chat, under the card's name. The list
is grouped by what it asks (need you, to review, reminders, scheduled, in
progress, stalled): the group says it once, a row is the title and when,
the reason on hover. Ordered by urgency inside each group; *ir* opens the
chat or the page (clicking the row does too), *depois* sinks a row for the
day, *feito* closes a request. The board stays the team's: a demand may
point at a card, it never replaces one. "O que eu tenho hoje?" in Hoje
answers from this list, zero tokens. Cards stalled for more than two weeks
are forgotten work: they collapse into one line that counts them, opens
the board, and archives them all with one click. `hark demands` prints
the same list from the terminal.

**What answers for free, and what goes up.** Short, whole questions about
the running workers, the board, the spend and the list of the day are
answered from the local index. A question that hooks into the turn before
("mas quais as pendências?"), a long one, or one about *one* thing ("as
pendências do patch?") goes to the model with the snapshot, and so does a
long sentence that happens to carry an order's verb in the middle: orders
are short and come first. The snapshot carries, for each session, your
last requests and the agent's **last reply**, so "status do KR 1.3" is
answered by what the agent said last. The first launch after this change
rebuilds the index from your session files once.

**Foco.** "Foca no billing" (typed in Hoje, or *focar* on a demand) pins
Início's composer to that chat without leaving Início: a chip at the top
says which, what you write goes there, and a condensed mirror above the
composer shows its tools as one line each and its words whole; the full
chat is one click away in Code. "Volta" or "pausa" releases it. When the
chat in focus finishes its turn, a line in Hoje says so and lists what
waits.

**The bell.** Hark never interrupts a turn you are in. What finishes,
asks or gets ready while you are elsewhere becomes a row behind the bell
at the top: a chat's turn ended, a permission asked, a page an agent
wrote. The sidebar shows the chat in strong type with a red dot; opening
the chat reads its rows on the spot, and seeing them in the panel reads
them too. A read row leaves the panel ten minutes later; an unread one
never does; "ver anteriores" brings the history back from the local
index. The same line lands in Hoje, with "ir" beside it.

**Hoje opens with the day.** With no chat yet, Início greets you with one
line of counts, the composer right under it, the hints, and three boxes
at a glance (who needs you, what is ready to review, what stalled, three
rows each with a way to the full list); the first message is still yours.
Once the day has a chat, Hoje is that chat.

**Início › Hoje is Hark itself**, not an agent session: one chat over every
project, kept in a local journal with the days before it listed in the
sidebar (click one to read it back). Nothing you type there opens a
long-lived worker. Questions it can answer from the
local index — what is on the board, which workers are running, what you spent
today — never reach a model at all. Questions that only need rephrasing go to a
light model with a small context (`models.inicio` in `config.toml` names
that model; the light tier by default). Only real reasoning pays full
price. A sentence that is work ("continua a migração do webhook") is
resolved against every project and confirmed before it runs; it never
opens a window. Work with no chat to land in gets a hint, not a session:
name the chat, or create one in Code › Criar.

## Artifacts

**Artifacts** is the documentation of this machine: pages you and the agents
write, kept as a tree the way a wiki keeps them, one *space* per project plus
a personal one. A page is a markdown or an HTML file under
`~/.hark/artifacts/<space>/`; the tree is an `index.json` beside them, and
a file dropped there by hand is adopted at the root on the next read.

- **The tree column** lists every space and page. Click a page to read it,
  drag one onto another to move it under it, the `+` on a row makes a
  child, the `⋯` renames or removes (removing deletes the file and its
  children — it is your file, so it asks nothing twice).
- **The space home** shows what moved last. The tree is the documentation;
  the home is only the freshest touches.
- **A page** opens with its breadcrumb, its kind, when it was edited and
  the chat it was born in, when an agent wrote it: that chip opens the chat.
  Markdown reads rendered and edits in place (the pencil). HTML renders in
  an isolated frame with scripts off; turn them on for a slide deck that
  needs them — even then the frame has no access to Hark.
- **The view is documentation, not a chat.** There is no composer here.
  "Levar para…" on a page offers Hark, the chat it was born in, recent
  chats or a new one, and puts the page's link in that chat's field;
  dragging a page onto a chat in the sidebar does the same. A page also
  reaches a chat by being named in a message
  ("…como ficou no ADR 004"), by its `hark://` link, through the session
  panel's outputs, or by opening its file in the files window.
- **A new chat gets a name.** The first message of a chat, when it reads
  as a sentence, is handed to the light model for a short title (a few
  cents, once); a short message is its own name.
- **Links** (the panel beside the page) says what the page is, where it
  came from and what it ties to: the origin chat, child pages, citations.

- **Six kinds.** A document (`.md`), a deck and a design (`.html`), and
  three the agent writes as json and Hark computes and draws: a **sheet**
  (`.sheet.json`), a **whiteboard** (`.board.json`) and a **dashboard**
  (`.dash.json`). The extension says what the page is; the home of a
  space counts them and lists one kind on a click.
- **A sheet** opens as Hark's own grid: tabs, a formula bar, the values
  computed here with a small engine (SOMA/SUM, MÉDIA/AVERAGE, MÍN, MÁX,
  CONT.SE, SOMA.SE, SE/IF, ARRED, HOJE…), zero tokens. An error the
  engine cannot resolve is named in the cell (`#NOME?`, `#CICLO!`,
  `#DIV/0!`). Double-click edits a cell and writes the file; the open tab
  copies as csv with values. The grid is yours to shape: a column's
  letter opens its menu (insert, remove, a format: money, percent,
  integer, date, text), dragging the letter moves the column, its edge
  resizes it; a row number inserts or removes rows; a tab renames on a
  double click and the plus adds one. Formulas follow their cells when
  rows and columns move (a removed cell leaves `#REF!`).
- **A dashboard** is panels over data on this machine: a csv or json
  file of the project, a sqlite file (read-only, SELECT only) or a
  command of the project that prints csv or json. The numbers are cached
  beside the page (`x.dash.cache.json`) with the instant they were taken;
  "refresh now" runs every source, and that is the only time a command
  runs. A query you **certify** is yours: the agent uses it and never
  rewrites it; a panel on an uncertified query says "agent's draft".
  "Sources and queries" shows each query, its preview and the badge,
  and lets you edit it in place (sql for sqlite, a column pick and a
  limit for the rest), add and remove queries and sources; a query you
  save is certified by that. A pencil on a panel opens its form (title,
  query, chart, columns), with move and remove; the cadence is a select
  in the header.
  "Data story" asks the light model for one paragraph over the panels
  (one call, when you click); it becomes a child document page or goes
  to a chat, after you read it. Every panel copies its data as csv.
- **A whiteboard** is notes on a canvas, groups around them and a
  timeline. The agent writes notes without coordinates and Hark lays
  them out; you drag them (into a group joins it), double-click to edit,
  add notes and groups, change colours. A selected note offers "link":
  the next click names the target and an arrow is drawn (click it to
  label or remove it). A group renames on a double click and its corner
  resizes it; the timeline opens a small form to add, edit or remove a
  milestone. A lasso on the background selects notes, and "ask about
  the selection" takes them to a chat as text (only the excerpt enters,
  with the page's link).
- **Words edited in place.** With scripts on, a double click on a text
  in a deck, a design or an html document makes it editable right there
  (Esc cancels; Enter closes a heading). Hark places the change in the
  file by the element's old text, and refuses rather than guess when
  that text is not in the file exactly once ("edit the source" then). A
  markdown document edits with the source on the left and the rendered
  page on the right.
- **Comments, and `@hark`.** "Comment" opens a column beside the page.
  Whatever is selected is the anchor (a cell, a note, a panel, a
  passage you selected in a document or a deck; else the whole page),
  and the comment is kept beside the page in `x.comments.json`, never
  inside it. A comment that names `@hark` is a request: "send to hark"
  checks itself, the chat the page was born in is the default (any
  recent chat, Hark's chat or a new one can be picked), and the request
  goes as a message naming the page's link, the anchor and your words.
  When that chat writes the page, the comment reads "answered by hark"
  and the page shows the agent's version with accept and undo, as
  always. Plain comments stay yours and never enter a chat.
- **Versions and marked changes.** A page keeps the versions you saw:
  the first opening takes the baseline, "accept" takes the current text,
  going back keeps the current one first. Thirty live hidden beside the
  page (`.history/`). What an agent changed since you last accepted is
  marked (green cells in a sheet), with *accept* and *undo all* above;
  your own edits through Hark are never something to review: each one
  is a version the page accepts on the spot.

Agents write pages through `HARK_ARTIFACTS_DIR`, set on every worker; a
`Write` into that directory is caught as the page's origin, so the tree
knows which chat produced which page. Nothing here costs tokens to browse.

## Sharing

The share button on a chat's title bar and on an artifact page opens the
same popover. Every row delivers something on this machine today: a local
link (`hark://chat/<session>`, `hark://artifacts/<space>/<page>`) that
reopens Hark right there, the `claude --resume` command, the transcript as
markdown, the page as markdown or as one html file in `~/.hark/exports`,
the file in the Finder, and "save brief as a page" (the chat's local brief
becomes a page in its project's space). The footer says the limit of
today: nothing leaves this machine; shared spaces are a later phase, with
a link to vote for them.

## Project windows

A chat can be **detached** into a window of its own (the ↗ button in the
chat's title bar). A detached window keeps receiving its project's tasks,
spoken or clicked, until it is closed. Detached or not, a project's chat
reads like the terminal you already use:

- **Session history** in the sidebar, indexed from the agent CLI's own logs — a
  chat you started in the terminal three weeks ago is right there. An agent
  that keeps no logs (the ACP agents) gets its history written by Hark as the
  turns happen, so its chats are searchable, readable and resumable too; the
  honest limit is that a session opened outside Hark stays invisible.
- **Transcripts** in real markdown: headings, tables, code with syntax
  highlighting, tool calls folded, bursts of shell commands grouped.
- **Deliverables** surface as file cards; CSVs open as tables; there is a real
  PTY terminal and a local file editor in the same window.
- **The diff** of the workspace (the git-compare door in the chat's title
  bar): every file changed since the last commit, untracked ones too, the
  lines of the one you open, and a tick per file you have read — kept on
  this machine, cleared when the file changes again. A readout over `git`;
  it commits nothing.
- **A browser** (the compass door): a real page beside the chat, in a
  webview of its own — its own origin, cookies and history, and no way to
  reach Hark. The bar is Hark's (back, forward, reload, the address, open
  outside); the page is whatever you point it at. One page per window,
  remembered per chat, closed with the panel.
- **The board, filtered** to that project, plus what the project has spent.

## Dispatching work

Say — or type — what you want done: *"continua a migração do webhook"*. Hark
resolves it against the sessions on your disk by whole-word matching (never
substrings, so "core" does not match "hark-core"), resumes the winner **in its
own workspace with your full settings**, and streams every event back into the
window. A rare term outweighs a common one; a close call surfaces as candidates
instead of a guess.

Before an expensive worker starts, a cheap evaluator checks the instruction
against the session it is about to land in — a few cents to avoid dollars spent
in the wrong conversation. Heavy sessions get an honest warning first
("histórico de 3.8 MB") with the option to compact before sending.

Workers run in parallel across projects. Every turn — including the ones that
fail — lands in the ledger.

## Permissions

Every privileged tool call renders as an inline card in the thread: the tool,
the payload, and allow / always / deny. Answer by click, by keyboard, or by
voice from any window. Nothing blocks the rest of the interface — a permission
waiting in one thread never freezes another.

**There is a hard floor.** Commands that touch production — `kubectl apply`,
`terraform apply`, `helm`, force-pushes, destructive SQL, cloud deletes, package
publishes — are never auto-approved, whichever agent runs them. Standing
"always allow" rules are ignored for them, the card turns red, and in bypass
mode those CLIs are blocked outright on Claude Code. That block is a Claude Code
flag with no equivalent elsewhere, so on any other agent a bypass pick opens
the thread in accept-edits instead and says so in the chat — the pill's bypass
row is disabled there with the reason. Hark can be careless with your tokens;
it is not allowed to be careless with your infrastructure.

## Switching agents

Every chat belongs to the agent that opened it, and the pills beside the
composer (permission mode, model tier, effort) speak to that agent in its own
vocabulary — the model tier you pick is said as `haiku` to Claude and as
`gemini-2.5-flash-lite` to Gemini. A knob an agent does not offer stays on
screen, disabled, with the reason.

Two spoken commands move between agents:

- *"abre um chat gemini no webhook"* / *"open a chat on gemini in webhook"* —
  a new chat in that project, on that agent.
- *"troca esse chat pro gemini"* / *"switch this chat to gemini"* — the thread
  you are in moves: a fresh session on the other agent opens with a local,
  zero-token brief of the conversation so far, on the same card. The ⓘ popover
  of a live thread has a button per available agent for the same move.

## The board

A kanban of tasks, in Início next to Hoje and Demandas (the Code door keeps
a shortcut that opens it as a panel beside the chat). A **task is a
deliberate record**: nothing creates one for you. It is born when you ask
for it — "cria uma task patch do windows", "registra a task chamada PR da
subnet", "create task fix the dns" — when you accept the chat's own offer
(after a few turns in a chat no card has, one line above the composer says
"este chat não tem task · registrar · agora não", once), or on the board
itself. The model and the dispatcher only move cards that exist. A task is
linked to the chat it was asked in, and one chat may carry several tasks,
as in an issue tracker; a task may also gather several chats. Moving a
card does not move a ticket somewhere else — it tracks the conversations
doing the work, so resuming means resuming, not starting over. The board
is global and a detached project window shows its slice.

### A chat's name is the chat's

Renaming is the chat's, not a card's: "renomeia esse chat para patch do
windows", "rename this chat to dns fix", `/rename patch do windows`, or
"Mudar o nome" in the row's `⋯` menu. The name is written into the
session's **own history** — for Claude Code, the same `custom-title` line
its `/rename` writes — so `claude --resume`, Hark's rows, Demandas and
every other reader of that history show one name. A name you gave in the
CLI shows in Hark the same way. A card keeps its own name on the board.
A brand-new chat opened with a sentence gets a short name from the light
model after its first reply, written the same way (as a generated name:
the CLI's own namer and yours outrank it).

## Costs

Every turn lands in a local SQLite ledger: kind, project, task, model, tokens,
cost, whether it failed. The costs tab breaks it down by day or week, by kind,
project and model, shows your cache hit ratio, and lists the most expensive and
the heaviest sessions.

Two rules keep the numbers honest, and they are printed at the bottom of the
panel:

- **USD comes only from the agent's own reporting.** There is no hardcoded price
  table anywhere in Hark. Claude Code and the Claude ACP adapter price their
  turns; Codex reports tokens only; Gemini reports nothing — and a turn nobody
  priced shows as `$ –` in the footer and in the panel, never as `$0.00`.
- **Token counts and dollars are never summed together.** They come from
  different sources — the live stream and the session logs — and the logs cover
  all of your agent usage, not just what went through Hark.

The subscription window percentages (5h / 7d) come from an **opt-in** bridge: a
one-click wrapper around the Claude Code status line that tees its JSON to a
file Hark reads. It backs up your settings first, changes nothing until you
click, and uninstalls cleanly. Over the Claude ACP adapter the meter arrives
with every turn and needs no bridge at all.

## Plugins

Settings → **Plugins** is the catalog: every agent Hark knows, whether its
binary is on this machine, what each plugin can do (resume, permissions, cost
reporting, history, slash commands…), what the agent calls each model tier,
which build you have against what the ACP registry publishes (and the update
command when yours is behind), and which one opens new chats. Claude Code and
the ACP agents — Gemini CLI, Codex, Claude Code over ACP, DeepSeek, Kiro,
Antigravity — ship today. See [PLUGINS.md](PLUGINS.md).

## The `hark` CLI

The same core, headless — useful for scripting, CI, or just a terminal habit:

```
hark listen                                   # the voice loop, in the terminal
hark hear                                     # transcribe one utterance
hark setup                                    # download the speech model
hark ask "what's pending today?"              # a cheap, snapshot-fed question, on the agent the app would use
hark dispatch [--session <id>] "<instruction>"
hark ps [--clear]                             # running workers
hark spend [--day|--week|--project] [--rebuild] [--export csv|json]
hark index                                    # (re)index session history
hark sessions                                 # what is on disk
hark rename <session-id> <name>               # name a chat, in its own history (the CLI sees it)
hark use <context> | hark contexts            # kubectl-style focus areas
hark board                                    # the board, as text
hark demands                                  # the list of the day, as the window computes it
```

## Updates

Hark keeps itself current without ever surprising you. When a new release
lands, the app downloads it in the background, verifies its signature, and
shows one quiet pill in the main window: **restart to update**. Click it and
the app closes and reopens on the new version. Ignore it and nothing happens —
next launch checks again.

The check is a single GET against this repository (the only network call Hark
makes on its own; nothing identifying is sent). Turn it off with
`auto_update = false`, and update by hand whenever you like:

```bash
curl -fsSL https://harkharness.web.app/install.sh | bash
```

The `hark` CLI in `~/.local/bin` is updated by that same script — the in-app
updater replaces only the app bundle.

## Reporting a problem

**Sidebar → "Report a problem"**, Settings, or the app menu. The dialog asks
for a title and what happened, and opens the GitHub issue form in your
browser, under your own account, already filled in: you review and edit
everything before pressing Submit. Nothing leaves your machine through Hark.

Tick **"Include diagnostics and logs"** to attach what the machine knows:
Hark's version, macOS and architecture, the agents it found and their
versions, your config with every secret masked, and the tail of Hark's own
log (`<data dir>/logs/hark.log`, rotated at 1 MB). That log records what the
app did — spawns, turn outcomes, error classes — never prompts, replies or
tool outputs, and it is scrubbed as it is written: home directories become
`~`, tokens, e-mails and credentials inside URLs are removed. The dialog
shows the exact text before anything is sent. **The issue is public**: read
the preview.

When the diagnostics do not fit the link, they go to your clipboard and the
dialog says so — paste them into the form's "Logs or screenshots" field. With
`gh` installed and logged in, **"Create directly with gh"** files the issue,
logs included, and opens it for editing. **"Save file"** writes the whole
report under `<data dir>/reports/` and reveals it in Finder, for people
without a GitHub account.

## Where things live

| Path | What |
|---|---|
| `~/.hark/config.toml` | your configuration ([reference](CONFIG.md)) |
| `~/.hark/index.db` | session index and spend ledger |
| `~/.hark/models/` | speech models |
| `~/.hark/HARK.md` | the assistant's personality and learnings, mirrored into each agent's own memory file name (`CLAUDE.md`, `GEMINI.md`, `AGENTS.md`) beside it |
| `~/.hark/artifacts/<space>/` | the documentation tree: pages (`.md`, `.html`) and the `index.json` that orders them |
| `~/.hark/state.json` | board, workers, active context, what each agent turned out to offer |
| `~/.hark/sessions/<agent>/<id>.jsonl` | Hark's own record of sessions on agents that keep none |
| `<project>/.hark/` | per-project journal, state and dispatch briefs |

Older installs kept these under `~/.config/hark` and
`~/Library/Application Support/hark`; Hark moves them to `~/.hark` on first
launch. None of it leaves your machine.
