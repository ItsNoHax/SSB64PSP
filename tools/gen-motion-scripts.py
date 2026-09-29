#!/usr/bin/env python3
"""Generate ssb-game's fighter motion scripts from the decompilation.

A status's hitboxes, hit-status (intangibility) windows and flag timings are
not tables in the original: they are `ftMotionCommand` bytecode in each
fighter's `relocData/<id>_<Name>MainMotion.c`, which `ftMainSetStatus` starts
and `ftMainUpdateMotionEventsAll` runs frame by frame. This script rebuilds
that bytecode word for word -- each `ftMotionCommand*()` macro is expanded
through `src/ft/ftdef.h`'s own definitions -- so `ssb_game::motion` can run
the same interpreter over the same words. Pointers (`Goto`, `Subroutine`,
`SetParallelScript`) become word indices into the fighter's script blob.

For every motion it also records the figatree length the decompilation's
animation sources give (`tools/gen-anim-table.py`'s decoder), which is what
`ftAnimEndCheckSetStatus` and friends wait for.

The layout is checked against each file's recorded US size, so a macro
expanded to the wrong number of words fails the build of the table instead of
silently shifting every later offset.

Usage:
    tools/gen-motion-scripts.py [--refs refs/ssb-decomp-re]
"""
import argparse
import importlib.util
import os
import re
import sys

PROJECT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(PROJECT, "crates/ssb-game/src/motion/scripts.rs")

spec = importlib.util.spec_from_file_location(
    "gen_anim_table", os.path.join(PROJECT, "tools/gen-anim-table.py"))
anim = importlib.util.module_from_spec(spec)
spec.loader.exec_module(anim)

# (fighter, MainMotion file, special status header, special status enum prefix)
FIGHTERS = [
    ("Mario", "202_MarioMainMotion.c", "ftmario/ftmariostatus.h"),
    ("Fox", "208_FoxMainMotion.c", "ftfox/ftfoxstatus.h"),
    ("Donkey", "212_DonkeyMainMotion.c", "ftdonkey/ftdonkeystatus.h"),
    ("Samus", "216_SamusMainMotion.c", "ftsamus/ftsamusstatus.h"),
    ("Luigi", "220_LuigiMainMotion.c", "ftluigi/ftluigistatus.h"),
    ("Link", "224_LinkMainMotion.c", "ftlink/ftlinkstatus.h"),
    ("Yoshi", "246_YoshiMainMotion.c", "ftyoshi/ftyoshistatus.h"),
    ("Captain", "235_CaptainMainMotion.c", "ftcaptain/ftcaptainstatus.h"),
    ("Kirby", "228_KirbyMainMotion.c", "ftkirby/ftkirbystatus.h"),
    ("Pikachu", "242_PikachuMainMotion.c", "ftpikachu/ftpikachustatus.h"),
    ("Purin", "232_PurinMainMotion.c", "ftpurin/ftpurinstatus.h"),
    ("Ness", "238_NessMainMotion.c", "ftness/ftnessstatus.h"),
]

COMMENT_RE = re.compile(r"/\*.*?\*/|//[^\n]*", re.S)
NONE_PTR = 0xFFFFFFFF
# Word indexes into the shared `FTCommonMoveset` file carry this bit.
COMMON_BIT = 0x40000000


def split_args(text):
    out, depth, cur = [], 0, ""
    for ch in text:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def enum_values(src, enum_name, known=None):
    body = re.search(r"typedef enum " + enum_name + r"\s*\{(.*?)\}", src, re.S).group(1)
    body = COMMENT_RE.sub(" ", body)
    out, nxt = {}, 0
    for item in split_args(body):
        m = re.match(r"(\w+)\s*(?:=\s*(.+))?$", item.strip())
        if not m:
            continue
        name, val = m.group(1), m.group(2)
        if val is not None:
            val = val.strip()
            if val in out:
                nxt = out[val]
            elif known and val in known:
                nxt = known[val]
            else:
                nxt = int(val, 0)
        out[name] = nxt
        nxt += 1
    return out


