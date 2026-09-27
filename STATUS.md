# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** shared damage and attack clips. Every playable
  fighter now plays its damage, attack, taunt and `LandingAirX` clips
  through one status → slot mapping. A landed hit is now visible (RE-355).
- **Next batch:** stage-specific map controllers and hazards (moving-floor
  shadow placement included). Guard/teeter/item clips are a small follow-up
  (TODO).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Damage and attack clips | 53 shared slots plus `MarioAttack13`; generator `motion_enum` alias fix; per-fighter common-attack tables removed; `LandingAirNull` plays at its landing-lag rate | RE-355 |
| Weapon lifecycle and shield hops | Pool blast-zone test; Boomerang off-camera check; `Weapon::on_shield` for every kind; Fireball free flight | RE-354 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with absolute `SSB64_ROM`: **1,171 pass**,
  including 506 gameplay tests. Clippy (`--workspace --all-targets --
  -D warnings`) and workspace rustfmt pass.
- Both PSP release builds pass; game also with
  `regression_capture,strict_render`. Existing viewer warnings remain.
- Full golden matrix: 73 of 73 match after rebaselining `f1-training-jab`
  (hit now visible) and `r2-stage-bonus2-{samus,kirby}` (stale before this
  batch, likely RE-353). No PPSSPP process left running. No hardware run.
- Pack v41: 25,197,920 bytes, SHA-256
  `fa0c02848bfa2f1a9e69ff17671f3bd241e19bfa3b751f3417fbc15c5b46b719`.
- Physical PSP last checked in RE-341 (PSP-2000 Slim, 6.61 ARK, pack v37).

## Blockers and remaining scope

- Stage-specific map controllers/hazards and moving-floor shadow placement remain.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341). Rendering performance remains `P5`.
