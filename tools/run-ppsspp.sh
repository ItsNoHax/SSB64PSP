#!/usr/bin/env bash
#
# Build and run the PSP executable under PPSSPP, capturing a screenshot.
#
#   tools/run-ppsspp.sh [--no-build] [--backend software|opengl] [--seconds N]
#                        [--audit-stages N] [--audit-animations N]
#                        [--audit-effects N]
#                        [--audit-effect-animations N]
#                        [--audit-effect-materials N]
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
#     script now snapshots ppsspp.ini before the run and restores it after --
#     *and* resets `SoftwareRenderer` explicitly, because a snapshot is only
#     as clean as the file it was taken from. A run killed with SIGKILL leaves
#     the key set, the next run snapshots that, and the restore then puts it
#     back. It survived that way for days, forcing the software rasteriser for
#     every game on the machine. Snapshot-and-restore is not sufficient for a
#     setting this script is itself responsible for.
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
#  7. **A locked screen looks exactly like a broken build.** This is the one
#     that cost the most. With the session locked nothing composites: PPSSPP
#     hangs forever at "Initializing Vulkan...", `import` fails on every window
#     -- root included -- with the misleading ``missing an image filename``,
#     and `spectacle` *succeeds*, exits 0, and writes a pure-white PNG. The old
#     message, "screenshot failed or timed out", read like the emulator broke
#     and sent me looking at my own rendering code. So: report the window title
#     on failure, because "Initializing Vulkan..." names the real cause at a
#     glance.
#
#     `import` is not broken and ImageMagick is not missing X11 -- I believed
#     both for an hour. Unlock the screen and it works first try.
#
#  8. **More than one way to take the picture, and a check that it is a
#     picture.** Tools are tried in turn so a single broken one is not fatal,
#     and a capture whose standard deviation is ~0 is treated as no capture:
#     otherwise the blank PNG above gets read as evidence about the build.
#
# Requires: flatpak org.ppsspp.PPSSPP, wmctrl, and one of ImageMagick
# (`import`), `spectacle`, `grim`, `scrot` or `maim`. An unlocked session.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${PPSSPP_TEST_DIR:-$HOME/ppsspp-test}"
BACKEND=software
SECONDS_TO_RUN=12
BUILD=1
AUDIT_STAGES=0
AUDIT_ANIMATIONS=0
AUDIT_EFFECTS=0
AUDIT_EFFECT_ANIMATIONS=0
AUDIT_EFFECT_MATERIALS=0

while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    --backend)  BACKEND="$2"; shift 2 ;;
    --seconds)  SECONDS_TO_RUN="$2"; shift 2 ;;
    --audit-stages) AUDIT_STAGES="$2"; shift 2 ;;
    --audit-animations) AUDIT_ANIMATIONS="$2"; shift 2 ;;
    --audit-effects) AUDIT_EFFECTS="$2"; shift 2 ;;
    --audit-effect-animations) AUDIT_EFFECT_ANIMATIONS="$2"; shift 2 ;;
    --audit-effect-materials) AUDIT_EFFECT_MATERIALS="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

case "$AUDIT_STAGES" in
  ''|*[!0-9]*) echo "--audit-stages needs a non-negative integer" >&2; exit 2 ;;
esac
case "$AUDIT_ANIMATIONS" in
  ''|*[!0-9]*) echo "--audit-animations needs a non-negative integer" >&2; exit 2 ;;
esac
case "$AUDIT_EFFECTS" in
  ''|*[!0-9]*) echo "--audit-effects needs a non-negative integer" >&2; exit 2 ;;
esac
case "$AUDIT_EFFECT_ANIMATIONS" in
  ''|*[!0-9]*) echo "--audit-effect-animations needs a non-negative integer" >&2; exit 2 ;;
esac
case "$AUDIT_EFFECT_MATERIALS" in
  ''|*[!0-9]*) echo "--audit-effect-materials needs a non-negative integer" >&2; exit 2 ;;
esac
ACTIVE_AUDITS=$(( (AUDIT_STAGES > 0) + (AUDIT_ANIMATIONS > 0) + (AUDIT_EFFECTS > 0) + (AUDIT_EFFECT_ANIMATIONS > 0) + (AUDIT_EFFECT_MATERIALS > 0) ))
if [ "$ACTIVE_AUDITS" -gt 1 ]; then
  echo "choose only one exhaustive audit per run" >&2
  exit 2
