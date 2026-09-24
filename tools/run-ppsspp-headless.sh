#!/usr/bin/env bash
# Build and run a deterministic PSP scene through PPSSPPHeadless.
#
#   tools/run-ppsspp-headless.sh [--no-build] [--crate psp-asset-viewer|psp-game]
#                                [--feature FEATURE] [--backend software]
#                                [--seconds N] [--pack PATH]
#                                [--scene SPEC] [--job NAME]
#
# --scene writes SPEC to capture_scene.txt beside the staged EBOOT and
# defaults the feature to `golden_capture`: one EBOOT then captures any golden
# scene (spec format: crates/ssb-capture) and exits a few frames after its
# screenshot. The timeout defaults to 30 s in this mode and is a failure,
# since a scene-file capture must exit by itself.
#
# --job gives this run its own memstick game directory
# (ssb64_regression_<NAME>) and output directory ($OUT/<NAME>), so several
# runs can share one build in parallel. PPSSPPHeadless keeps its memstick at
# $HOME/.ppsspp and never saves ppsspp.ini there, so the game directory and
# the per-job copy of no-status-overlay.ini are its only per-run state.
#
# --pack stages another pack instead of assets/generated/ssb64.pak (A/B
# captures). The native 480x272 frame is also kept as screenshot-native.png.
#
# --crate selects which `cargo psp` crate to build and run; default
# `psp-asset-viewer` (the debug/rendering-validation application's
# `regression_capture` scenes; `psp` is accepted as a backwards-compatible
# alias). `psp-game` (F1's front end/Training Mode) uses the same
# `regression_capture`/`headless_capture` feature names for its own,
# unrelated deterministic scripted-menu-input capture (see
# `psp-game/src/main.rs`).

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HEADLESS_BIN="${PPSSPP_HEADLESS_BIN:-$HOME/.local/src/ppsspp/build-headless/PPSSPPHeadless}"
OUT="${PPSSPP_HEADLESS_TEST_DIR:-$HOME/ppsspp-headless-test}"
FEATURE=
BACKEND=software
SECONDS_TO_RUN=
SCENE=
JOB=
BUILD=1
CRATE=psp-asset-viewer

while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    --crate)    CRATE="$2"; shift 2 ;;
    --feature) FEATURE="$2"; shift 2 ;;
    --backend) BACKEND="$2"; shift 2 ;;
    --seconds) SECONDS_TO_RUN="$2"; shift 2 ;;
    --pack)    PACK_OVERRIDE="$2"; shift 2 ;;
    --scene)   SCENE="$2"; shift 2 ;;
    --job)     JOB="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

# `psp` is a backwards-compatible alias for the renamed `psp-asset-viewer`.
[ "$CRATE" = psp ] && CRATE=psp-asset-viewer

case "$CRATE" in
  psp-asset-viewer|psp-game) ;;
  *) echo "--crate must be psp-asset-viewer or psp-game" >&2; exit 2 ;;
esac

if [ -n "$SCENE" ]; then
  FEATURE="${FEATURE:-golden_capture}"
  SECONDS_TO_RUN="${SECONDS_TO_RUN:-30}"
else
  FEATURE="${FEATURE:-regression_capture}"
  SECONDS_TO_RUN="${SECONDS_TO_RUN:-8}"
fi
if [ -n "$JOB" ]; then
  case "$JOB" in
    *[!A-Za-z0-9._-]*) echo "--job NAME may only use letters, digits, '.', '_' and '-'" >&2; exit 2 ;;
  esac
  OUT="$OUT/$JOB"
fi

[ -x "$HEADLESS_BIN" ] || {
  echo "PPSSPPHeadless not found: $HEADLESS_BIN" >&2
  echo "Build it with: cmake -DHEADLESS=ON -B build-headless && cmake --build build-headless --target PPSSPPHeadless" >&2
  exit 1
}
command -v ffmpeg >/dev/null || { echo "missing required tool: ffmpeg" >&2; exit 1; }

if [ "$BUILD" = 1 ]; then
  echo "==> building EBOOT ($CRATE, feature=$FEATURE,headless_capture)"
  ( cd "$REPO/$CRATE" && cargo psp --release --features "$FEATURE,headless_capture" )
fi

