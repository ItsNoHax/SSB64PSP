"""Runs the ROM's own syMatrixTraRotRpyRSca and syMatrixTraRotRpyR
(sys/matrix.c) through the r4300 interpreter over a fixed input set, and
prints the hashes and sample vectors pinned in
crates/ssb-rom/src/scene.rs (`display_matrix_matches_the_rom`).

Inputs: angles as f32 bit patterns from a deterministic sweep (negative,
multi-turn, exact quarter turns), scales around 1 including the non-1/256
values fighters and stage parts carry. The hash is FNV-1a 64 over the
nine s15.16 words of the 3x3 (the original's Mtx [row][col], integer and
fraction halves recombined), in input order. No ROM bytes are stored here
(D-048): the code and gSYSinTable are read from the ROM at run time.

    python3 tools/r4300/matrix_hashes.py [ROM]
"""
import struct, sys
import numpy as np
import r4300
from r4300 import CPU, Trap, romword, f2b, sx16, MAGIC

FN = {'TraRotRpyR': 0x8001C21C, 'TraRotRpyRSca': 0x8001C588}
OUT = 0x80400000
FNV_OFFSET = 0xcbf29ce484222325
FNV_PRIME = 0x100000001b3


class MtxCPU(CPU):
    """The integer ops the matrix builders add to the trig set."""

    def __init__(self):
        super().__init__()
        self.hi = 0
        self.lo = 0
        self.fcsr = 0  # read back by cfc1; conversions are trunc.w.s

    def lw(self, a):
        if a & 3:
            raise Trap(f'unaligned {a:#x}')
        return super().lw(a)

    def step(self, w, pc):
        op = w >> 26; rs = (w >> 21) & 31; rt = (w >> 16) & 31; rd = (w >> 11) & 31
        sh = (w >> 6) & 31; fn = w & 63; imm = w & 0xffff; simm = sx16(imm)
        r = self.r

        def setr(i, v):
            if i:
                r[i] = v & 0xffffffff

        def s32(v):
            return v - (1 << 32) if v & 0x80000000 else v

        if op == 0:
            if fn == 0x18:      # mult
                p = s32(r[rs]) * s32(r[rt]) & 0xffffffffffffffff
                self.hi, self.lo = p >> 32, p & 0xffffffff
                return None
            if fn == 0x19:      # multu
                p = r[rs] * r[rt]
                self.hi, self.lo = p >> 32, p & 0xffffffff
                return None
            if fn == 0x12: setr(rd, self.lo); return None   # mflo
            if fn == 0x10: setr(rd, self.hi); return None   # mfhi
            if fn == 0x04: setr(rd, r[rt] << (r[rs] & 31)); return None  # sllv
            if fn == 0x06: setr(rd, r[rt] >> (r[rs] & 31)); return None  # srlv
            if fn == 0x07: setr(rd, s32(r[rt]) >> (r[rs] & 31)); return None  # srav
            if fn == 0x2a: setr(rd, 1 if s32(r[rs]) < s32(r[rt]) else 0); return None
            if fn == 0x2b: setr(rd, 1 if r[rs] < r[rt] else 0); return None
            if fn == 0x26: setr(rd, r[rs] ^ r[rt]); return None
            if fn == 0x27: setr(rd, ~(r[rs] | r[rt])); return None
        if op == 0x0e: setr(rt, r[rs] ^ imm); return None   # xori
        if op == 0x0b: setr(rt, 1 if r[rs] < (simm & 0xffffffff) else 0); return None
        if op in (0x25, 0x21):  # lhu, lh
            a = (r[rs] + simm) & 0xffffffff
            word = self.lw(a & ~3)
            half = (word >> (16 if (a & 2) == 0 else 0)) & 0xffff
            if op == 0x21 and half & 0x8000:
                half -= 0x10000
            setr(rt, half)
            return None
        if op == 0x29:          # sh
            a = (r[rs] + simm) & 0xffffffff
            base = a & ~3
            word = self.mem.get(base, 0)
            shift = 16 if (a & 2) == 0 else 0
            word = (word & ~(0xffff << shift)) | ((r[rt] & 0xffff) << shift)
            self.mem[base] = word & 0xffffffff
            return None
        if op in (6, 7, 1):     # blez, bgtz, regimm bltz/bgez
            v = s32(r[rs])
            btarget = (pc + 4 + (simm << 2)) & 0xffffffff
            if op == 6: take = v <= 0
            elif op == 7: take = v > 0
            elif rt in (0, 2): take = v < 0
            elif rt in (1, 3): take = v >= 0
            else: raise Trap(f'regimm {rt} at {pc:#x}')
            if take: return btarget
            return 'nullify' if (op == 1 and rt in (2, 3)) else None
        if op == 0x11 and rs == 2:   # cfc1
            setr(rt, self.fcsr if rd == 31 else 0); return None
        if op == 0x11 and rs == 6:   # ctc1
            if rd == 31:
                self.fcsr = r[rt]
            return None
        return super().step(w, pc)

    def call_int(self, addr, words):
        """o32: the first four argument words in a0-a3, the rest on the
        stack from sp+16."""
        self.r = [0] * 32
        self.mem = {}
        sp = 0x80300000
        self.r[29] = sp
        self.r[31] = MAGIC
        for i, v in enumerate(words):
            if i < 4:
                self.r[4 + i] = v
            else:
                self.mem[sp + 16 + 4 * (i - 4)] = v
        self.steps = 0
        pc, npc = addr, addr + 4
        while pc != MAGIC:
            self.steps += 1
            if self.steps > 100000:
                raise Trap('runaway')
            w = romword(pc)
            target = self.step(w, pc)
            if target is None:
                pc, npc = npc, npc + 4
            elif target == 'nullify':
                pc, npc = npc + 4, npc + 8
            else:
                ds = romword(npc)
                if self.step(ds, npc) is not None:
                    raise Trap('branch in delay slot')
                pc, npc = target, target + 4


