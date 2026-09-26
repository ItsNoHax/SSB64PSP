# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  Order: fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Kirby's normals, specials, grab and throw motion scripts
  (`P2`), continuing the remaining movesets in `FTKind` order.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| RE-339/RE-340 validation | Merged 69ed3a0 (records renumbered RE-339/340). Strict mode clean over v37 and widened to material animations; `strict_render` reaches Training. Scene residency measured. Extern linker matches `lbRelocGetAllocSize` and live N64 RDRAM placement. Costume goldens; dummy takes the free costume. Six `psp-game` scenes and tick timing on PSP-2000 | RE-341 |
| Weapon staling | Weapons capture the owner's stale at spawn and feed its queue on hit | RE-342 |
| Captain Falcon moveset | Normals, rapid jab, Falcon Punch/Kick/Dive, grabs and throws; host gameplay only | RE-338 |

## Verification baseline

- `cargo test --workspace` with `SSB64_ROM`: 995 pass. Clippy (1.98,
  `-D warnings`) clean.
- `romtool anims --verify`: 602 lengths agree; the animation table
  reproduces byte for byte. `romtool matcolors --pack`: 145 textures and
  33 CLUTs, 0 differ. `romtool strict`: 0 unresolved.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v37 rebuilt from the ROM: 23,037,360 bytes, SHA-256
  `2629e02d8bddedf58def1a8b4159a0993b0bc550aba108074d677743504cfe2e`.
- PPSSPPHeadless: 72 of 72 goldens match twice (RE-341).
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: the six `psp-game`
  scenes agree with PPSSPP; Training simulation 1.74 ms/tick, loop
  33.4 ms/tick (30 Hz). Viewer stage 40: 1.026 ms/tick (RE-329). Hand-input
  checks (R, throws, held-fighter Fireball, nub deadzone) not run.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Hits resolve one at a time; same-frame catcher/held hits and
  `recent_damage` need a deferred damage queue (RE-339).
- Four-player VS scenes exceed the ~700 KiB texture pool (RE-341).
- `psp-game` Training runs at 30 Hz on hardware (RE-341).
- Escape (roll), hit-status intangibility and the item system are unported.
- Samus, Luigi, Link, Yoshi and Captain Falcon are not selectable in
  `psp-game`.
