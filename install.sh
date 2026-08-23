#!/usr/bin/env bash
# Hark installer — thin wrapper over the public one. Binaries ship from the
# public releases-only repo (harkharness/hark); this private repo never
# exposes source through them.
set -euo pipefail
exec /bin/bash -c "$(curl -fsSL https://harkharness.web.app/install.sh)"
