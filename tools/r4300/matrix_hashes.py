"""Runs the ROM's own syMatrixTraRotRpyRSca, syMatrixTraRotRpyR,
syMatrixRotRpyR (sys/matrix.c) and lbCommonMatrixRotSca (lb/lbcommon.c,
in ovl0) through the r4300 interpreter over a fixed input set, and prints
the hashes and sample vectors pinned in crates/ssb-rom/src/scene.rs
(`display_matrix_matches_the_rom`). Then syMatrixPerspFastF and
syMatrixPerspF (scale 1) over the cameras and a field-of-view sweep: the
hash of every f32 of mf, pinned in crates/ssb-engine/src/math.rs
(`the_projections_match_the_rom`); persp_norm is printed, not pinned (the
port has no use for it). Then the animation-lock builders (RE-509):
lbCommonMatrixTraRotScaInv over parent scales INVS,
gmCollisionTransformMatrixAll, gmCollisionSetMatrixNcs (its matrix and the
vec_scale it stores) and syMatrixF2LFixedW, pinned beside the display
matrices in crates/ssb-rom/src/scene.rs.

Inputs: angles as f32 bit patterns from a deterministic sweep (negative,
multi-turn, exact quarter turns), scales around 1 including the non-1/256
values fighters and stage parts carry. The hash is FNV-1a 64 over the
nine s15.16 words of the 3x3 (the original's Mtx [row][col], integer and
fraction halves recombined), in input order. No ROM bytes are stored here
(D-048): the code and gSYSinTable are read from the ROM at run time.

    python3 tools/r4300/matrix_hashes.py [ROM]
"""
import math, struct, sys
import numpy as np
import r4300
from r4300 import CPU, Trap, romword, f2b, sx16, MAGIC

FN = {'TraRotRpyR': 0x8001C21C, 'TraRotRpyRSca': 0x8001C588,
      'RotRpyR': 0x8001BF20, 'PerspFastF': 0x8001B248, 'PerspF': 0x8001B4CC,
      'lbCommonMatrixRotSca': 0x800C82AC, 'lbCommonMatrixTraRotScaInv': 0x800C7DB4,
      'syMatrixF2LFixedW': 0x80019EA0, 'gmCollisionTransformMatrixAll': 0x800ECDE4,
      'gmCollisionSetMatrixNcs': 0x800ECFF4}
# Parent scales for the animation-lock builders (RE-509): fighter sizes,
# scale-track values and 1.
INVS = [1.0, 1.1, 0.5, 1.24, 2.0, 0.91, 1.05, 5.0, 0.039794922]
DOBJ = 0x80410000   # translate.vec at +0x1C, rotate.vec +0x30, scale.vec +0x40
PARTS = 0x80411000  # vec_scale at +0x90
SMUL = 0x80412000
FMTX = 0x80413000
OUT2 = 0x80400100
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
        if op == 0x11 and rs == 16 and fn == 36:   # cvt.w.s in FCSR's mode
            a = r4300.s_in(self.f[rd])
            mode = self.fcsr & 3
            if mode == 0:
                v = round(float(a))  # Python rounds half to even
            elif mode == 1:
                v = math.trunc(float(a))
            elif mode == 2:
                v = math.ceil(float(a))
            else:
                v = math.floor(float(a))
            if not -2**31 <= v < 2**31:
                raise Trap('cvt range')
            self.f[sh] = v & 0xffffffff
            return None
        if op == 0x11 and rs == 2:   # cfc1
            setr(rt, self.fcsr if rd == 31 else 0); return None
        if op == 0x11 and rs == 6:   # ctc1
            if rd == 31:
                self.fcsr = r[rt]
            return None
        return super().step(w, pc)

    def call_int(self, addr, words, init=None):
        """o32: the first four argument words in a0-a3, the rest on the
        stack from sp+16. `init` presets memory words (address -> word)."""
        self.r = [0] * 32
        self.mem = dict(init or {})
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


