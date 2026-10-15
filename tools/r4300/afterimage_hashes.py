"""Runs the ROM's own ftDisplayMainDrawAfterImage (ft/ftdisplaymain.c,
ovl2 0x800F1020) through the r4300 interpreter, and prints the hash pinned
in crates/ssb-game/src/afterimage/tests.rs (`the_trail_vertices_match_the_rom`).

Each case fills an FTStruct's afterimage (fp + 0xA9C: is_itemswing,
drawstatus, desc_id, three {s16 translate[3], Vec3f vec}) from a small LCG
that the Rust test repeats, points gSYTaskmanGraphicsHeap.ptr and
gSYTaskmanDLHeads[1] at scratch memory, runs the function, and hashes the
vertex count and every emitted Vtx word (FNV-1a 64, as matrix_hashes.py).
It also checks the head-1 display list: the vertex DL call, one gSPVertex
of all vertices, the gSP2Triangles strip and the triangle DL call.
No ROM bytes are stored here (D-048).

    python3 tools/r4300/afterimage_hashes.py [ROM]
"""
import struct, sys
import numpy as np
import r4300
from r4300 import f2b, sx16
from matrix_hashes import MtxCPU, FNV_OFFSET, fnv

DRAW_AFTERIMAGE = 0x800F1020
DL_HEADS = 0x800465B0
GRAPHICS_HEAP = 0x800465D8   # SYMallocRegion {id, start, end, ptr}
FP = 0x80500000
AI = FP + 0xA9C
VTX = 0x80510000
DL = 0x80520000
CASES = 1200


class ByteCPU(MtxCPU):
    """The byte loads and stores the afterimage adds (lb, lbu, sb)."""

    def step(self, w, pc):
        op = w >> 26
        if op == 0 and (w & 63) in (0x1A, 0x34, 0x0D):   # div, teq, break
            rs = (w >> 21) & 31; rt = (w >> 16) & 31
            s32 = lambda v: v - (1 << 32) if v & 0x80000000 else v
            a, b = s32(self.r[rs]), s32(self.r[rt])
            if w & 63 == 0x1A:
                if b == 0:
                    raise r4300.Trap(f'div by zero at {pc:#x}')
                q = abs(a) // abs(b) * (1 if (a < 0) == (b < 0) else -1)
                self.lo, self.hi = q & 0xffffffff, (a - q * b) & 0xffffffff
            elif w & 63 == 0x34 and a == b:
                raise r4300.Trap(f'teq at {pc:#x}')
            elif w & 63 == 0x0D:
                raise r4300.Trap(f'break at {pc:#x}')
            return None
        if op in (0x20, 0x24, 0x28):
            rs = (w >> 21) & 31; rt = (w >> 16) & 31
            a = (self.r[rs] + sx16(w & 0xffff)) & 0xffffffff
            shift = 8 * (3 - (a & 3))
            if op == 0x28:
                word = self.mem.get(a & ~3, 0)
                word = (word & ~(0xff << shift)) | ((self.r[rt] & 0xff) << shift)
                self.mem[a & ~3] = word & 0xffffffff
                return None
            b = (self.lw(a & ~3) >> shift) & 0xff
            if op == 0x20 and b & 0x80:
                b -= 0x100
            if rt:
                self.r[rt] = b & 0xffffffff
            return None
        return super().step(w, pc)


class Lcg:
    """The inputs' generator; the Rust test has the same one."""

    def __init__(self, seed):
        self.x = seed

    def next(self, n):
        self.x = (self.x * 1103515245 + 12345) & 0x7fffffff
        return (self.x >> 8) % n

    def range(self, lo, hi):
        return lo + self.next(hi - lo + 1)


def case_inputs(k):
    """(is_itemswing, drawstatus, desc_id, [(translate, vec)] x 3)."""
    g = Lcg(k + 1)
    f = np.float32
    descs = []
    for d in range(3):
        t = tuple(g.range(-3000, 3000) for _ in range(3))
        v = tuple(f(g.range(-1000, 1000)) / f(1000.0) for _ in range(3))
        descs.append((t, v))
    shape = k % 10
    if shape == 0:      # parallel neighbours: no cross product
        descs[1] = (descs[1][0], descs[0][1])
    elif shape == 1:    # opposite neighbours: no cross product either
        descs[2] = (descs[2][0], tuple(-c for c in descs[1][1]))
    elif shape == 2:    # nearly parallel: under 30 degrees, no in-between pairs
        v = descs[0][1]
        descs[1] = (descs[1][0], (v[0], v[1] + f(0.01), v[2]))
    return (k & 1, 2 + ((k >> 1) & 1), (k >> 2) % 3, descs)


def setup(itemswing, drawstatus, desc_id, descs):
    mem = {}
    mem[AI] = (itemswing << 24) | ((drawstatus & 0xff) << 16) | (desc_id << 8)
    for d, (t, v) in enumerate(descs):
        base = AI + 4 + 0x14 * d
        mem[base] = ((t[0] & 0xffff) << 16) | (t[1] & 0xffff)
        mem[base + 4] = (t[2] & 0xffff) << 16
        for c in range(3):
            mem[base + 8 + 4 * c] = f2b(v[c])
    mem[GRAPHICS_HEAP + 0xC] = VTX
    mem[DL_HEADS + 4] = DL
    return mem


def check_dl(mem, n, case):
    words = []
    a = DL
    while a < mem[DL_HEADS + 4]:
        words.append((mem[a], mem[a + 4]))
        a += 8
    want = 1 + 1 + (n - 2 + 1) // 2 + 1
    if len(words) != want:
        raise SystemExit(f'case {case}: {len(words)} DL commands, want {want}')
    if words[0][0] >> 24 != 0xDE or words[-1][0] >> 24 != 0xDE:
        raise SystemExit(f'case {case}: DL calls {words[0]} {words[-1]}')
    w0, w1 = words[1]
    if w0 >> 24 != 0x01 or (w0 >> 12) & 0xff != n or w1 != VTX:
        raise SystemExit(f'case {case}: gSPVertex {w0:#x} {w1:#x}, want {n}')
    for j, (w0, w1) in enumerate(words[2:-1]):
        i = 2 * j
        tri = lambda a, b, c: (a * 2 << 16) | (b * 2 << 8) | (c * 2)
        if w0 != (0x06 << 24) | tri(i, i + 1, i + 2) or w1 != tri(i + 1, i + 3, i + 2):
            raise SystemExit(f'case {case}: gSP2Triangles {w0:#x} {w1:#x} at {i}')


def main():
    r4300.load_rom(sys.argv[1] if len(sys.argv) > 1 else None)
    cpu = ByteCPU()
    h = FNV_OFFSET
    total = 0
    interp = 0
    for k in range(CASES):
        ins = case_inputs(k)
        cpu.call_int(DRAW_AFTERIMAGE, [FP], setup(*ins))
        mem = cpu.mem
        n = (mem[GRAPHICS_HEAP + 0xC] - VTX) // 16
        check_dl(mem, n, k)
        h = fnv(h, n)
        for w in range(4 * n):
            word = mem.get(VTX + 4 * w, 0)
            h = fnv(h, word - (1 << 32) if word & 0x80000000 else word)
        total += n
        interp += n - 2 * ins[1]
        if k in (3, 517):
            print(f'  case {k}: {n} vertices, first {[hex(mem[VTX + 4 * w]) for w in range(4)]}')
    print(f'afterimage: {CASES} cases, {total} vertices ({interp} interpolated), hash {h:#018x}')


if __name__ == '__main__':
    main()
