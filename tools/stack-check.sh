#!/usr/bin/env bash
# Measure the game thread's deepest stack use in scenes outside the golden
# matrix (RE-469), from a `golden_capture` build's capture-log `stack` line.
#
#   tools/stack-check.sh [--no-build] [-j N] [SCENE...]
#
# Defaults to the 1P Game and opening scenes, which the golden matrix does
# not capture. Fails when a scene's peak comes within an eighth of the
# stack (`GAME_STACK_BYTES`), or when a scene logs no stack line. PPSSPP
# does not enforce the bound a PSP does, so this log line is the witness.

set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD=1
JOBS=8
scenes=()
while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) BUILD=0; shift ;;
    -j) JOBS="$2"; shift 2 ;;
    *) scenes+=("$1"); shift ;;
  esac
done
if [ ${#scenes[@]} -eq 0 ]; then
  scenes=(onepgame onepbonus onepplatforms oneprace onepboss onepmetal onepgiant onepzako
          onepending onepfinale op-jungle op-yoster op-sector explain autodemo)
fi
if [ "$BUILD" = 1 ]; then
  (cd "$REPO/psp-game" && cargo psp --release --features golden_capture) > /dev/null
fi
OUT="$REPO/target/stack-check"
rm -rf "$OUT"; mkdir -p "$OUT"
printf '%s\n' "${scenes[@]}" | xargs -P "$JOBS" -I{} sh -c '
  job="stack-$(echo "{}" | tr "@" "_")"
  PPSSPP_HEADLESS_TEST_DIR="$1/jobs" "$0/tools/run-ppsspp-headless.sh" --no-build \
    --crate psp-game --scene "{}" --job "$job" --log --seconds 300 > "$1/$job.log" 2>&1 || true
  grep -a -o "stack tick=[0-9]* peak=[0-9]* size=[0-9]*" "$1/jobs/$job/ppsspp-headless.log" \
    > "$1/$job.stack" 2>/dev/null || true
  rm -rf "${PPSSPP_MEMSTICK_DIR:-$HOME/.ppsspp/PSP/GAME}/ssb64_game_regression_$job"
' "$REPO" "$OUT"
fail=0
for s in "${scenes[@]}"; do
  f="$OUT/stack-$(echo "$s" | tr @ _).stack"
  line=$(tail -n 1 "$f" 2>/dev/null || true)
  if [ -z "$line" ]; then
    echo "FAIL $s: no stack line"; fail=1; continue
  fi
  peak=$(sed -n 's/.*peak=\([0-9]*\).*/\1/p' <<<"$line")
  size=$(sed -n 's/.*size=\([0-9]*\).*/\1/p' <<<"$line")
  if [ $((peak * 8)) -gt $((size * 7)) ]; then
    echo "FAIL $s: $line"; fail=1
  else
    echo "ok   $s: $line"
  fi
done
exit $fail
