#!/usr/bin/env python3
# Compares two psp-game builds' layout for the functions link-order.txt
# lists (RE-500): a measurement build (frame_timer) must keep the searched
# block where the release build has it, or a ranking on it does not carry.
# usage: cmpaddr.py order.txt nmA.txt nmB.txt [N]
#          address and size of the first N listed names (default all)
#        cmpaddr.py --elf order.txt elfA elfB [N]
#          the same from the ELFs, plus each function's bytes (a word that
#          differs only as a relocated address is counted apart: the
#          HI16/LO16 halves of an address, a call to the same function) and the data
#          symbols' addresses (same address, same offset in a 64-byte
#          D-cache line, same set of the 16 KB two-way D-cache: address
#          mod 8 KB)
# nm.txt: psp-nm -S --defined-only elf | awk '$3 ~ /^[tTwW]$/' | sort
# Names compare without crate hashes, as tools/psp-link.py matches them.
import importlib.util, os, re, struct, subprocess, sys

sys.dont_write_bytecode = True
_spec = importlib.util.spec_from_file_location('psp_link', os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'psp-link.py'))
_link = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_link)
stable = _link.stable

def parse(lines, kinds):
    d = {}
    for l in lines:
        f = l.split(None, 3)
        if len(f) < 4 or f[2] not in kinds:
            continue
        d.setdefault(stable(f[3].strip()), (int(f[0], 16), int(f[1], 16)))
    return d

def nm(elf):
    return subprocess.run(['psp-nm', '-S', '--defined-only', elf], capture_output=True, text=True, check=True).stdout.splitlines()

def sections(path):
    b = open(path, 'rb').read()
    shoff, = struct.unpack_from('<I', b, 0x20)
    shentsize, shnum, shstrndx = struct.unpack_from('<HHH', b, 0x2E)
    sh = [struct.unpack_from('<IIIIIIIIII', b, shoff + i * shentsize) for i in range(shnum)]
    strtab = sh[shstrndx][4]
    out = {}
    for s in sh:
        name = b[strtab + s[0]:b.index(0, strtab + s[0])].decode()
        out[name] = (s[3], s[4], s[5], s[1], s[2])  # addr, offset, size, type, flags
    return b, out

def read(img, addr, size):
    b, secs = img
    for name, (a, off, sz, typ, flags) in secs.items():
        # SHT_NOBITS has no bytes; only SHF_ALLOC sections are loaded.
        if typ != 8 and flags & 2 and a <= addr and addr + size <= a + sz:
            return b[off + addr - a:off + addr - a + size]
    return None

args = sys.argv[1:]
elf = args[0] == '--elf'
if elf:
    args = args[1:]
order = [l.strip() for l in open(args[0]) if l.strip() and not l.startswith('#')]
if len(args) > 3:
    order = order[:int(args[3])]
text = 'tTwW'
if elf:
    la, lb = nm(args[1]), nm(args[2])
else:
    la, lb = open(args[1]).read().splitlines(), open(args[2]).read().splitlines()
A, B = parse(la, text), parse(lb, text)
same = diff = miss = 0
for n in order:
    if n not in A or n not in B:
        miss += 1; print('missing', n, n in A, n in B); continue
    if A[n] == B[n]:
        same += 1
    else:
        diff += 1
        if diff <= 15:
            print('diff', n, '%x/%x' % A[n], '%x/%x' % B[n])
print(f'same {same} diff {diff} missing {miss} of {len(order)}')
if not elf:
    sys.exit(0)

ia, ib = sections(args[1]), sections(args[2])

def relocs(img):
    # .rel.text (--emit-relocs): offset -> (type, symbol index), in order.
    b, secs = img
    _, off, size, _, _ = secs['.rel.text']
    return [struct.unpack_from('<II', b, off + i) for i in range(0, size, 8)]

def symbols(lines):
    # Every defined symbol, sorted, for naming a target address.
    out = []
    for l in lines:
        f = l.split()
        if len(f) in (3, 4) and f[-2] in 'tTwWR':  # functions, import stubs
            out.append((int(f[0], 16), stable(f[-1])))
    out.sort()
    return out

def name_of(syms, addr):
    import bisect
    i = bisect.bisect_right(syms, (addr, '￿')) - 1
    return f'{syms[i][1]}+{addr - syms[i][0]:#x}' if i >= 0 else f'{addr:#x}'

def targets(img, rel, syms, lo, hi):
    # The relocated words in [lo, hi): a jal/j names its target function;
    # a HI16/LO16 half of an address (data, mostly unnamed constants whose
    # $anon numbering is not stable between builds) is marked 'addr'.
    out = {}
    for off, info in rel:
        if not lo <= off < hi:
            continue
        t = info & 0xff
        if t == 4:  # R_MIPS_26
            w = struct.unpack_from('<I', read(img, off, 4))[0]
            out[off] = name_of(syms, ((off + 4) & 0xf0000000) | ((w & 0x3ffffff) << 2))
        elif t in (5, 6):  # R_MIPS_HI16, R_MIPS_LO16
            out[off] = 'addr'
    return out

ra, rb = relocs(ia), relocs(ib)
sa, sb = symbols(la), symbols(lb)
hot = [n for n in order if n in A and n in B and A[n] == B[n]]
lo = min(A[n][0] for n in hot)
hi = max(A[n][0] + A[n][1] for n in hot)
ta, tb = targets(ia, ra, sa, lo, hi), targets(ib, rb, sb, lo, hi)
bsame = bdiff = 0
addr_words = call_words = other_words = 0
for n in hot:
    ba, bb = read(ia, *A[n]), read(ib, *B[n])
    if ba is None or bb is None:
        sys.exit(f'no bytes for {n}')
    if ba == bb:
        bsame += 1
        continue
    bdiff += 1
    for i in range(0, len(ba), 4):
        if ba[i:i + 4] == bb[i:i + 4]:
            continue
        a = A[n][0] + i
        if a in ta and ta[a] == tb.get(a):
            addr_words += ta[a] == 'addr'
            call_words += ta[a] != 'addr'
        else:
            other_words += 1
            if other_words <= 15:
                print('word differs', n, f'{a:#x}', ba[i:i + 4].hex(), bb[i:i + 4].hex(), ta.get(a), tb.get(a))
print(f'bytes: same {bsame} differ {bdiff}; differing words: {addr_words} HI16/LO16 address halves, '
      f'{call_words} calls to the same function at another address, {other_words} other')

DA, DB = parse(la, 'bBdDrRgGsS'), parse(lb, 'bBdDrRgGsS')
common = sorted(set(DA) & set(DB), key=lambda n: DA[n][0])
moved = [n for n in common if DA[n][0] != DB[n][0]]
line = [n for n in moved if DA[n][0] % 64 != DB[n][0] % 64]
setx = [n for n in moved if DA[n][0] % 0x2000 != DB[n][0] % 0x2000]
print(f'data: {len(common)} common, {len(set(DA) - set(DB))} only in A, {len(set(DB) - set(DA))} only in B; '
      f'moved {len(moved)}, line offset changed {len(line)}, set index changed {len(setx)}')
for n in (line or setx)[:15]:
    print('data moved', n, '%x' % DA[n][0], '%x' % DB[n][0])
for name in ('.text', '.rodata', '.data', '.bss', '.sbss', '.sdata'):
    a, b = ia[1].get(name), ib[1].get(name)
    if a or b:
        print(f'{name}: A {a[0]:#x}+{a[2]:#x}' if a else f'{name}: A -', f'B {b[0]:#x}+{b[2]:#x}' if b else 'B -')
