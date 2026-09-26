"""One-shot transcription aid for Jigglypuff's motion-script collisions.

Prints `h!`/`mv!` declarations and joint lists for `purin_attack.rs` from
relocData/232_PurinMainMotion.c (US branch), and Kirby's Pound from
228_KirbyMainMotion.c. Review the output against the source before adding it
to the portable game crate.

Windows are half open. A `SetAttackCollDamage` or `SetAttackCollSize` ends
the slot's window and opens a new one in the same hit generation; a
`ClearAttackCollAll` starts a new generation, and `RefreshAttackCollID`
re-creates a slot in the current one.
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "refs/ssb-decomp-re/src/relocData"
PURIN = ROOT / "232_PurinMainMotion.c"
KIRBY = ROOT / "228_KirbyMainMotion.c"

# (Rust name, script label, figatree length, landing percent)
MOVES = [
    ("ATTACK11", "Jab1", 18, None),
    ("ATTACK12", "Jab2", 20, None),
    ("ATTACKDASH", "DashAttack", 40, None),
    ("ATTACKS3HI", "FTiltHigh", 28, None),
    ("ATTACKS3", "FTilt", 28, None),
    ("ATTACKS3LW", "FTiltLow", 28, None),
    ("ATTACKHI3", "UTilt", 24, None),
    ("ATTACKLW3", "DTilt", 40, None),
    ("ATTACKS4", "FSmash", 40, None),
    ("ATTACKHI4", "USmash", 55, None),
    ("ATTACKLW4", "DSmash", 55, None),
    ("ATTACKAIRN", "AttackAirN", 50, 50),
    ("ATTACKAIRF", "AttackAirF", 40, None),
    ("ATTACKAIRB", "AttackAirB", 40, None),
    ("ATTACKAIRHI", "AttackAirU", 40, 40),
    ("ATTACKAIRLW", "AttackAirD", 50, 50),
    ("POUND", "PoundGround", 55, None),
    ("SING", "SingAir", 180, None),
    ("REST", "0x16F4", 250, None),
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


def extract(prefix, name, source):
    pattern = rf"ftMotionCommand {prefix}_{name}\[\] = \{{(.*?)\n\}};"
    body = us_region(re.search(pattern, source, re.S).group(1))
    tick = 0
    generation = 0
    active = {}
    windows = []
    loop = None
    commands = re.findall(r"ftMotionCommand(\w+)\(([^)]*)\)", body)
    i = 0
    while i < len(commands):
        command, raw_args = commands[i]
        args = [value.strip() for value in raw_args.split(",")]

        def reopen(slot, **changes):
            old = active[slot]
            old["end"] = tick
            hit = {**old, "start": tick, "end": None, **changes}
            windows.append(hit)
            active[slot] = hit

        if command == "Wait":
            tick += int(args[0])
        elif command == "WaitAsync":
            tick = max(tick, int(args[0]))
        elif command == "LoopBegin":
            loop = [i, int(args[0])]
        elif command == "LoopEnd":
            loop[1] -= 1
            if loop[1] > 0:
                i = loop[0]
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
                       joint=int(args[2]), damage=int(args[3]),
                       element=int(args[5]), size=int(args[6]),
                       x=int(args[7]), y=int(args[8]), z=int(args[9]),
                       angle=int(args[10]), scale=int(args[11]),
                       weight=int(args[12]), ga=int(args[13]),
                       base=int(args[17]))
            windows.append(hit)
            active[slot] = hit
        elif command == "SetAttackCollDamage":
            reopen(int(args[0]), damage=int(args[1]))
        elif command == "SetAttackCollSize":
            reopen(int(args[0]), size=int(args[1]))
        elif command == "RefreshAttackCollID":
            slot = int(args[0])
            last = [w for w in windows if w["slot"] == slot][-1]
            hit = {**last, "start": tick, "end": None, "generation": generation}
            windows.append(hit)
            active[slot] = hit
        i += 1
    for hit in active.values():
        hit["end"] = tick
    for hit in windows:
        assert hit["end"] is not None and hit["end"] > hit["start"], (name, hit)
    return windows


def emit(rust, length, landing, hits):
    lag = "None" if landing is None else f"Some({landing})"
    print(f"mv!(\n    {rust},\n    {length}.0,\n    {lag},\n    [")
    for h in hits:
        box = (f"h!({h['damage']}, {h['size']}.0, {h['x']}.0, {h['y']}.0, {h['z']}.0, "
               f"{h['angle']}, {h['scale']}, {h['weight']}, {h['base']}, "
               f"{h['start']}.0, {h['end']}.0)")
        if h["generation"]:
            box += f".with_hit_generation({h['generation']})"
        if h["element"]:
            box += f"  /* element {h['element']} */"
        if h["ga"] != 3:
            box += f"  /* ga {h['ga']} */"
        print(f"        {box},")
    print("    ]\n);")
    print(f"// joints {rust}: {[h['joint'] for h in hits]}\n")


if __name__ == "__main__":
    source = PURIN.read_text()
    for rust, name, length, landing in MOVES:
        emit(rust, length, landing, extract("dPurinMainMotion", name, source))
    emit("COPY_POUND", 55, None,
         extract("dKirbyMainMotion", "Pound", KIRBY.read_text()))
