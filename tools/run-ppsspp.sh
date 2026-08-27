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
#     flag was observed not to take effect.
#
#     NOTE: `--appendconfig` settings ARE written back to the user's
#     ppsspp.ini on exit. An earlier version of this script claimed otherwise
#     and silently left `SoftwareRenderer = True` in the user's config. The
#     script now snapshots ppsspp.ini before the run and restores it after.
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
#  7. **More than one way to take the picture.** ImageMagick 7.1.2 ships an
#     `import` built without X11 support: every capture fails with "missing an
#     image filename", including `-window root`. The script had one capture
#     path and reported that as "screenshot failed or timed out", which reads
#     like the emulator broke. Try each available tool in turn and say which
#     one worked.
#
#  8. **A locked screen looks exactly like a broken build.** With the session
#     locked, nothing composites: PPSSPP hangs at "Initializing Vulkan...",
#     the window exists but cannot be grabbed, and any capture returns the lock
#     screen. Report the window title on failure -- it names the real cause
#     immediately, where a generic failure message sent me looking at my own
#     rendering code.
#
# Requires: flatpak org.ppsspp.PPSSPP, wmctrl, and one of ImageMagick
# (`import`), `spectacle`, `grim`, `scrot` or `maim`.

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

for tool in flatpak wmctrl; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

# At least one capture tool has to exist. `import` is preferred because it can
# grab a single window; the rest capture the whole screen, which still shows
# the emulator but includes whatever else is on the desktop.
CAPTURE_TOOLS=()
for tool in import spectacle grim scrot maim; do
  command -v "$tool" >/dev/null && CAPTURE_TOOLS+=("$tool")
done
if [ ${#CAPTURE_TOOLS[@]} -eq 0 ]; then
  echo "no screenshot tool found (tried: import spectacle grim scrot maim)" >&2
  exit 1
fi

# A capture of a single flat colour is not a screenshot of anything.
#
# This matters more than it sounds: with the session locked, `spectacle`
# succeeds, writes a 600 KB PNG, and exits 0 -- and the PNG is pure white. The
# script would then announce a screenshot and the image would be taken as
# evidence about the build. Uniform output is treated as failure so it cannot
# be. `magick identify` does no X11 work, so it still functions on the same
# install whose `import` does not.
is_blank() {
  local f="$1" sd
  command -v magick >/dev/null || return 1
  sd=$(magick identify -format "%[fx:standard_deviation]" "$f" 2>/dev/null || echo 1)
  awk -v s="$sd" 'BEGIN { exit !(s < 0.002) }'
}

# Try each tool in turn. A tool that is installed is not necessarily working:
# ImageMagick 7.1.2 ships an `import` compiled without X11 and it fails on
# every window, root included. Success is judged by the file, not by the exit
# status, because some of these exit 0 having written nothing.
capture() {
  local win="$1" out="$2" tool
  rm -f "$out"
  for tool in "${CAPTURE_TOOLS[@]}"; do
    case "$tool" in
      # `import` blocks forever if the target window vanishes mid-capture, and
      # a hung import kept this script -- and therefore PPSSPP -- alive
      # indefinitely. Every branch is bounded for that reason.
      import)    timeout 20 import -window "$win" "$out" >/dev/null 2>&1 || true ;;
      spectacle) timeout 25 spectacle -b -n -f -o "$out" >/dev/null 2>&1 || true ;;
      grim)      timeout 20 grim "$out" >/dev/null 2>&1 || true ;;
      scrot)     timeout 20 scrot -o "$out" >/dev/null 2>&1 || true ;;
      maim)      timeout 20 maim "$out" >/dev/null 2>&1 || true ;;
    esac
    if [ -s "$out" ] && ! is_blank "$out"; then
      echo "$tool"
      return 0
    fi
  done
  return 1
}

if [ "$BUILD" = 1 ]; then
  echo "==> building EBOOT"
  ( cd "$REPO/psp" && cargo psp --release )
fi

EBOOT="$REPO/psp/target/mipsel-sony-psp/release/EBOOT.PBP"
[ -f "$EBOOT" ] || { echo "EBOOT not found: $EBOOT" >&2; exit 1; }

mkdir -p "$OUT"
cp "$EBOOT" "$OUT/"

