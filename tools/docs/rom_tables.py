"""Find large numeric tables committed in the portable crates (D-048 ratchet).

A const or static array (or slice) in `crates/ssb-game`, `crates/ssb-rom` or
`crates/ssb-engine` whose initializer holds 24 or more numeric literals may
be a hand transcription of a table the game reads from the ROM. Such tables
are generated at build time by `crates/ssb-tablegen` instead (D-048).
`tools/docs/rom_tables_baseline.tsv` lists the tables known today: layout
(offsets, sizes, counts), fixtures, port constants and the open D-048
groups in PLAN.md. `validate_docs.py` fails on a large table not listed
there, and on a listed one that no longer exists, so the list only shrinks.

Usage: python3 tools/docs/rom_tables.py   # print every hit as TSV
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATES = ("crates/ssb-game", "crates/ssb-rom", "crates/ssb-engine")
BASELINE = ROOT / "tools" / "docs" / "rom_tables_baseline.tsv"
MIN_LITERALS = 24

DECL_RE = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:\s*(&?\s*(?:'static\s+)?\[[^=]*?)=(?![=>])",
    re.M)
NUM_RE = re.compile(
    r"(?<![\w.])(?:0[xX][0-9a-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)"
    r"(?:_?(?:u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize|f32|f64))?(?![\w.])")
COMMENT_RE = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
STRING_RE = re.compile(r'b?"(?:\\.|[^"\\])*"')


def _initializer(text, start):
    """Text from `start` to the `;` that ends the item, at bracket depth 0."""
    depth = 0
    for i in range(start, len(text)):
        c = text[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        elif c == ";" and depth == 0:
            return text[start:i]
    return text[start:]


def scan():
    """Yield (relative path, item name, literal count) for every large table."""
    for crate in CRATES:
        for path in sorted((ROOT / crate).rglob("*.rs")):
            if "target" in path.relative_to(ROOT).parts:
                continue
            raw = path.read_text(encoding="utf-8", errors="ignore")
            text = COMMENT_RE.sub(" ", STRING_RE.sub('""', raw))
            for m in DECL_RE.finditer(text):
                body = _initializer(text, m.end())
                count = len(NUM_RE.findall(body))
                if count >= MIN_LITERALS:
                    yield str(path.relative_to(ROOT)), m.group(1), count


def baseline():
    """{(path, name): reason} from the baseline file."""
    known = {}
    for line in BASELINE.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        known[(cols[0], cols[1])] = cols[2] if len(cols) > 2 else ""
    return known


def problems():
    """One line per unlisted large table and per stale baseline row."""
    out = []
    known = baseline()
    found = set()
    for path, name, count in scan():
        found.add((path, name))
        if (path, name) not in known:
            out.append(f"{path}: {name} holds {count} numeric literals and is not in "
                       "tools/docs/rom_tables_baseline.tsv; generate a ROM table with "
                       "crates/ssb-tablegen (D-048) or list it with its reason")
    for path, name in sorted(set(known) - found):
        out.append(f"tools/docs/rom_tables_baseline.tsv: {path} {name} is no longer a large "
                   "table; remove the row")
    return out


if __name__ == "__main__":
    for path, name, count in scan():
        print(f"{path}\t{name}\t{count}")
