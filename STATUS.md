# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  Order: fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Jigglypuff's moveset (`P2`) in `FTKind` order, with Kirby's
  Jigglypuff copy (`ftkirbycopypurinspecialn.c`). Ness and its copy follow.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Pikachu and Kirby's Pikachu copy | Normals, grabs/throws, Thunder Jolt crawler, Thunder head/trails, Quick Attack and copied Thunder Jolt; 33 figatrees plus grab/throw slots; host gameplay only | RE-345 |
| Kirby copy abilities | Copies of Mario/Luigi, Fox, Samus, Donkey Kong, Link, Captain Falcon and Yoshi | RE-344 |
| Kirby moveset | Normals, rapid jab, five aerial jumps, suplex, Final Cutter, Stone and Inhale | RE-343 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with `SSB64_ROM`: 1047 pass.
  Clippy (1.98, `--workspace --all-targets -- -D warnings`) clean.
- `romtool anims --verify`: 712 lengths agree; the generated animation
  table reproduces byte for byte.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v38 rebuilt: 23,376,928 bytes, SHA-256
  `850fec85cce315c15b8989580fcdf1eb70faa3be7bc4198bbf6eaeb08e2ee00c`.
  Collision vertices retain original IDs for weapon edge transitions.
- PPSSPPHeadless: Pikachu fighter smoke matches (0 differing pixels).
  The full 72-scene matrix last passed in RE-344; not rerun this batch.
- Physical PSP last checked in RE-341: PSP-2000 Slim, firmware 6.61 ARK,
  PSPLink v3.2.1. Training simulation 1.74 ms/tick, loop 33.4 ms/tick
  (30 Hz). This batch has no physical-PSP validation.

## Blockers

- Fighter map collision is floor-only. Quick Attack wall/ceiling cancellation
  and cliff catch need the shared solver.
- Hits resolve one at a time; same-frame catcher/held hits and
  `recent_damage` need a deferred damage queue (RE-339).
- Four-player VS scenes exceed the ~700 KiB texture pool (RE-341).
- `psp-game` Training runs at 30 Hz on hardware (RE-341).
- Escape (roll), hit-status intangibility and the item system are unported.
- Samus, Luigi, Link, Yoshi, Captain Falcon, Kirby and Pikachu are not
  selectable in `psp-game`; Pikachu's weapons/effects are not drawn.
- No RNG source: Kirby copy loss, `DamageFlyRoll` and `DeadUpFall` wait on it.
