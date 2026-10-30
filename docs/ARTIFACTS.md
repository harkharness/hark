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
  .claude/skills/hark-artifact/SKILL.md   the skill Hark installs (see below)
```

A **space** is a folder. The app lists the folders on disk plus one per
registered project plus `pessoal`, so a project has a space before its
first page. Hidden folders and symlinks are never spaces.

A **page** is a `.md` or `.html` file inside a space, at any depth. Its
`id` is the slug of its path; its title is the first `# heading` of a
markdown page, or the file name. Folders are not nodes: a page's parent is
the page whose slug names the folder it sits in, and a folder with no such
page hangs its children at the root.

`index.json` is Hark's. It keeps, per node: `id`, `title`, `path`,
`parent`, `kind` (`doc`, `slides`, `design`), `origin` (the session and
task that wrote it), `links`, `updated_at`, and the children `order`. On
every read the tree **reconciles** with the disk: a file nobody indexed is
adopted at the root, a node whose file is gone is marked `orphan` (shown
struck through, never deleted by Hark). Editing the index by hand is not
needed and not advised; the agents are told never to touch it.

## The screen

**Artefatos** in the sidebar of the main window. The tree column lists
every space and page in index order; drag a page onto another to move it
under it, `+` on a row makes a child, `⋯` renames or removes. Removing
deletes the file and its children: it is your file, so it asks nothing
twice.

The home of a space shows what moved last. A page opens with its
breadcrumb, kind, last touch and the chat it was born in (the chip opens
that chat). Markdown renders and edits in place; HTML renders in an
`<iframe sandbox>` with scripts off, and "fonte" edits the source. Turn
scripts on for a deck that needs them: even then the frame has no origin
and no access to Hark. **Ligações**, the rail panel beside a page, lists
the origin chat, child pages and citations.

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

- **Not from the artifacts view.** That view is documentation: no
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
