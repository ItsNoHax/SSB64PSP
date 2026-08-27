#!/usr/bin/env python3
"""Extract DObjDesc array offsets from the decomp, for validating our scanner.

`crates/ssb-rom/src/scene.rs` recovers scene graphs from raw file bytes using
heuristics. The decomp has already typed 363 of these arrays by hand, and its
build byte-compares every one against the original ROM -- so those declarations
are ground truth in the strongest available sense.

    tools/dobjdesc-ground-truth.py > /tmp/gt.tsv
    cargo run --release -p romtool -- scene rom/*.z64 --expect /tmp/gt.tsv

Emits TSV: file-id, byte offset, entry count (terminator included), symbol name.

Requires refs/ssb-decomp-re to be checked out. Nothing here reads or writes ROM
data; it only parses C source.
"""

import os
import re
import sys
import glob

# Arrays such as Mario's joint tree carry region-specific rows. Counting both
# branches inflates the entry count -- which is exactly what made the scanner
# look wrong on file 296 when it was in fact right. Keep the US branch.
IF_JP = re.compile(r"#if\s+defined\(REGION_JP\)(.*?)(?:#else(.*?))?#endif", re.S)

# Only arrays carrying an `@ 0xNNNN` offset comment are usable: without it we
# know the array exists but not where it starts.
DECL = re.compile(
    r"/\*\s*DObjDesc:\s*\S+\s*@\s*(0x[0-9A-Fa-f]+)\s*\(\d+\s*entries\)\s*\*/\s*\n"
    r"DObjDesc\s+(\w+)\s*\[[^\]]*\]\s*=\s*\{(.*?)^\};",
    re.S | re.M,
)
ROW = re.compile(r"\{\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,")

DEFAULT_ROOT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..",
    "refs",
    "ssb-decomp-re",
    "src",
    "relocData",
)


def main() -> int:
    root = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    sources = sorted(glob.glob(os.path.join(root, "*.c")))
    if not sources:
        print(f"no relocData sources under {root}", file=sys.stderr)
        return 1

    count = 0
    for path in sources:
        name = os.path.basename(path)
        fid = re.match(r"(\d+)_", name)
        if not fid:
            continue
        with open(path, encoding="utf-8", errors="replace") as fh:
            text = IF_JP.sub(lambda m: m.group(2) or "", fh.read())
        for decl in DECL.finditer(text):
            offset = int(decl.group(1), 16)
            entries = len(ROW.findall(decl.group(3)))
            print(f"{int(fid.group(1))}\t{offset}\t{entries}\t{decl.group(2)}")
            count += 1

    print(f"{count} annotated DObjDesc arrays", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