fi

for tool in flatpak wmctrl; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
if [ "$AUDIT_STAGES" -gt 0 ] || [ "$AUDIT_ANIMATIONS" -gt 0 ] || [ "$AUDIT_EFFECTS" -gt 0 ] || [ "$AUDIT_EFFECT_ANIMATIONS" -gt 0 ] || [ "$AUDIT_EFFECT_MATERIALS" -gt 0 ]; then
  if ! command -v xdotool >/dev/null && ! python3 -c 'import Xlib' 2>/dev/null; then
    echo "exhaustive audits require xdotool or Python Xlib to advance the PSP D-pad" >&2
    exit 1
  fi
fi
if { [ "$AUDIT_ANIMATIONS" -gt 0 ] || [ "$AUDIT_EFFECTS" -gt 0 ] || [ "$AUDIT_EFFECT_ANIMATIONS" -gt 0 ] || [ "$AUDIT_EFFECT_MATERIALS" -gt 0 ]; } && ! command -v magick >/dev/null; then
  echo "animation/effect audits require ImageMagick for per-frame content checks" >&2
  exit 1
fi

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
# be.
is_blank() {
  local f="$1" sd
  command -v magick >/dev/null || return 1
  sd=$(magick identify -format "%[fx:standard_deviation]" "$f" 2>/dev/null || echo 1)
  awk -v s="$sd" 'BEGIN { exit !(s < 0.002) }'
}

# Try each tool in turn, so one that cannot capture right now is not fatal.
# Success is judged by the file, not by the exit status, because some of these
# exit 0 having written nothing -- and `spectacle` exits 0 having written a
# blank image when the screen is locked.
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
  if [ "$AUDIT_EFFECT_MATERIALS" -gt 0 ]; then
    ( cd "$REPO/psp" && cargo psp --release --features effect_material_audit_capture )
  elif [ "$AUDIT_EFFECT_ANIMATIONS" -gt 0 ]; then
    ( cd "$REPO/psp" && cargo psp --release --features effect_animation_audit_capture )
  elif [ "$AUDIT_EFFECTS" -gt 0 ]; then
    ( cd "$REPO/psp" && cargo psp --release --features effect_audit_capture )
  elif [ "$AUDIT_ANIMATIONS" -gt 0 ]; then
    ( cd "$REPO/psp" && cargo psp --release --features animation_audit_capture )
  else
    ( cd "$REPO/psp" && cargo psp --release )
  fi
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
    # Belt and braces. Restoring a snapshot only helps if the snapshot was
    # clean, and it is not always: a run killed with SIGKILL (or one from
    # before this trap existed) leaves SoftwareRenderer=True behind, and every
    # later run then snapshots that and faithfully restores it. It stayed set
    # for days that way, silently forcing the software rasteriser for every
    # game the user played. So the key this script actually sets is also reset
    # explicitly, which is idempotent and does not depend on the snapshot.
    if [ -f "$PPSSPP_INI" ] && grep -q '^SoftwareRenderer = True' "$PPSSPP_INI"; then
      sed -i 's/^SoftwareRenderer = True/SoftwareRenderer = False/' "$PPSSPP_INI"
      echo "==> reset SoftwareRenderer=False (leftover from an earlier run)" >&2
    fi
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

