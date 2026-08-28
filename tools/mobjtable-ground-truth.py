#!/usr/bin/env python3
"""Extract `MObjSub **table[]` offsets from the decomp, for validating the
material-table search.

`crates/ssb-rom/src/mobj.rs` pairs a scene graph with its material table from
records that name both: `FTCommonPart` for fighters, `MPGroundDesc` for stage
layers. 71 graphs are named by neither -- the original passes the two offsets
as link-time constants to a setup function, so the pairing exists only in the
game's code (RE-046). Recovering those means *searching* for a table that fits
a graph, and a search needs an answer key.

This is that key. The decomp declares a material table with a distinctive
type -- `MObjSub **name[N]`, two stars, as against the one-star `MObjSub *[]`
of a chain and the bare `MObjSub[]` of a struct -- and its build byte-compares
every one against the original ROM.

    tools/mobjtable-ground-truth.py > /tmp/mobj-tables.tsv
    cargo run --release -p romtool -- mobj rom/*.z64 --search --expect-tables \\
        /tmp/mobj-tables.tsv

Emits TSV: file-id, byte offset, entry count, symbol name.

Offsets come from the same two sources as tools/mobjsub-ground-truth.py, in the
same order of preference: an `@ 0xNNNN` comment, else the hex numbers the
decomp's auto-namer builds symbol names out of.

Requires refs/ssb-decomp-re to be checked out. Nothing here reads or writes ROM
data; it only parses C source.
"""

import glob
import os
import re
import sys

# Keep the US branch, matching the ROM everything is validated against.
IF_JP = re.compile(r"#if\s+defined\(REGION_JP\)(.*?)(?:#else(.*?))?#endif", re.S)

# `MObjSub **foo[N]` -- two stars is what makes this a table of chains rather
# than a chain of structs. The `@ ...` comment above it is optional and its
# wording varies ("MObjSub-list table", "MObjSub-list pointer"), so only the
# offset is parsed out of it.
DECL = re.compile(
    r"(?:/\*\s*@\s*([^\n]*?)\s*\*/\s*\n)?"
    r"^MObjSub\s*\*\s*\*\s*(\w+)\s*\[\s*(\d*)\s*\]\s*=",
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
    """The table's file offset, or None if neither source yields one."""
    if annotation:
        # The comment reads like "0x1F50 -- MObjSub-list table (4 entries)";
        # the offset is the first hex number in it, and the entry count that
        # follows must not be mistaken for it.
        found = HEX.search(annotation)
        if found:
            return int(found.group(1), 16)
    parts = HEX.findall(symbol)
    return sum(int(p, 16) for p in parts) if parts else None


def main() -> int:
    root = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    sources = sorted(glob.glob(os.path.join(root, "*.c")))
    if not sources:
        print(f"no relocData sources under {root}", file=sys.stderr)
        return 1

    tables = 0
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
            print(f"{int(fid.group(1))}\t{base}\t{count or 1}\t{symbol}")
            tables += 1

    print(
        f"{tables} MObjSub tables ({skipped} declarations skipped: no offset "
        f"comment and no offset in the symbol name)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
