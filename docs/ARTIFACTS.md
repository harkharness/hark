# Artifacts: the documentation of this machine

Artifacts are pages you and the agents write, kept as a tree the way a
wiki keeps them. Everything is plain files under `~/.hark/artifacts`; the
app only reads them back, and nothing costs tokens to browse.

## Where things live

```
~/.hark/artifacts/
  <space>/                 one folder per space: a project's name, or "pessoal"
    index.json             the tree: nodes, order, origin, links (Hark's file)
    visao-geral.md         a page (markdown)
    decisoes/
      adr-004.md           a page under "Decisões"
    pranchas.html          a page (html: slides, designs — one file)
    custos.sheet.json      a spreadsheet the agent writes and Hark computes
    frentes.board.json     a whiteboard: notes, groups, links, a timeline
    entregas.dash.json     a dashboard over data on this machine
    entregas.dash.cache.json   its numbers, cached beside it (Hark's file)
    custos.comments.json   the comments of a page, beside it
    .versions/             the versions a page keeps (Hark's folder)
  .claude/skills/hark-artifact/SKILL.md   the skill Hark installs (see below)
```

A **space** is a folder. The app lists the folders on disk plus one per
registered project plus `pessoal`, so a project has a space before its
first page. Hidden folders and symlinks are never spaces.

A **page** is a `.md`, `.html`, `.sheet.json`, `.board.json` or
`.dash.json` file inside a space, at any depth. Its `id` is the slug of its
path; its title is the first `# heading` of a markdown page, the `title`
of a json page, or the file name. Its **kind** (`doc`, `slides`, `design`,
`sheet`, `board`, `data`) decides the icon and how it opens: markdown is a
document; an html page says what it is with
`<meta name="hark-kind" content="slides">`, and without the tag a run of
`class="slide"` sections reads as a deck, anything else as a document; a
json page is what its extension says. The `⋯` on a row changes it by hand. Folders are not nodes: a page's parent is
the page whose slug names the folder it sits in, and a folder with no such
page hangs its children at the root.

`index.json` is Hark's. It keeps, per node: `id`, `title`, `path`,
`parent`, `kind`, `origin` (the session and
task that wrote it), `links`, `updated_at`, and the children `order`. On
every read the tree **reconciles** with the disk: a file nobody indexed is
adopted at the root, a node whose file is gone leaves the tree (its
children, whose files are still there, move up to where their folder puts
them; Hark never deletes a file by itself). One file gone and one new file
beside it, same folder, in the same pass is read as a `mv` — or as the page
rewritten in the other format (`home.md` → `home.html`): the page keeps its
id, origin and links, and takes the new path, heading and kind. Editing the index by hand is not
needed and not advised; the agents are told never to touch it.

## The screen

**Artefatos** in the sidebar of the main window. The tree column lists
every space and page in index order; drag a page onto another to move it
under it, `+` on a row makes a child, `⋯` renames or removes. Removing
deletes the file and its children: it is your file, so it asks nothing
twice.

The home of a space is a gallery: a tab per kind, tiles to create a page
of each, and a card per page with a thumbnail drawn from the file itself
(a document's first lines, a sheet's first cells, a board at scale, a
dashboard's panels, a deck's first slide). A page opens with its
breadcrumb, kind, last touch and the chat it was born in (the chip opens
that chat). Markdown renders and edits in place; HTML renders in an
`<iframe sandbox>`: a document opens with scripts off, a deck or a design
with its own scripts on (a pager needs them); either way the frame has
no origin and no access to Hark. The viewer is Hark's, by kind: a deck is
a run of `<section>`s on a 1280×720 stage scaled to the room, with Hark's
pager, filmstrip, arrow keys and present mode — always Hark's: the page's
own scripts never run in the frame and whatever is not a slide is hidden,
and a deck that brought a pager of its own offers to have it stripped from
the file; a design is a canvas at its own
size (`<meta name="hark-canvas" content="WxH">`) on a board Hark zooms and
pans; a document is a reading column. "Fonte" edits the source. **Ligações**, the rail panel beside a page, lists
the origin chat, child pages and citations.

**Beside a chat.** A page the agent wrote, clicked in the session bar's
outputs or through its `hark://` link in a reply, opens in an *artifact*
panel of the chat's rail, stacked with the terminal and the files: the
page alone, with its source, comment and "take to…" controls. The panel's
expand button opens the full view above with "back to chat" at the top of
the tree. The chat is never left for a page.

## The json kinds

Three kinds are data the agent writes as JSON and Hark computes and draws,
so the agent never spends tokens laying anything out:

