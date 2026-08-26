#!/usr/bin/env bash
#
# Build and run the PSP executable under PPSSPP, capturing a screenshot.
#
#   tools/run-ppsspp.sh [--no-build] [--backend software|opengl] [--seconds N]
#
# Three non-obvious things are handled here, each of which cost real debugging
# time to work out:
#
#  1. **Software rasteriser by default.** PPSSPP's hardware backends do not
#     reflect CPU writes to emulated VRAM, and `sceGuDebugFlush` paints the
#     debug overlay exactly that way. Under OpenGL the diagnostics are computed
#     but invisible. See docs/reverse-engineering.md RE-014.
#
#     Note this is forced through `--appendconfig`, not `--graphics=software`;
#     the command-line flag was observed not to take effect. The config file is
#     merged into the running config and does NOT modify the user's ppsspp.ini.
#
#  2. **Window identification by difference.** Picking "the first window matching
#     ppsspp" grabs the wrong one if the developer already has PPSSPP open --
#     which silently screenshots an unrelated window and makes the run look
#     broken. We snapshot window ids before launching and take whichever is new.
#
#  3. **Absolute paths + explicit X11.** PPSSPP here is a flatpak; a relative
#     path resolves inside the sandbox and the file is never found, and leaving
#     the video driver to autodetect can fail to produce a window at all.
#
# Requires: flatpak org.ppsspp.PPSSPP, wmctrl, ImageMagick (`import`).

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${PPSSPP_TEST_DIR:-$HOME/ppsspp-test}"
BACKEND=software
SECONDS_TO_RUN=12
BUILD=1

while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    --backend)  BACKEND="$2"; shift 2 ;;
    --seconds)  SECONDS_TO_RUN="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

for tool in flatpak wmctrl import; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

if [ "$BUILD" = 1 ]; then
  echo "==> building EBOOT"
  ( cd "$REPO/psp" && cargo psp --release )
fi

EBOOT="$REPO/psp/target/mipsel-sony-psp/release/EBOOT.PBP"
[ -f "$EBOOT" ] || { echo "EBOOT not found: $EBOOT" >&2; exit 1; }

mkdir -p "$OUT"
cp "$EBOOT" "$OUT/"

# Merged into PPSSPP's config for this run only.
if [ "$BACKEND" = software ]; then
  printf '[Graphics]\nSoftwareRenderer = True\n' > "$OUT/backend.ini"
else
  printf '[Graphics]\nSoftwareRenderer = False\n' > "$OUT/backend.ini"
fi

export DISPLAY="${DISPLAY:-:0}"
unset WAYLAND_DISPLAY || true

ppsspp_windows() { wmctrl -l -x 2>/dev/null | grep -i ppsspp | awk '{print $1}' | sort; }
BEFORE=$(ppsspp_windows)

echo "==> launching PPSSPP (backend=$BACKEND)"
flatpak run --env=SDL_VIDEODRIVER=x11 --env=DISPLAY="$DISPLAY" --filesystem=home \
  org.ppsspp.PPSSPP \
  --appendconfig="$OUT/backend.ini" \
  --windowed --xres 960 --yres 544 \
  "$OUT/EBOOT.PBP" > "$OUT/ppsspp.log" 2>&1 &
PID=$!

WIN=""
for _ in $(seq 1 60); do
  NEW=$(comm -13 <(echo "$BEFORE") <(ppsspp_windows) | head -1 || true)
  if [ -n "$NEW" ]; then WIN="$NEW"; break; fi
  kill -0 $PID 2>/dev/null || break
  sleep 0.5
done

if [ -z "$WIN" ]; then
  echo "no PPSSPP window appeared; see $OUT/ppsspp.log" >&2
  kill $PID 2>/dev/null || true
  exit 1
fi
echo "==> window $WIN; running ${SECONDS_TO_RUN}s"

sleep "$SECONDS_TO_RUN"
import -window "$WIN" "$OUT/screenshot.png"
kill $PID 2>/dev/null || true
wait $PID 2>/dev/null || true

echo "==> screenshot: $OUT/screenshot.png"
echo "==> log:        $OUT/ppsspp.log"
