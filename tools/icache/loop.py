#!/usr/bin/env python3
# The code one iteration of a loop touches (RE-482): from a block trace
# (ppsspp-icache.patch, PPSSPP_ICACHE_TRACE), the distinct 64-byte lines run
# between consecutive entries of FUNCTION inside the phase, by function.
# usage: loop.py trace.bin elf FUNCTION > loop.txt
import array, bisect, collections, os, subprocess, sys

BASE = 0x08804000
raw = array.array('I')
with open(sys.argv[1], 'rb') as f:
    raw.frombytes(f.read())
nm = subprocess.run([os.environ.get('PSP_NM', 'psp-nm'), '-C', '-S', '--defined-only', sys.argv[2]],
                    capture_output=True, text=True).stdout
syms = []
for l in nm.splitlines():
    p = l.split(' ', 3)
    if len(p) == 4 and p[2] in 'tTwW':
        syms.append((int(p[0], 16), int(p[1], 16), p[3]))
syms.sort()
A = [s[0] for s in syms]


def fn(line):
    i = bisect.bisect_right(A, line * 64 - BASE + 32) - 1
    return syms[i][2] if i >= 0 else '?'


entry = BASE + [s[0] for s in syms if s[2] == sys.argv[3]][0]
iters, tot, started = 0, 0, False
per = collections.Counter()
cur = set()
for k in range(0, len(raw), 2):
    addr, w = raw[k], raw[k + 1]
    if not (w >> 16) & 1:
        continue
    if addr == entry:
        if started:
            iters += 1
            tot += len(cur)
            for l in cur:
                per[fn(l)] += 1
        started, cur = True, set()
    a, words = addr & 0x3FFFFFFF, w & 0xFFFF
    for l in range(a >> 6, ((a + max(words, 1) * 4 - 1) >> 6) + 1):
        cur.add(l)
print(f"iterations {iters}, mean unique lines/iter {tot / iters:.0f} = {64 * tot / iters / 1024:.1f} KB")
for f, c in per.most_common(40):
    print(f"{64 * c / iters:7.0f}B {f}")
