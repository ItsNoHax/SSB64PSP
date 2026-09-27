# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** weapon lifecycle and shield-hop callbacks.
  Blast-zone removal, the Boomerang's off-camera removal and every
  `can_hop` weapon's hop and `proc_shield` are ported. The Fireball now
  moves in open air (RE-354).
- **Next batch:** clips for damage statuses and normals. `DamageHi1`…
  `DamageFlyRoll`, `DamageFall` and most normals still map to the Wait slot,
  so a landed hit is invisible (TODO, RE-351).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Weapon lifecycle and shield hops | Pool blast-zone test; Boomerang `homing_delay`/ninth-frame camera projection (`Camera::project`); `Weapon::on_shield` for every kind; attack records shared by every weapon; Fireball free flight | RE-354 |
| Moving map groups and cliff recovery | Relative sweeps, group carry and visibility, all 16 cliff clips and authored kinetics | RE-353 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with absolute `SSB64_ROM`: **1,170 pass**,
  including 506 gameplay tests. Clippy (`--workspace --all-targets --
  -D warnings`), workspace rustfmt and engine/game/ROM no_std target builds pass.
- Both PSP release builds pass; game also with
  `regression_capture,strict_render`. Existing viewer warnings remain.
- `tools/golden.sh verify --filter '^f1-training'`: 6 of 7 match;
  `f1-training-fireball` was rebaselined (2,476 pixels at 2x, Fireball free
  flight). No PPSSPP process left running. No hardware or full golden run.
- Pack v40 unchanged: 24,593,552 bytes, SHA-256
  `a3e1a8313eafe260f553b4a4989554723172e2036f1a7eb55a35e26b07a399d9`.
  Last full 73-scene golden matrix: RE-351.
- Physical PSP last checked in RE-341 (PSP-2000 Slim, 6.61 ARK, pack v37).

## Blockers and remaining scope

- Stage-specific map controllers/hazards and moving-floor shadow placement remain.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Damage statuses and most normals lack pack clips; hits are not visible.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341). Rendering performance remains `P5`.
