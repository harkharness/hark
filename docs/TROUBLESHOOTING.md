# Troubleshooting

[← back to the README](../README.md)

## "hark.app is damaged and can't be opened"

macOS quarantines anything downloaded from the internet, and Hark is not
code-signed yet (an Apple Developer account is on the list). The installer
clears the flag for you; if you downloaded the `.dmg` by hand, do it yourself:

```bash
xattr -cr /Applications/hark.app
```

Then open it normally. If Gatekeeper still refuses, right-click the app → Open
once, and macOS remembers the exception.

## The microphone does nothing

The first-run wizard records two seconds on purpose — that is what makes macOS
ask for permission with context. If you skipped it or denied the prompt, grant
it in **System Settings → Privacy & Security → Microphone**, then restart Hark.

Nothing gets recorded unless you press the hotkey, and audio never leaves the
machine: transcription runs locally.

## The hotkey does not open anything

Another app owns the same shortcut — `cmd+shift+space` is popular. Change it in
Settings, or in `~/.hark/config.toml`:

```toml
hotkey = "cmd+shift+h"
```

## "agent plugin not found"

Hark needs at least one agent CLI installed and logged in — it does not ship
one and it cannot authenticate for you. Check that it works on its own first:

```bash
claude --version
gemini --version
codex-acp --version
```

If the `claude` binary lives somewhere unusual, point Hark straight at it:

```toml
claude_bin = "/opt/homebrew/bin/claude"
```

For any other agent, the `cmd` of its `[agents.<id>]` entry is what Hark looks
for. Then Settings → Plugins → recheck. The catalog card of an agent that is
not installed shows the install command.

## The catalog says my agent is behind

Detected is not current. Settings → Plugins compares the version your binary
reports with the one the [ACP registry](https://github.com/agentclientprotocol/registry)
publishes and shows the update line, usually `npm install -g <package>@<version>`.
One symptom worth knowing: an old Gemini CLI answers every new session with
*"This client is no longer supported for Gemini Code Assist for individuals"* —
that is not a login problem, it is the update.

## The agent says it is not logged in

The card carries that agent's own login command (`claude /login`, `gemini`,
`codex login`…). Log in once in your terminal and send the message again; Hark
never types credentials for you.

## The speech model failed to download

It is ~466MB over `curl` and it is checksummed on arrival, so a truncated
download is rejected rather than silently used. Retry from Settings, or from the
terminal:

```bash
hark setup
```

Behind a proxy, whatever `curl` needs in your environment is what Hark needs.

## No session history in the sidebar

Hark indexes the agent CLI's own logs. If the directory is somewhere else, say
so:

```toml
projects_dir = "~/some/other/place"
```

Then rebuild the index:

```bash
hark index
```

An empty history is not an error — a fresh machine simply has nothing to show
yet.

## The costs panel shows tokens but no dollars

USD only ever comes from the agent's own reporting; Hark has no price table.
Claude Code and the Claude ACP adapter price their turns; Codex reports tokens
only; Gemini reports nothing. A turn nobody priced shows `$ –` with the reason
on hover rather than an estimate dressed up as a fact — the subscription paid,
the wire just did not carry the number. The subscription window percentages
come from the opt-in status-line bridge (Settings → Costs), or from the Claude
ACP adapter, which reports them on every turn.

## Uninstalling

```bash
rm -rf /Applications/hark.app
rm -f  ~/.local/bin/hark
rm -rf ~/.hark
```

The last one deletes your configuration, index, ledger, board, the assistant's
memory file and Hark's own session records. Installs older than September 2026
may also have `~/.config/hark` and `~/Library/Application Support/hark` left
over from before everything moved to `~/.hark`.
If you installed the status-line bridge, uninstall it from Settings first so
your agent CLI settings are restored from the backup Hark made.

## On Linux

The `.deb` belongs to `apt`: run the install command again to update, and
`sudo apt remove hark` to uninstall. The AppImage updates itself like the
Mac app does; it needs FUSE 2 to start on most distros (`libfuse2` on
Ubuntu and Debian, `fuse2` elsewhere), or run it with
`--appimage-extract-and-run`. The voice speaks through Speech Dispatcher: if
Hark is silent, check that the service answers on its own, and pick a voice
from its list in Settings:

```bash
spd-say -l pt-BR "olá"
spd-say -L
```

The microphone comes through PipeWire or PulseAudio; a desktop session has
one of them. Under a virtual machine, a blank window usually means the
WebKitGTK compositor and the virtual GPU disagree: start Hark with
`WEBKIT_DISABLE_DMABUF_RENDERER=1`.

## Something else

Open an [issue](https://github.com/harkharness/hark/issues) — include your macOS
version, your architecture, and the release you installed.
