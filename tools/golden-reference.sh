#!/usr/bin/env bash
# Capture reference screenshots of every golden scene with the per-feature
# pipeline (one `cargo psp` build and one fixed-timeout PPSSPPHeadless run per
# scene), then map each `SSB64_STAGE_INDEX` capture to the `r2-stage-*`
# golden it equals exactly.
#
#   tools/golden-reference.sh [--pack PATH]
#
# Output (outside Git): ${GOLDEN_REFERENCE_DIR:-$HOME/golden-reference}/
#   <golden>.png        one per non-stage scene
#   stage-<N>.png       one per stage index
#   SHA256SUMS
#   stage-index-map.tsv stage index -> golden name, exact matches only
#   timing.tsv          per-scene wall time (build + capture)
#
# The references are the current pipeline's real output, not tests/golden/:
# known-failing goldens differ from both. Later changes to the capture
# pipeline must reproduce these bytes exactly.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="${GOLDEN_REFERENCE_DIR:-$HOME/golden-reference}"
CAPTURE_DIR="$REF/.capture"
PACK_ARGS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --pack) PACK_ARGS=(--pack "$2"); shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

command -v magick >/dev/null || { echo "missing required tool: magick (ImageMagick)" >&2; exit 2; }
mkdir -p "$REF" "$CAPTURE_DIR"
printf 'name\tseconds\n' > "$REF/timing.tsv"

# crate:feature:output name. Stage indices are appended below.
scenes=(
  psp-asset-viewer:regression_capture:r0-dream-land-default
  psp-asset-viewer:regression_capture_scene2:r1-mvopeningroom
  psp-asset-viewer:regression_capture_scene3:r1-stage-sector
  psp-asset-viewer:regression_capture_scene4:r1-catch-swirl-flat-color
  psp-asset-viewer:regression_capture_scene5:r2-saffron-city-gate
  psp-asset-viewer:regression_capture_scene6:r2-metal-texgen
  psp-asset-viewer:regression_capture_scene7:r2-metal-texgen-rotated
  psp-asset-viewer:regression_capture_scene8:r2-metal-texgen-linear
  psp-asset-viewer:regression_capture_scene9:r2-metal-texgen-camera-rotated
  psp-asset-viewer:regression_capture_scene10:r2-peach-castle
  psp-asset-viewer:depth_mask_diagnostic:r2-depth-mask-diagnostic
  psp-asset-viewer:regression_capture_mario:r2-mario-fighter
  psp-asset-viewer:regression_capture_fox:r2-fox-fighter
  psp-asset-viewer:regression_capture_donkey_kong:r2-dk-fighter
  psp-asset-viewer:regression_capture_samus:r2-samus-fighter
  psp-asset-viewer:regression_capture_luigi:r2-luigi-fighter
  psp-asset-viewer:regression_capture_link:r2-link-fighter
  psp-asset-viewer:regression_capture_link_costume_1:r2-link-costume-1
  psp-asset-viewer:regression_capture_yoshi:r2-yoshi-fighter
  psp-asset-viewer:regression_capture_captain_falcon:r2-falcon-fighter
  psp-asset-viewer:regression_capture_kirby:r2-kirby-fighter
  psp-asset-viewer:regression_capture_pikachu:r2-pikachu-fighter
  psp-asset-viewer:regression_capture_purin:r2-purin-fighter
  psp-asset-viewer:regression_capture_ness:r2-ness-fighter
  psp-asset-viewer:regression_capture_metal_mario:r2-metal-mario-fighter
  psp-asset-viewer:regression_capture_mario_entry:r2-mario-entry-pipe
  psp-asset-viewer:regression_capture_bonus_platform:r2-bonus-platform-small
  psp-game:regression_capture_fireball:f1-training-fireball
  psp-game:regression_capture_shadows:f1-training-shadows
)
for i in 1 2 3 5 6 7 8 $(seq 10 40); do
  scenes+=("psp-asset-viewer:regression_capture_stage_index:stage-$i")
done

for scene in "${scenes[@]}"; do
  IFS=: read -r crate feature name <<<"$scene"
  stage_env=()
  case "$name" in stage-*) stage_env=(SSB64_STAGE_INDEX="${name#stage-}") ;; esac
  echo "==> $name ($crate, $feature ${stage_env[*]:-})"
  start=$(date +%s)
  env "${stage_env[@]}" PPSSPP_HEADLESS_TEST_DIR="$CAPTURE_DIR" \
    "$REPO/tools/run-ppsspp-headless.sh" --crate "$crate" --feature "$feature" \
    "${PACK_ARGS[@]}"
  cp -f "$CAPTURE_DIR/screenshot.png" "$REF/$name.png"
  printf '%s\t%s\n' "$name" "$(( $(date +%s) - start ))" >> "$REF/timing.tsv"
done

# Exact-match stage map: an index maps to a golden only when every pixel is
# equal. Anything else is reported, never guessed.
pixel_diff() {
  magick "$1" "$2" -compose difference -composite -threshold 0 \
    -format '%[fx:round(mean*w*h)]' info:
}
printf 'stage_index\tgolden\n' > "$REF/stage-index-map.tsv"
unmatched=()
for i in 1 2 3 5 6 7 8 $(seq 10 40); do
  match=""
  for golden in "$REPO"/tests/golden/r2-stage-*.png; do
    if [ "$(pixel_diff "$REF/stage-$i.png" "$golden")" = 0 ]; then
      [ -n "$match" ] && { echo "stage $i matches both $match and $(basename "$golden" .png)" >&2; exit 1; }
      match=$(basename "$golden" .png)
    fi
  done
  if [ -n "$match" ]; then
    printf '%s\t%s\n' "$i" "$match" >> "$REF/stage-index-map.tsv"
  else
    unmatched+=("$i")
  fi
done

( cd "$REF" && sha256sum ./*.png > SHA256SUMS )
echo "==> references: $REF"
echo "==> mapped stage indices: $(( $(wc -l < "$REF/stage-index-map.tsv") - 1 )) of 38"
if [ ${#unmatched[@]} -gt 0 ]; then
  echo "unmatched stage indices (no exact golden): ${unmatched[*]}" >&2
  exit 1
fi
