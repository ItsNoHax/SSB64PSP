#!/usr/bin/env bash
# Verify or rebaseline golden screenshots from tests/golden/scenes.tsv.
#
#   tools/golden.sh verify     [--filter REGEX] [-j N] [--twice] [--no-build]
#   tools/golden.sh rebaseline [--filter REGEX] [-j N] [--no-build] --reason TEXT
#
# Builds each crate the selected rows need once with `golden_capture`, then
# captures every scene from that one EBOOT in parallel (default: nproc jobs).
# Each scene's EBOOT reads its scene from capture_scene.txt and exits after
# its screenshot (docs/visual-regression/README.md).
#
# Output: target/golden-run/<timestamp>/
#   candidates/<golden>.png   this run's captures
#   masks/<golden>.png        white where the candidate differs from its golden
#   summary.tsv               one row per scene
#   index.html                side-by-side review, changed scenes first
#
# verify exits non-zero on any unexpected difference: a `pass` row that
# differs, a `known-failing` row that now matches (update the manifest), a
# failed capture, or (with --twice) two captures of one scene that differ.
#
# rebaseline always captures twice, then copies each changed candidate over
# its golden. Unchanged goldens are never touched. `known-failing` rows are
# only rebaselined when --filter is given. It prints a Markdown table for the
# evidence record.

set -euo pipefail
# Decimal points in timings and a stable sort order.
export LC_ALL=C

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$REPO/tests/golden/scenes.tsv"
# shellcheck source=lib/pixel-diff.sh
. "$REPO/tools/lib/pixel-diff.sh"

usage() {
  sed -n '2,5p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

# Internal: one capture, run by xargs. Arguments: run dir, golden, crate,
# pass number, scene spec.
if [ "${1:-}" = __capture ]; then
  run="$2" golden="$3" crate="$4" pass="$5" spec="$6"
  job="golden-$golden-$pass"
  dest="$run/candidates"
  [ "$pass" = 2 ] && dest="$run/candidates-2"
  log="$run/logs/$golden-$pass.log"
  start=$(date +%s.%N)
  status=ok
  if ! PPSSPP_HEADLESS_TEST_DIR="$run/jobs" \
      "$REPO/tools/run-ppsspp-headless.sh" --no-build --crate "$crate" \
      --scene "$spec" --job "$job" > "$log" 2>&1; then
    status=capture-failed
  else
    cp -f "$run/jobs/$job/screenshot.png" "$dest/$golden.png"
  fi
  end=$(date +%s.%N)
  printf '%s\t%s\t%s\t%s\n' "$golden" "$pass" "$status" \
    "$(awk -v a="$start" -v b="$end" 'BEGIN { printf "%.1f", b - a }')" \
    > "$run/status/$golden-$pass.tsv"
  # Each job stages a 6 MB EBOOT; remove its memstick directory and its
  # output directory once the candidate is copied out.
  memstick_name=ssb64_regression_$job
  [ "$crate" = psp-game ] && memstick_name=ssb64_game_regression_$job
  rm -rf -- "${PPSSPP_MEMSTICK_DIR:-$HOME/.ppsspp/PSP/GAME}/$memstick_name"
  [ "$status" = ok ] && rm -rf -- "${run:?}/jobs/$job"
  exit 0
fi

MODE="${1:-}"
case "$MODE" in
  verify|rebaseline) shift ;;
  *) usage ;;
esac

FILTER=
JOBS=$(nproc)
TWICE=0
BUILD=1
REASON=
while [ $# -gt 0 ]; do
  case "$1" in
    --filter) FILTER="$2"; shift 2 ;;
    -j) JOBS="$2"; shift 2 ;;
    -j*) JOBS="${1#-j}"; shift ;;
    --twice) TWICE=1; shift ;;
    --no-build) BUILD=0; shift ;;
    --reason) REASON="$2"; shift 2 ;;
    *) echo "unknown option: $1" >&2; usage ;;
  esac
done
if [ "$MODE" = rebaseline ]; then
  [ -n "$REASON" ] || { echo "rebaseline requires --reason TEXT" >&2; exit 2; }
  TWICE=1
fi
case "$JOBS" in ''|*[!0-9]*|0) echo "-j needs a positive integer" >&2; exit 2 ;; esac
command -v magick >/dev/null || { echo "missing required tool: magick (ImageMagick)" >&2; exit 2; }

# ---- manifest --------------------------------------------------------------
goldens=() crates=() specs=() statuses=()
while IFS=$'\t' read -r golden crate spec status _evidence; do
  case "$golden" in ''|'#'*) continue ;; esac
  [ "$golden" = golden ] && continue
  case "$status" in pass|known-failing) ;; *)
    echo "$MANIFEST: $golden: status must be pass or known-failing, not '$status'" >&2; exit 2 ;;
  esac
  if [ -n "$FILTER" ] && ! grep -Eq -- "$FILTER" <<<"$golden"; then
    continue
  fi
  goldens+=("$golden") crates+=("$crate") specs+=("$spec") statuses+=("$status")
