"""Minimal R4300 interpreter over the SSB64 (USA) ROM's machine code, for
the syUtils* trig functions and libultra __sinf/__cosf.

It executes the original instructions read from the ROM at run time; no ROM
bytes are stored here (D-048). FCSR is modelled as osInitialize sets it
(FS|EV|RN): round to nearest, denormal results flushed to signed zero,
invalid operations / NaN operands / denormal operands raise (the real CPU
traps). Only the instructions these functions use are implemented; anything
else raises Trap, so an unmodelled path cannot pass silently.

Used by trig_hashes.py to pin the hashes in
crates/ssb-engine/src/math/trig_golden.rs.
"""
import hashlib, math, os, struct
import numpy as np
np.seterr(all='ignore')

ROM_SHA1 = 'e2929e10fccc0aa84e5776227e798abc07cedabf'  # USA, as crates/ssb-rom/src/rom.rs
rom = None


def load_rom(path=None):
    """Reads the ROM: `path`, else $SSB64_ROM, else the decomp's baserom."""
    global rom
    if path is None:
        path = os.environ.get('SSB64_ROM') or os.path.join(
            os.path.dirname(os.path.abspath(__file__)), '..', '..',
            'refs', 'ssb-decomp-re', 'baserom.us.z64')
    with open(path, 'rb') as f:
        data = f.read()
    digest = hashlib.sha1(data).hexdigest()
    if digest != ROM_SHA1:
        raise SystemExit(f'{path}: SHA-1 {digest}, want the USA ROM {ROM_SHA1}')
    rom = data
FN = {'ArcTan': 0x80018514, 'ArcTan2': 0x8001863c, 'ArcSin': 0x8001873c,
      'ArcCos': 0x800187c0, 'Tan': 0x800184e0, 'sinf': 0x800303f0, 'cosf': 0x80035cd0}
SQRTF = 0x80033510
MAGIC = 0x0dead000

class Trap(Exception):
    pass

def romword(a):
    if rom is None:
        load_rom()
    if OVL0_VRAM <= a < OVL0_END:
        o = a - OVL0_VRAM + OVL0_ROM  # ovl0 (lb*): text, data and rodata
    elif OVL2_VRAM <= a < OVL2_END:
        o = a - OVL2_VRAM + OVL2_ROM  # ovl2 (ft*, gm*): text, data and rodata
    else:
        o = a - 0x7FFFF400  # the boot segment: ROM 0x1000 at RAM 0x80000400
    return struct.unpack('>I', rom[o:o + 4])[0]


# ovl0 (smashbrothers.us.yaml): ROM 0x043220..0x051C90 at 0x800C7840; its
# .bss follows and is not in the ROM.
OVL0_ROM = 0x043220
OVL0_VRAM = 0x800C7840
OVL0_END = OVL0_VRAM + (0x051C90 - OVL0_ROM)
OVL2_ROM = 0x051C90
OVL2_VRAM = 0x800D6490
OVL2_END = OVL2_VRAM + (0x0AC540 - OVL2_ROM)

def f2b(f):
    return struct.unpack('<I', struct.pack('<f', f))[0]
def b2f(b):
    return np.frombuffer(struct.pack('<I', b), dtype=np.float32)[0]
def d2b(d):
    return struct.unpack('<Q', struct.pack('<d', d))[0]
def b2d(b):
    return struct.unpack('<d', struct.pack('<Q', b))[0]

def s_in(b):
    e = (b >> 23) & 0xff
    if e == 0 and (b & 0x7fffff):
        raise Trap('denormal operand')
    if e == 0xff and (b & 0x7fffff):
        raise Trap('NaN operand')
    return b2f(b)
def d_in(b):
    e = (b >> 52) & 0x7ff
    m = b & ((1 << 52) - 1)
    if e == 0 and m:
        raise Trap('denormal operand d')
    if e == 0x7ff and m:
        raise Trap('NaN operand d')
    return b2d(b)
def s_out(f):
    b = f2b(np.float32(f))
    e = (b >> 23) & 0xff
    if e == 0xff and (b & 0x7fffff):
        raise Trap('invalid')
    if e == 0 and (b & 0x7fffff):
        b &= 0x80000000  # FS flush
    return b
def d_out(d):
    b = d2b(float(d))
    e = (b >> 52) & 0x7ff
    m = b & ((1 << 52) - 1)
    if e == 0x7ff and m:
        raise Trap('invalid d')
    if e == 0 and m:
        b &= 1 << 63
    return b

