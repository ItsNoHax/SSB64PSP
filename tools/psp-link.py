#!/usr/bin/env python3
"""Linker for `psp-game`: `rust-lld` with the hot functions placed first.

The PSP's 16 KB two-way instruction cache cannot hold the draw's per-mesh
code where LTO happens to lay it out; placing the functions that code runs
through next to each other removes 8% of the draw's instruction-cache
misses on a PSP-2000 (RE-482). `psp-game/link-order.txt` lists them, hottest
first, as v0-mangled names with the crate hashes removed (`stable`): a
crate's hash changes with its feature set and the checkout's path, the rest
of the name does not. Names match without their impl disambiguators
(`key`), which an impl added above another changes. This script
reads the function symbols of the objects being linked, maps the listed
names onto them, appends the unlisted functions sorted by stable name (a
checkout-independent layout) and passes the result to `rust-lld` as a
`--symbol-ordering-file`. It changes where code goes, never the code.
It then links again with zero bytes after the code (`OUT.pad.o`) so that
`.bss` keeps one data-cache placement and `.rodata` one offset in its
cache line however large the code is (RE-580, RE-584); a second pad pins the import stubs
to one instruction-cache set (`STUB_SET`, RE-585).
Names after a `# after unlisted` line go after the unlisted functions,
followed by a pad that puts the code behind them on the cache sets it has
without them: code one scene runs then leaves every other scene's layout
alone (RE-585). A regenerated `link-order.txt` must keep that block.

Set as `psp-game`'s linker in `psp-game/.cargo/config.toml`; rustc calls it
with `rust-lld`'s arguments and has `rust-lld` on `PATH`.

`PSP_LINK_ORDER` replaces `link-order.txt`. `PSP_LINK_SAVE=DIR` copies the
link's inputs into DIR, so `tools/icache/relink.py` can link them again with
another order without recompiling (RE-494).

`psp-link.py --stable` rewrites the names of `psp-nm -S` lines on stdin
into this form, for `tools/icache/layout.c`.
"""
import os
import re
import struct
import sys

LIST = os.environ.get('PSP_LINK_ORDER') or os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'psp-game', 'link-order.txt')
# A v0 crate root: `C`, the disambiguator `s<base62>_`, then the crate name's
# `<length><identifier>`.
CRATE_HASH = re.compile(r'Cs[0-9A-Za-z]*_(?=\d)')
BASE62 = '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
BASIC_TYPES = set('abcdefhijlmnostuvxyzp')
# The data cache is 16 KB two-way: an address's set is its offset in an
# 8 KB way. BSS_SET is the `.bss` offset measured fastest of those tried
# on a PSP (RE-580).
DCACHE_WAY = 0x2000
BSS_SET = 0x1d00
# `.rodata`'s offset in a 64-byte data-cache line, pinned with `.bss`: the
# code's size otherwise moves the constants and `.data` by its remainder
# mod 64 (1P intro +0.05-0.07 ms busy on a PSP-2000 for a 16-byte shift).
# Measured fastest of 0x00-0x30 (RE-584). PSP_LINK_RODATA_LINE overrides it, for a
# screen.
RODATA_LINE = int(os.environ.get('PSP_LINK_RODATA_LINE', '0x00'), 0)
LINE = 64
PAD_SYMBOL = 'psp_link_pad'
TAIL_PAD_SYMBOL = 'psp_link_tail_pad'
# The import stubs' offset in an 8 KB instruction-cache way: the timed
# build's at 563dacf8. `.eh_frame` otherwise moves them with each new
# function (Pikachu shock r18 +0.11 ms busy on a PSP-2000 for -0xf0, RE-585).
# The stub pad sorts last in the linker script's `.sceStub.text.*`, after
# the stubs and before `.eh_frame`. PSP_LINK_STUB_SET overrides it, for a
# screen.
STUB_SET = int(os.environ.get('PSP_LINK_STUB_SET', '0x19c8'), 0)
STUB_PAD_SYMBOL = 'psp_link_stub_pad'
STUB_PAD_SECTION = '.sceStub.text.~psp_link_stub_pad'


