#!/usr/bin/env bash
# Verify every playable fighter's deterministic neutral-model golden.
#
# Each feature selects the original high-detail FTCommonPart graph and configures
# the same stage-directional fighter light that the game enables immediately
# before ftDisplayMainProcDisplay.  The capture runner freezes at tick 240;
# this script therefore expects exact pixels, not a tolerance.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CANDIDATE_DIR="${PPSSPP_HEADLESS_TEST_DIR:-$HOME/ppsspp-headless-test}"

scenes=(
  'Mario:regression_capture_mario:r2-mario-fighter.png'
  'Fox:regression_capture_fox:r2-fox-fighter.png'
  'Donkey Kong:regression_capture_donkey_kong:r2-dk-fighter.png'
  'Samus:regression_capture_samus:r2-samus-fighter.png'
  'Luigi:regression_capture_luigi:r2-luigi-fighter.png'
  'Link:regression_capture_link:r2-link-fighter.png'
  'Yoshi:regression_capture_yoshi:r2-yoshi-fighter.png'
  'Captain Falcon:regression_capture_captain_falcon:r2-falcon-fighter.png'
  'Kirby:regression_capture_kirby:r2-kirby-fighter.png'
  'Pikachu:regression_capture_pikachu:r2-pikachu-fighter.png'
  'Jigglypuff:regression_capture_purin:r2-purin-fighter.png'
  'Ness:regression_capture_ness:r2-ness-fighter.png'
)

for scene in "${scenes[@]}"; do
  IFS=: read -r name feature golden <<<"$scene"
  echo "==> $name ($feature)"
  "$REPO/tools/run-ppsspp-headless.sh" --feature "$feature"
  "$REPO/tools/compare-screenshot.sh" "$REPO/tests/golden/$golden" \
    "$CANDIDATE_DIR/screenshot.png"
done

echo "==> all ${#scenes[@]} fighter goldens passed"
