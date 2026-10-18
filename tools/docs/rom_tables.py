"""Find numeric tables committed in the portable crates (D-048 ratchet).

Two scans over `crates/ssb-game`, `crates/ssb-rom` and `crates/ssb-engine`:

- large: a const or static array (or slice) whose initializer holds 24 or
  more numeric literals may be a hand transcription of a table the game
  reads from the ROM;
- cited: a const or static of any type whose initializer holds 2 or more
  numeric literals and whose doc comment (the comment and attribute lines
  directly above it) names a decomp data symbol (`dIFCommon...`,
  `dSC1PGame...`: `d`, an upper-case module prefix, then the name) is the
  port's copy of that `d*` array, unless the initializer reads the
  generated `crate::d_arrays` or the doc links it (a helper the initializer
  calls reads it): its literals are then indices and conversions.

Such tables are generated at build time by `crates/ssb-tablegen` instead
(D-048). `tools/docs/rom_tables_baseline.tsv` lists the tables known today:
layout (offsets, sizes, counts), fixtures, port constants, function-local
arrays (an open D-048 question) and tables that cite a symbol without
copying it. `validate_docs.py` fails on a hit not listed there, and on a
listed one that no longer is a hit, so the list only shrinks.

Not seen: a small table whose doc names no `d*` symbol, a small table that
reads `d_arrays` and also keeps ROM literals of its own, values written in
`match` arms or function bodies, and tables outside the three crates.

Usage: python3 tools/docs/rom_tables.py   # print every hit as TSV
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATES = ("crates/ssb-game", "crates/ssb-rom", "crates/ssb-engine")
BASELINE = ROOT / "tools" / "docs" / "rom_tables_baseline.tsv"
MIN_LITERALS = 24
MIN_CITED_LITERALS = 2

DECL_RE = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:\s*(&?\s*(?:'static\s+)?\[[^=]*?)=(?![=>])",
    re.M)
NUM_RE = re.compile(
    r"(?<![\w.])(?:0[xX][0-9a-fA-F_]+|0[bB][01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)"
    r"(?:_?(?:u8|u16|u32|u64|u128|usize|i8|i16|i32|i64|i128|isize|f32|f64))?(?![\w.])")
# Any const/static item: `NAME: TYPE =`.
ITEM_RE = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\s+(?:mut\s+)?([A-Z_][A-Z0-9_]*)\s*:\s*([^=]*?)=(?![=>])",
    re.M)
# A decomp data symbol: `d`, a module prefix of two or more capitals or
# digits (`IF`, `SC1P`, `MN`), then a capitalised word.
DSYM_RE = re.compile(r"\bd[A-Z][A-Z0-9]+[A-Z][a-z]\w*")
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


def _blank(m):
    """`m`'s text with everything but newlines turned to spaces, so offsets
    in the stripped text are offsets in the source."""
    return re.sub(r"[^\n]", " ", m.group())


def _doc(lines, line):
    """The comment and attribute lines directly above line index `line`."""
    out = []
    i = line - 1
    while i >= 0 and re.match(r"\s*(//|#\[)", lines[i]):
        out.append(lines[i])
        i -= 1
    return "\n".join(out)


def _sources():
    for crate in CRATES:
        for path in sorted((ROOT / crate).rglob("*.rs")):
            if "target" in path.relative_to(ROOT).parts:
                continue
            raw = path.read_text(encoding="utf-8", errors="ignore")
            text = COMMENT_RE.sub(_blank, STRING_RE.sub(_blank, raw))
            yield str(path.relative_to(ROOT)), raw, text


def scan():
    """Yield (relative path, item name, literal count) for every large table."""
    for rel, _, text in _sources():
        for m in DECL_RE.finditer(text):
            body = _initializer(text, m.end())
            count = len(NUM_RE.findall(body))
            if count >= MIN_LITERALS:
                yield rel, m.group(1), count


def scan_cited():
    """Yield (relative path, item name, literal count, symbols) for every
    smaller const or static whose doc comment names a `d*` symbol."""
    for rel, raw, text in _sources():
        lines = raw.split("\n")
        for m in ITEM_RE.finditer(text):
            body = _initializer(text, m.end())
            count = len(NUM_RE.findall(body))
            if count < MIN_CITED_LITERALS or count >= MIN_LITERALS:
                continue
            doc = _doc(lines, text.count("\n", 0, m.start()))
            if "d_arrays" in body or "crate::d_arrays" in doc:
                continue
            symbols = sorted(set(DSYM_RE.findall(doc)))
            if symbols:
                yield rel, m.group(1), count, symbols


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
    for path, name, count, symbols in scan_cited():
        found.add((path, name))
        if (path, name) not in known:
            out.append(f"{path}: {name} holds {count} numeric literals and its doc names "
                       f"{', '.join(symbols)}; generate the d* array with crates/ssb-tablegen "
                       "(D-048) or list it in tools/docs/rom_tables_baseline.tsv with its reason")
    for path, name in sorted(set(known) - found):
        out.append(f"tools/docs/rom_tables_baseline.tsv: {path} {name} is no longer a large "
                   "or d*-citing table; remove the row")
    return out


if __name__ == "__main__":
    for path, name, count in scan():
        print(f"{path}\t{name}\t{count}")
    for path, name, count, symbols in scan_cited():
        print(f"{path}\t{name}\t{count}\t{','.join(symbols)}")
