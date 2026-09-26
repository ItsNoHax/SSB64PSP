# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  Order: fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Pikachu's moveset (`P2`) in `FTKind` order, then Kirby's
  Pikachu copy (`ftkirbycopypikachuspecialn.c`). Jigglypuff and Ness
  follow, each with its Kirby copy.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Kirby copy abilities | Copied Fireball (Mario/Luigi), Blaster, Charge Shot, Giant Punch, Boomerang, Falcon Punch and Egg Lay; 25 copy figatree slots; host gameplay only | RE-344 |
| Kirby moveset | Normals, rapid jab, five aerial jumps, suplex, Final Cutter, Stone, Inhale with swallow/spit/copy victim statuses | RE-343 |
| RE-339/RE-340 validation | ROM, PPSSPP, N64 RDRAM and PSP-2000 checks of held damage, staling, strict rendering and extern linking | RE-341 |

## Verification baseline

- `cargo test --workspace` with `SSB64_ROM`: 1022 pass. Clippy (1.98,
  `cargo +1.98.0 clippy --workspace --all-targets -- -D warnings`) clean.
- `romtool anims --verify`: 676 lengths agree; the animation table
  reproduces byte for byte. `romtool matcolors --pack`: 145 textures and
  33 CLUTs, 0 differ. `romtool strict`: 0 unresolved.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v37 rebuilt from the ROM: 23,228,080 bytes, SHA-256
  `b7dbbb1fb62f511d5e69eaeceb4b5e2e6d08c0addcc94373a6ba37d8d8ecb556`.
- PPSSPPHeadless: 72 of 72 goldens match (RE-344).
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
- Samus, Luigi, Link, Yoshi, Captain Falcon and Kirby are not selectable
  in `psp-game`.
- No RNG source: Kirby copy loss, `DamageFlyRoll` and `DeadUpFall` wait on it.