def stable(name, impls=True):
    """`name` without its crate hashes (and, with `impls` false, without
    its impl disambiguators: see `key`). A v0 back-reference (`B<base62>_`)
    is a byte offset into the name, so it moves with the hashes' lengths:
    each is re-pointed into the hash-free name, which then does not depend
    on the hashes at all (a generic function's name otherwise matches only
    in builds whose hashes have the searched build's lengths). A name the
    v0 grammar below does not cover keeps its back-references."""
    if name.startswith('_R'):
        try:
            return _V0(name, impls).stable()
        except (ValueError, IndexError, KeyError):
            pass
    return CRATE_HASH.sub('C', name)


def key(name):
    """The name a `link-order.txt` line and a function match on: `stable`
    without the v0 impl disambiguators (`M`/`X` then `s<base62>_`). rustc
    numbers a module's impl blocks in source order, so an impl added above
    another renames every method of the later one: batch 90's `impl
    Display for PackError` above `impl Pack` dropped the whole `Pack` part
    of the order (RE-583). The unlisted functions still sort by `stable`,
    so this changes no layout."""
    return stable(name, impls=False)


class _V0:
    """One pass over a v0 symbol (the Rust v0 mangling grammar), copying it
    without crate disambiguators and with back-references re-encoded."""

    def __init__(self, name, impls=True):
        self.s, self.i, self.out, self.at = name, 2, ['_R'], {}
        self.impls = impls
        self.length = 2

    def stable(self):
        if self.peek().isdigit():
            self.decimal()
        self.path()
        if self.i < len(self.s) and self.s[self.i] != '.':
            self.path()  # the instantiating crate
        self.emit(self.s[self.i:])
        return ''.join(self.out)

    def peek(self):
        return self.s[self.i:self.i + 1]

    def emit(self, text):
        self.out.append(text)
        self.length += len(text)

    def take(self):
        self.emit(self.s[self.i])
        self.i += 1

    def base62(self, emit=True):
        end = self.s.index('_', self.i)
        digits = self.s[self.i:end]
        value = 0
        for c in digits:
            value = value * 62 + BASE62.index(c)
        if emit:
            self.emit(self.s[self.i:end + 1])
        self.i = end + 1
        return value + 1 if digits else 0

    def decimal(self):
        start = self.i
        self.i += 1
        if self.s[start] != '0':
            while self.peek().isdigit():
                self.i += 1
        self.emit(self.s[start:self.i])
        return int(self.s[start:self.i])

    def identifier(self, crate=False):
        if self.peek() == 's':
            if crate:
                self.i += 1
                self.base62(emit=False)
            else:
                self.take()
                self.base62()
        self.undisambiguated()

    def undisambiguated(self):
        if self.peek() == 'u':
            self.take()
        n = self.decimal()
        if self.peek() == '_':
            self.take()
        self.emit(self.s[self.i:self.i + n])
        self.i += n

    def mark(self):
        # Back-references count bytes from just after `_R`.
        self.at[self.i - 2] = self.length - 2

    def backref(self):
        self.i += 1
        v = self.at[self.base62(emit=False)]
        digits = ''
        if v:
            v -= 1
            while True:
                digits = BASE62[v % 62] + digits
                v //= 62
                if not v:
                    break
        self.emit('B' + digits + '_')

    def path(self):
        self.mark()
        c = self.peek()
        if c == 'B':
            return self.backref()
        self.take()
        if c == 'C':
            self.identifier(crate=True)
        elif c == 'M':
            self.impl_path()
            self.type()
        elif c == 'X':
            self.impl_path()
            self.type()
            self.path()
        elif c == 'Y':
            self.type()
            self.path()
        elif c == 'N':
            self.take()
            self.path()
            self.identifier()
        elif c == 'I':
            self.path()
            while self.peek() != 'E':
                self.generic_arg()
            self.take()
        else:
            raise ValueError(c)

    def impl_path(self):
        if self.peek() == 's':
            if self.impls:
                self.take()
                self.base62()
            else:
                self.i += 1
                self.base62(emit=False)
        self.path()

    def generic_arg(self):
        c = self.peek()
        if c == 'L':
            self.take()
            self.base62()
        elif c == 'K':
            self.take()
            self.const()
        else:
            self.type()

    def const(self):
        self.mark()
        c = self.peek()
        if c == 'B':
            return self.backref()
        if c == 'p':
            return self.take()
        self.type()
        if self.peek() == 'n':
            self.take()
        while self.peek() != '_':
            self.take()
        self.take()

    def binder(self):
        if self.peek() == 'G':
            self.take()
            self.base62()

    def lifetime(self):
        if self.peek() == 'L':
            self.take()
            self.base62()

    def type(self):
        c = self.peek()
        if c in 'CMXYNI':
            return self.path()
        self.mark()
        if c in BASIC_TYPES:
            return self.take()
        if c == 'B':
            return self.backref()
        self.take()
        if c == 'A':
            self.type()
            self.const()
        elif c in 'SPO':
            self.type()
        elif c == 'T':
            while self.peek() != 'E':
                self.type()
            self.take()
        elif c in 'RQ':
            self.lifetime()
            self.type()
        elif c == 'F':
            self.binder()
            if self.peek() == 'U':
                self.take()
            if self.peek() == 'K':
                self.take()
                if self.peek() == 'C':
                    self.take()
                else:
                    self.undisambiguated()
            while self.peek() != 'E':
                self.type()
            self.take()
            self.type()
        elif c == 'D':
            self.binder()
            while self.peek() != 'E':
                self.path()
                while self.peek() == 'p':
                    self.take()
                    self.undisambiguated()
                    self.type()
            self.take()
            self.lifetime()
        else:
            raise ValueError(c)