def row3_ok(cpu):
    """The Mtx at OUT has no translation: row 3 is (0, 0, 0, 1)."""
    w = [cpu.mem.get(OUT + 4 * i, 0) for i in range(16)]
    return w[6] == 0 and w[7] == 1 and w[14] == 0 and w[15] == 0


def persp_inputs():
    """(fovy, aspect, near, far) as f32: the game's cameras, then a sweep.
    The sweep starts where the table index leaves 0 (a zero sine divides
    to infinity on the N64; the port divides by 1, RE-469)."""
    f = np.float32
    cams = [(38.0, 300.0 / 220.0, 256.0, 39936.0), (30.0, 300.0 / 220.0, 256.0, 39936.0),
            (18.607187, 300.0 / 220.0, 128.0, 16384.0), (11.982265, 4.0 / 3.0, 100.0, 15000.0),
            (60.0, 4.0 / 3.0, 10.0, 12800.0), (45.0, 1.0, 1.0, 4000.0)]
    out = [tuple(f(v) for v in c) for c in cams]
    aspects = [300.0 / 220.0, 4.0 / 3.0, 1.0, 16.0 / 9.0]
    planes = [(256.0, 39936.0), (128.0, 16384.0), (32.0, 6400.0), (1.0, 1.5)]
    k = 0
    fov = 0.5
    while fov < 170.0:
        a = aspects[k % len(aspects)]
        n, fa = planes[(k // 4) % len(planes)]
        out.append((f(fov), f(a), f(n), f(fa)))
        fov += 0.37
        k += 1
    return out


def main():
    r4300.load_rom(sys.argv[1] if len(sys.argv) > 1 else None)
    cpu = MtxCPU()
    ins = inputs()
    for name in ('TraRotRpyRSca', 'TraRotRpyR', 'lbCommonMatrixRotSca', 'RotRpyR'):
        h = FNV_OFFSET
        samples = []
        for n, (r, p, y, sx, sy, sz) in enumerate(ins):
            if name.startswith('Tra'):
                args = [OUT, f2b(np.float32(1.0)), f2b(np.float32(2.0)), f2b(np.float32(3.0)),
                        f2b(r), f2b(p), f2b(y)]
            else:
                args = [OUT, f2b(r), f2b(p), f2b(y)]
            if name.endswith('Sca'):
                args += [f2b(sx), f2b(sy), f2b(sz)]
            cpu.call_int(FN[name], args)
            words = mtx_words(cpu)
            if not name.startswith('Tra') and not row3_ok(cpu):
                raise SystemExit(f'{name} input {n}: a translation row')
            for v in words:
                h = fnv(h, v)
            if n in (1, 5, 40, 123):
                samples.append((n, words))
        print(f'{name}: {len(ins)} inputs, hash {h:#018x}')
        for n, words in samples:
            print(f'  input {n}: {ins[n]} -> {words}')
    animlock_hashes(cpu, ins)
    pins = persp_inputs()
    for name in ('PerspFastF', 'PerspF'):
        h = FNV_OFFSET
        norms = FNV_OFFSET
        for n, (fovy, aspect, near, far) in enumerate(pins):
            cpu.call_int(FN[name], [OUT, OUT2, f2b(fovy), f2b(aspect), f2b(near), f2b(far),
                                    f2b(np.float32(1.0))])
            for i in range(16):
                w = cpu.mem[OUT + 4 * i]
                h = fnv(h, w - (1 << 32) if w & 0x80000000 else w)
            norms = fnv(norms, cpu.mem[OUT2] >> 16)
            if n in (0, 7, 200):
                print(f'  {name} input {n}: {(float(fovy), float(aspect), float(near), float(far))} -> '
                      f'{[hex(cpu.mem[OUT + 4 * i]) for i in (0, 5, 10, 11, 14, 15)]} norm {cpu.mem[OUT2] >> 16}')
        print(f'{name}: {len(pins)} inputs, mf hash {h:#018x}, persp_norm hash {norms:#018x}')


def inv_for(n):
    f = np.float32
    return tuple(f(INVS[(n + k) % len(INVS)]) for k in range(3))


def animlock_hashes(cpu, ins):
    """RE-509: lbCommonMatrixTraRotScaInv over (angles, scale_inv = inv_for(n),
    scale); gmCollisionTransformMatrixAll and gmCollisionSetMatrixNcs
    (scale_mul = inv_for(n), NULL every fifth input) over every f32 of the
    3x3 and vec_scale; syMatrixF2LFixedW of each TransformMatrixAll matrix."""
    f = np.float32
    h = FNV_OFFSET
    for n, (r, p, y, sx, sy, sz) in enumerate(ins):
        inv = inv_for(n)
        args = [OUT] + [f2b(v) for v in (f(1.0), f(2.0), f(3.0), r, p, y, *inv, sx, sy, sz)]
        cpu.call_int(FN['lbCommonMatrixTraRotScaInv'], args)
        words = mtx_words(cpu)
        for v in words:
            h = fnv(h, v)
        if n in (1, 5, 40, 123):
            print(f'  ScaInv input {n}: inv {inv} -> {words}')
    print(f'lbCommonMatrixTraRotScaInv: {len(ins)} inputs, hash {h:#018x}')
    hall = hf2l = hncs = FNV_OFFSET

    def dobj_init(r, p, y, sx, sy, sz):
        init = {}
        for i, v in enumerate((f(1.0), f(2.0), f(3.0))):
            init[DOBJ + 0x1C + 4 * i] = f2b(v)
        for i, v in enumerate((r, p, y)):
            init[DOBJ + 0x30 + 4 * i] = f2b(v)
        for i, v in enumerate((sx, sy, sz)):
            init[DOBJ + 0x40 + 4 * i] = f2b(v)
        return init

    for n, (r, p, y, sx, sy, sz) in enumerate(ins):
        init = dobj_init(r, p, y, sx, sy, sz)
        cpu.call_int(FN['gmCollisionTransformMatrixAll'], [DOBJ, PARTS, FMTX], init)
        mtx = [cpu.mem.get(FMTX + 4 * i, 0) for i in range(16)]
        for row in range(3):
            for col in range(3):
                hall = fnv(hall, s32w(mtx[row * 4 + col]))
        init2 = {FMTX + 4 * i: mtx[i] for i in range(16)}
        init2[FMTX + 0x3C] = f2b(f(1.0))
        cpu.call_int(FN['syMatrixF2LFixedW'], [FMTX, OUT], init2)
        for v in mtx_words(cpu):
            hf2l = fnv(hf2l, v)
        if n in (1, 5, 40, 123):
            print(f'  TransformMatrixAll input {n}: {[hex(mtx[i]) for i in (0, 1, 2, 4, 5, 6, 8, 9, 10)]}')
        inv = inv_for(n)
        init = dobj_init(r, p, y, sx, sy, sz)
        smul = 0
        if n % 5 != 0:
            smul = SMUL
            for i, v in enumerate(inv):
                init[SMUL + 4 * i] = f2b(v)
        cpu.call_int(FN['gmCollisionSetMatrixNcs'], [DOBJ, PARTS, FMTX, smul], init)
        mtx = [cpu.mem.get(FMTX + 4 * i, 0) for i in range(16)]
        for row in range(3):
            for col in range(3):
                hncs = fnv(hncs, s32w(mtx[row * 4 + col]))
        for i in range(3):
            hncs = fnv(hncs, s32w(cpu.mem.get(PARTS + 0x90 + 4 * i, 0)))
    print(f'gmCollisionTransformMatrixAll: {len(ins)} inputs, f32 hash {hall:#018x}')
    print(f'syMatrixF2LFixedW of those: hash {hf2l:#018x}')
    print(f'gmCollisionSetMatrixNcs: {len(ins)} inputs, f32 hash {hncs:#018x}')


def s32w(w):
    return w - (1 << 32) if w & 0x80000000 else w


if __name__ == '__main__':
    main()
