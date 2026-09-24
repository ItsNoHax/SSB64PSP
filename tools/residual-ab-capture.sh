#!/usr/bin/env bash
# RE-312 visual review: A/B captures of section 11 review candidates.
#
#   tools/residual-ab-capture.sh [candidate ...]     (default: all)
#
# Builds, under assets/generated/three-point-visual-review/ (gitignored,
# ROM-derived):
#   packs/ship.pak      the shipped pack (A)
#   packs/c<V>.pak      shipped + review candidate V only (B)
#   packs/h<V>.pak      B with the candidate's cutout alpha zeroed (silhouette probe)
# then captures each candidate's real stage and graph with PPSSPPHeadless
# (software GE, frozen tick, 480x272) and writes per-candidate metrics and
# images with tools/residual-ab-metrics.py. Packs build in parallel.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROM="${SSB64_ROM:-$REPO/rom/Super Smash Bros. (USA).z64}"
OUT="$REPO/assets/generated/three-point-visual-review"
P="$OUT/packs"
mkdir -p "$P" "$OUT/cap" "$OUT/ab"

# variant  scene-kind  scene (stage index or file:graph)
SCENES=(
  "397 stage 5" "397 object 107:0x6C00"
  "1004 stage 2" "1004 object 157:0xB08"
  "404 stage 6" "404 object 108:0x8B18"
  "252 object 86:0x5458"
  "837 stage 31" "837 object 140:0x1CA8"
  "799 stage 29" "799 object 138:0x2998"
  "800 stage 29" "800 object 138:0x2998"
  "902 stage 35" "902 object 144:0x4198"
  "901 stage 35" "901 object 144:0x4198"
  "609 stage 14" "609 object 117:0x1B10"
  "599 stage 13" "599 object 116:0x4170"
)
CUTOUT=" 1004 252 76 "
WANT=" ${*:-397 1004 404 252 837 799 800 902 901 609 599 76} "

cargo build --release -p romtool
ROMTOOL="$REPO/target/release/romtool"
jobs=()
[ -f "$P/ship.pak" ] || { "$ROMTOOL" pack "$ROM" --out "$P/ship.pak" > "$P/ship.log" 2>&1 & jobs+=($!); }
for v in $WANT; do
  [ -f "$P/c$v.pak" ] || { "$ROMTOOL" pack "$ROM" --residual-candidates "v$v" --out "$P/c$v.pak" > "$P/c$v.log" 2>&1 & jobs+=($!); }
  if [[ "$CUTOUT" == *" $v "* ]] && [ ! -f "$P/h$v.pak" ]; then
    "$ROMTOOL" pack "$ROM" --residual-candidates "v$v" --residual-hide-probe --out "$P/h$v.pak" > "$P/h$v.log" 2>&1 & jobs+=($!)
  fi
done
for j in "${jobs[@]}"; do wait "$j"; done

capture() { # id kind value pack...
  local id=$1 kind=$2 value=$3; shift 3
  if [ "$kind" = stage ]; then
    ( cd "$REPO/psp-asset-viewer" && SSB64_STAGE_INDEX=$value cargo psp --release --features regression_capture_stage_index,headless_capture ) > "$OUT/cap/$id-build.log" 2>&1
  else
    ( cd "$REPO/psp-asset-viewer" && SSB64_CAPTURE_OBJECT=$value cargo psp --release --features regression_capture_object,headless_capture ) > "$OUT/cap/$id-build.log" 2>&1
  fi
  for pack in "$@"; do
    local tag; tag=$(basename "$pack" .pak)
    "$REPO/tools/run-ppsspp-headless.sh" --no-build --pack "$pack" > "$OUT/cap/$id-$tag.log" 2>&1
    cp "${PPSSPP_HEADLESS_TEST_DIR:-$HOME/ppsspp-headless-test}/screenshot-native.png" "$OUT/cap/$id-$tag.png"
  done
}

for entry in "${SCENES[@]}"; do
  read -r v kind value <<< "$entry"
  [[ "$WANT" == *" $v "* ]] || continue
  id="v$v-$kind"
  packs=("$P/ship.pak" "$P/c$v.pak")
  [[ "$CUTOUT" == *" $v "* ]] && packs+=("$P/h$v.pak")
  capture "$id" "$kind" "$value" "${packs[@]}"
  hide=()
  [[ "$CUTOUT" == *" $v "* ]] && hide=("$OUT/cap/$id-h$v.png")
  python3 "$REPO/tools/residual-ab-metrics.py" "$id" "$OUT/cap/$id-ship.png" "$OUT/cap/$id-c$v.png" "$OUT/ab" "${hide[@]}"
done
# v76 (52:0x24660) is a loose mesh no pack object graph draws; nothing to capture.