def functions(path):
    """Names of the function symbols in a 32-bit little-endian ELF object."""
    with open(path, 'rb') as f:
        data = f.read()
    if data[:6] != b'\x7fELF\x01\x01':
        return []
    shoff, = struct.unpack_from('<I', data, 0x20)
    shentsize, shnum = struct.unpack_from('<HH', data, 0x2E)
    sections = [struct.unpack_from('<IIIIIIIIII', data, shoff + i * shentsize) for i in range(shnum)]
    names = []
    for sh in sections:
        if sh[1] != 2:  # SHT_SYMTAB
            continue
        strtab = sections[sh[6]]
        for off in range(sh[4], sh[4] + sh[5], 16):
            st_name, _, _, st_info, _, _ = struct.unpack_from('<IIIBBH', data, off)
            if st_info & 0xF == 2 and st_name:  # STT_FUNC
                start = strtab[4] + st_name
                names.append(data[start:data.index(b'\0', start)].decode())
    return names


def main():
    args = sys.argv[1:]
    if args == ['--stable']:
        for line in sys.stdin:
            f = line.rstrip('\n').split(' ', 3)
            if len(f) == 4:
                f[3] = stable(f[3])
            print(' '.join(f))
        return
    out = args[args.index('-o') + 1] if '-o' in args else 'a.out'
    by_key = {}
    for arg in args:
        if arg.endswith('.o') and os.path.isfile(arg):
            for name in functions(arg):
                by_key.setdefault(key(name), []).append(name)
    order = []
    # Lines after `# after unlisted` go after the unlisted functions, so code
    # added for one scene does not move the unlisted code's cache sets for
    # every other scene (RE-585).
    last = []
    into = order
    unmatched = []
    with open(LIST) as f:
        for line in f:
            line = line.strip()
            if line == '# after unlisted':
                into = last
            elif line and not line.startswith('#'):
                names = by_key.get(key(line), [])
                into.extend(names)
                if not names:
                    unmatched.append(line)
    # The order is traced on the timed build (`frame_timer`), so there every
    # line names a function, and one that does not is a function renamed or
    # removed since: the linker warns. A release build also lacks the
    # timer's own functions, listed in OUT.unmatched without a warning.
    if not unmatched and os.path.exists(out + '.unmatched'):
        os.remove(out + '.unmatched')
    if unmatched:
        with open(out + '.unmatched', 'w') as f:
            f.write(''.join(line + '\n' for line in unmatched))
        if any('11frame_timer' in k for k in by_key):
            sys.stderr.write('psp-link: %d link-order lines match no function (%s.unmatched)\n' % (len(unmatched), out))
    # The functions the list leaves out follow by stable name: in rustc's
    # output order they would sit by crate hash, so in another order in
    # every checkout, and their placement moves the listed code's timing
    # (RE-580).
    listed = set(order + last)
    order.extend(sorted((n for names in by_key.values() for n in names if n not in listed), key=lambda n: (stable(n), n)))
    order.extend(last)
    if last:
        order.append(TAIL_PAD_SYMBOL)
    save = os.environ.get('PSP_LINK_SAVE')
    if save:
        # Keep the link's inputs, so another function order can be linked
        # without recompiling: rustc deletes its temporary objects.
        import json, shutil
        os.makedirs(save, exist_ok=True)
        kept = []
        for i, arg in enumerate(args):
            if arg.endswith('.o') and os.path.isfile(arg):
                dst = os.path.join(save, '%d-%s' % (i, os.path.basename(arg)))
                shutil.copy(arg, dst)
                arg = dst
            else:
                # rustc's temporary directory (an rlib, the linker script).
                m = re.search(r'^(.*/rustc[0-9A-Za-z]{6})(/.*)?$', arg)
                if m and os.path.exists(m.group(1)):
                    dst = os.path.join(save, 'rustc-tmp')
                    if not os.path.exists(dst):
                        shutil.copytree(m.group(1), dst)
                    arg = dst + (m.group(2) or '')
            kept.append(arg)
        with open(os.path.join(save, 'args.json'), 'w') as f:
            json.dump({'args': kept, 'out': out, 'cwd': os.getcwd()}, f)
    order_path = out + '.order'
    with open(order_path, 'w') as f:
        f.write(''.join(name + '\n' for name in order))
    lld = ['rust-lld'] + args + ['--symbol-ordering-file=' + order_path, '--no-warn-symbol-ordering']
    run(lld)
    objects = [a for a in args if a.endswith('.o') and os.path.isfile(a)]
    # Pad after the `# after unlisted` functions so the code behind them (the
    # archives' functions) keeps the data- and instruction-cache sets it has
    # without them (RE-585).
    if last and objects:
        tail_path = out + '.tail.o'
        write_pad_object(tail_path, 0, elf_flags(objects[0]), TAIL_PAD_SYMBOL)
        lld = lld[:lld.index('-o')] + [tail_path, '--undefined=' + TAIL_PAD_SYMBOL] + lld[lld.index('-o'):]
        run(lld)
        addr = symbol_addrs(out, [last[0], TAIL_PAD_SYMBOL])
        write_pad_object(tail_path, (addr[last[0]] - addr[TAIL_PAD_SYMBOL]) % DCACHE_WAY, elf_flags(objects[0]), TAIL_PAD_SYMBOL)
        run(lld)
    # Pad the end of the code so the import stubs start at STUB_SET in the
    # instruction cache, and the end of the stubs so `.bss` starts at BSS_SET
    # in the data cache, whatever the code's size (RE-580, RE-585): the
    # stubs, the statics and the kernel blocks after them then map onto the
    # same cache sets in every build, timed or released, before and after a
    # code change. `.eh_frame`, between the two pads, grows with the code.
    # `.rodata` goes first to its line offset (a step under one line), then
    # whole lines move `.bss` to its set without moving `.rodata` in its line.
    pads = [[out + '.pad.o', PAD_SYMBOL, None, 0], [out + '.stubpad.o', STUB_PAD_SYMBOL, STUB_PAD_SECTION, 0]]
    for _ in range(8):
        start = section_addr(out, '.bss')
        rodata = section_addr(out, '.rodata')
        stub = section_addr(out, '.sceStub.text')
        if start is None or not objects:
            break
        if stub is not None and (stub - STUB_SET) % DCACHE_WAY:
            pads[0][3] = (pads[0][3] + STUB_SET - stub) % DCACHE_WAY
        elif rodata is not None and (rodata - RODATA_LINE) % LINE:
            pads[1][3] = (pads[1][3] + (RODATA_LINE - rodata) % LINE) % DCACHE_WAY
        elif (start - BSS_SET) % DCACHE_WAY:
            pads[1][3] = (pads[1][3] + BSS_SET - start) % DCACHE_WAY
        else:
            break
        extra = []
        for path, symbol, section, size in pads:
            write_pad_object(path, size, elf_flags(objects[0]), symbol, section)
            extra += [path, '--undefined=' + symbol]
        run(lld[:lld.index('-o')] + extra + lld[lld.index('-o'):])
    else:
        sys.exit('psp-link: stubs at %#x, .bss at %#x, .rodata at %#x: not at %#x, %#x and line offset %#x in the caches'
                 % (section_addr(out, '.sceStub.text'), section_addr(out, '.bss'), section_addr(out, '.rodata'),
                    STUB_SET, BSS_SET, RODATA_LINE))


