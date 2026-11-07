#!/usr/bin/env bash
# Build the PSP release folder: the game and its first-launch installer.
#
# Usage:
#   tools/psp-release.sh [OUT]
#
# OUT defaults to target/psp-release/ssb64. It receives EBOOT.PBP (the
# game, psp-game) and INSTALL.PBP (the installer, psp-installer): copy the
# folder to ms0:/PSP/GAME/ with the user's ROM beside them. The game
# LoadExecs INSTALL.PBP from its own folder when it finds no valid
# ssb64.pak (psp-runtime/src/launch.rs, RE-588). No pack and no ROM data
# go in the folder (D-048); run tools/rom_data_scan.py on the builds.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$REPO/target/psp-release/ssb64}"

for crate in psp-game psp-installer; do
  # Unset rather than empty: any RUSTFLAGS hides `.cargo/config.toml`'s
  # link arguments (RE-471).
  (cd "$REPO/$crate" && env -u RUSTFLAGS cargo psp --release)
done

mkdir -p "$OUT"
cp "$REPO/psp-game/target/mipsel-sony-psp/release/EBOOT.PBP" "$OUT/EBOOT.PBP"
cp "$REPO/psp-installer/target/mipsel-sony-psp/release/EBOOT.PBP" "$OUT/INSTALL.PBP"
ls -l "$OUT"
