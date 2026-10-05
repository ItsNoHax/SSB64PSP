# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** bonus pause map zoom, retry and 12-tick scene-entry
  fade (RE-456), following playable campaign Race to the Finish (RE-455).
- **Next gameplay batch:** 1P Master Hand controller, wallpaper/fade and
  assets; preserve the existing campaign scene boundaries.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Race to the Finish | Gate, hazards, campaign stocks, camera and results | RE-455 |
| Bonus pause and entry fade | Source map zoom, L: Retry and black 12-tick fade | RE-456 |
| Board the Platforms | Landing credits, child trees/materials, Bumpers and results | RE-454 |
| Break the Targets | Bonus world, target hits/motion, end rules and results | RE-452 |

## Verification baseline

- `cargo check --workspace --all-targets` passes. This continuation did not
  run workspace tests.
- Pack v100 rebuilt: 40,913,488 bytes, SHA-256
  `54f9d130cdfcbb158344e7464298f968e4157a43b2653aabb165340f11d48db0`.
  It adds the bonus pause camera points. No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings.
- The production game starts under PPSSPP software for 20 seconds without
  reported faults. This smoke does not exercise the bonus pause or fade.
- Golden manifest remains 195; full matrix unchanged and not rerun.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Bonus course audio; Master Hand, special fighters, boss wallpaper/fade,
  ending/challenger/message scenes,
  saves/unlocks, select reconciliation, shade and 1P tags.
  Details live in `TODO.md`; later stages remain unvalidated.
- Audio/rumble, N64 evidence, select spotlight, fighter jostling and Yoshi's
  double-jump apex hang remain. Fidelity, performance and VRAM belong to P5.