done < "$MANIFEST"
[ ${#goldens[@]} -gt 0 ] || { echo "no manifest rows match '${FILTER}'" >&2; exit 2; }

RUN="$REPO/target/golden-run/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RUN"/{candidates,candidates-2,masks,logs,status,jobs}
echo "==> $MODE: ${#goldens[@]} scenes, -j $JOBS, run dir $RUN"
run_start=$(date +%s)

# ---- build once per crate --------------------------------------------------
if [ "$BUILD" = 1 ]; then
  for crate in $(printf '%s\n' "${crates[@]}" | sort -u); do
    echo "==> building $crate (golden_capture)"
    ( cd "$REPO/$crate" && cargo psp --release --features golden_capture ) \
      > "$RUN/logs/build-$crate.log" 2>&1 || {
      echo "build failed: $RUN/logs/build-$crate.log" >&2; exit 1; }
  done
fi

# ---- capture in parallel ---------------------------------------------------
capture_start=$(date +%s)
passes=1
[ "$TWICE" = 1 ] && passes="1 2"
{
  for pass in $passes; do
    for i in "${!goldens[@]}"; do
      printf '%s\0%s\0%s\0%s\0%s\0' "$RUN" "${goldens[$i]}" "${crates[$i]}" \
        "$pass" "${specs[$i]}"
    done
  done
} | xargs -0 -n 5 -P "$JOBS" sh -c '"$0" __capture "$1" "$2" "$3" "$4" "$5"' \
    "$REPO/tools/golden.sh"
capture_seconds=$(( $(date +%s) - capture_start ))
build_seconds=$(( capture_start - run_start ))

# ---- compare ---------------------------------------------------------------
printf 'golden\tcrate\tscene_spec\tstatus\tpixels\ttwice_pixels\tresult\tcapture_seconds\n' \
  > "$RUN/summary.tsv"
failures=0
for i in "${!goldens[@]}"; do
  golden=${goldens[$i]} status=${statuses[$i]}
  candidate="$RUN/candidates/$golden.png"
  pixels=- twice=- seconds=-
  if [ -f "$RUN/status/$golden-1.tsv" ]; then
    seconds=$(cut -f4 "$RUN/status/$golden-1.tsv")
  fi
  if [ ! -f "$candidate" ]; then
    result=capture-failed
  elif ! pixels=$(pixel_diff_count "$REPO/tests/golden/$golden.png" "$candidate"); then
    pixels=- result=size-mismatch
  else
    if [ "$pixels" -gt 0 ]; then
      pixel_diff_mask "$REPO/tests/golden/$golden.png" "$candidate" "$RUN/masks/$golden.png"
    fi
    case "$status:$pixels" in
      pass:0) result=match ;;
      pass:*) result=changed ;;
      known-failing:0) result=now-matches ;;
      known-failing:*) result=known-failing ;;
    esac
  fi
  if [ "$TWICE" = 1 ] && [ -f "$candidate" ]; then
    if [ ! -f "$RUN/candidates-2/$golden.png" ]; then
      result=capture-failed
    elif ! twice=$(pixel_diff_count "$candidate" "$RUN/candidates-2/$golden.png") \
        || [ "$twice" != 0 ]; then
      result=nondeterministic
    fi
  fi
  case "$result" in
    match|known-failing) ;;
    changed) [ "$MODE" = rebaseline ] || failures=$((failures + 1)) ;;
    *) failures=$((failures + 1)) ;;
  esac
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$golden" "${crates[$i]}" "${specs[$i]}" \
    "$status" "$pixels" "$twice" "$result" "$seconds" >> "$RUN/summary.tsv"
done

