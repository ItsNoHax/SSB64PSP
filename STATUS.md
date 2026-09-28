# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** stage controller material animation (RE-364).
  Whispy's eye and mouth textures, the acid material and the cloud fades
  play on per-object clocks. The cloud fade gates its line through
  `mat_anim_idle`. Pack v46 carries 13 `GROUND_MAT` tables.
- **Next gameplay batch:** controller geometry built from display lists:
  the Yoshi's Island clouds (which already have material clocks), the
  Mushroom Kingdom scales and the Castle ground. Stage selection is in
  `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Controller material animation | 13 tables packed; 11 object tables match a ROM replay track for track; cloud fades idle after 101 ticks; goldens unchanged | RE-364 |
| Zebes acid object | Acid graph and clip packed; root Y follows the level; surface follows the animated child (−720..180) | RE-363 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,201 pass**, including 519 gameplay tests. Pinned 1.98.0 Clippy with
  warnings denied, rustfmt and ROM thumb `no_std` check pass.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-364.
  No capture shows Whispy's animated textures.
- `psp-game` boots to the Intro at 60 FPS in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v46: 26,016,160 bytes, SHA-256
  `b2dbfa6320dd5c437e5d5086de6ecc462db99bb3ab20dc95b0475d11f54fb78e`.
  Adds `AnimDesc::GROUND_MAT`; the layout does not change.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Castle ground, cloud and scale geometry and stage selection remain (TODO).
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items remain; Twister/TaruCann
  have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
