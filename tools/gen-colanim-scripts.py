#!/usr/bin/env python3
"""Generate ssb-game's colour-animation scripts from the decompilation.

`dGMColScriptsDescs` (`src/gm/gmcolscripts.c`) is the table
`ftParamCheckSetColAnimID` indexes by colour-animation id: each entry names
a `u32` script written with the `gmColCommand*` macros of `src/gm/gmdef.h`,
its priority and whether a status change ends it (`is_unlocked`). This
script transcribes every script (US side of `#if defined(REGION_..)`) into
`ColEvent`s so `ssb_game::colanim` runs the same events as
`ftMainUpdateColAnim`, and the table in its own order.

A script that runs off its end into the next array (`FoxSpecialHiStart`,
`PikachuSpecialHiStart`) gets an explicit `Goto` to that array, which is
what falling through does.

The table is indexed by id, and the ids are the table's order: the
`GMColAnimKind` names 41 to 49 do not match the scripts at those indices
(for example id 41, `nGMColAnimFighterDonkeySpecialNLoop`, is
`dGMColScriptsFighterFoxSpecialLw`, and Fox's motion script sets it), so the
generated id constants are named after the table's scripts.

Usage:
    tools/gen-colanim-scripts.py [--refs refs/ssb-decomp-re]
"""
import argparse
import os
import re

PROJECT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(PROJECT, "crates/ssb-game/src/colanim_scripts.rs")
COMMENT_RE = re.compile(r"/\*.*?\*/|//[^\n]*", re.S)


def preprocess(text):
    """Keeps the `REGION_US` side of `#if defined(REGION_..)` blocks."""
    out, stack = [], []
    for line in text.split("\n"):
        t = line.strip()
        m = re.match(r"#if\s+(!?)\s*defined\s*\(\s*(\w+)\s*\)$", t)
        if m:
            region = m.group(2).startswith("REGION_")
            stack.append((m.group(2) == "REGION_US" or not region) != (m.group(1) == "!"))
            continue
        if t.startswith("#if"):
            # Include guards and the like: kept.
            stack.append(True)
            continue
        if t == "#else":
            stack[-1] = not stack[-1]
            continue
        if t == "#endif":
            stack.pop()
            continue
        if all(stack):
            out.append(line)
    return "\n".join(out)


def enum_values(text, name):
    block = re.search(r"typedef enum " + name + r"\s*\{(.*?)\}", text, re.S).group(1)
    block = COMMENT_RE.sub(" ", preprocess(block))
    out, nxt = {}, 0
    for item in block.split(","):
        item = item.strip()
        if not item:
            continue
        if "=" in item:
            key, val = (x.strip() for x in item.split("=", 1))
            nxt = out[val] if val in out else int(val, 0)
        else:
            key = item
        out[key] = nxt
        nxt += 1
    return out


def split_args(s):
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
            continue
        depth += ch == "("
        depth -= ch == ")"
        cur += ch
    if cur.strip():
        out.append(cur.strip())
    return out


def camel(sym):
    return sym[len("dGMColScripts"):]


def screaming(name):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name).upper()


FIGHTER_ENUMS = [
    # (FighterKind ordinal, header, enum, function prefix)
    (0, "ftchar/ftmario/ftmario.h", "ftMarioStatus", "Mario"),
    (1, "ftchar/ftfox/ftfox.h", "ftFoxStatus", "Fox"),
    (2, "ftchar/ftdonkey/ftdonkey.h", "ftDonkeyStatus", "Donkey"),
    (3, "ftchar/ftsamus/ftsamus.h", "ftSamusStatus", "Samus"),
    (4, "ftchar/ftluigi/ftluigi.h", "ftLuigiStatus", "Luigi"),
    (5, "ftchar/ftlink/ftlink.h", "ftLinkStatus", "Link"),
    (6, "ftchar/ftyoshi/ftyoshi.h", "ftYoshiStatus", "Yoshi"),
    (7, "ftchar/ftcaptain/ftcaptain.h", "ftCaptainStatus", "Captain"),
    (8, "ftchar/ftkirby/ftkirby.h", "ftKirbyStatus", "Kirby"),
    (9, "ftchar/ftpikachu/ftpikachu.h", "ftPikachuStatus", "Pikachu"),
    (10, "ftchar/ftpurin/ftpurin.h", "ftPurinStatus", "Purin"),
    (11, "ftchar/ftness/ftness.h", "ftNessStatus", "Ness"),
]
# Any fighter: the common statuses.
ANY_FIGHTER = 0xFF
ANY_STATUS = 0xFFFF
FUNC_SUFFIXES = ("SwitchStatusGround", "SwitchStatusAir", "ProcUpdate", "ProcMap",
                 "SetStatusFromGround", "DecideSetStatus", "SetStatus")


