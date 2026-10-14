#!/usr/bin/env bash
# summ.sh LOG... : mean of each key's first value over `prof n=` reports
# 3..18 of a profile or frame_timer log (RE-499, RE-500); reports 1-2 hold
# the scene's loading. KEYS overrides the keys.
K=${KEYS:-fps frame cpu draw vblank dstage dfighters dhud dmagnify skin material submit nodemat billboard compose}
for f in "$@"; do grep "^prof n=\([3-9]\|1[0-8]\) " $f | awk -v F=$(basename $f) -v K="$K" '{for(i=3;i<=NF;i++){split($i,a,"=");split(a[2],b,"/");s[a[1]]+=b[1]} n++} END{split(K,k," "); printf "%-16s n=%d", F, n; for(j=1;j in k;j++) printf " %s=%.*f", k[j], (k[j]=="fps"?2:0), s[k[j]]/n; printf "\n"}'; done
