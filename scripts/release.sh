#!/usr/bin/env bash
# Cut a release: bump, commit, tag and push in one step.
#
# The single step is the point. v0.2.2 and v0.2.3 both shipped WITHOUT
# fixes that were already written, because the version bump and the tag
# push were separate actions and work landed in between. There is no
# window here.
#
#   scripts/release.sh 0.2.4 ["what this release is, in one line"]
#
# The second argument becomes the tag's annotation ("v0.2.4 — <line>"),
# so `git tag -n` reads as a changelog. Without it the tag is plain.

set -euo pipefail

die() { printf 'release: %s\n' "$*" >&2; exit 1; }

VERSION="${1:-}"
HEADLINE="${2:-}"
[ -n "$VERSION" ] || die "usage: scripts/release.sh <version> [headline]   e.g. 0.2.4"
case "$VERSION" in
  v*) die "pass the number without the leading v" ;;
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) die "not a version: $VERSION" ;;
esac

cd "$(git rev-parse --show-toplevel)"

git diff --quiet && git diff --cached --quiet || die "working tree is dirty"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[ "$BRANCH" = "main" ] || die "release from main, not $BRANCH"

git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] \
  || die "HEAD and origin/main disagree — push or pull first"

! git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null \
  || die "v$VERSION already exists"

# Both places the release workflow's guard compares against the tag, plus
# package.json — nothing reads it for the build, but a third number left
# behind is a third number to explain (v0.3.1 was bumped by hand for it).
perl -0pi -e "s/version = \"\\d+\\.\\d+\\.\\d+\"/version = \"$VERSION\"/" Cargo.toml
perl -0pi -e "s/\"version\": \"\\d+\\.\\d+\\.\\d+\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json
perl -0pi -e "s/\"version\": \"\\d+\\.\\d+\\.\\d+\"/\"version\": \"$VERSION\"/" package.json

# The lockfile pins the workspace crates' own versions; without this the
# release commit leaves a dirty Cargo.lock behind (v0.2.4 did).
cargo update -q --workspace

CONF="$(sed -n 's/.*"version": "\(.*\)",.*/\1/p' src-tauri/tauri.conf.json | head -1)"
CRATE="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
PKG="$(sed -n 's/.*"version": "\(.*\)",.*/\1/p' package.json | head -1)"
[ "$CONF" = "$VERSION" ] && [ "$CRATE" = "$VERSION" ] && [ "$PKG" = "$VERSION" ] \
  || die "bump failed (tauri.conf=$CONF Cargo=$CRATE package.json=$PKG) — nothing was tagged"

git commit -aqm "chore: v$VERSION"
if [ -n "$HEADLINE" ]; then
  git tag -a "v$VERSION" -m "v$VERSION — $HEADLINE"
else
  git tag "v$VERSION"
fi
git push -q origin main "v$VERSION"

printf 'release: v%s pushed at %s\n' "$VERSION" "$(git rev-parse --short HEAD)"
printf 'release: watch it with  gh run list --workflow=release.yml --limit 1\n'