def mtx_words(cpu):
    """The 3x3 of the Mtx at OUT as s15.16 words, [row][col]."""
    w = [cpu.mem.get(OUT + 4 * i, 0) for i in range(16)]
    out = []
    for row in range(3):
        for col in range(3):
            k = row * 4 + col
            word = k // 2
            hi_half = (k % 2) == 0
            i_part = (w[word] >> 16) if hi_half else (w[word] & 0xffff)
            f_part = (w[8 + word] >> 16) if hi_half else (w[8 + word] & 0xffff)
            v = (i_part << 16) | f_part
            out.append(v - (1 << 32) if v & 0x80000000 else v)
    return out


def inputs():
    """(r, p, y, sx, sy, sz) as f32, deterministic."""
    f = np.float32
    angles = [0.0, 1.5707964, -1.5707964, 3.1415927, -3.1415927, 0.1, -0.1,
              0.001, -0.001, 0.785398, 2.5, -2.5, 6.2831855, -6.2831855,
              12.0, -12.0, 0.0015339808, -0.0015339808]
    scales = [1.0, 0.5, 1.0537, 0.997, 2.0, 1.2, 0.8, 1.5, 0.003]
    out = []
    for i, r in enumerate(angles):
        for j, p in enumerate(angles):
            y = angles[(i * 7 + j * 3) % len(angles)]
            s = scales[(i + j) % len(scales)]
            t = scales[(i * 5 + j) % len(scales)]
            out.append(tuple(f(v) for v in (r, p, y, s, t, 1.0)))
    # A sweep of every table step across two turns, both signs.
    for k in range(-8192, 8192, 7):
        a = f(k * 0.0015339808 + 0.0003)
        out.append((a, f(-a * 0.5), f(a * 2.0), f(1.0), f(1.0), f(1.0)))
    return out


def fnv(h, word):
    for b in struct.pack('<i', word):
        h = ((h ^ b) * FNV_PRIME) & 0xffffffffffffffff
    return h


def main():
    r4300.load_rom(sys.argv[1] if len(sys.argv) > 1 else None)
    cpu = MtxCPU()
    ins = inputs()
    for name in ('TraRotRpyRSca', 'TraRotRpyR'):
        h = FNV_OFFSET
        samples = []
        for n, (r, p, y, sx, sy, sz) in enumerate(ins):
            args = [OUT, f2b(np.float32(1.0)), f2b(np.float32(2.0)), f2b(np.float32(3.0)),
                    f2b(r), f2b(p), f2b(y)]
            if name == 'TraRotRpyRSca':
                args += [f2b(sx), f2b(sy), f2b(sz)]
            cpu.call_int(FN[name], args)
            words = mtx_words(cpu)
            for v in words:
                h = fnv(h, v)
            if n in (1, 5, 40, 123):
                samples.append((n, words))
        print(f'{name}: {len(ins)} inputs, hash {h:#018x}')
        for n, words in samples:
            print(f'  input {n}: {ins[n]} -> {words}')


if __name__ == '__main__':
    main()
