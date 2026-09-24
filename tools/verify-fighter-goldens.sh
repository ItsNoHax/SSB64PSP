#!/usr/bin/env bash
# Verify every playable fighter's deterministic neutral-model golden, plus the
# Link costume-1 scene. A wrapper around tools/golden.sh; extra arguments
# (-j N, --twice, --no-build) pass through.
#
# Each scene selects the original high-detail FTCommonPart graph and
# configures the same stage-directional fighter light that the game enables
# immediately before ftDisplayMainProcDisplay. Captures freeze at tick 240, so
# this expects exact pixels, not a tolerance.

set -euo pipefail

exec "$(dirname "${BASH_SOURCE[0]}")/golden.sh" verify --filter 'fighter|link-costume' "$@"
