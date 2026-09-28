# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** remaining shared status clips (RE-366). Rebirth,
  `WalkEnd`, `TurnRun`, guard, teeter, pipe, item pickup, throw, swing,
  shoot and hammer statuses play their own clips through the common
  pairing. -1/-2 statuses keep the previous clip. Pack v48.
- **Next gameplay batch:** shield tilt pose and clip start frame (TODO,
  RE-366). `Guard` layers `shield_anim_joints[angle]` onto the joints
  (`ftCommonGuardUpdateJoints`/`InitJoints`). `set_status` drops
  `anim_frame_begin`. Stage selection is in `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Remaining shared status clips | 66 slots, 2,407 ROM lengths agree with the decomp; goldens unchanged; boot frame identical to v47 control | RE-366 |
| Display-list controller objects | Clouds, scales and Castle ground packed; Castle X and Retract blink match ROM replays; goldens unchanged | RE-365 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,205 pass**. Pinned 1.98.0 Clippy with warnings denied, rustfmt and
  the `thumbv7em-none-eabi` `no_std` builds pass.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-366.
  No golden enters the new statuses.
- `psp-game` boots to the Intro at 60 FPS in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v48: 26,905,648 bytes, SHA-256
  `4e264113cb1413760fc8abef3b5363d348b62b205377c026a583ec504fc8b76c`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- `Guard` holds `GuardOn`'s last frame without shield tilt; clips start
  at frame 0 (RE-366).
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