- **Sheet** (`.sheet.json`): tabs of cells with values and formulas (`SUM`,
  `AVERAGE`, `MIN`, `MAX`, `COUNTIF`, `SUMIF`, `IF`, `ROUND`, `TODAY`,
  references and ranges), formats and widths per column. Hark computes the
  formulas; `#REF!` and `#NOME?` are shown, never hidden.
- **Board** (`.board.json`): notes with colour and text, groups, links
  (arrows) between notes, and a timeline of milestones. The agent writes
  no coordinates: Hark lays the board out.
- **Data** (`.dash.json`): panels over sources that live on this machine
  (a csv, a sqlite file, a command), with queries, a cadence and the
  numbers cached beside the page.

The skill tells the agent the exact shape of each. `docs/USAGE.md`
(*Artifacts*) says how each kind is edited by hand.

## Editing and comments

Every page is the person's to edit, and every edit is a version the page
keeps (the `.versions/` folder; "n versions" on the page lists them). What
the agent changed since the last version the person accepted is marked,
with "undo all" and "accept". A sheet is edited cell by cell, with rows and
columns moved, inserted and removed (formulas follow); a board's notes,
links, groups and milestones by hand; a dashboard's panels, queries and
sources in forms; the words of an html page in place, by double-click in
the frame; markdown with its source beside the preview.

Comments live beside the page, in `<page>.comments.json`, each anchored to
the whole page, a cell, a note, a panel, a slide, a quoted run of words or
a region drawn over an html page with the markup bar (pen, rectangle,
text). A comment that says `@hark` becomes a request: "send to hark" takes
it to the chat the page was born in (or the chat the person picks), and
the comment reads "answered by hark" when that chat writes the page again.

## How the agents use pages

Three things make an agent write a page in the right place, with no
per-turn cost:

1. **`HARK_ARTIFACTS_DIR`** is set on every worker: the root above.
2. **`--add-dir ~/.hark/artifacts`** rides on every Claude worker (the app
   and the `hark` CLI alike). The folder is granted like the working
   directory — pages are written without a permission prompt — and
   Claude Code loads the skill under it by itself. Nothing else of
   `~/.hark` is granted: the config, the state and the index stay out of
   reach. Agents that have no such notion (ACP) get only the soul section.
3. **The `hark-artifact` skill** at `.claude/skills/hark-artifact/SKILL.md`
   under the root, written by Hark at boot and rewritten when its text
   changes (it is Hark's file). It says which space to pick, how to name
   the file, how to nest it, and what never to do. The soul file
   (`~/.hark/HARK.md`) gets a short section pointing at it, appended once
   and never rewritten.

A `Write` into the root is caught by the worker's event reader: the page
learns its `origin` (session, task) and the window gets `artifact_written`,
so the tree updates without a reload. First origin sticks; later edits do
not move a page to another chat.

## Pages in the prompt

- **"Levar para…" on a page** is a menu, never a silent default: Hark
  (Início), the chat the page was born in, a few recent chats, or a new
  chat in its project. The page's link goes in that chat's field; a
  message that carries `hark://artifacts/<space>/<id>` takes the page
  along. Dragging a page from the tree onto a chat in the sidebar does
  the same.
- **Not from the artifacts view itself.** That view is documentation: no
  composer, no scope. (A "page as scope" composer existed for a day and
  confused more than it helped, 30/10.) `domain::scope::page_brief` is
  the budgeted body a citation carries.
- **A page cited by title.** "…como ficou no ADR 004" in a message to a
  task attaches the cited pages (up to four thousand characters each) in
  an attributed block, the way a cited chat's lines are attached. The
  thread says which pages went along.
- **A task's pages in its briefs.** When a task is resumed from a local
  brief or handed to another agent, the brief ends with the pages the task
  wrote or is tied to: title and place only; the body never rides unless
  the page is the scope.

## Sharing

The share button on a page (and on a chat) is useful today and honest
about tomorrow. Every row delivers something on this machine: a local link
(`hark://artifacts/<space>/<id>`, `hark://chat/<session>`) that reopens
this Hark in place, the markdown or source to paste, one html file in
`~/.hark/exports` shown in the Finder, the file itself in the Finder; a
chat also offers its `claude --resume` command, its transcript as markdown
with tools folded, and "save brief as a page" — the task's local brief
becomes a page in its project's space, tied to the chat.

The footer says the limit of today: pages are files on this machine, and
shared spaces with teams and permissions are a later phase, with a link to
the GitHub discussions to follow and vote. Copying a page's folder to
another machine's `~/.hark/artifacts` already works (the index reconciles).

The `hark://` scheme is registered by the installed bundle; a `tauri dev`
build does not register it, so links open the installed app.
