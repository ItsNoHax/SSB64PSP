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
| Combat audit | Joint hurtboxes from `damage_coll_descs` with Hi/N/Lw placement; knockback counts the hit's own damage; crouch reduction; 999% cap; `InitDamageVars` launch/slide/bounce; damage velocity decays independently of hitstun; hitlag on both sides; immediate shield break; weapon push by travel; `DamageAir1-3` exit to `Fall`. Host-only: no ROM, PPSSPP or hardware run | RE-343 |
| RE-339/RE-340 validation | Merged 69ed3a0 (records renumbered RE-339/340). Strict mode clean over v37 and widened to material animations; `strict_render` reaches Training. Scene residency measured. Extern linker matches `lbRelocGetAllocSize` and live N64 RDRAM placement. Costume goldens; dummy takes the free costume. Six `psp-game` scenes and tick timing on PSP-2000 | RE-341 |
| Weapon staling | Weapons capture the owner's stale at spawn and feed its queue on hit | RE-342 |

## Verification baseline

- `cargo test --workspace` without `SSB64_ROM`: 1,014 pass (RE-343); the
  last ROM run was 995 (RE-341). Clippy (1.98, `-D warnings`) clean;
  `ssb-game` builds `no_std`.
- `romtool anims --verify`: 602 lengths agree; the animation table
  reproduces byte for byte. `romtool matcolors --pack`: 145 textures and
  33 CLUTs, 0 differ. `romtool strict`: 0 unresolved.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v37 rebuilt from the ROM: 23,037,360 bytes, SHA-256
  `2629e02d8bddedf58def1a8b4159a0993b0bc550aba108074d677743504cfe2e`.
- PPSSPPHeadless: 72 of 72 goldens matched twice at RE-341; not re-run
  after RE-343, which changes hurtboxes and hitlag.
- PSP-2000 Slim, firmware 6.61 ARK, PSPLink v3.2.1: the six `psp-game`
  scenes agree with PPSSPP; Training simulation 1.74 ms/tick, loop
  33.4 ms/tick (30 Hz). Viewer stage 40: 1.026 ms/tick (RE-329). Hand-input
  checks (R, throws, held-fighter Fireball, nub deadzone) not run.

## Blockers

- Fighter map collision is floor-only (no wall/ceiling solver).
- Hits resolve one at a time; same-frame catcher/held hits need a
  deferred per-frame hit log (RE-339, RE-343).
- RE-343's hurtboxes and hitlag need a ROM run, goldens and a PPSSPP
  smoke test (no ROM in the audit environment).
- Four-player VS scenes exceed the ~700 KiB texture pool (RE-341).
- `psp-game` Training runs at 30 Hz on hardware (RE-341).
- Escape (roll), hit-status intangibility, Smash DI and the item system
  are unported.
- Samus, Luigi, Link, Yoshi and Captain Falcon are not selectable in
  `psp-game`.