send_right() {
  local win="$1"
  if command -v xdotool >/dev/null; then
    xdotool key --window "$win" Right
  else
    python3 "$REPO/tools/send-x11-key.py" "$win" Right
  fi
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

if [ "$AUDIT_EFFECT_MATERIALS" -gt 0 ]; then
  AUDIT_OUT="$OUT/effect-material-audit"
  mkdir -p "$AUDIT_OUT"
  rm -f "$AUDIT_OUT"/effect-material-*.png
  echo "==> capturing frame 4 of $AUDIT_EFFECT_MATERIALS manager effect material animations"
  AUDIT_FAILED=0
  HEADER_HASHES="$(mktemp)"
  for ((i = 0; i < AUDIT_EFFECT_MATERIALS; i++)); do
    printf -v AUDIT_FILE '%s/effect-material-%02d.png' "$AUDIT_OUT" "$i"
    if TOOL=$(capture "$WIN" "$AUDIT_FILE"); then
      CENTRE_SD=$(magick "$AUDIT_FILE" -gravity center -crop '60%x76%+0+0' \
        +repage -format '%[fx:standard_deviation]' info: 2>/dev/null || echo 0)
      case "$i" in
        # Source-backed rest-invisible exceptions among the 26
        # material-animated effects, in `effect_material_slots` order
        # (RE-176): slot 9 is NessPKFlash (X/Y scale 1e-5, needs its
        # transform stream too -- this material-only audit does not tick
        # it), slot 23 is LinkSpinAttack (primitive alpha ramps from zero
        # but is still imperceptibly dim at frame 4, RE-175).
        9|23)
          if awk -v s="$CENTRE_SD" 'BEGIN { exit !(s >= 0.003) }'; then
            echo "warning: effect material $i should remain rest-invisible" >&2
            AUDIT_FAILED=1
          fi
          ;;
        *)
          if awk -v s="$CENTRE_SD" 'BEGIN { exit !(s < 0.003) }'; then
            echo "warning: effect material $i has no measurable central render content" >&2
            AUDIT_FAILED=1
          fi
          ;;
      esac
      magick "$AUDIT_FILE" -gravity north -crop '100%x15%+0+0' +repage \
        -format '%#\n' info: >> "$HEADER_HASHES"
      echo "==> effect material $i: $AUDIT_FILE (via $TOOL, centre sd $CENTRE_SD)"
    else
      echo "warning: effect material $i capture failed" >&2
      AUDIT_FAILED=1
    fi
    if [ "$i" -lt $((AUDIT_EFFECT_MATERIALS - 1)) ]; then
      wmctrl -i -a "$WIN"
      send_right "$WIN"
      interruptible_sleep 0.5
    fi
  done
  UNIQUE_HEADERS=$(sort -u "$HEADER_HASHES" | wc -l)
  rm -f "$HEADER_HASHES"
  if [ "$UNIQUE_HEADERS" -ne "$AUDIT_EFFECT_MATERIALS" ]; then
    echo "warning: only $UNIQUE_HEADERS/$AUDIT_EFFECT_MATERIALS identity headers were unique" >&2
    AUDIT_FAILED=1
  fi
  [ "$AUDIT_FAILED" -eq 0 ] || exit 1
  {
    echo "commit=$(git -C "$REPO" rev-parse HEAD)"
    echo "backend=$BACKEND"
    echo "manager_effect_material_animation_count=$AUDIT_EFFECT_MATERIALS"
    echo "capture_frame=4"
    echo "visible_material_samples=24"
    echo "rest_invisible_material_samples=2"
    echo "unique_identity_headers=$UNIQUE_HEADERS"
    echo "eboot_sha256=$(sha256sum "$EBOOT" | awk '{print $1}')"
    echo "pack_sha256=$(sha256sum "$PACK" | awk '{print $1}')"
    echo "captures:"
    sha256sum "$AUDIT_OUT"/effect-material-*.png | sed "s|$AUDIT_OUT/||"
  } > "$AUDIT_OUT/manifest.txt"
  echo "==> manifest: $AUDIT_OUT/manifest.txt"
  echo "==> effect material audit: $AUDIT_OUT"