# ---- report ----------------------------------------------------------------
html_escape() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }
{
  cat <<'HTML'
<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Golden Review</title>
<style>
:root { --bg: #fff; --fg: #1a1a1a; --muted: #666; --line: #ddd; --bad: #b3261e; --ok: #1e6b35; --warn: #8a5a00; }
@media (prefers-color-scheme: dark) { :root { --bg: #151515; --fg: #e8e8e8; --muted: #999; --line: #333; --bad: #f28b82; --ok: #81c995; --warn: #fdd663; } }
body { background: var(--bg); color: var(--fg); font: 14px/1.4 system-ui, sans-serif; margin: 0 auto; max-width: 1500px; padding: 16px; }
h1 { font-size: 20px; } h2 { font-size: 16px; margin: 24px 0 8px; }
.scene { border-top: 1px solid var(--line); padding: 12px 0; }
.meta { color: var(--muted); } .meta b { color: var(--fg); }
.r-changed, .r-nondeterministic, .r-capture-failed, .r-size-mismatch, .r-now-matches { color: var(--bad); }
.r-known-failing { color: var(--warn); } .r-match { color: var(--ok); }
.row { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin-top: 8px; }
.row figure { margin: 0; } .row img { width: 100%; image-rendering: pixelated; border: 1px solid var(--line); }
figcaption { color: var(--muted); font-size: 12px; }
table { border-collapse: collapse; } td, th { padding: 2px 10px 2px 0; text-align: left; }
@media (max-width: 700px) { .row { grid-template-columns: 1fr; } }
</style></head><body>
HTML
  echo "<h1>Golden review: $MODE</h1>"
  echo "<p class=meta>$(date -Is) &middot; commit $(git -C "$REPO" rev-parse --short HEAD)$(git -C "$REPO" diff --quiet || echo '+dirty') &middot; ${#goldens[@]} scenes &middot; -j $JOBS &middot; build ${build_seconds}s &middot; captures ${capture_seconds}s &middot; unexpected: <b>$failures</b></p>"
  # Changed scenes first, largest pixel count first; everything else below.
  echo "<h2>Differing or failed</h2>"
  # A failed capture has no pixel count; sort it above every count.
  tail -n +2 "$RUN/summary.tsv" | awk -F'\t' '$7 != "match"' \
    | awk -F'\t' '{ print ($5 == "-" ? 1e12 : $5) "\t" $0 }' \
    | sort -t$'\t' -k1,1gr | cut -f2- \
    | while IFS=$'\t' read -r golden crate spec status pixels twice result seconds; do
        echo "<div class=scene id=\"$golden\"><div><b>$golden</b> <span class=\"r-$result\">$result</span></div>"
        echo "<div class=meta>$crate &middot; scene: $(printf '%s' "$spec" | html_escape) &middot; manifest: $status &middot; pixels: $pixels &middot; twice: $twice &middot; ${seconds}s</div>"
        echo "<div class=row>"
        echo "<figure><img loading=lazy src=\"../../../tests/golden/$golden.png\" alt=\"\"><figcaption>golden</figcaption></figure>"
        echo "<figure><img loading=lazy src=\"candidates/$golden.png\" alt=\"\"><figcaption>candidate</figcaption></figure>"
        if [ -f "$RUN/masks/$golden.png" ]; then
          echo "<figure><img loading=lazy src=\"masks/$golden.png\" alt=\"\"><figcaption>difference mask</figcaption></figure>"
        fi
        echo "</div></div>"
      done
  echo "<details><summary><h2 style=\"display:inline\">Unchanged ($(awk -F'\t' 'NR > 1 && $7 == "match"' "$RUN/summary.tsv" | wc -l))</h2></summary><table>"
  tail -n +2 "$RUN/summary.tsv" | awk -F'\t' '$7 == "match"' \
    | while IFS=$'\t' read -r golden _crate spec _status _pixels _twice _result seconds; do
        echo "<tr><td><a href=\"candidates/$golden.png\">$golden</a></td><td class=meta>$(printf '%s' "$spec" | html_escape)</td><td class=meta>${seconds}s</td></tr>"
      done
  echo "</table></details></body></html>"
} > "$RUN/index.html"

awk -F'\t' 'NR == 1 || $7 != "match"' "$RUN/summary.tsv" | column -t -s$'\t'
echo "==> $(awk -F'\t' 'NR > 1 && $7 == "match"' "$RUN/summary.tsv" | wc -l) of ${#goldens[@]} match; build ${build_seconds}s, captures ${capture_seconds}s"
echo "==> report: $RUN/index.html"

# ---- rebaseline ------------------------------------------------------------
if [ "$MODE" = rebaseline ]; then
  if [ "$failures" -gt 0 ]; then
    echo "not rebaselining: $failures scene(s) failed to capture or were not deterministic" >&2
    exit 1
  fi
  copied=()
  while IFS=$'\t' read -r golden _crate _spec status pixels _twice result _seconds; do
    case "$result" in
      changed) ;;
      known-failing) [ -n "$FILTER" ] || continue ;;
      *) continue ;;
    esac
    cp -f "$RUN/candidates/$golden.png" "$REPO/tests/golden/$golden.png"
    copied+=("$golden	$pixels	$status")
  done < <(tail -n +2 "$RUN/summary.tsv")
  if [ ${#copied[@]} -eq 0 ]; then
    echo "==> nothing to rebaseline: every selected golden already matches"
    exit 0
  fi
  echo
  echo "| Golden | Differing pixels (2x) | Reason |"
  echo "|---|---:|---|"
  for row in "${copied[@]}"; do
    IFS=$'\t' read -r golden pixels status <<<"$row"
    echo "| \`$golden\` | $pixels | $REASON |"
  done
  echo
  for row in "${copied[@]}"; do
    IFS=$'\t' read -r golden _pixels status <<<"$row"
    [ "$status" = known-failing ] && \
      echo "note: $golden was known-failing; set its status to pass in tests/golden/scenes.tsv" >&2
  done
  exit 0
fi

[ "$failures" -eq 0 ] || { echo "FAIL: $failures unexpected result(s)" >&2; exit 1; }
echo "PASS"
