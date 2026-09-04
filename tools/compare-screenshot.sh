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

# `compare -metric AE` writes "<count> (<normalized>)" to stderr and the diff
# image to stdout's target; discard the image, keep the first field. Exit
# status is 1 when the images differ at all under AE, which is expected and
# not itself a script failure -- the pixel count is the real verdict.
AE_RAW=$(magick compare -metric AE "$GOLDEN" "$CANDIDATE" null: 2>&1 || true)
AE="${AE_RAW%% *}"

if ! [[ "$AE" =~ ^[0-9]+(\.[0-9]+)?(e[+-][0-9]+)?$ ]]; then
  echo "compare did not return a pixel count (are the images the same size?): $AE_RAW" >&2
  exit 2
fi

# bash's builtin `printf %f` rejects scientific notation on this system, so
# round through awk instead: it parses "2.34427e+06" natively into an
# integer the shell's `-gt` can compare. A plain integer count passes
# through unchanged.
AE_INT=$(awk -v n="$AE" 'BEGIN { printf "%.0f", n }')
echo "differing pixels: $AE_INT (threshold: $MAX_DIFF)"

if [ "$AE_INT" -gt "$MAX_DIFF" ]; then
  echo "FAIL: $CANDIDATE differs from $GOLDEN by more than $MAX_DIFF pixels" >&2
  exit 1
fi

echo "PASS"