elif [ "$AUDIT_EFFECT_ANIMATIONS" -gt 0 ]; then
  AUDIT_OUT="$OUT/effect-animation-audit"
  mkdir -p "$AUDIT_OUT"
  rm -f "$AUDIT_OUT"/effect-animation-*.png
  echo "==> capturing frame 4 of $AUDIT_EFFECT_ANIMATIONS manager effect transform animations"
  AUDIT_FAILED=0
  HEADER_HASHES="$(mktemp)"
  for ((i = 0; i < AUDIT_EFFECT_ANIMATIONS; i++)); do
    printf -v AUDIT_FILE '%s/effect-animation-%02d.png' "$AUDIT_OUT" "$i"
    if TOOL=$(capture "$WIN" "$AUDIT_FILE"); then
      # The entry-star billboard's authored frame-4 translation reaches the
      # left side of the active 16:9 viewport, so retain 80% while still
      # excluding the pillarbox bars and one-line HUD.
      CENTRE_SD=$(magick "$AUDIT_FILE" -gravity center -crop '80%x76%+0+0' \
        +repage -format '%[fx:standard_deviation]' info: 2>/dev/null || echo 0)
      case "$i" in
        # Link Spin Attack's transform runs while its independent material
        # AObjEvent32 stream owns visibility; primitive alpha is still the
        # source-authored zero until that next runtime slice lands.
        29)
          HIDDEN_SD=$(magick "$AUDIT_FILE" -gravity center -crop '60%x76%+0+0' \
            +repage -format '%[fx:standard_deviation]' info: 2>/dev/null || echo 0)
          if awk -v s="$HIDDEN_SD" 'BEGIN { exit !(s >= 0.003) }'; then
            echo "warning: effect animation $i should remain material-hidden" >&2
            AUDIT_FAILED=1
          fi
          ;;
        *)
          if awk -v s="$CENTRE_SD" 'BEGIN { exit !(s < 0.003) }'; then
            echo "warning: effect animation $i has no measurable frame-4 content" >&2
            AUDIT_FAILED=1
          fi
          ;;
      esac
      magick "$AUDIT_FILE" -gravity north -crop '100%x15%+0+0' +repage \
        -format '%#\n' info: >> "$HEADER_HASHES"
      echo "==> effect animation $i: $AUDIT_FILE (via $TOOL, centre sd $CENTRE_SD)"
    else
      echo "warning: effect animation $i capture failed" >&2
      AUDIT_FAILED=1
    fi
    if [ "$i" -lt $((AUDIT_EFFECT_ANIMATIONS - 1)) ]; then
      wmctrl -i -a "$WIN"
      send_right "$WIN"
      # Leave two audit-render frames plus input-release headroom. Shorter
      # delays intermittently left PPSSPP on the previous identity and shifted
      # every later capture while the animation itself was already frozen.
      interruptible_sleep 0.5
    fi
  done
  UNIQUE_HEADERS=$(sort -u "$HEADER_HASHES" | wc -l)
  rm -f "$HEADER_HASHES"
  if [ "$UNIQUE_HEADERS" -ne "$AUDIT_EFFECT_ANIMATIONS" ]; then
    echo "warning: only $UNIQUE_HEADERS/$AUDIT_EFFECT_ANIMATIONS identity headers were unique" >&2
    AUDIT_FAILED=1
  fi
  [ "$AUDIT_FAILED" -eq 0 ] || exit 1
  {
    echo "commit=$(git -C "$REPO" rev-parse HEAD)"
    echo "backend=$BACKEND"
    echo "manager_effect_transform_animation_count=$AUDIT_EFFECT_ANIMATIONS"
    echo "capture_frame=4"
    echo "visible_transform_samples=34"
    echo "material_hidden_transform_samples=1"
    echo "unique_identity_headers=$UNIQUE_HEADERS"
    echo "eboot_sha256=$(sha256sum "$EBOOT" | awk '{print $1}')"
    echo "pack_sha256=$(sha256sum "$PACK" | awk '{print $1}')"
    echo "captures:"
    sha256sum "$AUDIT_OUT"/effect-animation-*.png | sed "s|$AUDIT_OUT/||"
  } > "$AUDIT_OUT/manifest.txt"
  echo "==> manifest: $AUDIT_OUT/manifest.txt"
  echo "==> effect animation audit: $AUDIT_OUT"
