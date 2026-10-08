#!/usr/bin/env bash
# Drive the original N64 ROM headless, frame-exact, through Mupen64Plus
# inside the M64Py flatpak sandbox. See .claude/skills/n64-emulator/SKILL.md.
#
#   tools/run-n64-headless.sh [--no-build] [--visible] [--rom PATH]
#                              [--frames N] [--route SPEC]
#                              [--screenshot-every N] [--final-screenshot]
#                              [--audio-dump PATH]
#
# --route/--screenshot-every/--final-screenshot/--audio-dump are forwarded
# to n64_driver.py verbatim; see that file's --help for the route grammar.
# --audio-dump swaps audio-sdl for n64_audio_dump.so and writes the game's
# exact AI PCM (raw s16le stereo, L,R) to PATH plus a sidecar PATH.txt with
# the DAC rate and per-buffer video-frame/length records. PATH must be
# writable inside the sandbox (e.g. under ~/ppsspp-test; the flatpak's /tmp
# is private). The game's Mono/Stereo option comes from the shared SRAM save
# the M64Py core always loads, which is set to Mono; for a stereo reference
# use n64-headless/record_audio_ref.py --force-stereo.
#
# mupen64plus-video-rice creates its render surface via SDL2, not Qt --
# M64Py's GUI is never launched (this drives the Core API directly). SDL2
# auto-detects Wayland over X11 when both are available and creates a real,
# visible, focusable window on the real desktop regardless of $DISPLAY -- an
# Xvfb/DISPLAY override does NOT stop this (confirmed: SDL ignores it and
# uses the sandbox's always-present Wayland socket instead). The only
# reliable fix is forcing SDL's `offscreen` video driver
# (`--env=SDL_VIDEODRIVER=offscreen`, set below), which creates no window at
# all -- confirmed no window appears (`wmctrl -l` before/after) while
# screenshot capture and RDRAM access still work normally. --visible drops
# this to render on the real desktop instead (useful for interactively
# watching a route play out).
#
# Requires: net.sourceforge.m64py.M64Py installed as a user flatpak, with
# filesystem access to this repo (read-only is enough) and to a writable
# scratch dir for screenshots/config (~/ppsspp-test by convention; grant with
# `flatpak override --user --filesystem=<dir> net.sourceforge.m64py.M64Py`
# if missing).

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLDIR="$REPO/tools/n64-headless"
ROM="$REPO/rom/Super Smash Bros. (USA).z64"
BUILD=1
SDL_DRIVER=offscreen
DRIVER_ARGS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    --visible) SDL_DRIVER=""; shift ;;
    --rom) ROM="$2"; shift 2 ;;
    *) DRIVER_ARGS+=("$1"); shift ;;
  esac
done

[ -f "$ROM" ] || { echo "ROM not found: $ROM" >&2; exit 1; }
flatpak info net.sourceforge.m64py.M64Py >/dev/null 2>&1 || {
  echo "flatpak net.sourceforge.m64py.M64Py is not installed" >&2
  exit 1
}

SO="$TOOLDIR/build/n64_input.so"
AUDIO_SO="$TOOLDIR/build/n64_audio_dump.so"
if [ "$BUILD" = 1 ]; then
  mkdir -p "$TOOLDIR/build"
  for name in n64_input n64_audio_dump; do
    out="$TOOLDIR/build/$name.so"
    if [ ! -f "$out" ] || [ "$TOOLDIR/$name.c" -nt "$out" ]; then
      echo "==> building $name.so" >&2
      gcc -shared -fPIC -O2 -I"$TOOLDIR/include" "$TOOLDIR/$name.c" -o "$out"
    fi
  done
fi
[ -f "$SO" ] || { echo "n64_input.so not found and --no-build given: $SO" >&2; exit 1; }

ENV_ARGS=()
[ -n "$SDL_DRIVER" ] && ENV_ARGS=(--env=SDL_VIDEODRIVER="$SDL_DRIVER")

exec flatpak run "${ENV_ARGS[@]}" --command=python3 net.sourceforge.m64py.M64Py \
  "$TOOLDIR/n64_driver.py" --rom "$ROM" --input-so "$SO" --audio-dump-so "$AUDIO_SO" \
  "${DRIVER_ARGS[@]}"
