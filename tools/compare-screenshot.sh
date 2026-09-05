#!/usr/bin/env bash
#
# Compare a captured screenshot against a golden reference (R0.17's visual
# regression methodology). Prints the differing-pixel count and exits
# nonzero if it exceeds the threshold, so this is safe to use as a CI-style
# gate, not just an eyeball check ("looks the same" is not evidence --
# AGENTS.md SS7).
#
#   tools/compare-screenshot.sh <golden.png> <candidate.png> [max-diff-pixels]
#
# Threshold defaults to 0: the R0.17 golden scene freezes every per-frame
# mutation (physics, skeleton/stage/material animation, and the live
# perf-counter HUD fields) behind `regression_capture`'s tick target, and two
# captures taken at different wall-clock times from that build have been
# measured byte-identical (docs/reverse-engineering.md RE-123). A real
# rendering regression should therefore show as a nonzero, not a
# barely-over-threshold, difference -- so the default stays at 0 rather than
# picking a tolerance that would hide a real one.

set -euo pipefail

if [ $# -lt 2 ]; then
  echo "usage: $0 <golden.png> <candidate.png> [max-diff-pixels]" >&2
  exit 2
fi

GOLDEN="$1"
CANDIDATE="$2"
MAX_DIFF="${3:-0}"

for f in "$GOLDEN" "$CANDIDATE"; do
  [ -f "$f" ] || { echo "not found: $f" >&2; exit 2; }
done

command -v magick >/dev/null || { echo "missing required tool: magick (ImageMagick)" >&2; exit 2; }

# ImageMagick 7's `compare -metric AE` reports accumulated channel error on
# this host, despite the metric's historical "absolute-error pixel count"
# name. RE-142 caught it reporting 9,835,820 for an image with only 836 changed
# pixels. Build a binary per-pixel difference image instead; its mean times its
# area is the actual number of pixels whose RGB value differs.
GOLDEN_SIZE=$(magick identify -format '%wx%h' "$GOLDEN")
CANDIDATE_SIZE=$(magick identify -format '%wx%h' "$CANDIDATE")
if [ "$GOLDEN_SIZE" != "$CANDIDATE_SIZE" ]; then
  echo "image sizes differ: golden=$GOLDEN_SIZE candidate=$CANDIDATE_SIZE" >&2
  exit 2
fi
AE=$(magick "$GOLDEN" "$CANDIDATE" -compose difference -composite \
  -threshold 0 -format '%[fx:mean*w*h]' info:)

# Round defensively for ImageMagick builds which print a floating-point value.
AE_INT=$(awk -v n="$AE" 'BEGIN { printf "%.0f", n }')
echo "differing pixels: $AE_INT (threshold: $MAX_DIFF)"

if [ "$AE_INT" -gt "$MAX_DIFF" ]; then
  echo "FAIL: $CANDIDATE differs from $GOLDEN by more than $MAX_DIFF pixels" >&2
  exit 1
fi

echo "PASS"
