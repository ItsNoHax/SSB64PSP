# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** display-list controller objects (RE-365). The
  Yoshi's Island clouds (three `Kind48` puffs per cloud), the Mushroom
  Kingdom pulley, strings and platforms, and the Castle ground root pack
  as one-node graphs. They draw and move with their controllers. Pack v47.
- **Next gameplay batch:** clips for the guard, teeter and item statuses
  (`GuardOn`/`Guard`/`GuardOff`, `Ottotto`, `Dokan*`, item pickup/throw).
  They still map to the Wait slot (TODO, RE-355). Stage selection is in
  `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Display-list controller objects | Clouds, scales and Castle ground packed; Castle X and Retract blink match ROM replays; clouds and scales seen in PPSSPPHeadless probes; goldens unchanged | RE-365 |
| Controller material animation | 13 tables packed; 11 object tables match a ROM replay track for track; cloud fades idle after 101 ticks | RE-364 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,204 pass**. Pinned 1.98.0 Clippy with warnings denied, rustfmt and
  the `thumbv7em-none-eabi` `no_std` builds pass.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-365.
  No golden uses Yoshi's Island, Mushroom Kingdom or Castle.
- `psp-game` boots to the Intro at 60 FPS in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v47: 26,026,528 bytes, SHA-256
  `a5be80d21ceb53fea03c1b7c910807f389b367186d69a28b4c992282f48f5b1f`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
