#!/usr/bin/env bash
#
# Build and run the PSP executable under PPSSPP, capturing a screenshot.
#
#   tools/run-ppsspp.sh [--no-build] [--backend software|opengl] [--seconds N]
#
# Everything here is defensive against a specific failure that actually
# happened. Do not simplify without reading the reasons.
#
#  1. **Always terminates PPSSPP.** `kill` on the `flatpak run` wrapper does not
#     kill the app inside the bwrap sandbox; orphaned instances accumulated at
#     ~420 MB each until the machine ran low on memory. Cleanup runs from a
#     trap (so it also fires on error and Ctrl-C), uses `flatpak kill`, and
#     verifies the process is gone rather than assuming.
#
#  2. **Interruptible sleep.** Bash defers trap handlers until the current
#     foreground command finishes, so a plain `sleep 60` swallows Ctrl-C for a
#     full minute -- and PPSSPP leaks if the script is killed meanwhile.
#
#  3. **Bounded screenshot.** `import -window` blocks forever if the target
#     window disappears. A hung `import` kept the script, and therefore PPSSPP,
#     alive indefinitely.
#
#  4. **Software rasteriser by default.** PPSSPP's hardware backends do not
#     reflect CPU writes to emulated VRAM, and `sceGuDebugFlush` paints the
#     debug overlay exactly that way, so under OpenGL the diagnostics are
#     computed but invisible (docs/reverse-engineering.md RE-014). Forced
#     through `--appendconfig`, not `--graphics=software` -- the command-line
#     flag was observed not to take effect. The config is merged for this run
#     only and does NOT modify the user's ppsspp.ini.
#
#  5. **Window identification by difference.** Picking "the first window
#     matching ppsspp" grabs the wrong one if the developer already has PPSSPP
#     open, silently screenshotting an unrelated window and making the run look
#     broken.
#
#  6. **Absolute paths + explicit X11.** PPSSPP here is a flatpak; a relative
#     path resolves inside the sandbox and is never found, and leaving the
#     video driver to autodetect can fail to produce a window at all.
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

# --- cleanup -----------------------------------------------------------------
#
# `kill` on the `flatpak run` wrapper does NOT reliably terminate PPSSPP: the
# real process runs inside a bwrap sandbox and survives, leaking ~420 MB per
# run. `flatpak kill` addresses the app by id and does terminate it.
#
# Registered as a trap so it also runs when the script errors out or is
# interrupted, which is exactly when leaks used to happen.
cleanup() {
  local status=$?
  trap - EXIT INT TERM   # don't re-enter while cleaning up
  [ -n "${SLEEP_PID:-}" ] && kill "$SLEEP_PID" 2>/dev/null || true
  [ -n "${PID:-}" ] && kill "$PID" 2>/dev/null || true
  flatpak kill org.ppsspp.PPSSPP 2>/dev/null || true
  # Confirm rather than assume; escalate if the polite kill did not land.
  sleep 1
  if pgrep -f PPSSPPSDL >/dev/null 2>&1; then
    pkill -9 -f PPSSPPSDL 2>/dev/null || true
    sleep 1
  fi
  if pgrep -f PPSSPPSDL >/dev/null 2>&1; then
    echo "warning: PPSSPP survived cleanup - kill it manually" >&2
  fi
  exit $status
}
trap cleanup EXIT INT TERM

# Sleeps must be interruptible. Bash defers trap handlers until the current
# foreground command finishes, so a plain `sleep 60` swallows Ctrl-C for a full
# minute and PPSSPP leaks if the script is killed meanwhile. Backgrounding the
# sleep and `wait`-ing on it lets the trap fire immediately.
interruptible_sleep() {
  sleep "$1" &
  SLEEP_PID=$!
  wait "$SLEEP_PID" 2>/dev/null || true
  SLEEP_PID=""
}

# Trailing `|| true` matters: `grep` exits non-zero when nothing matches, which
# is the normal case when no PPSSPP is open, and `set -euo pipefail` would turn
# that into a silent fatal exit before any output.
ppsspp_windows() {
  wmctrl -l -x 2>/dev/null | grep -i ppsspp | awk '{print $1}' | sort || true
}
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
  exit 1   # the EXIT trap cleans up
fi
echo "==> window $WIN; running ${SECONDS_TO_RUN}s"

interruptible_sleep "$SECONDS_TO_RUN"

# `import` blocks forever if the target window vanishes mid-capture, and a hung
# import kept this script (and therefore PPSSPP) alive indefinitely. Cap it.
if ! timeout 20 import -window "$WIN" "$OUT/screenshot.png"; then
  echo "warning: screenshot failed or timed out" >&2
fi

echo "==> screenshot: $OUT/screenshot.png"
echo "==> log:        $OUT/ppsspp.log"
# The EXIT trap terminates PPSSPP and verifies it is gone.