def run(cmd):
    import subprocess
    rc = subprocess.run(cmd).returncode
    if rc:
        sys.exit(rc)


def elf_sections(data):
    shoff, = struct.unpack_from('<I', data, 0x20)
    shentsize, shnum, shstrndx = struct.unpack_from('<HHH', data, 0x2E)
    sections = [struct.unpack_from('<IIIIIIIIII', data, shoff + i * shentsize) for i in range(shnum)]
    names = sections[shstrndx][4]
    return {data[names + s[0]:data.index(b'\0', names + s[0])].decode(): s for s in sections}


def section_addr(path, name):
    with open(path, 'rb') as f:
        section = elf_sections(f.read()).get(name)
    return section[3] if section else None


def symbol_addrs(path, names):
    """The addresses of `names` in a linked ELF."""
    with open(path, 'rb') as f:
        data = f.read()
    symtab = elf_sections(data)['.symtab']
    strtab = elf_sections(data)['.strtab']
    found = {}
    for off in range(symtab[4], symtab[4] + symtab[5], 16):
        st_name, st_value = struct.unpack_from('<II', data, off)
        start = strtab[4] + st_name
        name = data[start:data.index(b'\0', start)].decode(errors='replace')
        if name in names:
            found[name] = st_value
    return found


def elf_flags(path):
    with open(path, 'rb') as f:
        return struct.unpack_from('<I', f.read(0x28), 0x24)[0]