elif [ "$AUDIT_EFFECTS" -gt 0 ]; then
  AUDIT_OUT="$OUT/effect-audit"
  mkdir -p "$AUDIT_OUT"
  rm -f "$AUDIT_OUT"/effect-*.png
  echo "==> capturing $AUDIT_EFFECTS manager effects in source order"
  AUDIT_FAILED=0
  HEADER_HASHES="$(mktemp)"
  for ((i = 0; i < AUDIT_EFFECTS; i++)); do
    printf -v AUDIT_FILE '%s/effect-%02d.png' "$AUDIT_OUT" "$i"
    if TOOL=$(capture "$WIN" "$AUDIT_FILE"); then
      # Object view fits each hierarchy to the centre. Require actual varied
      # pixels there so the one-line HUD cannot make a missing effect pass.
      CENTRE_SD=$(magick "$AUDIT_FILE" -gravity center -crop '60%x76%+0+0' \
        +repage -format '%[fx:standard_deviation]' info: 2>/dev/null || echo 0)
      case "$i" in
        # Source-backed rest-invisible exceptions (RE-173): Ness PK Flash has
        # X/Y scale 1e-5, Samus's entry point has Y scale 1e-5, and Link's
        # spin material starts at primitive alpha zero. Their AObjEvent32
        # scripts make them visible; this static-object audit must verify the
        # authored invisible rest state rather than pretending it is a draw.
        10|28|38)
          if awk -v s="$CENTRE_SD" 'BEGIN { exit !(s >= 0.003) }'; then
            echo "warning: effect $i should be invisible at authored rest" >&2
            AUDIT_FAILED=1
          fi
          ;;
        *)
          if awk -v s="$CENTRE_SD" 'BEGIN { exit !(s < 0.003) }'; then
            echo "warning: effect $i has no measurable central render content" >&2
            AUDIT_FAILED=1
          fi
          ;;
      esac
      magick "$AUDIT_FILE" -gravity north -crop '100%x15%+0+0' +repage \
        -format '%#\n' info: >> "$HEADER_HASHES"
      echo "==> effect $i: $AUDIT_FILE (via $TOOL, centre sd $CENTRE_SD)"
    else
      echo "warning: effect $i capture failed" >&2
      AUDIT_FAILED=1
    fi
    if [ "$i" -lt $((AUDIT_EFFECTS - 1)) ]; then
      wmctrl -i -a "$WIN"
      send_right "$WIN"
      interruptible_sleep 0.25
    fi
  done
  UNIQUE_HEADERS=$(sort -u "$HEADER_HASHES" | wc -l)
  rm -f "$HEADER_HASHES"
  if [ "$UNIQUE_HEADERS" -ne "$AUDIT_EFFECTS" ]; then
    echo "warning: only $UNIQUE_HEADERS/$AUDIT_EFFECTS identity headers were unique" >&2
    AUDIT_FAILED=1
  fi
  [ "$AUDIT_FAILED" -eq 0 ] || exit 1
  {
    echo "commit=$(git -C "$REPO" rev-parse HEAD)"
    echo "backend=$BACKEND"
    echo "manager_effect_count=$AUDIT_EFFECTS"
    echo "unique_identity_headers=$UNIQUE_HEADERS"
    echo "eboot_sha256=$(sha256sum "$EBOOT" | awk '{print $1}')"
    if [ -f "$PACK" ]; then
      echo "pack_sha256=$(sha256sum "$PACK" | awk '{print $1}')"
    else
      echo "pack_sha256=absent"
    fi
    echo "captures:"
    sha256sum "$AUDIT_OUT"/effect-*.png | sed "s|$AUDIT_OUT/||"
  } > "$AUDIT_OUT/manifest.txt"
  echo "==> manifest: $AUDIT_OUT/manifest.txt"
  echo "==> effect audit: $AUDIT_OUT"
