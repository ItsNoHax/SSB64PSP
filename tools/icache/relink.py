#!/usr/bin/env python3
# Links a saved `psp-game` build with another function order, without
# recompiling (RE-494): a PRX for a hardware A/B of a layout in seconds.
# Build once with PSP_LINK_SAVE=DIR set (tools/psp-link.py then keeps the
# link's inputs in DIR), then:
# usage: relink.py DIR link-order.txt OUT_DIR
#        relink.py --functions DIR > orderable.txt
# OUT_DIR gets the ELF (ssb64-psp-game) and PRX (ssb64-psp-game.prx). The
# steps after the link are cargo-psp's: fix the import stub counts, prxgen.
# --functions prints the names link-order.txt can order: the functions of
# the linked objects, as tools/psp-link.py matches them (layout.c's input).
import glob, importlib.util, json, os, struct, subprocess, sys

here = os.path.dirname(os.path.abspath(__file__))
if sys.argv[1] == '--functions':
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location('psp_link', os.path.join(here, '..', 'psp-link.py'))
    link = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(link)
    args = json.load(open(os.path.join(sys.argv[2], 'args.json')))['args']
    names = {link.stable(n) for a in args if a.endswith('.o') for n in link.functions(a)}
    print('\n'.join(sorted(names)))
    sys.exit(0)
save, order, out_dir = sys.argv[1:4]
game = os.path.join(here, '..', '..', 'psp-game')
os.makedirs(out_dir, exist_ok=True)
elf = os.path.join(out_dir, 'ssb64-psp-game')

args = json.load(open(os.path.join(save, 'args.json')))['args']
args[args.index('-o') + 1] = os.path.abspath(elf)
sysroot = subprocess.run(['rustc', '--print', 'sysroot'], cwd=game, capture_output=True, text=True, check=True).stdout.strip()
env = dict(os.environ, PSP_LINK_ORDER=os.path.abspath(order))
env['PATH'] = os.pathsep.join(glob.glob(os.path.join(sysroot, 'lib', 'rustlib', '*', 'bin')) + [env['PATH']])
subprocess.run([sys.executable, os.path.join(here, '..', 'psp-link.py')] + args, cwd=game, env=env, check=True)

# cargo-psp's fix_imports: LTO can drop imported functions, so each
# `.lib.stub` entry's stub_count is set from where the next library's NIDs
# start in `.rodata.sceNid`.
with open(elf, 'rb') as f:
    b = bytearray(f.read())
shoff, = struct.unpack_from('<I', b, 0x20)
shentsize, shnum, shstrndx = struct.unpack_from('<HHH', b, 0x2E)
sh = [struct.unpack_from('<IIIIIIIIII', b, shoff + i * shentsize) for i in range(shnum)]
strtab = sh[shstrndx][4]
sec = {b[strtab + s[0]:b.index(0, strtab + s[0])].decode(): s for s in sh}
if '.lib.stub' in sec:
    start, end = sec['.lib.stub'][4], sec['.lib.stub.btm'][4]
    nid = sec['.rodata.sceNid']
    entries = [(start + i, struct.unpack_from('<I', b, start + i + 12)[0]) for i in range(0, end - start, 20)]
    tables = sorted(t for _, t in entries) + [nid[3] + nid[5]]
    for off, table in entries:
        struct.pack_into('<H', b, off + 10, (tables[tables.index(table) + 1] - table) // 4)
    with open(elf, 'wb') as f:
        f.write(b)
subprocess.run(['prxgen', elf, elf + '.prx'], check=True, stdout=subprocess.DEVNULL)
