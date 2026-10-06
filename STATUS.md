# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** the N64 logo and the opening movie (RE-467):
  `mnStartup`, all 19 `mvOpening*` scenes on their music tics with camera
  animation, scripted and posed fighters, the room's transition and the
  title's opening layout (logo tree, slash, fire particles). The opening's
  models load in their own pack, out of the resident one.
- **Next gameplay batch:** fighter fidelity against scripted references:
  How to Play's divergence at frame 736 (sub-unit figatree poses, RE-466)
  and the opening battles' movement gaps (Donkey Kong's slap loop, Samus in
  the jungle), with the grapple beam and Yoshi's tongue drawn.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Opening movie | N64 logo, 19 opening scenes, title opening layout; pack v106 | RE-467 |
| Fighter movement | How to Play matches the N64 to frame 735 | RE-466 |
| Attract modes | How to Play and auto demo ported; pack v105 | RE-465 |
| VS results presentation | Wipe, fade-in, stock snaps, scores, team steal | RE-464 |

## Verification baseline

- 2,128 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,885,168 bytes, SHA-256 `fceb01eb…dadb`.
  `ssb64-menus.pak` (28 scenes, the opening's models included):
  13,163,676 bytes. No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game boots into
  the N64 logo and the opening at 60 FPS under PPSSPP software.
- Lowest free memory in the opening: 2,570,752 bytes (1,569,792
  contiguous) in the Jungle scene, PPSSPP.
- Golden matrix: 195 of 195 match (one rebaselined, RE-467). Diagnostics
  `opening@N` and `op-<scene>@N`. Physical PSP last checked RE-361.

## Blockers and remaining scope

- No blocker. The opening's remaining differences (RE-467): the room's
  fall pose and spotlight cone, Fox's laser under the rolled camera, the
  standoff ground's clipping and the Yoster clouds.
- The CPUs' special effects, the select's spotlight, Sound Test, rumble and
  all audio remain (`TODO.md`).
