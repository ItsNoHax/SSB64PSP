"""Runs the ROM's own lbParticleUpdateStruct (lb/lbparticle.c, ovl0
0x800CEF4C) on a vortex particle (LBPARTICLE_FLAG_VORTEX) through the
r4300 interpreter, and prints the hash pinned in
crates/ssb-game/src/particle_tests.rs (`the_vortex_motion_matches_the_rom`).

Each case builds an LBParticle and its kind-2 LBGenerator in memory (no
bytecode wait, no blends, a long lifetime, no transform), runs TICKS
updates, and hashes the f32 bits of pos and vel after every one (FNV-1a
64, as matrix_hashes.py). The inputs sweep the axis angles (the particle's
gravity and friction), its angle, radius fraction and height (vel), and
the generator's spin (gravity), radius (unk_gn_0x38), pitch (unk_gn_0x3C),
rise (vortex.f) and position, including the grhyrule scripts' values.
No ROM bytes are stored here (D-048).

    python3 tools/r4300/vortex_hashes.py [ROM]
"""
import struct, sys
import numpy as np
import r4300
from r4300 import f2b
from matrix_hashes import MtxCPU, FNV_OFFSET, fnv

UPDATE_STRUCT = 0x800CEF4C
PC = 0x80500000
GN = 0x80500100
TICKS = 12
FLAG_VORTEX = 0x4


def inputs():
    f = np.float32
    axes = [(0.0, 0.0), (0.3, 0.698), (-1.2, -0.5), (1.5707964, 2.2), (3.0, -6.0),
            (-3.1415927, 0.1745), (7.0, 1.0)]
    gens = [  # (gravity, unk38, unk3C, vortex.f, pos)
        (0.5, 100.0, 0.698, 25.0, (10.0, -20.0, 30.0)),
        (0.0, 50.0, 0.0, 40.0, (0.0, 0.0, 0.0)),
        (-0.32, -60.0, -0.1745, 12.5, (1000.5, 200.25, -3.0)),
        (0.05, 0.0, 0.3, 0.0, (-512.0, 64.0, 7.5)),
        (3.0, 75.0, 1.2, 7.0, (1.0, 2.0, 3.0)),
    ]
    vels = [(0.0, 1.0, 0.0), (2.5, 0.37, 0.0), (6.0, 0.9, 13.0), (-1.0, 0.5, -4.0)]
    out = []
    for a1, a2 in axes:
        for g in gens:
            for v in vels:
                out.append((f(a1), f(a2), tuple(f(x) for x in v), (f(g[0]), f(g[1]), f(g[2]), f(g[3])),
                            tuple(f(x) for x in g[4])))
    return out


def setup(a1, a2, vel, g, gpos):
    mem = {}
    for i in range(24):
        mem[PC + 4 * i] = 0
        mem[GN + 4 * i] = 0
    mem[PC + 0x04] = FLAG_VORTEX              # generator_id 0, flags
    mem[PC + 0x08] = 0x01000000               # bank_id 1
    mem[PC + 0x1C] = 1000                     # loop_ptr 0, lifetime
    for k in range(3):
        mem[PC + 0x2C + 4 * k] = f2b(vel[k])
    mem[PC + 0x38] = f2b(a1)                  # gravity: the axis angle 1
    mem[PC + 0x3C] = f2b(a2)                  # friction: the axis angle 2
    mem[PC + 0x58] = GN
    mem[GN + 0x08] = 0x02010000               # kind 2, bank_id 1
    for k in range(3):
        mem[GN + 0x14 + 4 * k] = f2b(gpos[k])
    mem[GN + 0x2C] = f2b(g[0])
    mem[GN + 0x38] = f2b(g[1])
    mem[GN + 0x3C] = f2b(g[2])
    mem[GN + 0x50] = f2b(g[3])
    return mem


def main():
    r4300.load_rom(sys.argv[1] if len(sys.argv) > 1 else None)
    cpu = MtxCPU()
    h = FNV_OFFSET
    ins = inputs()
    for n, (a1, a2, vel, g, gpos) in enumerate(ins):
        mem = setup(a1, a2, vel, g, gpos)
        for t in range(TICKS):
            cpu.call_int(UPDATE_STRUCT, [PC, 0, 1], mem)
            mem = cpu.mem
            if cpu.r[2] != 0:
                raise SystemExit(f'case {n}: returned {cpu.r[2]:#x}, want next (NULL)')
            words = [mem[PC + 0x20 + 4 * k] for k in range(6)]
            for w in words:
                h = fnv(h, w - (1 << 32) if w & 0x80000000 else w)
            if n in (0, 37, 101) and t == TICKS - 1:
                print(f'  case {n} tick {t}: pos/vel {[hex(w) for w in words]}')
        if mem[PC + 0x1C] & 0xffff != 1000 - TICKS:
            raise SystemExit(f'case {n}: lifetime {mem[PC + 0x1C] & 0xffff}')
    print(f'vortex: {len(ins)} cases x {TICKS} ticks, hash {h:#018x}')


if __name__ == '__main__':
    main()
