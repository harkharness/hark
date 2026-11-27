# Security

## Reporting a vulnerability

Report privately through GitHub: **Security → Report a vulnerability** on this
repository. Please do not open a public issue. You will get a first answer
within a few days, and credit in the release notes if you want it.

Only the latest release is supported. Hark updates itself with signed updates,
so fixes ship as new releases.

## What a bug report contains

"Report a problem" never sends anything by itself: it opens a GitHub issue
form in your browser, under your account, for you to review. Diagnostics
ride along only when you tick the box, and they are scrubbed before you
see them: home directories, tokens, e-mails and URL credentials are
removed, agents' `env` values are masked, and the app log they come from
holds event kinds and error classes, not prompts, replies or tool outputs.
The issue is public; the dialog shows the exact text first. If you find
something in a report that should not have been there, tell us through the
vulnerability channel above.

## What Hark defends against

Hark drives agent CLIs on your machine with your permissions, so its threat
model starts from two facts:

- **Agent output is untrusted.** A repository can prompt-inject an agent into
  printing anything. Links in transcripts open only web URLs and documents,
  never executables, bundles or other schemes. Code blocks cannot close the
  fence they are shown in. "Insert in terminal" never runs what it inserts.
  Nothing an agent says is spoken as a command-line argument.
- **Repositories are untrusted until you trust them.** `claude -p` skips
  Claude Code's folder-trust dialog, so for Claude Code sessions Hark does not
  load a folder's own hooks, allow rules, env or MCP servers until you have
  trusted that folder in `claude` — and says so in the chat. Hark's
  per-project `.hark/` files are never written or read through a symlink.

Also by design:

- Pages that come with a repository are repository content. A
  project can keep its pages in `docs/hark/`, and a clone brings whatever
  is there. They are read like any page: no path that climbs, nothing
  through a symlink (`docs` or `docs/hark` being a link is refused, not
  followed), html in a frame that cannot reach the app. A dashboard's
  command runs only when you press refresh, and one that came with a
  repository is somebody else's command: read it first. Hark writes that
  folder into a repository only when you ask it to, per project.
- Every privileged tool call is a permission card. Commands that touch
  production are never auto-approved.
- `!` in the composer runs what you typed, in the chat's folder, with
  your permissions and no permission card: it is your command, as in a
  terminal. Only the keyboard takes that path. Text placed in the field
  from a page or a link is made prose first, nothing an agent prints or
  the microphone hears is run, and a command's output is shown as plain
  text.
- Updates are signed; installed apps refuse an update the release key did not
  sign. The key lives in a private pipeline and never meets a job that runs
  project code or dependencies: the build runs without it, and a separate job
  signs.
- Hark makes one network call of its own (the update check) and has no
  telemetry.

