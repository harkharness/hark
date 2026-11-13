#!/usr/bin/env bash
# Set up an Ubuntu 24.04 machine to build Hark from source, then run it.
#
#   scripts/dev-linux.sh            # setup + sync + run
#   scripts/dev-linux.sh setup      # packages, Rust, Node, the Claude CLI
#   scripts/dev-linux.sh sync       # copy the source tree to the work dir
#   scripts/dev-linux.sh run        # npm ci when the lockfile changed, then tauri dev
#
# The source tree may live on a shared folder (a VM's virtiofs mount, say)
# that also holds the host's node_modules and target: native modules and
# object files from another OS. So the tree is copied to a work dir on the
# local disk and built there; run `sync` again after editing on the host.
#
#   HARK_SRC       the source tree (default: the repository this script is in)
#   HARK_WORK      the work dir on local disk (default: ~/hark-dev)
#   HARK_PROXY_CA  a PEM certificate to trust system-wide, for a network
#                  behind an inspecting proxy (default: none)
#   RUST_VERSION   the toolchain to install (default: the one CI pins)
set -euo pipefail

HARK_SRC="${HARK_SRC:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
HARK_WORK="${HARK_WORK:-$HOME/hark-dev}"
RUST_VERSION="${RUST_VERSION:-1.98.0}"
NODE_MAJOR="${NODE_MAJOR:-22}"

say() { printf '\033[1;36m==> %s\033[0m\n' "$*"; }
die() { printf '\033[1;31m!!  %s\033[0m\n' "$*" >&2; exit 1; }

[ -f "$HARK_SRC/package.json" ] && [ -f "$HARK_SRC/Cargo.toml" ] \
  || die "no Hark source tree at $HARK_SRC (set HARK_SRC)"

setup() {
  command -v apt-get >/dev/null || die "this script knows apt (Ubuntu/Debian) only"
  export DEBIAN_FRONTEND=noninteractive

  if [ -n "${HARK_PROXY_CA:-}" ]; then
    say "trusting $HARK_PROXY_CA system-wide"
    sudo install -m 0644 "$HARK_PROXY_CA" /usr/local/share/ca-certificates/hark-proxy-ca.crt
    sudo update-ca-certificates >/dev/null
    sudo tee /etc/profile.d/hark-ca.sh >/dev/null <<'EOF'
export SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
export REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt
export NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-certificates.crt
EOF
    # shellcheck disable=SC1091
    . /etc/profile.d/hark-ca.sh
  fi

  say "packages: the Tauri toolchain, ALSA for the microphone, speech-dispatcher for the voice"
  sudo apt-get update -q
  sudo apt-get install -y -q \
    build-essential curl file git rsync pkg-config cmake clang patchelf \
    libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
    libasound2-dev speech-dispatcher espeak-ng

  if ! command -v node >/dev/null || [ "$(node -p 'process.versions.node.split(".")[0]')" != "$NODE_MAJOR" ]; then
    say "Node $NODE_MAJOR"
    curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | sudo -E bash -
    sudo apt-get install -y -q nodejs
  fi

  if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
    say "Rust $RUST_VERSION"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain "$RUST_VERSION" --profile minimal -c clippy
  fi
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
  rustup toolchain install "$RUST_VERSION" --profile minimal -c clippy >/dev/null
  rustup default "$RUST_VERSION" >/dev/null

  if ! command -v claude >/dev/null; then
    say "the Claude Code CLI (log in once with: claude)"
    # -E: npm must keep NODE_EXTRA_CA_CERTS behind an inspecting proxy.
    sudo -E npm install -g @anthropic-ai/claude-code
  fi

  # A tree on a virtiofs share mounts again at boot.
  local dev fstype
  dev=$(findmnt -n -o SOURCE --target "$HARK_SRC" || true)
  fstype=$(findmnt -n -o FSTYPE --target "$HARK_SRC" || true)
  if [ "$fstype" = "virtiofs" ] && ! grep -q "^$dev " /etc/fstab; then
    local mnt; mnt=$(findmnt -n -o TARGET --target "$HARK_SRC")
    say "remembering the share $dev at $mnt in /etc/fstab"
    echo "$dev $mnt virtiofs rw,nofail 0 0" | sudo tee -a /etc/fstab >/dev/null
  fi

  say "setup done: $(node --version), cargo $(cargo --version | cut -d' ' -f2), claude $(claude --version 2>/dev/null | head -1)"
}

sync() {
  say "sync $HARK_SRC -> $HARK_WORK/src"
  mkdir -p "$HARK_WORK/src"
  rsync -a --delete \
    --exclude node_modules --exclude target --exclude dist --exclude .git \
    --exclude 'src-tauri/gen' --exclude '.DS_Store' \
    "$HARK_SRC/" "$HARK_WORK/src/"
}

run() {
  # shellcheck disable=SC1091
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  cd "$HARK_WORK/src" || die "no work tree: run sync first"

  local stamp="$HARK_WORK/.lock-sha"
  local now; now=$(sha256sum package-lock.json | cut -d' ' -f1)
  if [ ! -d node_modules ] || [ "$(cat "$stamp" 2>/dev/null)" != "$now" ]; then
    say "npm ci"
    npm ci
    echo "$now" > "$stamp"
  fi

  # Object files stay on the local disk, outside the synced tree.
  export CARGO_TARGET_DIR="$HARK_WORK/target"
  # WebKitGTK under a virtual GPU paints a blank window without these.
  export WEBKIT_DISABLE_DMABUF_RENDERER=1
  export WEBKIT_DISABLE_COMPOSITING_MODE=1

  say "tauri dev (first build takes a while: whisper.cpp compiles here)"
  exec npm run tauri dev
}

case "${1:-all}" in
  setup) setup ;;
  sync) sync ;;
  run) run ;;
  all) setup; sync; run ;;
  *) die "usage: $0 [setup|sync|run|all]" ;;
esac