class Macros:
    def __init__(self, ftdef):
        text = ftdef.replace("\\\n", " ")
        self.defs = {}
        for m in re.finditer(r"^#define\s+(ftMotion\w+)\(([^)]*)\)\s*(.*)$", text, re.M):
            params = [p.strip() for p in m.group(2).split(",") if p.strip()]
            body = COMMENT_RE.sub(" ", m.group(3)).strip()
            self.defs[m.group(1)] = (params, body)
        self.enums = enum_values(ftdef, "FTMotionEvent")
        self.unknown = set()

    def expand(self, call):
        """A macro invocation -> list of word expressions."""
        m = re.match(r"(\w+)\s*\((.*)\)$", call.strip(), re.S)
        if not m or m.group(1) not in self.defs:
            return [call.strip()]
        params, body = self.defs[m.group(1)]
        args = split_args(m.group(2))
        if len(args) != len(params):
            raise ValueError(f"{call}: {len(args)} args for {params}")
        for p, a in zip(params, args):
            body = re.sub(r"\b" + re.escape(p) + r"\b", "(" + a + ")", body)
        words = []
        for piece in split_args(body):
            piece = piece.strip()
            while piece.startswith("(") and piece.endswith(")") and self._balanced(piece[1:-1]):
                inner = piece[1:-1].strip()
                if re.match(r"\w+\s*\(.*\)$", inner, re.S) and inner.split("(")[0].strip() in self.defs:
                    piece = inner
                else:
                    break
            if re.match(r"ftMotion\w+\s*\(", piece):
                words += self.expand(piece)
            else:
                words.append(piece)
        return words

    @staticmethod
    def _balanced(s):
        depth = 0
        for ch in s:
            depth += ch == "("
            depth -= ch == ")"
            if depth < 0:
                return False
        return depth == 0


def fieldset(x, start, length):
    return (x & ((1 << length) - 1)) << start


def evaluate(expr, macros, symbols):
    """-> int word, or ("ptr", symbol, byte offset)."""
    e = re.sub(r"\((?:u32|s32|uintptr_t|void|u16|s16|u8|ftMotionCommand)\s*\*?\s*\)", "", expr)
    e = e.replace("GC_FIELDSET", "F")
    stripped = e.strip().strip("()").strip()
    m = re.match(r"\(*\s*&?\s*(\w+)\s*\)*\s*(?:\+\s*(0x[0-9A-Fa-f]+|\d+))?\s*\)*$", stripped)
    if m and (m.group(1) in symbols or m.group(1).startswith("dFTCommonMoveset") or
              re.match(r"d\w+MainMotion_", m.group(1))):
        return ("ptr", m.group(1), int(m.group(2), 0) if m.group(2) else 0)

    def ident(mo):
        name = mo.group(0)
        if name in ("F",):
            return name
        if name in macros.enums:
            return str(macros.enums[name])
        if name in ("TRUE",):
            return "1"
        if name in ("FALSE", "NULL"):
            return "0"
        if re.match(r"0x[0-9A-Fa-f]+$", name) or name.isdigit():
            return name
        macros.unknown.add(name)
        return "0"
    e = re.sub(r"\b[A-Za-z_]\w*\b", ident, e)
    e = re.sub(r"(\d+)[uU]\b", r"\1", e)
    try:
        val = eval(e, {"__builtins__": {}}, {"F": fieldset})
    except SyntaxError:
        raise ValueError(f"cannot evaluate {expr!r} -> {e!r}")
    return val & 0xFFFFFFFF


def preprocess(text):
    """Keeps the `REGION_US` side of `#if defined(REGION_..)` blocks."""
    out, stack = [], []
    for line in text.split("\n"):
        t = line.strip()
        m = re.match(r"#if\s+(!?)defined\((\w+)\)$", t)
        if m:
            stack.append((m.group(2) == "REGION_US") != (m.group(1) == "!"))
            continue
        if t == "#else":
            stack[-1] = not stack[-1]
            continue
        if t == "#endif":
            stack.pop()
            continue
        if t.startswith("#if"):
            raise ValueError(f"unhandled conditional {t!r}")
        if all(stack) and not t.startswith("#include"):
            out.append(line)
    return "\n".join(out)


