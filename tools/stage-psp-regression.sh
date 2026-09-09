#!/usr/bin/env bash
# Build and stage the deterministic R0 regression scene on a mounted PSP.
#
# Usage:
#   tools/stage-psp-regression.sh <psp-mount-root> [rom]
#
# The mount root must contain PSP/GAME. The optional ROM defaults to the
# repository's normal USA ROM path. ROM-derived data is generated locally and
# is never copied anywhere except the requested PSP destination.

set -euo pipefail

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
  echo "usage: $0 <psp-mount-root> [rom]" >&2
  exit 2
fi

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MOUNT_ROOT="$(realpath "$1")"
ROM="${2:-$REPO/rom/Super Smash Bros. (USA).z64}"
GAME_ROOT="$MOUNT_ROOT/PSP/GAME"
DEST="$GAME_ROOT/ssb64"
PACK="$REPO/assets/generated/ssb64.pak"
EBOOT="$REPO/psp/target/mipsel-sony-psp/release/EBOOT.PBP"

[ -d "$GAME_ROOT" ] || {
  echo "not a PSP mount root (missing $GAME_ROOT)" >&2
  exit 2
}
[ -f "$ROM" ] || { echo "ROM not found: $ROM" >&2; exit 2; }

echo "==> rebuilding asset pack from the verified local ROM"
(cd "$REPO" && cargo run --release -p romtool -- pack "$ROM")

echo "==> rebuilding deterministic regression EBOOT"
rm -f "$EBOOT"
(cd "$REPO/psp" && cargo psp --release --features regression_capture)
[ -s "$EBOOT" ] || { echo "build did not produce $EBOOT" >&2; exit 1; }
[ -s "$PACK" ] || { echo "pack build did not produce $PACK" >&2; exit 1; }

mkdir -p "$DEST"
install -m 0644 "$EBOOT" "$DEST/EBOOT.PBP"
install -m 0644 "$PACK" "$DEST/ssb64.pak"

{
  echo "git_commit=$(git -C "$REPO" rev-parse HEAD)"
  echo "build=release+regression_capture"
  echo "generated_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  sha256sum "$DEST/EBOOT.PBP" "$DEST/ssb64.pak" | sed "s|$DEST/||"
} > "$DEST/regression-manifest.txt"

sync "$DEST"
echo "==> staged deterministic capture build in $DEST"
sed 's/^/    /' "$DEST/regression-manifest.txt"
echo "==> safely eject the PSP, run SSB64PSP, and wait at least 5 seconds before capture"
