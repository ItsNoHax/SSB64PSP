# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  Order: fighter-common machinery → combat systems → all 12 fighters → match.
- **Next batch:** Ness's moveset (`P2`) in `FTKind` order, with Kirby's Ness
  copy (`ftkirbycopynessspecialn.c`, PK Fire). That completes the 12 fighters.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Jigglypuff and Kirby's Jigglypuff copy | Normals, grabs/throws, five aerial jumps, Pound, Sing with shared sleep element and `FuraSleep`, Rest with intangibility; fixed Kirby copy ordinals, Pikachu throw status and timed landings; host gameplay only | RE-346 |
| Pikachu and Kirby's Pikachu copy | Normals, grabs/throws, Thunder Jolt crawler, Thunder head/trails, Quick Attack and copied Thunder Jolt; 33 figatrees plus grab/throw slots; host gameplay only | RE-345 |
| Kirby copy abilities | Copies of Mario/Luigi, Fox, Samus, Donkey Kong, Link, Captain Falcon and Yoshi | RE-344 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with `SSB64_ROM`: 1059 pass.
  Clippy (1.98, `--workspace --all-targets -- -D warnings`) clean.
- `romtool anims --verify`: 750 lengths agree; the generated animation
  table reproduces byte for byte.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v38 rebuilt: 23,511,568 bytes, SHA-256
  `7a4a9aa630684576e661d83d90d8da6081d91fa906ac6e6b192582ec66988382`.
  Appended animation slots need no version bump.
- PPSSPPHeadless: Jigglypuff fighter smoke matches (0 differing pixels).
  The full 72-scene matrix last passed in RE-344; not rerun this batch.
- Physical PSP last checked in RE-341: PSP-2000 Slim, firmware 6.61 ARK,
  PSPLink v3.2.1. Training simulation 1.74 ms/tick, loop 33.4 ms/tick
  (30 Hz). This batch has no physical-PSP validation.

## Blockers

- Fighter map collision is floor-only. Quick Attack wall/ceiling cancellation
  and cliff catch, and Pound/Sing ledge stops, need the shared solver.
- Hits resolve one at a time; same-frame catcher/held hits and
  `recent_damage` need a deferred damage queue (RE-339).
- Four-player VS scenes exceed the ~700 KiB texture pool (RE-341).
- `psp-game` Training runs at 30 Hz on hardware (RE-341).
- Escape (roll), per-part and timed hit status (except Rest's window) and
  the item system are unported.
- Samus, Luigi, Link, Yoshi, Captain Falcon, Kirby, Pikachu and Jigglypuff
  are not selectable in `psp-game`; their weapons/effects are not drawn.
- No RNG source: Kirby copy loss, `DamageFlyRoll` and `DeadUpFall` wait on it.
