"""One-shot transcription aid for Fox's non-special motion scripts.

Prints Rust MoveData declarations from relocData/208_FoxMainMotion.c. Review
the output against the source before adding it to the portable game crate.
"""

import re
from pathlib import Path


SOURCE = Path(__file__).resolve().parents[1] / "refs/ssb-decomp-re/src/relocData/208_FoxMainMotion.c"
NAMES = [
    "Jab1", "Jab2", "JabLoop", "DashAttack", "FTiltHigh", "FTiltMidHigh",
    "FTilt", "FTiltMidLow", "FTiltLow", "UTilt", "DTilt", "FSmash",
    "USmash", "DSmash", "AttackAirN", "AttackAirF", "AttackAirB",
    "AttackAirU", "AttackAirD",
]


def us_region(body):
    lines = []
    branch = None
    for line in body.splitlines():
        if line.startswith("#if defined(REGION_JP)"):
            branch = "jp"
        elif line.startswith("#else") and branch == "jp":
            branch = "us"
        elif line.startswith("#endif") and branch is not None:
            branch = None
        elif branch != "jp":
            lines.append(line)
    return "\n".join(lines)


def extract(name, source):
    pattern = rf"ftMotionCommand dFoxMainMotion_{name}\[\] = \{{(.*?)\n\}};"
    body = us_region(re.search(pattern, source, re.S).group(1))
    tick = 0
    generation = 0
    active = {}
    previous = {}
    windows = []
    landing = None
    for command, raw_args in re.findall(r"ftMotionCommand(\w+)\(([^)]*)\)", body):
        args = [value.strip() for value in raw_args.split(",")]
        if command == "Wait":
            tick += int(args[0])
        elif command == "WaitAsync":
            # ftMainParseMotionEvent sets script_wait to value - anim_frame.
            # A past target advances immediately rather than rewinding time.
            tick = max(tick, int(args[0]))
        elif command == "SetFlag1" and name.startswith("AttackAir") and args[0] != "0":
            landing = int(args[0])
        elif command == "ClearAttackCollAll":
            for hit in active.values():
                hit["end"] = tick
            active = {}
            generation += 1
        elif command == "MakeAttackColl":
            slot = int(args[0])
            if slot in active:
                active[slot]["end"] = tick
            hit = dict(slot=slot, start=tick, end=None, generation=generation,
                       damage=int(args[3]), radius=int(args[6]) / 2,
                       x=int(args[7]), y=int(args[8]), z=int(args[9]),
                       angle=int(args[10]), scale=int(args[11]),
                       weight=int(args[12]), base=int(args[17]))
            windows.append(hit)
            active[slot] = previous[slot] = hit
        elif command == "RefreshAttackCollID":
            slot = int(args[0])
            old = previous[slot]
            hit = {**old, "start": tick, "end": None, "generation": generation}
            windows.append(hit)
            active[slot] = previous[slot] = hit
    for hit in active.values():
        hit["end"] = tick
    for hit in windows:
        assert hit["end"] is not None and hit["end"] > hit["start"], (name, hit)
    return tick, landing, windows


def emit(name, length, landing, hits):
    print(f"/// Source: `dFoxMainMotion_{name}`, `relocData/208_FoxMainMotion.c` (US branch).")
    print(f"pub static FOX_{name.upper()}: MoveData = MoveData {{")
    print("    hitboxes: &[")
    for h in hits:
        print("        ActiveHitbox::new(Hitbox {")
        print(f"            damage: {h['damage']}, offset: Vec3::new({h['x']}.0, {h['y']}.0, {h['z']}.0),")
        print(f"            radius: {h['radius']}, angle: {h['angle']}, kb_scale: {h['scale']},")
        print(f"            kb_weight: {h['weight']}, kb_base: {h['base']},")
        print(f"        }}, {h['start']}.0, {h['end']}.0).with_hit_generation({h['generation']}),")
    print("    ],")
    print(f"    length_frames: {length}.0,")
    # Fox has dedicated LandingAirF/B scripts; their SetFlag1(1) is an
    # enable gate, not a percentage fallback.
    if name in ("AttackAirF", "AttackAirB"):
        landing = None
    print(f"    landing_lag_percent: {'None' if landing is None else f'Some({landing})'},")
    print("};\n")


if __name__ == "__main__":
    source = SOURCE.read_text()
    for name in NAMES:
        emit(name, *extract(name, source))
