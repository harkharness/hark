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
Settings, or in `~/.config/hark/config.toml`:

```toml
hotkey = "cmd+shift+h"
```

## "agent plugin not found"

Hark needs the agent CLI installed and logged in — it does not ship one and it
cannot authenticate for you. Check that it works on its own first:

```bash
claude --version
```

If the binary lives somewhere unusual, point Hark straight at it:

```toml
claude_bin = "/opt/homebrew/bin/claude"
```

Then Settings → Plugins → recheck.

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

USD only ever comes from the agent CLI's own reporting; Hark has no price table.
If a turn did not report a cost, its row shows tokens and no dollars rather than
an estimate dressed up as a fact. The subscription window percentages need the
opt-in status-line bridge (Settings → Costs).

## Uninstalling

```bash
rm -rf /Applications/hark.app
rm -f  ~/.local/bin/hark
rm -rf ~/.config/hark
rm -rf ~/Library/"Application Support"/hark
```

The last one deletes your index, ledger, board and the assistant's memory file.
If you installed the status-line bridge, uninstall it from Settings first so
your agent CLI settings are restored from the backup Hark made.

## Something else

Open an [issue](https://github.com/harkharness/hark/issues) — include your macOS
version, your architecture, and the release you installed.
