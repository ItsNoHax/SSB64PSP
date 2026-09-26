"""One-shot transcription aid for Ness's motion-script collisions.

Prints `h!`/`mv!` declarations and joint lists for `ness_attack.rs` from
relocData/238_NessMainMotion.c (US branch). The script walk is
`extract_purin_moves.extract`; review the output against the source before
adding it to the portable game crate.
"""

from pathlib import Path

from extract_purin_moves import ROOT, emit, extract

NESS = ROOT / "238_NessMainMotion.c"

# (Rust name, script label, figatree length, landing percent). `DKTAAir` is
# PK Thunder 2's looping figatree; the status counts its own 28 frames.
MOVES = [
    ("ATTACK11", "Jab1", 18, None),
    ("ATTACK12", "Jab2", 20, None),
    ("ATTACK13", "Jab3", 25, None),
    ("ATTACKDASH", "DashAttack", 37, None),
    ("ATTACKS3HI", "FTiltHigh", 35, None),
    ("ATTACKS3", "FTilt", 35, None),
    ("ATTACKS3LW", "FTiltLow", 35, None),
    ("ATTACKHI3", "UTilt", 35, None),
    ("ATTACKLW3", "DTilt", 14, None),
    ("ATTACKS4", "FSmash", 50, None),
    ("ATTACKHI4", "USmash", 40, None),
    ("ATTACKLW4", "DSmash", 55, None),
    ("ATTACKAIRN", "AttackAirN", 40, 50),
    ("ATTACKAIRF", "AttackAirF", 42, None),
    ("ATTACKAIRB", "AttackAirB", 40, None),
    ("ATTACKAIRHI", "AttackAirU", 42, None),
    ("ATTACKAIRLW", "AttackAirD", 30, None),
    ("THROWF", "ThrowF", 45, None),
    ("THROWB", "ThrowB", 45, None),
    ("PKJIBAKU", "DKTAAir", 28, None),
]


if __name__ == "__main__":
    source = NESS.read_text()
    for rust, name, length, landing in MOVES:
        emit(rust, length, landing, extract("dNessMainMotion", name, source))
