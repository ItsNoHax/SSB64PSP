# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports; shared machinery and PSP game
  integration remain before `P2` is complete.
- **Next batch:** damage map responses (`WallDamage`, `StopCeil`,
  `DownBounceD`, surface reflections) and pack clips for the reaction
  statuses (RE-350).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Combat audit 2 | Motion scripts for all 12 fighters drive attacks, hit status and flags; per-frame hit log with clank, shields, rebound, elements, `damage_resist`, Smash DI; knockdown/tech/roll; shared RNG; all hurtboxes; weapon attributes fixed. Host-only | RE-350 |
| Shared static fighter map collision | Diamond wall/ceiling/floor processing, platform passes, topology/edge stops, authored cliff reach and two-fighter occupancy; Fire Fox, Quick Attack, PK Thunder blast, Falcon Kick and Kirby star responses | RE-348 |

## Verification baseline

- `cargo test --workspace` without `SSB64_ROM`: 1,108 pass (RE-350; the
  removed move tables took their tests). Last ROM run: 1,120 (RE-349).
  Clippy (`--workspace --all-targets -- -D warnings`), host rustfmt and
  no-default-features workspace build pass.
- Map and combat regressions include independent Dream Land ROM geometry.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v38 unchanged: 23,659,808 bytes, SHA-256
  `0353ac343aa59236e88317b36539f1152902567fcf0dc6cd9e7503e7809c8f4b`.
  No asset-pipeline changes or ROM-derived output committed.
- PPSSPPHeadless `f1-training-fireball` smoke matched at RE-349; not rerun
  after RE-350, which changes attack timing and hit resolution (no ROM or
  PPSSPP in that environment).
- Physical PSP last checked in RE-341: PSP-2000 Slim, firmware 6.61 ARK,
  PSPLink v3.2.1. Training simulation 1.74 ms/tick, loop 33.4 ms/tick
  (30 Hz). This batch has no physical-PSP validation.

## Blockers

- Map processing uses static groups and a fixed collision diamond. Moving
  surface velocities, changing diamonds and damage map callbacks
  (`WallDamage`, `StopCeil`, `DownBounceD`) remain (RE-347–348).
- The pack has no clips for Escape, Down, Passive, ShieldBreak, FuraFura
  and Rebound; roll and tech root motion needs them (RE-350).
- Cliff root placement and clip clocks still need authored poses/TransN.
- Weapon–attack clank, reflector collisions and general items remain.
- RE-350 needs a ROM run, goldens and a PPSSPP smoke test.
- Samus through Ness are host-only; PSP selection and weapon/effect drawing
  remain. `DeadUpFall` still needs the RNG.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341).