def parse_file(path, macros, common=None, base=0):
    src = COMMENT_RE.sub(" ", preprocess(open(path).read()))
    size = re.search(r"File size: US (\d+) bytes", open(path).read())
    arrays = []
    for m in re.finditer(r"^(\w+)\s+(\w+)\s*(\[[^\]]*\])?\s*=\s*\{(.*?)\n\};", src, re.M | re.S):
        if m.group(1) == "ftMotionCommand" or "ftMotion" in m.group(4):
            items = [i for i in split_args(m.group(4)) if i.strip()]
            words = []
            for item in items:
                words += macros.expand(item)
        else:
            # Embedded data (`FTThrowHitDesc`, `FTSpecialColl`): every scalar
            # is one 32-bit word. Only its size matters to the scripts.
            scalars = [t for t in re.split(r"[{},\s]+", m.group(4)) if t]
            if m.group(1) == "FTKirbyCopy":
                # `{ s16 fkind, u16 copy, f32 scale, s32 damage }`: 12 bytes.
                words = ["0"] * (len(scalars) // 4 * 3)
            else:
                per_word = 2 if m.group(1) == "Vec2h" else 1  # s16 pairs
                words = ["0"] * (len(scalars) // per_word)
        arrays.append((m.group(2), words))
    symbols, cursor = {}, 0
    for name, words in arrays:
        symbols[name] = cursor
        cursor += len(words)
    for name, off in symbols.items():
        # Auto-named arrays carry their own US file offset.
        mo = re.search(r"_0x([0-9A-F]{4})$", name)
        if mo and int(mo.group(1), 16) != off * 4:
            raise ValueError(f"{path}: {name} lands at 0x{off * 4:04X}")
    # The archive pads each file to 16 bytes.
    if size and not 0 <= int(size.group(1)) - cursor * 4 < 16:
        raise ValueError(f"{path}: {cursor * 4} bytes, file records {size.group(1)}")
    out = []
    for name, words in arrays:
        for w in words:
            v = evaluate(w, macros, symbols)
            if isinstance(v, tuple):
                _, sym, off = v
                if sym in symbols:
                    if off % 4:
                        raise ValueError(f"{sym} + {off}: unaligned")
                    v = base + symbols[sym] + off // 4
                elif common and sym in common:
                    v = COMMON_BIT | (common[sym] + off // 4)
                else:
                    v = NONE_PTR  # throw/damage descriptors
            out.append(v)
    return out, symbols


MAIN_FILES = {
    "Mario": "203_MarioMain.c", "Fox": "209_FoxMain.c", "Donkey": "213_DonkeyMain.c",
    "Samus": "217_SamusMain.c", "Luigi": "221_LuigiMain.c", "Link": "225_LinkMain.c",
    "Yoshi": "247_YoshiMain.c", "Captain": "236_CaptainMain.c",
    "Kirby": "229_KirbyMain.c", "Pikachu": "243_PikachuMain.c", "Purin": "233_PurinMain.c",
    "Ness": "239_NessMain.c",
}

ATTR_FIELDS = ["size", "rebound_anim_length", "shield_size", "shield_break_vel_y",
               "jostle_width", "jostle_x", "hit_detect_range", "effect_joint_ids",
               "joint_itemlight_id"]


def fl(x):
    """A Rust `f32` literal."""
    t = repr(float(x))
    return t if "e" not in t else f"{x:.6f}"


def combat_attrs(path):
    """`FTAttributes` fields the combat code reads, by their `/* name */`."""
    src = preprocess(open(path).read())
    body = re.search(r"FTAttributes \w+_attr = \{(.*?)\n\};", src, re.S).group(1)
    out = {}
    for m in re.finditer(r"^\s*(.+?),\s*/\*\s*(\w+)\s*\*/", body, re.M):
        if m.group(2) in ATTR_FIELDS:
            nums = [float(x.rstrip("fF")) for x in re.findall(r"-?\d+\.?\d*[fF]?", m.group(1))]
            out[m.group(2)] = nums if len(nums) > 1 else nums[0]
    missing = [f for f in ATTR_FIELDS if f not in out]
    if missing:
        raise ValueError(f"{path}: missing {missing}")
    return out


def motion_descs(refs):
    """Fighter -> [(anim symbol or None, script word expr)]."""
    src = COMMENT_RE.sub(" ", open(os.path.join(refs, "src/ft/ftdata.c")).read())
    out = {}
    for m in re.finditer(r"^FTMotionDesc dFT(\w+)MotionDescs\[\]\s*=\s*$", src, re.M):
        start = src.index("{", m.end())
        body = src[start + 1:src.index("\n};", start)]
        words = [w.strip() for w in body.replace("{", " ").replace("}", " ").split(",")]
        words = [w for w in words if w]
        entries = []
        for i in range(0, len(words), 3):
            sym = re.match(r"&ll(\w+?)FileID$", words[i])
            entries.append((sym.group(1) if sym else None, words[i + 1]))
        out[m.group(1)] = entries
    return out


def status_motion_ids(path, motion_enum):
    src = COMMENT_RE.sub(" ", open(path).read())
    out = {}
    for m in re.finditer(r"FTStatusDesc\s+\w+\[[^\]]*\]\s*=", src):
        pass
    raw = open(path).read()
    parts = re.split(r"//\s*Status (\d+) \(0x[0-9A-Fa-f]+\):[^\n]*", raw)
    for i in range(1, len(parts), 2):
        body = COMMENT_RE.sub(" ", parts[i + 1])
        first = body.strip().lstrip("{").strip().split(",")[0].strip()
        if first in motion_enum:
            out[int(parts[i])] = motion_enum[first]
        else:
            out[int(parts[i])] = int(first, 0)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--refs", default=os.path.join(PROJECT, "refs/ssb-decomp-re"))
    args = ap.parse_args()
    refs = args.refs
    ftdef = open(os.path.join(refs, "src/ft/ftdef.h")).read()
    macros = Macros(ftdef)
    # `SetColAnim` and `Effect` name their colour animation and effect by
    # enum (`GMColAnimKind`, `gm/gmdef.h`; `efKind`, `ef/efdef.h`, US side).
    for header, enum_name in (("src/gm/gmdef.h", "GMColAnimKind"), ("src/ef/efdef.h", "efKind")):
        text = open(os.path.join(refs, header)).read()
        block = re.search(r"typedef enum " + enum_name + r"\s*\{.*?\}", text, re.S).group(0)
        macros.enums.update(enum_values(preprocess(block), enum_name))
    common_motion = enum_values(ftdef, "FTCommonMotion")
    common_status = enum_values(ftdef, "FTCommonStatus")
    special_start = common_status["nFTCommonStatusSpecialStart"]
    common_ids = status_motion_ids(os.path.join(refs, "src/ft/ftcommon/ftcommonstatus.h"),
                                   common_motion)
    files = anim.anim_files(refs)
    descs = motion_descs(refs)
    frames_cache = {}

    def frames(sym):
        if sym is None or sym not in files:
            return 0
        fid, path = files[sym]
        if fid not in frames_cache:
            try:
                n = anim.file_frames(path)
            except (ValueError, TypeError):
                n = None
            frames_cache[fid] = 0xFFFF if n is None else n
        return frames_cache[fid]

    w = []
    w.append("// Generated by tools/gen-motion-scripts.py from refs/ssb-decomp-re.\n")
    w.append("// Do not edit by hand; re-run the generator instead.\n\n")
    w.append("use super::{CombatAttrs, FighterScripts, MotionDesc, NO_SCRIPT};\n\n")
    n_common = max(common_ids) + 1
    w.append(f"/// `dFTCommonActionStatusDescs[..].mflags.motion_id`, indexed by status id\n")
    w.append(f"/// (`-1`/`-2`: no motion).\n")
    w.append(f"#[rustfmt::skip]\npub static COMMON_STATUS_MOTION: [i16; {n_common}] = [\n")
    row = [str(common_ids.get(i, -1)) for i in range(n_common)]
    for i in range(0, n_common, 16):
        w.append("    " + ", ".join(row[i:i + 16]) + ",\n")
    w.append("];\n\n")
    w.append(f"/// `nFTCommonStatusSpecialStart`.\npub const SPECIAL_STATUS_START: u16 = {special_start};\n\n")
    cwords, csyms = parse_file(os.path.join(refs, "src/relocData", "201_FTCommonMoveset.c"),
                               macros, base=COMMON_BIT)
    w.append("/// `FTCommonMoveset` (relocData 201): the item-swing, damage and star\n"
             "/// scripts fighter scripts call as subroutines. A script index with\n"
             "/// [`super::COMMON_BIT`] set points here.\n")
    w.append(f"#[rustfmt::skip]\npub static COMMON_MOVESET_WORDS: [u32; {len(cwords)}] = [\n")
    for i in range(0, len(cwords), 8):
        w.append("    " + ", ".join(f"0x{x:08X}" for x in cwords[i:i + 8]) + ",\n")
    w.append("];\n\n")
    for name, mfile, shdr in FIGHTERS:
        words, symbols = parse_file(os.path.join(refs, "src/relocData", mfile), macros, csyms)
        header = os.path.join(refs, "src/ft/ftchar", shdr)
        fsrc = open(os.path.join(refs, "src/ft/ftchar", shdr.split("/")[0], shdr.split("/")[0] + ".h")).read()
        m_enum = dict(common_motion)
        m_enum.update(enum_values(fsrc, f"ft{name}Motion", common_motion))
        special = status_motion_ids(header, m_enum)
        motions = []
        for anim_sym, script in descs[name]:
            s = script.strip()
            if s in ("0x80000000", "0"):
                start = NONE_PTR
            elif re.match(r"0x[0-9A-Fa-f]+$", s):
                # A raw file offset (Pikachu's motion 0x.. entry).
                start = int(s, 16) // 4
            else:
                mm = re.match(r"(\w+)\s*(?:\+\s*(0x[0-9A-Fa-f]+|\d+))?$", s)
                off = int(mm.group(2), 0) if mm.group(2) else 0
                if mm.group(1) in symbols:
                    start = symbols[mm.group(1)] + off // 4
                else:
                    start = COMMON_BIT | (csyms[mm.group(1)] + off // 4)
            motions.append((start, frames(anim_sym)))
        up = name.upper()
        w.append(f"#[rustfmt::skip]\nstatic {up}_WORDS: [u32; {len(words)}] = [\n")
        for i in range(0, len(words), 8):
            w.append("    " + ", ".join(f"0x{x:08X}" for x in words[i:i + 8]) + ",\n")
        w.append("];\n\n")
        w.append(f"#[rustfmt::skip]\nstatic {up}_MOTIONS: [MotionDesc; {len(motions)}] = [\n")
        for start, fr in motions:
            s = "NO_SCRIPT" if start == NONE_PTR else str(start)
            w.append(f"    MotionDesc {{ script: {s}, anim_length: {fr} }},\n")
        w.append("];\n\n")
        n_sp = (max(special) - special_start + 1) if special else 0
        sp = [str(special.get(special_start + i, -1)) for i in range(n_sp)]
        w.append(f"#[rustfmt::skip]\nstatic {up}_SPECIAL_MOTION: [i16; {n_sp}] = [{', '.join(sp)}];\n\n")
        a = combat_attrs(os.path.join(refs, "src/relocData", MAIN_FILES[name]))
        r = a["hit_detect_range"]
        w.append(f"pub static {up}_ATTRS: CombatAttrs = CombatAttrs {{\n"
                 f"    size: {fl(a['size'])},\n    rebound_anim_length: {fl(a['rebound_anim_length'])},\n"
                 f"    shield_size: {fl(a['shield_size'])},\n    shield_break_vel_y: {fl(a['shield_break_vel_y'])},\n"
                 f"    jostle_width: {fl(a['jostle_width'])},\n    jostle_x: {fl(a['jostle_x'])},\n"
                 f"    hit_detect_range: [{fl(r[0])}, {fl(r[1])}, {fl(r[2])}],\n"
                 f"    effect_joint_ids: [{', '.join(str(int(j)) for j in a['effect_joint_ids'])}],\n"
                 f"    joint_itemlight_id: {int(a['joint_itemlight_id'])},\n}};\n\n")
        w.append(f"pub static {up}: FighterScripts = FighterScripts {{\n"
                 f"    words: &{up}_WORDS,\n    motions: &{up}_MOTIONS,\n"
                 f"    special_status_motion: &{up}_SPECIAL_MOTION,\n}};\n\n")
    if macros.unknown:
        print("note: identifiers read as 0:", " ".join(sorted(macros.unknown)),
              f"({len(macros.unknown)})", file=sys.stderr)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        f.write("".join(w))
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
