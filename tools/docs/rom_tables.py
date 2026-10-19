"""Find numeric tables committed in the port's crates (D-048 ratchet).

Three scans over every Rust crate but `crates/ssb-tablegen` (whose tables
are the ROM layout by design): `crates/ssb-game`, `ssb-rom`, `ssb-engine`,
`ssb-capture`, `psp-runtime`, `psp-game`, `psp-asset-viewer` and
`tools/romtool`:

- large: a const or static array (or slice) whose initializer holds 8 or
  more numeric literals (24 in test code: `*tests*.rs`, `tests/`, or past a
  file's first `#[cfg(test)]` module) may be a hand transcription of a table
  the game reads from the ROM, unless the initializer reads the generated
  `d_arrays`;
- struct: a const or static of any other type (a struct literal, a tuple)
  holding 8 or more numeric literals, likewise;
- cited: a const or static of any type whose initializer holds 2 or more
  numeric literals and whose doc comment (the comment and attribute lines
  directly above it) names a decomp data symbol is the port's copy of that
  data, unless the initializer reads the generated `crate::d_arrays` or the
  doc links it (a helper the initializer calls reads it): its literals are
  then indices and conversions. A data symbol is a `d*` name with a module
  prefix (`dIFCommon...`, `dSC1PGame...`) or a fighter's (`dMarioMain_attr`,
  `dYoshiMainMotion_0x16F8`), an `ll*` reloc label, or a
  `relocData/NNN_*.c` file.

Literals inside an index (`d_arrays::FOO[3]`, `x[i][2]`) are positions, not
data, and are not counted.

Such tables are generated at build time by `crates/ssb-tablegen` instead
(D-048). `tools/docs/rom_tables_baseline.tsv` lists the tables known today:
layout (offsets, sizes, counts, SpriteFile manifests, ll* labels),
fixtures, tooling, port constants, function-body constants (outside D-048)
and tables that cite a symbol without copying it. `validate_docs.py` fails on a hit not listed there, and on a
listed one that no longer is a hit, so the list only shrinks.

Not seen: a table of fewer than 8 literals whose doc names no data symbol,
a table that reads `d_arrays` and also keeps ROM literals of its own, and
values written in `match` arms or function bodies (D-048 leaves those to
review).

Usage: python3 tools/docs/rom_tables.py   # print every hit as TSV
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATES = (
    "crates/ssb-game",
    "crates/ssb-rom",
    "crates/ssb-engine",
    "crates/ssb-capture",
    "psp-runtime",
    "psp-game",
    "psp-asset-viewer",
    "tools/romtool",
)
BASELINE = ROOT / "tools" / "docs" / "rom_tables_baseline.tsv"
MIN_LITERALS = 8
MIN_TEST_LITERALS = 24
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
# digits (`IF`, `SC1P`, `MN`), then a capitalised word; or `d`, a fighter's
# name and its file (`dMarioMain_attr`, `dYoshiMainMotion_0x16F8`); an
# `ll*` reloc label; or a `relocData` source file.
DSYM_RE = re.compile(
    r"\bd[A-Z][A-Z0-9]+[A-Z][a-z]\w*"
    r"|\bd[A-Z][a-z]+(?:Main|MainMotion|Special\d|Model)\w*"
    r"|\bll[A-Z][A-Za-z0-9]+[A-Z]\w*"
    r"|\brelocData/\d+_\w+\.c")
# An index into an array: `[...]` right after a name, `]` or `)`. Its
# literals are positions, not data (`d_arrays::FOO[3]`).
INDEX_RE = re.compile(r"(?<=[\w\])])\[[^\[\]]*\]")
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


def _count(body):
    """The numeric literals in `body`, leaving out array indices."""
    while True:
        stripped = INDEX_RE.sub("", body)
        if stripped == body:
            return len(NUM_RE.findall(body))
        body = stripped


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


def _is_test(rel):
    """A test file: `tests/`, or `*tests*.rs` (the crates' `#[cfg(test)]`
    modules in their own files)."""
    parts = Path(rel).parts
    return "tests" in parts or "tests" in parts[-1]


def _test_start(text):
    """Offset of the first `#[cfg(test)]` module in a file: items past it
    are test items."""
    m = re.search(r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", text)
    return m.start() if m else len(text)


def _least(rel, text, at):
    """The literal count that makes the item at offset `at` a large table."""
    test = _is_test(rel) or at >= _test_start(text)
    return MIN_TEST_LITERALS if test else MIN_LITERALS


def _sources():
    for crate in CRATES:
        for path in sorted((ROOT / crate).rglob("*.rs")):
            if "target" in path.relative_to(ROOT).parts:
                continue
            raw = path.read_text(encoding="utf-8", errors="ignore")
            text = COMMENT_RE.sub(_blank, STRING_RE.sub(_blank, raw))
            yield str(path.relative_to(ROOT)), raw, text


def scan():
    """Yield (relative path, item name, literal count) for every large table:
    arrays, then other items (struct literals)."""
    for rel, _, text in _sources():
        arrays = set()
        for m in DECL_RE.finditer(text):
            arrays.add(m.start())
            body = _initializer(text, m.end())
            count = _count(body)
            if "d_arrays" not in body and count >= _least(rel, text, m.start()):
                yield rel, m.group(1), count
        for m in ITEM_RE.finditer(text):
            if m.start() in arrays:
                continue
            body = _initializer(text, m.end())
            count = _count(body)
            if "d_arrays" not in body and count >= _least(rel, text, m.start()):
                yield rel, m.group(1), count


def scan_cited():
    """Yield (relative path, item name, literal count, symbols) for every
    smaller const or static whose doc comment names a `d*` symbol."""
    for rel, raw, text in _sources():
        lines = raw.split("\n")
        for m in ITEM_RE.finditer(text):
            body = _initializer(text, m.end())
            count = _count(body)
            if count < MIN_CITED_LITERALS or count >= _least(rel, text, m.start()):
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
