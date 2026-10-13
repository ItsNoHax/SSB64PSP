#!/usr/bin/env python3
# Symbolise the I-cache simulator's per-line dump against the ELF.
# usage: sym.py dump elf [col] [top]   col: 1 misses, 3 phase misses
import bisect, os, subprocess, sys
from collections import defaultdict

BASE = 0x08804000
dump, elf = sys.argv[1], sys.argv[2]
col = int(sys.argv[3]) if len(sys.argv) > 3 else 3
top = int(sys.argv[4]) if len(sys.argv) > 4 else 40
nm = subprocess.run([os.environ.get('PSP_NM', 'psp-nm'), '-C', '-S', '--defined-only', elf],
                    capture_output=True, text=True).stdout
syms = []
for l in nm.splitlines():
    p = l.split(' ', 3)
    if len(p) == 4 and p[2] in 'tTwW':
        syms.append((int(p[0], 16), int(p[1], 16), p[3]))
syms.sort()
addrs = [s[0] for s in syms]
head = open(dump).readline().split()
hd = dict(zip(head[1::2], map(int, head[2::2])))
frames = hd.get('frames') or 1
per = defaultdict(lambda: [0, 0, 0, 0])  # misses, fetches, phase misses, size
unknown = 0
for l in open(dump):
    if l.startswith('#'):
        continue
    v = l.split()
    a = int(v[0], 16) - BASE
    vals = list(map(int, v[1:]))
    # attribute the 64-byte line to each function overlapping it by bytes
    i = bisect.bisect_right(addrs, a + 63) - 1
    hits = []
    while i >= 0 and syms[i][0] + max(syms[i][1], 1) > a:
        lo, hi = max(a, syms[i][0]), min(a + 64, syms[i][0] + syms[i][1])
        if hi > lo:
            hits.append((hi - lo, syms[i]))
        if syms[i][0] <= a:
            break
        i -= 1
    if not hits:
        unknown += vals[col - 1]
        continue
    tot = sum(h[0] for h in hits)
    for w, s in hits:
        e = per[s[2]]
        e[0] += vals[0] * w / tot
        e[1] += vals[1] * w / tot
        e[2] += vals[2] * w / tot
        e[3] = s[1]
print(f"# frames {frames}; misses/frame {hd['misses']/frames:.0f}; phase misses/frame {hd['phase_misses']/frames:.0f}; "
      f"fetches/frame {hd['fetches']/frames:.0f}; unknown {unknown/frames:.0f}")
key = {1: 0, 2: 1, 3: 2}[col]
rows = sorted(per.items(), key=lambda kv: -kv[1][key])
tot = sum(v[key] for v in per.values())
acc = 0
for name, v in rows[:top]:
    acc += v[key]
    print(f"{v[key]/frames:8.0f} {100*acc/tot:5.1f}% size={v[3]:6d} fetch={v[1]/frames:8.0f} {name[:110]}")
