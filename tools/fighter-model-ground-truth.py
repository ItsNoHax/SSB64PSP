#!/usr/bin/env python3
"""Extract each fighter's model-file id from the decomp's relocData naming.

`crates/ssb-rom/src/fighter.rs`'s `common_parts` reads a fighter's mesh/
costume file straight out of the ROM: an intern relocation from
`FTAttributes.commonparts_container` to an `FTCommonPartContainer`, then an
extern relocation from each of its two `FTCommonPart` entries into the model
file (`dobjdesc`). That is a real archive-recorded pairing, not a guess -- but
it still needs an independent check, the same way `tools/mobjsub-ground-
truth.py` checks `mobj.rs` against hand-typed decomp structs.

The decompilation's own relocData source tree gives that check for free: each
fighter's model file is a separate source file named `<id>_<Name>Model.c`
(`dFoxMain_commonparts_container`'s entries point into `313_FoxModel.c`, for
example), where `<Name>` is the same symbol prefix `fighter::FIGHTER_FILES`
already uses for that fighter's `*Main` file. The archive-file id is right in
the filename, assigned by the decomp's own build tooling, so it is ground
truth in the same sense the relocData offsets are: a second, independently
authored source naming the same fact.

    tools/fighter-model-ground-truth.py > /tmp/fighter-model-gt.tsv

Emits TSV: fighter name (matching `FighterFile::name`), model file id.

Not every `FTKind` has its own entry: Giant DK shares Donkey Kong's model
file (scaled up via `FTAttributes.size`) and has no `GDonkeyModel.c` of its
own, so it is deliberately absent here rather than guessed.

Requires refs/ssb-decomp-re to be checked out. Nothing here reads or writes
ROM data; it only parses filenames.
"""

import glob
import os
import re
import sys

FILENAME = re.compile(r"^(\d+)_(\w+)Model\.c$")

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
    paths = sorted(glob.glob(os.path.join(root, "*Model.c")))
    if not paths:
        print(f"no *Model.c sources under {root}", file=sys.stderr)
        return 1

    rows = []
    for path in paths:
        m = FILENAME.match(os.path.basename(path))
        if not m:
            continue
        fid, name = m.groups()
        rows.append((name, int(fid)))

    for name, fid in rows:
        print(f"{name}\t{fid}")

    print(f"{len(rows)} fighter model files named", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
