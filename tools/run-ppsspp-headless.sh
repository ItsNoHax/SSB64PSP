#!/usr/bin/env bash
# Build and run a deterministic PSP scene through PPSSPPHeadless.
#
#   tools/run-ppsspp-headless.sh [--no-build] [--feature FEATURE]
#                                [--backend software] [--seconds N]

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HEADLESS_BIN="${PPSSPP_HEADLESS_BIN:-$HOME/.local/src/ppsspp/build-headless/PPSSPPHeadless}"
OUT="${PPSSPP_HEADLESS_TEST_DIR:-$HOME/ppsspp-headless-test}"
FEATURE=regression_capture
BACKEND=software
SECONDS_TO_RUN=8
BUILD=1

while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    --feature) FEATURE="$2"; shift 2 ;;
    --backend) BACKEND="$2"; shift 2 ;;
    --seconds) SECONDS_TO_RUN="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

[ -x "$HEADLESS_BIN" ] || {
  echo "PPSSPPHeadless not found: $HEADLESS_BIN" >&2
  echo "Build it with: cmake -DHEADLESS=ON -B build-headless && cmake --build build-headless --target PPSSPPHeadless" >&2
  exit 1
}
command -v ffmpeg >/dev/null || { echo "missing required tool: ffmpeg" >&2; exit 1; }

if [ "$BUILD" = 1 ]; then
  echo "==> building EBOOT (feature=$FEATURE,headless_capture)"
  ( cd "$REPO/psp" && cargo psp --release --features "$FEATURE,headless_capture" )
fi

EBOOT="$REPO/psp/target/mipsel-sony-psp/release/EBOOT.PBP"
PACK="$REPO/assets/generated/ssb64.pak"
[ -f "$EBOOT" ] || { echo "EBOOT not found: $EBOOT" >&2; exit 1; }
[ -f "$PACK" ] || { echo "asset pack not found: $PACK" >&2; exit 1; }

# RE-255/RE-256: PPSSPPHeadless only reads PARAM.SFO's MEMSIZE key (needed
# for the real pack to fit in RAM) when the target is identified as an
# installed PSP_GAME directory, not a loose EBOOT.PBP path -- so this stages
# into PPSSPP's own memstick directory (~/.ppsspp/PSP/GAME) rather than a
# scratch directory. A dedicated, always-overwritten subfolder keeps this
# from colliding with any real installed homebrew there.
MEMSTICK="${PPSSPP_MEMSTICK_DIR:-$HOME/.ppsspp/PSP/GAME}/ssb64_regression"
mkdir -p "$OUT" "$MEMSTICK"
cp -f "$EBOOT" "$MEMSTICK/EBOOT.PBP"
cp -f "$PACK" "$MEMSTICK/ssb64.pak"
rm -f "$OUT/screenshot.bmp" "$OUT/screenshot.png" "$OUT/ppsspp-headless.log"

# RE-256: booting an installed PSP_GAME directory (needed for MEMSIZE above)
# also picks up PPSSPP's own "FPS: N.N" debug-stats overlay, which a loose
# EBOOT.PBP boot never showed. Force it off rather than let it bleed into
# golden pixels.
printf '[General]\niShowStatusFlags = 0\n' > "$OUT/no-status-overlay.ini"

echo "==> running PPSSPPHeadless (backend=$BACKEND, timeout=${SECONDS_TO_RUN}s)"
set +e
"$HEADLESS_BIN" \
    --graphics="$BACKEND" \
    --appendconfig="$OUT/no-status-overlay.ini" \
    --screenshot-save="$OUT/screenshot.bmp" \
    --timeout="$SECONDS_TO_RUN" \
    "$MEMSTICK/EBOOT.PBP" > "$OUT/ppsspp-headless.log" 2>&1
STATUS=$?
set -e

[ -s "$OUT/screenshot.bmp" ] || {
  echo "PPSSPPHeadless did not produce a screenshot (exit=$STATUS)" >&2
  sed -n '1,120p' "$OUT/ppsspp-headless.log" >&2
  exit 1
}

# Headless saves the 512-pixel framebuffer stride. Its BMP writer currently
# declares a two-byte-longer file than it emits, which ImageMagick rejects;
# FFmpeg decodes the otherwise valid framebuffer. The project goldens are 2x
# captures of the visible 480x272 display, matching the old windowed flow.
ffmpeg -loglevel error -y -i "$OUT/screenshot.bmp" \
  -vf 'crop=480:272:0:0,scale=960:544:flags=neighbor' \
  "$OUT/screenshot.png"
echo "==> screenshot: $OUT/screenshot.png"
echo "==> log:        $OUT/ppsspp-headless.log"

# A timeout is expected because the viewer is intentionally a persistent PSP
# program; the screenshot devctl fires at the frozen deterministic tick first.
if [ "$STATUS" -ne 0 ] && [ "$STATUS" -ne 1 ]; then
  echo "warning: PPSSPPHeadless exited $STATUS after saving the screenshot" >&2
fi
