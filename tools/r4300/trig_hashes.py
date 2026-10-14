#!/usr/bin/env python3
"""Hashes the ROM's own trig functions over trig_golden.rs's inputs.

    python3 -I tools/r4300/trig_hashes.py [--rom PATH] [ArcTan ArcTan2 ArcSin ArcCos sinf cosf]

Mirrors crates/ssb-engine/src/math/trig_golden.rs (`unary`, `binary`,
FNV-1a with NaN canonicalised) and runs the original machine code in the
R4300 interpreter (r4300.py), which reads the ROM at run time. Prints one
`hash=` per function: the values pinned as trig_golden::HASH_*. About 0.1 ms
per call.
"""
import os, struct, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import r4300
from r4300 import run
M = 0xffffffff
US = [0x00000000,0x80000000,0x3f800000,0x3f7fffff,0x3f800001,0xbf800000,0xbf7fffff,0xbf800001,
      0x3f7fff58,0x3f7fff57,0x3f7fff59,0xbf7fff58,0xbf7fff57,0xbf7fff59,0x7f800000,0xff800000,
      0x7f7fffff,0xff7fffff,0x00800000,0x80800000,0x3fc90fdb,0x40490fdb]
UR, USW, UN = 65536, 65536, 1024
BR, BG = 65536, 33*33
def mix(x):
    x ^= x >> 16; x = (x * 0x7feb352d) & M; x ^= x >> 15; x = (x * 0x846ca68b) & M; x ^= x >> 16; return x
def fb(f): return struct.unpack('<I', struct.pack('<f', f))[0]
def unary(i):
    if i < len(US): return US[i]
    i -= len(US)
    if i < UR:
        b = mix(i); e = (b >> 23) & 0xff
        return b & 0x80000000 if e == 0 else (b ^ 0x40000000 if e == 0xff else b)
    i -= UR
    if i < USW: return fb(i / 4096.0 - 8.0)
    i -= USW
    c = [0x3f800000, 0xbf800000, 0x3f7fff58, 0xbf7fff58][i // 256]
    return (c + i % 256 - 128) & M
def bounded(b):
    if b & 0xf == 0: return b & 0x80000000
    e = 87 + ((b >> 23) & 0xff) % 81
    return (b & 0x807fffff) | (e << 23)
def binary(i):
    if i < BR: return bounded(mix(2*i)), bounded(mix(2*i+1))
    i -= BR
    return fb((i // 33) * 0.25 - 4.0), fb((i % 33) * 0.25 - 4.0)
def fnv(h, bits):
    if ((bits >> 23) & 0xff) == 0xff and bits & 0x7fffff: bits = 0x7fc00000
    for k in range(4):
        h = ((h ^ ((bits >> (8*k)) & 0xff)) * 0x100000001b3) & 0xffffffffffffffff
    return h
OFF = 0xcbf29ce484222325
args = sys.argv[1:]
if '--rom' in args:
    i = args.index('--rom')
    r4300.load_rom(args[i + 1])
    del args[i:i + 2]
names = args or ['ArcTan', 'ArcTan2', 'ArcSin', 'ArcCos']
for name in names:
    t = time.time(); h = OFF; traps = 0
    n = BR + BG if name == 'ArcTan2' else len(US) + UR + USW + UN
    for i in range(n):
        args = binary(i) if name == 'ArcTan2' else (unary(i),)
        r = run(name, *args)
        if r is None:
            traps += 1; print('TRAP', name, i, [hex(a) for a in args]); r = 0x7fc00000
        h = fnv(h, r)
    print(f'{name} n={n} traps={traps} hash={h:#018x} {time.time()-t:.0f}s', flush=True)