elif [ "$AUDIT_ANIMATIONS" -gt 0 ]; then
  AUDIT_OUT="$OUT/animation-audit"
  mkdir -p "$AUDIT_OUT"
  rm -f "$AUDIT_OUT"/animation-*.png
  echo "==> capturing $AUDIT_ANIMATIONS fighter animations in pack order"
  AUDIT_FAILED=0
  HEADER_HASHES="$(mktemp)"
  for ((i = 0; i < AUDIT_ANIMATIONS; i++)); do
    printf -v AUDIT_FILE '%s/animation-%03d.png' "$AUDIT_OUT" "$i"
    if TOOL=$(capture "$WIN" "$AUDIT_FILE"); then
      # Audit builds draw only one identity line above an unobscured model.
      # A varied whole screenshot could therefore be only the HUD; require
      # measurable content in a centred crop where the posed fighter belongs.
      CENTRE_SD=$(magick "$AUDIT_FILE" -gravity center -crop '60%x76%+0+0' \
        +repage -format '%[fx:standard_deviation]' info: 2>/dev/null || echo 0)
      if awk -v s="$CENTRE_SD" 'BEGIN { exit !(s < 0.003) }'; then
        echo "warning: animation $i has no measurable central render content" >&2
        AUDIT_FAILED=1
      fi
      magick "$AUDIT_FILE" -gravity north -crop '100%x15%+0+0' +repage \
        -format '%#\n' info: >> "$HEADER_HASHES"
      echo "==> animation $i: $AUDIT_FILE (via $TOOL, centre sd $CENTRE_SD)"
    else
      echo "warning: animation $i capture failed" >&2
      AUDIT_FAILED=1
    fi
    if [ "$i" -lt $((AUDIT_ANIMATIONS - 1)) ]; then
      wmctrl -i -a "$WIN"
      send_right "$WIN"
      # About fifteen simulation ticks: enough to render a real posed frame,
      # short enough to keep the full 532-entry audit practical.
      interruptible_sleep 0.25
    fi
  done
  UNIQUE_HEADERS=$(sort -u "$HEADER_HASHES" | wc -l)
  rm -f "$HEADER_HASHES"
  if [ "$UNIQUE_HEADERS" -ne "$AUDIT_ANIMATIONS" ]; then
    echo "warning: only $UNIQUE_HEADERS/$AUDIT_ANIMATIONS identity headers were unique" >&2
    AUDIT_FAILED=1
  fi
  [ "$AUDIT_FAILED" -eq 0 ] || exit 1
  {
    echo "commit=$(git -C "$REPO" rev-parse HEAD)"
    echo "backend=$BACKEND"
    echo "fighter_animation_count=$AUDIT_ANIMATIONS"
    echo "unique_identity_headers=$UNIQUE_HEADERS"
    echo "eboot_sha256=$(sha256sum "$EBOOT" | awk '{print $1}')"
    if [ -f "$PACK" ]; then
      echo "pack_sha256=$(sha256sum "$PACK" | awk '{print $1}')"
    else
      echo "pack_sha256=absent"
    fi
    echo "captures:"
    sha256sum "$AUDIT_OUT"/animation-*.png | sed "s|$AUDIT_OUT/||"
  } > "$AUDIT_OUT/manifest.txt"
  echo "==> manifest: $AUDIT_OUT/manifest.txt"
  echo "==> animation audit: $AUDIT_OUT"
elif [ "$AUDIT_STAGES" -gt 0 ]; then
  AUDIT_OUT="$OUT/stage-audit"
  mkdir -p "$AUDIT_OUT"
  rm -f "$AUDIT_OUT"/stage-*.png
  echo "==> capturing $AUDIT_STAGES stages in pack order"
  AUDIT_FAILED=0
  for ((i = 0; i < AUDIT_STAGES; i++)); do
    printf -v AUDIT_FILE '%s/stage-%02d.png' "$AUDIT_OUT" "$i"
    if TOOL=$(capture "$WIN" "$AUDIT_FILE"); then
      echo "==> stage $i: $AUDIT_FILE (via $TOOL)"
    else
      echo "warning: stage $i capture failed" >&2
      AUDIT_FAILED=1
    fi
    if [ "$i" -lt $((AUDIT_STAGES - 1)) ]; then
      # The viewer starts at stage zero and maps keyboard arrows to the PSP
      # D-pad under PPSSPP's default desktop profile. Focus first: sending a
      # key to an unfocused SDL window was observed to be ignored on X11.
      wmctrl -i -a "$WIN"
      send_right "$WIN"
      interruptible_sleep 1
    fi
  done
  [ "$AUDIT_FAILED" -eq 0 ] || exit 1
  {
    echo "commit=$(git -C "$REPO" rev-parse HEAD)"
    echo "backend=$BACKEND"
    echo "stage_count=$AUDIT_STAGES"
    echo "eboot_sha256=$(sha256sum "$EBOOT" | awk '{print $1}')"
    if [ -f "$PACK" ]; then
      echo "pack_sha256=$(sha256sum "$PACK" | awk '{print $1}')"
    else
      echo "pack_sha256=absent"
    fi
    echo "captures:"
    sha256sum "$AUDIT_OUT"/stage-*.png | sed "s|$AUDIT_OUT/||"
  } > "$AUDIT_OUT/manifest.txt"
  echo "==> manifest: $AUDIT_OUT/manifest.txt"
  echo "==> stage audit: $AUDIT_OUT"
elif TOOL=$(capture "$WIN" "$OUT/screenshot.png"); then
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
