#!/usr/bin/env bash
# Hark installer — a thin wrapper over the one the website serves, which
# picks the right build from this repository's latest release.
set -euo pipefail
exec /bin/bash -c "$(curl -fsSL https://harkharness.web.app/install.sh)"