def write_pad_object(path, size, flags, symbol=PAD_SYMBOL, section=None):
    """A relocatable MIPS object holding `size` zero bytes of code in
    `section` (`.text.<symbol>` by default), defined by `symbol` (kept
    through `--gc-sections` by `--undefined`)."""
    shstr = b'\0' + (section or '.text.' + symbol).encode() + b'\0.symtab\0.strtab\0.shstrtab\0'
    strtab = b'\0' + symbol.encode() + b'\0'
    symtab = bytes(16) + struct.pack('<IIIBBH', 1, 0, 0, 0x10, 0, 1)  # STB_GLOBAL, STT_NOTYPE
    body = bytes(size) + symtab + strtab + shstr
    text_off = 0x34
    sym_off = text_off + size
    str_off = sym_off + len(symtab)
    shs_off = str_off + len(strtab)
    shoff = (shs_off + len(shstr) + 3) & ~3
    sh = [
        (0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
        (1, 1, 6, 0, text_off, size, 0, 0, 4, 0),  # SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR
        (shstr.index(b'.symtab'), 2, 0, 0, sym_off, len(symtab), 3, 1, 4, 16),  # SHT_SYMTAB
        (shstr.index(b'.strtab'), 3, 0, 0, str_off, len(strtab), 0, 0, 1, 0),  # SHT_STRTAB
        (shstr.index(b'.shstrtab'), 3, 0, 0, shs_off, len(shstr), 0, 0, 1, 0),
    ]
    header = b'\x7fELF\x01\x01\x01' + bytes(9) + struct.pack(
        '<HHIIIIIHHHHHH', 1, 8, 1, 0, 0, shoff, flags, 0x34, 0, 0, 40, len(sh), 4)
    with open(path, 'wb') as f:
        f.write(header + body + bytes(shoff - text_off - len(body)))
        for s in sh:
            f.write(struct.pack('<IIIIIIIIII', *s))


if __name__ == '__main__':
    main()
