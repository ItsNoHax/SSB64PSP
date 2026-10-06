# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** fighter fidelity against scripted references
  (RE-468): How to Play matches the N64 to frame 3273 (Luigi) and 4039
  (Mario), the opening's nine battle scenes frame for frame, and Samus's
  grapple beam and Yoshi's tongue are made by their motions.
- **Next batch:** physical-PSP pass via PSPLink: performance profiling on
  real hardware and the crash when a player is KO'd by being pushed off
  the stage.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Fighter fidelity | How to Play to frame 3273/4039; opening battles match | RE-468 |
| Opening movie | N64 logo, 19 opening scenes, title opening layout; pack v106 | RE-467 |
| Fighter movement | How to Play matches the N64 to frame 735 | RE-466 |
| Attract modes | How to Play and auto demo ported; pack v105 | RE-465 |

## Verification baseline

- 2,139 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,908,848 bytes, SHA-256 `a319e657…d341`.
  `ssb64-menus.pak` (28 scenes): 13,163,676 bytes. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game boots into
  the N64 logo and the opening at 60 FPS under PPSSPP software.
- Lowest free memory in the opening: 2,570,752 bytes (1,569,792
  contiguous) in the Jungle scene, PPSSPP (RE-467; not re-measured).
- Golden matrix: 195 of 195 match (105 rebaselined, RE-468). Physical
  PSP last checked RE-361.

## Blockers and remaining scope

- No blocker. How to Play's next divergence is frame 3274: a caught
  fighter is placed a frame late and a thrown one is released from the
  hand, not its joint 4 less 300 (RE-468, `TODO.md`).
- The setter first-frame audit, looping figatrees' `anim_frame`, the
  opening's rendering differences (RE-467), the CPUs' special effects,
  Sound Test, rumble and all audio remain (`TODO.md`).
