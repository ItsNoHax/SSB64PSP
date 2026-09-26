# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  Order: fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Kirby's copy abilities for the eight ported fighters
  (`ftkirbycopy*specialn.c`: Mario, Luigi, Fox, Samus, Donkey Kong, Link,
  Yoshi, Captain Falcon), then Pikachu's moveset (`P2`) in `FTKind` order.
  Pikachu, Jigglypuff and Ness copies follow their fighters.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Kirby moveset | Normals, rapid jab, five aerial jumps, suplex, Final Cutter and wave, Stone, Inhale with swallow/spit/copy victim statuses; copy recorded but copy specials not ported; host gameplay only | RE-343 |
| RE-339/RE-340 validation | ROM, PPSSPP, N64 RDRAM and PSP-2000 checks of held damage, staling, strict rendering and extern linking | RE-341 |
| Weapon staling | Weapons capture the owner's stale at spawn and feed its queue on hit | RE-342 |

## Verification baseline

- `cargo test --workspace` with `SSB64_ROM`: 1009 pass. Clippy (1.98,
  `cargo +1.98.0 clippy --workspace --all-targets -- -D warnings`) clean.
- `romtool anims --verify`: 654 lengths agree; the animation table
  reproduces byte for byte. `romtool matcolors --pack`: 145 textures and
  33 CLUTs, 0 differ. `romtool strict`: 0 unresolved.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v37 rebuilt from the ROM: 23,154,048 bytes, SHA-256
  `57076e194149d81665b45baa89f28f2bf1c7eacfa8408f125a30583ae29e94b5`.
- PPSSPPHeadless: 72 of 72 goldens match twice (RE-343).
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
