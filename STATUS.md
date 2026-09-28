# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** match stage loader (RE-362). Pack v44 carries each
  VS stage's hazard descriptor and the Zebes acid surface.
  `StageSetup` builds any VS stage's controller from the pack.
- **Next gameplay batch:** remaining stage controller objects: the Zebes
  acid object (`StageObj::Acid`, root-translate writes), then Whispy and
  cloud `MatAnimJoint` playback. Stage selection is in `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Match stage loader | Kind from `GR*Map` file; `0xBC` descriptors and acid surface packed; all nine VS controllers build from pack data; goldens unchanged; Zebes probe in PPSSPP | RE-362 |
| Capture buffer coherency | D-cache writeback and 64-byte alignment; goldens unchanged; PSP-2000 transition capture unchanged | RE-361 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,197 pass**, including 518 gameplay tests. Pinned 1.98.0 Clippy with warnings denied,
  rustfmt and ROM thumb `no_std` check pass.
- Both PSP release builds pass. Interactive outputs are restored after
  captures. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-362.
- Training golden scenes supply the integration smoke. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v44: 25,289,744 bytes, SHA-256
  `5bda1b314ec06e976a9a65cd6aeb11229238cfaafc41a5fce712de9e35b62fe0`.
  Only `StageDesc` grows (72 → 104 bytes).
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Acid root writes, Castle ground, clouds and scales, per-object material
  animation and stage selection remain (TODO).
- Sector Z Arwing, bonus stages and stage items remain; Twister/TaruCann
  have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