# Report what is actually being run. `cargo psp` only works from psp/; invoked
# from the repo root it exits 0 without rebuilding, so a hand-run build can
# leave a stale EBOOT that the next --no-build run happily screenshots. Two
# consecutive runs then "prove" a change that was never compiled.
echo "==> staged EBOOT $(du -h "$EBOOT" | cut -f1) ($(date -r "$EBOOT" '+%H:%M:%S'))"

# Stage the asset pack alongside the EBOOT.
#
# This was missing, and the failure was quiet in the worst way: a stale pack
# from an earlier format version stayed behind, the new EBOOT rejected it on
# version, and the viewer fell back to the built-in tetrahedron -- which looks
# exactly like "no assets yet" rather than "you are running last week's data".
# Copy it every run, and say so, so the screenshot can never silently describe
# a different build than the one just compiled.
PACK="$REPO/assets/generated/ssb64.pak"
if [ -f "$PACK" ]; then
  cp -f "$PACK" "$OUT/"
  echo "==> staged pack $(du -h "$PACK" | cut -f1) ($(date -r "$PACK" '+%H:%M:%S'))"
else
  echo "==> no asset pack at $PACK; run: cargo run --release -p romtool -- pack <rom>" >&2
fi

PPSSPP_INI="$HOME/.var/app/org.ppsspp.PPSSPP/config/ppsspp/PSP/SYSTEM/ppsspp.ini"

# Only the rasteriser is overridden, through --appendconfig, because
# `--graphics=software` was observed not to take effect.
#
# Do NOT also pin GraphicsBackend here or by editing ppsspp.ini. Both were
# tried and both left PPSSPP unable to open a window at all: it stores the
# value as "0 (OPENGL)" and a rewritten value does not round-trip, after which
# it marks every backend failed. Leave the user's backend choice alone.
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
  # Put the user's config back: PPSSPP persists whatever --appendconfig set.
  if [ -n "${INI_BACKUP:-}" ] && [ -f "$INI_BACKUP" ]; then
    cp -f "$INI_BACKUP" "$PPSSPP_INI" 2>/dev/null || true
    rm -f "$INI_BACKUP"
  fi
  exit $status
}
trap cleanup EXIT INT TERM

# Snapshot the user's config so the run can be made non-destructive.
INI_BACKUP=""
if [ -f "$PPSSPP_INI" ]; then
  INI_BACKUP="$(mktemp)"
  cp -f "$PPSSPP_INI" "$INI_BACKUP"
fi

# PPSSPP records a backend in FailedGraphicsBackends.txt if it dies before
# finishing graphics init -- which is exactly what the SIGKILL in cleanup()
# looks like. Once the file reads "VULKAN,OPENGL,ALL" it refuses to start any
# backend at all and every later run fails with "Did not switch failed
# backend", long after whatever caused the original crash is gone. Clearing it
# each run makes the harness self-healing.
rm -f "$(dirname "$PPSSPP_INI")/FailedGraphicsBackends.txt" 2>/dev/null || true


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

if TOOL=$(capture "$WIN" "$OUT/screenshot.png"); then
  echo "==> screenshot: $OUT/screenshot.png (via $TOOL)"
else
  # Name the likely cause instead of leaving it to be guessed. The window's
  # title is the single most useful clue: PPSSPP stuck on "Initializing
  # Vulkan..." means the session is locked or the GPU driver is wedged, and
  # nothing about the build under test is wrong.
  TITLE=$(wmctrl -l 2>/dev/null | grep -i "^$WIN" | cut -d' ' -f5- || true)
  echo "warning: no usable screenshot (tried: ${CAPTURE_TOOLS[*]})" >&2
  echo "         window $WIN title: ${TITLE:-<none>}" >&2
  if [ -s "$OUT/screenshot.png" ]; then
    echo "         a capture succeeded but the image is one flat colour:" >&2
    echo "         the screen is locked or blanked, so nothing composites." >&2
    echo "         Unlock the session and re-run." >&2
  fi
  case "$TITLE" in
    *Initializ*)
      echo "         PPSSPP never finished graphics init -- a locked screen" >&2
      echo "         does this, and so does a wedged GPU driver." >&2
      ;;
  esac
  echo "         see $OUT/ppsspp.log" >&2
fi

echo "==> log:        $OUT/ppsspp.log"
# The EXIT trap terminates PPSSPP and verifies it is gone.
