# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports; shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** moving map groups and authored cliff recovery.
  Original fighter diamonds alias their current shape; copied collision
  inputs now accept a separate previous shape (RE-353).
- **Next batch:** weapon lifecycle and shield-hop callbacks: map-bound
  removal, Boomerang off-camera removal and the remaining weapon hop responses.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Moving map groups and cliff recovery | Relative sweeps, float group offsets, first-substep carry including hitlag, group visibility, shared PSP stage poses; all 16 cliff clips, live TransN anchors, authored dispatch/action clocks and per-fighter phase-two kinetics | RE-353 |
| General item system | Shared item pool, Link Bomb lifecycle, PK Fire damage, throws/pickup, hit searches and PSP integration | RE-352 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with absolute `SSB64_ROM`: **1,165 pass**,
  including 501 gameplay tests. Clippy (`--workspace --all-targets --
  -D warnings`), workspace rustfmt and engine/game/ROM no_std target builds pass.
- Both PSP release builds pass; game also with
  `regression_capture,strict_render`. Existing viewer warnings remain.
- Scripted Training smoke via `tools/run-ppsspp.sh`: valid software-rendered
  Dream Land screenshot with both fighters; PPSSPP terminated afterwards.
  Interactive game EBOOT rebuilt. No hardware or full golden run this batch.
- Rebuilt pack v40: 24,593,552 bytes, SHA-256
  `a3e1a8313eafe260f553b4a4989554723172e2036f1a7eb55a35e26b07a399d9`.
  All 192 playable fighter cliff files checked independently from the ROM.
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
