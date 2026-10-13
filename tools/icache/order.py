#!/usr/bin/env python3
# Writes psp-game/link-order.txt (read by tools/psp-link.py, RE-482): the
# functions of LOOP (loop.py's output; those taking at least 28 bytes an
# iteration) in its order, then every other function the I-cache model saw
# run, by line fetches. Names are v0-mangled with the crate hashes removed.
# usage: order.py elf dump.txt loop.txt > psp-game/link-order.txt
import bisect, collections, os, re, subprocess, sys

BASE = 0x08804000
elf, dump, loop = sys.argv[1:4]


def nm(demangle):
    out = subprocess.run([os.environ.get('PSP_NM', 'psp-nm'), '-S', '--defined-only'] + (['-C'] if demangle else []) + [elf],
                         capture_output=True, text=True).stdout
    r = []
    for l in out.splitlines():
        p = l.split(' ', 3)
        if len(p) == 4 and p[2] in 'tTwW':
            r.append((int(p[0], 16), int(p[1], 16), p[3]))
    return r


syms = sorted(nm(True))
mangled = {a: n for a, s, n in nm(False)}
A = [s[0] for s in syms]
fetch = collections.Counter()
for l in open(dump):
    if l[0] == '#':
        continue
    v = l.split()
    i = bisect.bisect_right(A, int(v[0], 16) - BASE + 32) - 1
    if i >= 0:
        fetch[syms[i][0]] += int(v[2])
loop_names = []
for l in open(loop):
    if l.startswith('iterations'):
        continue
    size, _, name = l.strip().partition('B ')
    if int(size) >= 28:
        loop_names.append(name)
order, seen = [], set()
for n in loop_names:
    for a, s, dn in syms:
        if dn == n and a not in seen:
            order.append(a)
            seen.add(a)
for a, c in fetch.most_common():
    if a not in seen and c > 0:
        order.append(a)
        seen.add(a)
print('# Function order for tools/psp-link.py (RE-482), from tools/icache/order.py.')
for a in order:
    print(re.sub(r'Cs[0-9A-Za-z]*_(?=\d)', 'C', mangled[a]))
