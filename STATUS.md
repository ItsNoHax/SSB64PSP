# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Zebes acid object (RE-363). The controller writes
  the acid root Y and reads the animated child Y as the hazard surface.
  Pack v45 carries the acid clip.
- **Next gameplay batch:** per-object stage material animation:
  `MatAnimJoint` playback for Whispy's eyes and mouth, the Yoshi's Island
  clouds (`mat_anim_idle`) and the acid. Stage selection is in `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Zebes acid object | Acid graph and clip packed; root Y follows the level; surface follows the animated child (−720..180), not the rest Y; goldens unchanged; acid drawn in a PPSSPP Zebes probe | RE-363 |
| Match stage loader | Kind from `GR*Map` file; `0xBC` descriptors and acid surface packed; all nine VS controllers build from pack data; goldens unchanged; Zebes probe in PPSSPP | RE-362 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,199 pass**, including 519 gameplay tests. Pinned 1.98.0 Clippy with warnings denied,
  rustfmt and ROM thumb `no_std` check pass.
- Both PSP release builds pass. Interactive outputs are restored after
  captures. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-363.
- Training golden scenes supply the integration smoke. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v45: 25,293,312 bytes, SHA-256
  `745fe70c4943e3de479701ad4666a6ba4199115ee4aab5daab225e183bcfd2a0`.
  Adds ground clip slot 29 (acid); the format does not change.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Castle ground, clouds and scales, per-object material animation and
  stage selection remain (TODO).
- Sector Z Arwing, bonus stages and stage items remain; Twister/TaruCann
  have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
