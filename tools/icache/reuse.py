#!/usr/bin/env python3
# Replay the simulator's block trace: 2-way LRU misses (as the PSP model),
# and each line access's LRU stack distance (distinct lines since its last
# use). usage: reuse.py trace.bin elf
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


# accesses
acc_line = array.array('I')
acc_phase = bytearray()
for k in range(0, len(raw), 2):
    addr, w = raw[k], raw[k + 1]
    words, ph = w & 0xFFFF, (w >> 16) & 1
    a = addr & 0x3FFFFFFF
    first, last = a >> 6, (a + max(words, 1) * 4 - 1) >> 6
    for line in range(first, last + 1):
        acc_line.append(line)
        acc_phase.append(ph)
N = len(acc_line)

# Fenwick over time for stack distance
tree = array.array('i', [0]) * (N + 1)


def add(i, v):
    i += 1
    while i <= N:
        tree[i] += v
        i += i & -i


def pref(i):  # sum [0, i)
    s = 0
    while i > 0:
        s += tree[i]
        i -= i & -i
    return s


last = {}
tags = [[-1, -1] for _ in range(128)]
lru = [0] * 128
dist_draw = collections.Counter()   # draw-phase accesses by distance bucket
miss_draw_dist = collections.Counter()
miss2_draw = 0
miss_fn = collections.Counter()
draw_acc = 0
BUCKET = 16  # lines (1 KB)
INF = 10 ** 9
for t in range(N):
    line = acc_line[t]
    p = last.get(line)
    if p is None:
        d = INF
    else:
        d = pref(t) - pref(p + 1)
        add(p, -1)
    add(t, 1)
    last[line] = t
    s = line & 127
    if tags[s][0] == line:
        way, hit = 0, True
    elif tags[s][1] == line:
        way, hit = 1, True
    else:
        way, hit = lru[s], False
        tags[s][way] = line
    lru[s] = way ^ 1
    if acc_phase[t]:
        draw_acc += 1
        b = min(d // BUCKET, 64)
        dist_draw[b] += 1
        if not hit:
            miss2_draw += 1
            miss_draw_dist[b] += 1
            miss_fn[fn(line)] += 1

print(f"accesses {N}, draw-phase {draw_acc}, 2-way misses in draw {miss2_draw}")
print("fully-assoc LRU draw misses if the cache held S KB (accesses with distance >= S):")
tot = 0
cum = {}
for b in range(64, -1, -1):
    tot += dist_draw[b]
    cum[b] = tot
for kb in (8, 12, 16, 20, 24, 28, 32, 40, 48, 64):
    print(f"  {kb:3d} KB: {cum.get(kb, 0)}")
print("2-way misses in draw by reuse distance (KB):")
for b in range(0, 65):
    if miss_draw_dist[b]:
        print(f"  {b:3d}{'+' if b == 64 else ' '} {miss_draw_dist[b]}")
print("2-way draw misses by function:")
for k, v in miss_fn.most_common(25):
    print(f"  {v:7d} {k[:100]}")
