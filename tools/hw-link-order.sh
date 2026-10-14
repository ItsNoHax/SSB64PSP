#!/usr/bin/env bash
# Ranks link orders for psp-game on a PSP under PSPLink (RE-500): one saved
# `frame_timer` build relinked with each order (tools/icache/relink.py), run
# in shuffled, interleaved rounds, ranked by each run's busy time (the
# frame outside the vblank wait) over its `prof n=` reports 3..18.
#
# usage:
#   tools/hw-link-order.sh prep SAVE OUT ORDER.txt...
#       relinks SAVE (a `PSP_LINK_SAVE` directory of a `frame_timer` build)
#       with each order into OUT/NAME (NAME: the order's file name without
#       .txt), with the pack linked beside the PRX
#   tools/hw-link-order.sh run OUT SCENE.txt SEED ROUNDS NAME...
#       ROUNDS rounds of every NAME once, shuffled from SEED, with
#       SCENE.txt as each run's capture_scene.txt; appends to
#       OUT/results-PHASE.tsv (PHASE: environment, default "rank", also in
#       each run's log names, so passes do not overwrite each other)
#   tools/hw-link-order.sh rank OUT [REF]
#       the per-scene table of OUT/results-PHASE.tsv
#       (tools/icache/hwrank.py rank)
#
# OUT must be inside the repository: PSPLink's host0: is the repository
# root (usbhostfs_pc). The ms0: pack can be older than the build's, so the
# PRX reads the repository's pack through a link beside it (RE-498). Each
# run: ldstart, wait for 19 reports (240 s at most), a bare `reset` (a
# `kill` can hang PSPLink, RE-479) and a settle delay. Stops with exit 1 at
# the first run with fewer than 19 reports: PSPLink may need a person.
# DRY_RUN=1 prints the runs without touching the PSP.
set -u
REPO=$(cd "$(dirname "$0")/.." && pwd)
PACK=$REPO/assets/generated/ssb64.pak
SETTLE=${SETTLE:-10}
PHASE=${PHASE:-rank}
REPORTS=19
die() { echo "hw-link-order: $*" >&2; exit 1; }

prep() {
    local save=$1 out=$2; shift 2
    [ -f "$PACK" ] || die "no pack at $PACK"
    [ -f "$save/args.json" ] || die "$save is not a PSP_LINK_SAVE directory"
    mkdir -p "$out"
    local order name key
    for order in "$@"; do
        name=$(basename "$order" .txt)
        key=$(cat "$order" "$save/args.json" "$save"/*.o | sha256sum | cut -c1-16)
        if [ "$(cat "$out/$name/order.key" 2>/dev/null)" != "$key" ]; then
            python3 "$REPO/tools/icache/relink.py" "$save" "$order" "$out/$name" || die "relink $name failed"
            cp -f "$order" "$out/$name/link-order.txt"
            echo "$key" > "$out/$name/order.key"
        fi
        ln -sfn "$(realpath --relative-to="$out/$name" "$PACK")" "$out/$name/ssb64.pak"
        echo "$name $(sha256sum "$out/$name/ssb64-psp-game.prx" | cut -c1-16)"
    done
}

# Blocks until LOG has N `prof n=` lines or the deadline passes; tail -F
# follows the file by inotify.
wait_reports() {
    local log=$1 n=$2 secs=$3 count=0 line
    coproc TAILER { exec timeout "$secs" tail -n +1 -F "$log" 2>/dev/null; }
    local pid=$TAILER_PID
    while IFS= read -r line <&"${TAILER[0]}"; do
        case $line in "prof n="*) count=$((count + 1)); [ $count -ge "$n" ] && break ;; esac
    done
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
}

run_one() {
    local out=$1 scene_file=$2 round=$3 name=$4
    local scene d rel tag res
    scene=$(basename "$scene_file" .txt)
    d=$out/$name
    rel=$(realpath --relative-to="$REPO" "$d")
    tag=$PHASE-$scene-r$round
    [ -f "$d/ssb64-psp-game.prx" ] || die "no PRX in $d (run prep)"
    [ -e "$d/ssb64.pak" ] || die "the pack link in $d is broken"
    cp -f "$scene_file" "$d/capture_scene.txt"
    : > "$d/profile.log"
    printf 'cd ms0:/PSP/GAME/ssb64/\nldstart host0:/%s/ssb64-psp-game.prx\n' "$rel" \
        | timeout 20 pspsh > "$d/pspsh-$tag.log" 2>&1
    wait_reports "$d/profile.log" "$REPORTS" 240
    timeout 10 pspsh -e exlist < /dev/null > "$d/exlist-$tag.txt" 2>&1
    timeout 15 pspsh -e reset < /dev/null >> "$d/pspsh-$tag.log" 2>&1
    cp -f "$d/profile.log" "$d/profile-$tag.log"
    res=$(python3 "$REPO/tools/icache/hwrank.py" summ "$d/profile-$tag.log")
    set -- $res
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$round" "$scene" "$name" "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$(date +%H:%M:%S)" >> "$out/results-$PHASE.tsv"
    echo "r$round $scene $name reports=$1 fps=$2 frame=$3 max=$4 busy=$5 busy_max=$6 bfps=$7"
    [ "$1" -ge "$REPORTS" ] || die "$name: $1 reports (see $d/pspsh-$tag.log, $d/exlist-$tag.txt); stopping"
    sleep "$SETTLE"
}

run() {
    local out=$1 scene_file=$2 seed=$3 rounds=$4; shift 4
    [ -f "$scene_file" ] || die "no scene file $scene_file"
    case $(realpath "$out") in "$REPO"/*) ;; *) die "$out is not inside $REPO (host0:)" ;; esac
    [ -f "$out/results-$PHASE.tsv" ] || printf 'round\tscene\tname\treports\tfps\tframe_us\tmax_us\tbusy_us\tbusy_max_us\tbfps\ttime\n' > "$out/results-$PHASE.tsv"
    echo "seed $seed rounds $rounds scene $(basename "$scene_file" .txt): $(tr '\n' ' ' < "$scene_file")"
    local round name
    python3 "$REPO/tools/icache/hwrank.py" plan "$seed" "$rounds" "$@" | while read -r round name; do
        if [ -n "${DRY_RUN:-}" ]; then
            echo "r$round $name"
        else
            run_one "$out" "$scene_file" "$round" "$name" || exit 1
        fi
    done
}

cmd=${1:-}
[ $# -gt 0 ] && shift
case $cmd in
    prep) [ $# -ge 3 ] || die "usage: prep SAVE OUT ORDER.txt..."; prep "$@" ;;
    run) [ $# -ge 5 ] || die "usage: run OUT SCENE.txt SEED ROUNDS NAME..."; run "$@" ;;
    rank) [ $# -ge 1 ] || die "usage: rank OUT [REF]"; python3 "$REPO/tools/icache/hwrank.py" rank "$1/results-$PHASE.tsv" "${2:-}" ;;
    *) sed -n '2,31p' "$0"; exit 2 ;;
esac
