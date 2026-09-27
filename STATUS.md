# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** stage controllers and ground hazards. Eight VS stage
  controllers, the obstacle/hazard registries, damage floors, wind push and
  the Twister/Barrel Cannon statuses run on the host. The fighter tick is
  split at the stage slot, as the source process order requires (RE-356).
- **Next batch:** pack and draw the stage controller objects and their
  animations, add the runtime `StageObjects`, and turn on Whispy in Training
  (TODO). A match stage loader follows.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Stage controllers and hazards | `ssb_game::stage` (Pupupu, Zebes, Jungle, Hyrule, Yoster, Inishie, Yamabuki, Castle); `ssb_game::hazard`; `tick_interrupt`/`tick_physics_map`; `StageJoint` GObj clock; Whispy checked on ROM clips | RE-356 |
| Damage and attack clips | 53 shared slots plus `MarioAttack13`; per-fighter common-attack tables removed | RE-355 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with absolute `SSB64_ROM`: **1,185 pass**,
  including 518 gameplay tests. Clippy (`--workspace --all-targets --
  -D warnings`), workspace rustfmt and thumb `no_std` builds pass.
- Both PSP release builds pass; game also with
  `regression_capture,strict_render`. Existing viewer warnings remain.
- `tools/golden.sh verify --filter '^f1-'`: 7 of 7 match after the reorder.
  Full matrix last run in RE-355 (73 of 73). No PPSSPP process left
  running. No hardware run.
- Pack v41 unchanged: 25,197,920 bytes, SHA-256
  `fa0c02848bfa2f1a9e69ff17671f3bd241e19bfa3b751f3417fbc15c5b46b719`.
- Physical PSP last checked in RE-341 (PSP-2000 Slim, 6.61 ARK, pack v37).

## Blockers and remaining scope

- Stage objects are not packed: `psp-game` runs an empty stage slot.
- Sector Z Arwing, bonus stages and stage items remain; Twister/TaruCann
  have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341). Rendering performance remains `P5`.
