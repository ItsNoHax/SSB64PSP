#!/usr/bin/env python3
"""Extract MObjSub struct offsets from the decomp, for validating our material
extractor.

`crates/ssb-rom/src/mobj.rs` recovers a node's material chain by following
`table[node] -> MObjSub *[] -> MObjSub`, all from raw bytes. The decomp has
typed these structs by hand and its build byte-compares every one against the
original ROM, so their offsets are ground truth in the strongest available
sense.

    tools/mobjsub-ground-truth.py > /tmp/mobj-gt.tsv
    cargo run --release -p romtool -- mobj rom/*.z64 --expect /tmp/mobj-gt.tsv

Emits TSV: file-id, byte offset, symbol name.

Two sources of offset, in order of preference:

1. An `@ 0xNNNN` comment above the declaration. 622 of the 789 declarations
   carry one, and it is authoritative.
2. The symbol name. The decomp's auto-namer builds names from the offsets of a
   symbol and its parents (`..._gap_0x3B24_sub_0x6F4` sits at 0x4218), so
   summing the hex numbers in a name recovers its offset. That is a convention
   rather than a guarantee, so it is only used where no comment exists -- but
   where both are available it agrees 611 times out of 622, the 11 exceptions
   being hand-named symbols with no numbers at all, which are skipped.

Requires refs/ssb-decomp-re to be checked out. Nothing here reads or writes ROM
data; it only parses C source.
"""

import glob
import os
import re
import sys

# `sizeof(MObjSub)`: a declaration of `MObjSub foo[3]` covers three structs.
MOBJSUB_SIZE = 0x78

# Same region handling as tools/dobjdesc-ground-truth.py: keep the US branch,
# since that is the ROM we validate against.
IF_JP = re.compile(r"#if\s+defined\(REGION_JP\)(.*?)(?:#else(.*?))?#endif", re.S)

# An optional `@ ...` comment, then the declaration. `MObjSub *foo[]` is a
# chain array rather than a struct, so the lack of a `*` matters.
DECL = re.compile(
    r"(?:/\*\s*MObjSub\s*@\s*([^*]*?)\s*\*/\s*\n)?"
    r"^MObjSub\s+(\w+)\s*\[\s*(\d*)\s*\]\s*=",
    re.M,
)
HEX = re.compile(r"0x([0-9A-Fa-f]+)")

DEFAULT_ROOT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..",
    "refs",
    "ssb-decomp-re",
    "src",
    "relocData",
)


def offset_of(annotation, symbol):
    """The struct's file offset, or None if neither source yields one."""
    if annotation and HEX.fullmatch(annotation):
        return int(annotation, 16)
    parts = HEX.findall(symbol)
    return sum(int(p, 16) for p in parts) if parts else None


def main() -> int:
    root = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    sources = sorted(glob.glob(os.path.join(root, "*.c")))
    if not sources:
        print(f"no relocData sources under {root}", file=sys.stderr)
        return 1

    structs = 0
    skipped = 0
    for path in sources:
        name = os.path.basename(path)
        fid = re.match(r"(\d+)_", name)
        if not fid:
            continue
        with open(path, encoding="utf-8", errors="replace") as fh:
            text = IF_JP.sub(lambda m: m.group(2) or "", fh.read())
        for decl in DECL.finditer(text):
            annotation, symbol, count = decl.groups()
            base = offset_of(annotation, symbol)
            if base is None:
                skipped += 1
                continue
            for i in range(int(count) if count else 1):
                print(f"{int(fid.group(1))}\t{base + i * MOBJSUB_SIZE}\t{symbol}")
                structs += 1

    print(
        f"{structs} MObjSub structs ({skipped} declarations skipped: no offset "
        f"comment and no offset in the symbol name)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
