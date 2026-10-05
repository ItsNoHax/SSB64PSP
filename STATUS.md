# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All twelve fighters and Master Hand have host movesets. Shared machinery
  and PSP game integration remain before `P2` is complete.
- **Completed batch:** 1P Master Hand: all 33 boss statuses, attack
  choice, bullets, hit points and defeat; Final Destination's boss
  wallpaper, cameras and fades; the campaign stage through GAME CLEAR
  (RE-457).
- **Next gameplay batch:** 1P special fighters: Metal Mario, Giant Donkey
  Kong and the Fighting Polygon Team stage (pack and runtime), keeping the
  campaign's scene boundaries.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Master Hand | Boss fighter, bullets, wallpaper, cameras, defeat, GAME CLEAR | RE-457 |
| Bonus pause and entry fade | Source map zoom, L: Retry and black 12-tick fade | RE-456 |
| Race to the Finish | Gate, hazards, campaign stocks, camera and results | RE-455 |
| Board the Platforms | Landing credits, child trees/materials, Bumpers and results | RE-454 |

## Verification baseline

- 1,947 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v101: 41,035,424 bytes, SHA-256
  `2e51c9b827dc6ea22755018d1592a23ba525ea10b081a4e7a34904b5e428afb1`.
  No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game runs under
  PPSSPP software for 20 seconds without faults.
- PPSSPP `onepboss`/`onepbossdefeat` captures cover the intro, attacks, a
  KO, the seeded defeat and GAME CLEAR; one patched-boot N64 reference.
- Golden manifest remains 195; full matrix not rerun. Stack last measured
  RE-440; production ELF last measured RE-443. Physical PSP last checked
  RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- The ending, challenger and message scenes block; special fighters'
  stages block on assets. Saves/unlocks, select reconciliation, shade, 1P
  tags, audio and Master Hand's fidelity items are in `TODO.md`.
- Rumble, N64 evidence, select spotlight, fighter jostling and Yoshi's
  double-jump apex hang remain. Fidelity, performance and VRAM belong to P5.
