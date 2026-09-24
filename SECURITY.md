# Security

## Reporting a vulnerability

Report privately through GitHub: **Security → Report a vulnerability** on this
repository. Please do not open a public issue. You will get a first answer
within a few days, and credit in the release notes if you want it.

Only the latest release is supported. Hark updates itself with signed updates,
so fixes ship as new releases.

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

- Every privileged tool call is a permission card. Commands that touch
  production are never auto-approved.
- Updates are signed; installed apps refuse an update the release key did not
  sign. The key lives in a private pipeline and never meets a job that runs
  project code or dependencies: the build runs without it, and a separate job
  signs.
- Hark makes one network call of its own (the update check) and has no
  telemetry.

