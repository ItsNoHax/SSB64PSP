# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** playable campaign Race to the Finish (RE-455).
  Three Polygon opponents, finish gate, animated Bumpers, bomb-barrel
  drawing/pieces, follow camera, timer failure and campaign results bind.
- **Next gameplay batch:** bonus pause map zoom and the 12-tick scene fade.
  Preserve each bonus course's entry, camera and result handoffs.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Race to the Finish | Gate, hazards, campaign stocks, camera and results | RE-455 |
| Board the Platforms | Landing credits, child trees/materials, Bumpers and results | RE-454 |
| Break the Targets | Bonus world, target hits/motion, end rules and results | RE-452 |

## Verification baseline

- All 1,927 workspace/all-target tests pass, including 1,109 game tests
  (absolute `SSB64_ROM`, Rust 1.98.0, one thread). ROM checks cover all
  twelve Polygon models/movement clips, Race bindings and four Bumper
  scripts for 600 ticks each. Clippy with warnings denied, workspace
  rustfmt, diff and docs checks pass.
- Pack v99 rebuilt: 40,913,008 bytes, SHA-256
  `5c1f2d212718bda5655f54cab7a292ea9cc36dc386a2afa915ff8cbebaf51f1b`.
  Adds the barrel's directly referenced smash list. No ROM assets committed.
- Both production PSP releases pass (nightly-2026-08-26): no game warnings,
  five existing viewer warnings. Game restored to production after captures.
- PPSSPP software: live Race repeats are pixel-identical; real gate landing
  reaches COMPLETE, Timer RESULT and No Damage (44,500 total); a fall
  consumes a stock and rebirths; the unmodified minute reaches FAILURE and
  zero-time RESULT. Hazard-area captures show damage and smash pieces.
  Production game reaches the expected 20 s timeout, exit 0, without
  reported faults. The capture wrapper reports a missing screenshot because
  production emits none. These seeded diagnostics do not prove a full campaign,
  unmodified course traversal, original-N64 equivalence or physical PSP.
- Golden manifest remains 195; full matrix unchanged and not rerun.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Bonus pause map zoom, 12-tick fade and audio; Master Hand,
  special fighters, boss wallpaper/fade, ending/challenger/message scenes,
  saves/unlocks, select reconciliation, shade and 1P tags.
  Details live in `TODO.md`; later stages remain unvalidated.
- Audio/rumble, N64 evidence, select spotlight, fighter jostling and Yoshi's
  double-jump apex hang remain. Fidelity, performance and VRAM belong to P5.
