# Status

Replacement snapshot, not a journal. History lives in git and
`docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports; shared machinery and PSP game
  integration remain before `P2` is complete.
- **Next batch:** shared fighter map collision: walls, ceilings, pass-through
  floors and cliff queries, including the special callbacks blocked by it.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Ness and Kirby's Ness copy | Normals, jab 3, repeat down tilt, double jump, grabs/throws, bat reflector, PK Fire spark/flame, controlled/reflected PK Thunder and self-launch, PSI Magnet and copied PK Fire; host gameplay only | RE-347 |
| Jigglypuff and Kirby's Jigglypuff copy | Normals, grabs/throws, five aerial jumps, Pound, Sing/sleep and Rest | RE-346 |
| Pikachu and Kirby's Pikachu copy | Normals, grabs/throws, Thunder Jolt, Thunder and Quick Attack | RE-345 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with `SSB64_ROM`: 1077 pass.
  Clippy (`--workspace --all-targets -- -D warnings`) and host rustfmt clean.
- `romtool anims --verify`: 789 lengths agree; generated animation table
  reproduces byte for byte.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v38 rebuilt: 23,659,808 bytes, SHA-256
  `0353ac343aa59236e88317b36539f1152902567fcf0dc6cd9e7503e7809c8f4b`.
  Appended slots need no version bump; ROM-derived output is not committed.
- PPSSPPHeadless Ness fighter smoke matches (0 differing pixels).
  Full 72-scene matrix last passed in RE-344; not rerun this batch.
- Physical PSP last checked in RE-341: PSP-2000 Slim, firmware 6.61 ARK,
  PSPLink v3.2.1. Training simulation 1.74 ms/tick, loop 33.4 ms/tick
  (30 Hz). This batch has no physical-PSP validation.

## Blockers

- Fighter map collision is floor-only; special wall/ceiling responses and
  cliff catches need the shared solver.
- `DownBounceD` has no update/clip; a downward Ness self-launch or steep
  floor impact reaches that ordinal without recovery (RE-347).
- Hits resolve one at a time; same-frame catcher/held hits and
  `recent_damage` need a deferred damage queue (RE-339).
- Escape, per-part hit status and general items remain unported. PK Fire
  has a separate flame-item table, without incoming item damage callbacks.
- Samus through Ness are host-only; PSP selection and weapon/effect drawing
  remain. Kirby copy loss, `DamageFlyRoll` and `DeadUpFall` need an RNG.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341).
