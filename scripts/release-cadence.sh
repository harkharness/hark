#!/usr/bin/env bash
# The release cadence, enforced in the two places it can be: here (called
# by release.sh, before anything is pushed) and in the release workflow,
# before the two macOS builds start.
#
# The rule, decided 22/09/2026 after the month's Actions quota ran out on
# the 15th: at most ONE feature release per week. A bug-fix release — a
# patch bump, same major.minor — is exempt, because "wait six days to
# ship the fix" is not a rule anyone would keep.
#
#   scripts/release-cadence.sh <version> [annotation]
#
# An annotation containing [force] passes anyway: this is a guard rail,
# not a lock. Without the second argument the tag's own message is read,
# which is how CI calls it.

set -euo pipefail

NEW="${1:-}"
[ -n "$NEW" ] || { echo "usage: scripts/release-cadence.sh <version> [annotation]" >&2; exit 1; }
NEW="${NEW#v}"

ANNOTATION="${2-}"
if [ -z "$ANNOTATION" ] && git rev-parse -q --verify "refs/tags/v$NEW" >/dev/null 2>&1; then
  ANNOTATION="$(git tag -l --format='%(contents)' "v$NEW")"
fi
case "$ANNOTATION" in
  *"[force]"*) echo "cadence: [force] in the tag message — check skipped"; exit 0 ;;
esac

# The newest tag that is not the one being cut.
PREV="$(git for-each-ref --sort=-creatordate --format='%(refname:short)' 'refs/tags/v*' \
  | grep -v "^v${NEW}$" | head -1 || true)"
if [ -z "$PREV" ]; then
  echo "cadence: no previous tag — first release, nothing to compare"
  exit 0
fi

# A patch bump keeps major.minor: v0.6.0 -> v0.6.1 is a fix, v0.7.0 is not.
PREV_NUM="${PREV#v}"
PREV_SERIES="${PREV_NUM%.*}"
PREV_PATCH="${PREV_NUM##*.}"
NEW_SERIES="${NEW%.*}"
if [ "$PREV_SERIES" = "$NEW_SERIES" ]; then
  echo "cadence: v$NEW is a fix over $PREV — exempt"
  exit 0
fi

PREV_TS="$(git log -1 --format=%ct "$PREV")"
DAYS=$(( ( $(date +%s) - PREV_TS ) / 86400 ))
if [ "$DAYS" -lt 7 ]; then
  WAIT=$(( 7 - DAYS ))
  NEXT_FIX="v${PREV_SERIES}.$(( PREV_PATCH + 1 ))"
  {
    echo "cadence: $PREV is $DAYS day(s) old and v$NEW is a feature release."
    echo "  One feature release a week. The quota is 2000 Actions minutes a month,"
    echo "  a macOS minute counts ten, and a release builds two targets — it is the"
    echo "  most expensive thing this repo does. Three ways out:"
    echo "    - wait $WAIT day(s)"
    echo "    - cut $NEXT_FIX instead, if this is a fix"
    echo "    - put [force] in the tag message, if it really cannot wait"
  } >&2
  exit 1
fi

echo "cadence: $PREV is $DAYS day(s) old — v$NEW is clear"
