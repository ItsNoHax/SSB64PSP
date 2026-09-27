# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** pack and draw stage controller objects (RE-357).
  Pack v42 carries 29 clips for Whispy, flowers, barrel and gate. Training
  runs Whispy through packed object clocks and draws its face and flowers.
  `G_MODIFYVTX` ST writes fix the flowers and DK/Link texture regions.
- **Next batch:** the match stage loader: map stage kinds, controller
  initialization and ground hazard descriptors onto packed stage data.
  Remaining controller geometry and material animation are in `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Stage controller objects | Six object bindings, 29 clips; clock/pose/flag checks; runtime port and Training Whispy; display-link order; cached ST writes | RE-357 |
| Stage controllers and hazards | Eight host VS controllers; hazard registries, wind and damage floors; split fighter tick at stage slot | RE-356 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with absolute `SSB64_ROM`: **1,190 pass**,
  including 518 gameplay tests. Clippy (`--workspace --all-targets --
  -D warnings`), workspace rustfmt and thumb `no_std` builds pass.
- Both PSP release builds pass; game also with
  `regression_capture,strict_render`. Existing viewer warnings remain.
  Both outputs are restored to interactive builds.
- Full PPSSPPHeadless golden matrix: **73 of 73 match**. Ten reviewed
  goldens refreshed with two byte-identical captures (RE-357).
- Interactive PPSSPP Training runs at 60 FPS through a 45-second smoke.
  A temporary tick-1,200 probe confirms live wind push and looping flowers;
  probe edits reverted. No PPSSPP process remains. No hardware run.
- Pack v42: 25,231,232 bytes, SHA-256
  `3ac8ccc850cc389e475dd24e6282d20d73d1b708ac4f9e902ba7c69b1e83b76b`.
- Physical PSP last checked in RE-341 (PSP-2000 Slim, 6.61 ARK, pack v37).

## Blockers and remaining scope

- Acid root writes, Castle ground, clouds and scales, per-object material
  animation and the match stage loader remain (TODO).
- Sector Z Arwing, bonus stages and stage items remain; Twister/TaruCann
  have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341). Rendering performance remains `P5`.
