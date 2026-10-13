#!/usr/bin/env python3
"""Linker for `psp-game`: `rust-lld` with the hot functions placed first.

The PSP's 16 KB two-way instruction cache cannot hold the draw's per-mesh
code where LTO happens to lay it out; placing the functions that code runs
through next to each other removes 8% of the draw's instruction-cache
misses on a PSP-2000 (RE-482). `psp-game/link-order.txt` lists them, hottest
first, as v0-mangled names with the crate hashes removed: a crate's hash
changes with its feature set, the rest of the name does not. This script
reads the function symbols of the objects being linked, maps the listed
names onto them and passes the result to `rust-lld` as a
`--symbol-ordering-file`. It changes where code goes, never the code.

Set as `psp-game`'s linker in `psp-game/.cargo/config.toml`; rustc calls it
with `rust-lld`'s arguments and has `rust-lld` on `PATH`.

`PSP_LINK_ORDER` replaces `link-order.txt`. `PSP_LINK_SAVE=DIR` copies the
link's inputs into DIR, so `tools/icache/relink.py` can link them again with
another order without recompiling (RE-494).
"""
import os
import re
import struct
import sys

LIST = os.environ.get('PSP_LINK_ORDER') or os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'psp-game', 'link-order.txt')
# A v0 crate root: `C`, the disambiguator `s<base62>_`, then the crate name's
# `<length><identifier>`.
CRATE_HASH = re.compile(r'Cs[0-9A-Za-z]*_(?=\d)')


def stable(name):
    return CRATE_HASH.sub('C', name)


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
