#!/usr/bin/env bash
# hark installer — downloads the latest GitHub Release and installs:
#   - the `hark` CLI into ~/.local/bin
#   - the hark.app bundle into /Applications (falls back to ~/Applications)
#
# The repo is private for now, so this needs an authenticated `gh` CLI:
#   brew install gh && gh auth login
#
# The app is not code-signed yet; the quarantine bit is cleared after
# install (xattr -cr), which is fine for personal machines.

set -euo pipefail

REPO="jhonmike/hark-harness"

say() { printf '\033[1;36mhark\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mhark\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || die "only macOS for now (linux is on the backlog)"

case "$(uname -m)" in
  arm64)  TARGET="aarch64-apple-darwin" ;;
  x86_64) TARGET="x86_64-apple-darwin" ;;
  *) die "unsupported architecture: $(uname -m)" ;;
esac

command -v gh >/dev/null 2>&1 || die "gh CLI required while the repo is private: brew install gh && gh auth login"
gh auth status >/dev/null 2>&1 || die "gh is not authenticated: gh auth login"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

say "downloading the latest release for ${TARGET}…"
gh release download --repo "$REPO" --dir "$TMP" \
  --pattern "hark-cli-*-${TARGET}.tar.gz" \
  --pattern "hark-app-*-${TARGET}.app.tar.gz"

# CLI
mkdir -p "$HOME/.local/bin"
tar -xzf "$TMP"/hark-cli-*.tar.gz -C "$TMP"
install -m 0755 "$TMP/hark" "$HOME/.local/bin/hark"
say "CLI installed at ~/.local/bin/hark"
case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *) say 'note: add ~/.local/bin to your PATH (export PATH="$HOME/.local/bin:$PATH")' ;;
esac

# App
APP_DIR="/Applications"
[ -w "$APP_DIR" ] || APP_DIR="$HOME/Applications"
mkdir -p "$APP_DIR"
rm -rf "$APP_DIR/hark.app"
tar -xzf "$TMP"/hark-app-*.app.tar.gz -C "$APP_DIR"
xattr -cr "$APP_DIR/hark.app" 2>/dev/null || true
say "app installed at $APP_DIR/hark.app (quarantine cleared)"

say "done. next steps:"
say "  1. hark setup          # downloads the whisper speech model (~466MB)"
say "  2. open $APP_DIR/hark.app"