def sx16(v):
    return v - 0x10000 if v & 0x8000 else v

class CPU:
    def __init__(self):
        self.r = [0] * 32
        self.f = [0] * 32
        self.mem = {}
        self.cc = False
        self.steps = 0

    def lw(self, a):
        if a in self.mem:
            return self.mem[a]
        if 0x80000400 <= a < 0x80100000 or OVL0_VRAM <= a < OVL0_END or OVL2_VRAM <= a < OVL2_END:
            return romword(a)
        raise Trap(f'read {a:#x}')

    def gd(self, i):
        return d_in((self.f[i + 1] << 32) | self.f[i])
    def sd(self, i, d):
        b = d_out(d)
        self.f[i] = b & 0xffffffff
        self.f[i + 1] = b >> 32

    def call(self, addr, fargs):
        self.r = [0] * 32
        self.r[29] = 0x80300000
        self.r[31] = MAGIC
        for i, v in zip((12, 14), fargs):
            self.f[i] = f2b(np.float32(v)) if not isinstance(v, int) else v
        self.steps = 0
        pc, npc = addr, addr + 4
        while pc != MAGIC:
            self.steps += 1
            if self.steps > 100000:
                raise Trap('runaway')
            if pc == SQRTF:
                x = s_in(self.f[12])
                if x < 0:
                    raise Trap('sqrt neg')
                self.f[0] = s_out(np.sqrt(np.float32(x)))
                pc, npc = self.r[31], self.r[31] + 4
                continue
            w = romword(pc)
            target = self.step(w, pc)
            if target is None:
                pc, npc = npc, npc + 4
            elif target == 'nullify':
                pc, npc = npc + 4, npc + 8
            else:
                # execute delay slot
                ds = romword(npc)
                if self.step(ds, npc) is not None:
                    raise Trap('branch in delay slot')
                pc, npc = target, target + 4
        return self.f[0]

    def step(self, w, pc):
        op = w >> 26; rs = (w >> 21) & 31; rt = (w >> 16) & 31; rd = (w >> 11) & 31
        sh = (w >> 6) & 31; fn = w & 63; imm = w & 0xffff; simm = sx16(imm)
        r = self.r
        def setr(i, v):
            if i:
                r[i] = v & 0xffffffff
        btarget = (pc + 4 + (simm << 2)) & 0xffffffff
        if op == 0:
            if fn == 0: setr(rd, r[rt] << sh)
            elif fn == 2: setr(rd, r[rt] >> sh)
            elif fn == 3: setr(rd, (r[rt] - (1 << 32) if r[rt] & 0x80000000 else r[rt]) >> sh)
            elif fn == 8: return r[rs]
            elif fn == 0x21: setr(rd, r[rs] + r[rt])
            elif fn == 0x23: setr(rd, r[rs] - r[rt])
            elif fn == 0x24: setr(rd, r[rs] & r[rt])
            elif fn == 0x25: setr(rd, r[rs] | r[rt])
            else: raise Trap(f'special {fn} at {pc:#x}')
            return None
        if op == 0x0f: setr(rt, imm << 16); return None
        if op == 0x0d: setr(rt, r[rs] | imm); return None
        if op == 0x0c: setr(rt, r[rs] & imm); return None
        if op == 0x09: setr(rt, r[rs] + simm); return None
        if op == 0x0a:
            a = r[rs] - (1 << 32) if r[rs] & 0x80000000 else r[rs]
            setr(rt, 1 if a < simm else 0); return None
        if op == 0x23: setr(rt, self.lw((r[rs] + simm) & 0xffffffff)); return None
        if op == 0x2b: self.mem[(r[rs] + simm) & 0xffffffff] = r[rt]; return None
        if op == 0x31: self.f[rt] = self.lw((r[rs] + simm) & 0xffffffff); return None
        if op == 0x39: self.mem[(r[rs] + simm) & 0xffffffff] = self.f[rt]; return None
        if op == 0x35:
            a = (r[rs] + simm) & 0xffffffff
            self.f[rt + 1] = self.lw(a); self.f[rt] = self.lw(a + 4); return None
        if op == 0x3d:
            a = (r[rs] + simm) & 0xffffffff
            self.mem[a] = self.f[rt + 1]; self.mem[a + 4] = self.f[rt]; return None
        if op in (4, 5, 0x14, 0x15):
            take = (r[rs] == r[rt]) if op in (4, 0x14) else (r[rs] != r[rt])
            if take: return btarget
            return 'nullify' if op >= 0x14 else None
        if op == 3:
            r[31] = pc + 8
            return (pc & 0xf0000000) | ((w & 0x3ffffff) << 2)
        if op == 2:
            return (pc & 0xf0000000) | ((w & 0x3ffffff) << 2)
        if op == 0x11:
            return self.cop1(w, pc, rs, rt, rd, sh, fn, btarget)
        raise Trap(f'op {op:#x} at {pc:#x}')

    def cop1(self, w, pc, fmt, ft, fs, fd, fn, btarget):
        F = self.f
        if fmt == 8:
            tf = ft & 1; likely = ft & 2
            if self.cc == bool(tf): return btarget
            return 'nullify' if likely else None
        if fmt == 0: self.setgpr(ft, F[fs]); return None   # mfc1
        if fmt == 4: F[fs] = self.r[ft]; return None       # mtc1
        if fmt == 16:   # .s
            if fn >= 48:
                a = s_in(F[fs]); b = s_in(F[ft]); c = fn & 15
                self.cc = bool(((c & 4) and a < b) or ((c & 2) and a == b))
                return None
            if fn == 6: F[fd] = F[fs]; return None
            if fn == 7: F[fd] = F[fs] ^ 0x80000000; s_in(F[fs]); return None
            if fn == 5: s_in(F[fs]); F[fd] = F[fs] & 0x7fffffff; return None
            if fn == 33: self.sd(fd, float(s_in(F[fs]))); return None   # cvt.d.s
            a = s_in(F[fs])
            if fn == 4:
                if a < 0: raise Trap('sqrt neg')
                F[fd] = s_out(np.sqrt(a)); return None
            if fn in (13, 36):
                if fn == 36: raise Trap('cvt.w.s rounding unmodelled')
                v = math.trunc(float(a))
                if not -2**31 <= v < 2**31: raise Trap('cvt range')
                F[fd] = v & 0xffffffff; return None
            b = s_in(F[ft])
            if fn == 0: F[fd] = s_out(a + b)
            elif fn == 1: F[fd] = s_out(a - b)
            elif fn == 2: F[fd] = s_out(a * b)
            elif fn == 3:
                if b == 0 and a == 0: raise Trap('0/0')
                F[fd] = s_out(a / b) if b != 0 else s_out(np.float32(np.copysign(np.inf, a) * np.copysign(1, b)))
            else: raise Trap(f'cop1.s {fn} at {pc:#x}')
            return None
        if fmt == 17:   # .d
            if fn >= 48:
                a = self.gd(fs); b = self.gd(ft); c = fn & 15
                self.cc = bool(((c & 4) and a < b) or ((c & 2) and a == b))
                return None
            if fn == 6: F[fd] = F[fs]; F[fd + 1] = F[fs + 1]; return None
            a = self.gd(fs)
            if fn == 7: self.sd(fd, -a); return None
            if fn == 32: F[fd] = s_out(np.float32(a)); return None        # cvt.s.d
            if fn in (13, 36):
                if fn == 36: raise Trap('cvt.w.d rounding unmodelled')
                v = math.trunc(a)
                if not -2**31 <= v < 2**31: raise Trap('cvt range')
                F[fd] = v & 0xffffffff; return None
            b = self.gd(ft)
            if fn == 0: self.sd(fd, a + b)
            elif fn == 1: self.sd(fd, a - b)
            elif fn == 2: self.sd(fd, a * b)
            elif fn == 3:
                if b == 0: raise Trap('d div0')
                self.sd(fd, a / b)
            else: raise Trap(f'cop1.d {fn} at {pc:#x}')
            return None
        if fmt == 20:   # .w
            v = F[fs]; v = v - (1 << 32) if v & 0x80000000 else v
            if fn == 32: F[fd] = s_out(np.float32(v)); return None
            if fn == 33: self.sd(fd, float(v)); return None
        raise Trap(f'cop1 fmt {fmt} fn {fn} at {pc:#x}')

    def setgpr(self, i, v):
        if i:
            self.r[i] = v & 0xffffffff

cpu = CPU()
def run(name, *args):
    """args: f32 bit patterns (ints). Returns result bits, or None on a trap."""
    try:
        return cpu.call(FN[name], args)
    except Trap:
        return None
