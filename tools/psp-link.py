#!/usr/bin/env python3
"""Linker for `psp-game`: `rust-lld` with the hot functions placed first.

The PSP's 16 KB two-way instruction cache cannot hold the draw's per-mesh
code where LTO happens to lay it out; placing the functions that code runs
through next to each other removes 8% of the draw's instruction-cache
misses on a PSP-2000 (RE-482). `psp-game/link-order.txt` lists them, hottest
first, as v0-mangled names with the crate hashes removed (`stable`): a
crate's hash changes with its feature set and the checkout's path, the rest
of the name does not. This script
reads the function symbols of the objects being linked, maps the listed
names onto them and passes the result to `rust-lld` as a
`--symbol-ordering-file`. It changes where code goes, never the code.

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


def stable(name):
    """`name` without its crate hashes. A v0 back-reference (`B<base62>_`)
    is a byte offset into the name, so it moves with the hashes' lengths:
    each is re-pointed into the hash-free name, which then does not depend
    on the hashes at all (a generic function's name otherwise matches only
    in builds whose hashes have the searched build's lengths). A name the
    v0 grammar below does not cover keeps its back-references."""
    if name.startswith('_R'):
        try:
            return _V0(name).stable()
        except (ValueError, IndexError, KeyError):
            pass
    return CRATE_HASH.sub('C', name)


class _V0:
    """One pass over a v0 symbol (the Rust v0 mangling grammar), copying it
    without crate disambiguators and with back-references re-encoded."""

    def __init__(self, name):
        self.s, self.i, self.out, self.at = name, 2, ['_R'], {}
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
            self.take()
            self.base62()
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
    by_stable = {}
    for arg in args:
        if arg.endswith('.o') and os.path.isfile(arg):
            for name in functions(arg):
                by_stable.setdefault(stable(name), []).append(name)
    order = []
    with open(LIST) as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith('#'):
                order.extend(by_stable.get(line, []))
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
    os.execvp('rust-lld', ['rust-lld'] + args + ['--symbol-ordering-file=' + order_path, '--no-warn-symbol-ordering'])


if __name__ == '__main__':
    main()
