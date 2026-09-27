# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports; shared machinery and PSP game
  integration remain before `P2` is complete.
- **Next batch:** shared damage map responses and knockdown recovery:
  surface reflections, techs, DownBounce/wait/stand/roll/attack and StopCeil.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Combat audit integration | Joint hurtboxes for Mario through Captain, hit damage in knockback, crouch/999% cap, damage velocity, hitlag and immediate shield break; reconciled with sleep, Stone and later fighter/weapon ports | RE-349 |
| Shared static fighter map collision | Diamond wall/ceiling/floor processing, platform passes, topology/edge stops, authored cliff reach and two-fighter occupancy; Fire Fox, Quick Attack, PK Thunder blast, Falcon Kick and Kirby star responses | RE-348 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with `SSB64_ROM`: 1120 pass.
  Clippy (`--workspace --all-targets -- -D warnings`), host rustfmt and
  no-default-features workspace build pass.
- Map and combat regressions include independent Dream Land ROM geometry.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- Pack v38 unchanged: 23,659,808 bytes, SHA-256
  `0353ac343aa59236e88317b36539f1152902567fcf0dc6cd9e7503e7809c8f4b`.
  No asset-pipeline changes or ROM-derived output committed.
- PPSSPPHeadless `f1-training-fireball` gameplay smoke matches its golden
  (0 differing pixels). Full 72-scene matrix last passed in RE-344;
  not rerun this batch.
- Physical PSP last checked in RE-341: PSP-2000 Slim, firmware 6.61 ARK,
  PSPLink v3.2.1. Training simulation 1.74 ms/tick, loop 33.4 ms/tick
  (30 Hz). This batch has no physical-PSP validation.

## Blockers

- Map processing uses static groups and a fixed collision diamond. Moving
  surface velocities, changing diamonds and damage/tech callbacks remain.
- `DownBounceD` has no update/clip; a downward Ness self-launch or steep
  floor impact reaches that ordinal without recovery (RE-347–348).
- Cliff root placement and clip clocks still need authored poses/TransN.
- Hits resolve one at a time; `recent_damage` includes the current hit,
  but same-frame catcher/held hits need a deferred damage queue (RE-339, RE-349).
- Joint hurtbox tables cover Mario through Captain; Kirby, Pikachu,
  Jigglypuff and Ness retain the root-sphere fallback (RE-349).
- Escape, Smash DI, per-part hit status and general items remain unported. PK Fire
  has a separate flame-item table, without incoming item damage callbacks.
- Samus through Ness are host-only; PSP selection and weapon/effect drawing
  remain. Kirby copy loss, `DamageFlyRoll` and `DeadUpFall` need an RNG.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341).
