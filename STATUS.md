# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end runs the title's whole attract loop but the N64 logo and
  opening movie.
- **Completed batch:** fighter movement against How to Play's scripts
  (RE-466): Turn's pivot gate, TurnRun, Appeal, the squat, landing and
  jump chains, a `proc_update`'s new status running its interrupt that
  frame, fighters made standing on the floor, clips and TransN steps on
  the status's own frame, the jostle, hitlag landings and libm-accurate
  device trig. How to Play now matches an N64 trace through frame 735
  (Luigi) and 1475 (Mario), against 254 before.
- **Next gameplay batch:** the N64 logo and the opening movie
  (`mnStartup`, the `mvOpening*` scenes with their own fighters, cameras,
  key scripts and stages), the attract loop's last boundary.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Fighter movement | How to Play matches the N64 to frame 735; host replay of its scripts against N64 numbers; 175 goldens rebaselined | RE-466 |
| Attract modes | How to Play and auto demo ported; pack v105 | RE-465 |
| VS results presentation | Wipe, fade-in, stock snaps, scores, team steal | RE-464 |
| Golden drift triage | 116 goldens rebaselined | RE-463 |

## Verification baseline

- 2,096 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v105: 46,795,968 bytes, SHA-256 `e26b63eb…85e3`.
  `ssb64-menus.pak` (14 scenes): 7,548,456 bytes. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. Idle at the title, the
  production game enters How to Play at 60 FPS under PPSSPP software.
- Golden matrix: 195 of 195 match. Diagnostics `explain[@N]` (its `re465`
  log line carries status id, anim frame and velocities) and `autodemo`.
  Physical PSP last checked RE-361.

## Blockers and remaining scope

- No blocker. How to Play diverges at frame 736: a margin-limited hit
  decided by sub-unit figatree pose differences (TODO, RE-466).
- The CPUs' special effects, the select's spotlight, Sound Test, rumble and
  all audio remain (`TODO.md`).