def status_enums(refs):
    ftdef = open(os.path.join(refs, "src/ft/ftdef.h")).read()
    common = enum_values(ftdef, "FTCommonStatus")
    names = {}  # enum name -> (fighter, id)
    for name, v in common.items():
        names[name] = (ANY_FIGHTER, v)
    for kind, header, enum, _ in FIGHTER_ENUMS:
        text = open(os.path.join(refs, "src/ft", header)).read()
        block = re.search(r"typedef enum " + enum + r"\s*\{(.*?)\}", text, re.S).group(1)
        block = COMMENT_RE.sub(" ", preprocess(block))
        out, nxt = {}, 0
        for item in block.split(","):
            item = item.strip()
            if not item:
                continue
            if "=" in item:
                key, val = (x.strip() for x in item.split("=", 1))
                nxt = out[val] if val in out else common[val] if val in common else int(val, 0)
            else:
                key = item
            out[key] = nxt
            nxt += 1
        for name, v in out.items():
            names[name] = (kind, v)
    return names


def preserve_table(refs, flag="PRESERVE_COLANIM", doc="the colour animation"):
    """Every `ftMainSetStatus` whose flags keep `flag` (the colour animation
    or, for RE-425, the model parts), as
    (fighter, from status, to status) with the caller's own status as
    `from` (a `...SetStatus` function is called from its move's other
    statuses, so its `from` is any)."""
    names = status_enums(refs)
    rows = set()
    unresolved = []
    for root, _, files in os.walk(os.path.join(refs, "src/ft")):
        for fn in sorted(files):
            if not fn.endswith(".c") or fn in ("ftmain.c", "fthammer.c", "ftcommonrebirth.c"):
                continue
            text = COMMENT_RE.sub(" ", preprocess(open(os.path.join(root, fn)).read()))
            if flag not in text:
                continue
            flag_macros = {m.group(1) for m in re.finditer(r"#define\s+(\w+)\s+\(([^\n]*)\)", text)
                           if flag in m.group(2)}
            for m in re.finditer(r"^\w[\w \*]*?\b(ft\w+)\(GObj \*fighter_gobj\)\s*\{(.*?)^\}", text, re.S | re.M):
                func, body = m.group(1), m.group(2)
                for call in re.finditer(r"ftMainSetStatus\((.*?)\);", body, re.S):
                    args = split_args(call.group(1))
                    flags = args[4]
                    if flag not in flags and not any(f in flags for f in flag_macros):
                        continue
                    target = args[1]
                    if target == "status_id":
                        tos = [t for line in body.split("\n") if "status_id =" in line and "?" in line
                               for t in re.findall(r"nFT\w+Status\w+", line.split("?", 1)[1])]
                    else:
                        tos = re.findall(r"nFT\w+Status\w+", target)
                    stem = func[2:].replace("_", "")
                    for suffix in FUNC_SUFFIXES:
                        if stem.endswith(suffix):
                            stem = stem[: -len(suffix)]
                            break
                    frm = None
                    if not func.endswith("SetStatus") and not func.endswith("SetStatusFromGround"):
                        for _, _, _, prefix in FIGHTER_ENUMS + [(0, 0, 0, "Common")]:
                            if stem.startswith(prefix):
                                cand = f"nFT{prefix}Status{stem[len(prefix):]}"
                                if cand in names:
                                    frm = cand
                        if frm is None:
                            unresolved.append(func)
                    for to in tos:
                        kind, to_id = names[to]
                        from_id = names[frm][1] if frm else ANY_STATUS
                        rows.add((kind, from_id, to_id, frm or "*", to))
    w = [f"\n/// `ftMainSetStatus` calls whose flags carry `FTSTATUS_{flag}`:\n",
         "/// (fighter kind or 0xFF for any, status the call is made from or 0xFFFF\n",
         "/// for any, status entered). Rebirth and the items are not listed.\n",
         f"pub static {flag}: [(u8, u16, u16); {len(rows)}] = [\n"]
    for kind, f, t, fn_, tn in sorted(rows):
        w.append(f"    (0x{kind:02X}, 0x{f:04X}, 0x{t:04X}), // {fn_} -> {tn}\n")
    w.append("];\n")
    if unresolved:
        print("from status unresolved (any):", " ".join(sorted(set(unresolved))))
    return w


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--refs", default=os.path.join(PROJECT, "refs/ssb-decomp-re"))
    args = ap.parse_args()
    src = COMMENT_RE.sub(" ", preprocess(open(os.path.join(args.refs, "src/gm/gmcolscripts.c")).read()))
    efkind = enum_values(open(os.path.join(args.refs, "src/ef/efdef.h")).read(), "efKind")

    scripts = []
    for m in re.finditer(r"u32\s+(dGMColScripts\w+)\s*\[[^\]]*\]\s*=\s*\{(.*?)\};", src, re.S):
        scripts.append((m.group(1), split_args(m.group(2))))
    names = [s for s, _ in scripts]

    def num(x):
        x = x.strip()
        if x in efkind:
            return efkind[x]
        return int(x, 0)

    def event(cmd, sym):
        m = re.match(r"gmColCommand(\w+)\((.*)\)$", cmd.strip(), re.S)
        op, a = m.group(1), split_args(m.group(2))
        if op == "End":
            return "End"
        if op == "Wait":
            return f"Wait({num(a[0])})"
        if op == "Goto":
            return f"Goto(Script::{camel(a[0])})"
        if op == "Subroutine":
            return f"Subroutine(Script::{camel(a[0])})"
        if op == "Return":
            return "Return"
        if op == "LoopBegin":
            return f"LoopBegin({num(a[0])})"
        if op == "LoopEnd":
            return "LoopEnd"
        if op == "ClearColorAll":
            return "ClearColorAll"
        if op in ("SetColor1", "SetColor2"):
            rgba = ", ".join(f"0x{num(v):02X}" for v in a)
            return f"{op}([{rgba}])"
        if op in ("BlendColor1", "BlendColor2"):
            rgba = ", ".join(f"0x{num(v):02X}" for v in a[1:])
            # `gmColCommandBlendColor2S1` encodes `nGMColEventBlendColor1`.
            return f"BlendColor1({num(a[0])}, [{rgba}])"
        if op in ("Effect", "EffectItemHold"):
            v = [num(x) for x in a]
            return (f"Effect(ColEffect {{ joint: {v[0]}, kind: {v[1]}, flag: {v[2]}, "
                    f"offset: [{v[3]}, {v[4]}, {v[5]}], scatter: [{v[6]}, {v[7]}, {v[8]}], "
                    f"item_hold: {'true' if op == 'EffectItemHold' else 'false'} }})")
        if op == "SetLight":
            return f"SetLight({num(a[0])}, {num(a[1])})"
        if op == "ClearLight":
            return "ClearLight"
        if op == "PlayFGM":
            return "PlayFgm"
        if op == "SetSkeletonID":
            return f"SetSkeletonId({num(a[0])})"
        raise ValueError(f"{sym}: unhandled {cmd!r}")

    w = ["// @generated by tools/gen-colanim-scripts.py from `src/gm/gmcolscripts.c`.\n",
         "// Do not edit by hand.\n\n",
         "use super::{ColDesc, ColEffect, ColEvent, ColEvent::*};\n\n",
         "/// One `dGMColScripts*` array.\n",
         "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n",
         "pub enum Script {\n"]
    for s in names:
        w.append(f"    /// `{s}`.\n    {camel(s)},\n")
    w.append("}\n\nimpl Script {\n    /// The script's events.\n")
    w.append("    pub fn events(self) -> &'static [ColEvent] {\n        match self {\n")
    for s in names:
        w.append(f"            Script::{camel(s)} => &{screaming(camel(s))},\n")
    w.append("        }\n    }\n}\n\n")
    for i, (s, cmds) in enumerate(scripts):
        evs = [event(c, s) for c in cmds]
        last = evs[-1] if evs else ""
        if not (last == "End" or last.startswith("Goto(") or last == "Return"):
            # Runs off its end into the next array.
            evs.append(f"Goto(Script::{camel(names[i + 1])})")
        w.append(f"/// `{s}`.\nstatic {screaming(camel(s))}: [ColEvent; {len(evs)}] = [\n")
        for e in evs:
            w.append(f"    {e},\n")
        w.append("];\n\n")

    d = re.search(r"GMColDesc dGMColScriptsDescs\[[^\]]*\]\s*=\s*\{(.*?)\};", src, re.S).group(1)
    descs = re.findall(r"\{\s*(\w+)\s*,\s*(\d+)\s*,\s*(\w+)\s*\}", d)
    w.append(f"/// `dGMColScriptsDescs`, indexed by colour-animation id.\n")
    w.append(f"pub static DESCS: [ColDesc; {len(descs)}] = [\n")
    for sym, prio, unlocked in descs:
        script = "None" if sym == "NULL" else f"Some(Script::{camel(sym)})"
        w.append(f"    ColDesc {{ script: {script}, priority: {prio}, is_unlocked: "
                 f"{'true' if unlocked == 'TRUE' else 'false'} }},\n")
    w.append("];\n\n")
    w.append("impl super::ColAnimId {\n")
    for i, (sym, _, _) in enumerate(descs):
        if sym == "NULL":
            continue
        w.append(f"    /// Id {i}: `{sym}`.\n    pub const {screaming(camel(sym))}: Self = Self({i});\n")
    w.append("}\n")
    w.extend(preserve_table(args.refs))
    w.extend(preserve_table(args.refs, "PRESERVE_MODELPART"))
    with open(OUT, "w") as f:
        f.write("".join(w))
    print(f"wrote {OUT}: {len(scripts)} scripts, {len(descs)} descs")


if __name__ == "__main__":
    main()