EBOOT="$REPO/$CRATE/target/mipsel-sony-psp/release/EBOOT.PBP"
PACK="${PACK_OVERRIDE:-$REPO/assets/generated/ssb64.pak}"
[ -f "$EBOOT" ] || { echo "EBOOT not found: $EBOOT" >&2; exit 1; }

# RE-255/RE-256: PPSSPPHeadless only reads PARAM.SFO's MEMSIZE key (needed
# for the real pack to fit in RAM) when the target is identified as an
# installed PSP_GAME directory, not a loose EBOOT.PBP path -- so this stages
# into PPSSPP's own memstick directory (~/.ppsspp/PSP/GAME) rather than a
# scratch directory. A dedicated, always-overwritten subfolder keeps this
# from colliding with any real installed homebrew there, and each crate gets
# its own subfolder so a `psp-asset-viewer` and a `psp-game` capture can't
# clobber each other's staged EBOOT.
MEMSTICK_NAME=ssb64_regression
[ "$CRATE" = psp-game ] && MEMSTICK_NAME=ssb64_game_regression
[ -n "$JOB" ] && MEMSTICK_NAME="${MEMSTICK_NAME}_$JOB"
MEMSTICK="${PPSSPP_MEMSTICK_DIR:-$HOME/.ppsspp/PSP/GAME}/$MEMSTICK_NAME"
mkdir -p "$OUT" "$MEMSTICK"
cp -f "$EBOOT" "$MEMSTICK/EBOOT.PBP"

# Stage the 29 MB pack by hard link (copy across filesystems), and not at all
# when the staged file is already the same inode, or has the same size and
# mtime (a previous `cp -p` of the same pack).
stage_pack() {
  local src="$1" dst="$2"
  if [ -f "$dst" ]; then
    [ "$src" -ef "$dst" ] && return 0
    if [ "$(stat -c '%s %Y' "$src")" = "$(stat -c '%s %Y' "$dst")" ]; then
      return 0
    fi
  fi
  # Unlink first: writing through an existing hard link would overwrite the
  # previously staged pack's source file.
  rm -f "$dst"
  ln "$src" "$dst" 2>/dev/null || cp -p "$src" "$dst"
}

# The scene file is written for --scene runs and removed otherwise, so a
# stale spec from an earlier run can never pick this run's scene.
if [ -n "$SCENE" ]; then
  printf '%s\n' "$SCENE" > "$MEMSTICK/capture_scene.txt"
else
  rm -f "$MEMSTICK/capture_scene.txt"
fi
# psp-game now loads the pack too (`assets.rs`, `plans/gameplay/F1.md`'s
# "Scene loading" section), but its Training screen only recolours the
# background on a missing/bad pack rather than failing to boot, so staging
# it stays best-effort here, same as for psp-asset-viewer.
if [ -f "$PACK" ]; then
  stage_pack "$PACK" "$MEMSTICK/ssb64.pak"
elif [ "$CRATE" != psp-game ]; then
  echo "asset pack not found: $PACK" >&2
  exit 1
fi
rm -f "$OUT/screenshot.bmp" "$OUT/screenshot.png" "$OUT/screenshot-native.png" "$OUT/ppsspp-headless.log"

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
ffmpeg -loglevel error -y -i "$OUT/screenshot.bmp" -vf 'crop=480:272:0:0' \
  "$OUT/screenshot-native.png"
echo "==> screenshot: $OUT/screenshot.png"
echo "==> log:        $OUT/ppsspp-headless.log"

# Without --scene a timeout is expected: the per-scene builds are persistent
# PSP programs, and the screenshot devctl fires at the frozen deterministic
# tick first. A --scene build exits by itself after its screenshot, so a
# timeout there means it hung. PPSSPPHeadless exits 0 either way; the log's
# TIMEOUT line is the only signal.
if [ -n "$SCENE" ] && grep -qx 'TIMEOUT' "$OUT/ppsspp-headless.log"; then
  echo "FAIL: scene '$SCENE' did not exit within ${SECONDS_TO_RUN}s" >&2
  exit 1
fi
if [ "$STATUS" -ne 0 ] && [ "$STATUS" -ne 1 ]; then
  echo "warning: PPSSPPHeadless exited $STATUS after saving the screenshot" >&2
fi
