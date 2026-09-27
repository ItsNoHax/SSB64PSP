# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** framebuffer capture coherency (RE-361). The
  transition and wallpaper capture buffers are cache-line aligned and
  written back after each capture.
- **Next gameplay batch:** the match stage loader: map stage kinds,
  controller initialization and ground hazard descriptors onto packed data.
  Remaining controller geometry and material animation are in `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Capture buffer coherency | D-cache writeback and 64-byte alignment; goldens unchanged; PSP-2000 transition capture unchanged | RE-361 |
| Training at 60 Hz on hardware | Loop 33.4 → 16.7 ms on the PSP-2000; draw-list build 19.4 → 7.4 ms; goldens unchanged | RE-360 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,193 pass**, including 518 gameplay tests. Pinned 1.98.0 Clippy with warnings denied,
  rustfmt and ROM thumb `no_std` check pass.
- Both PSP release builds pass. Interactive outputs are restored after
  captures. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-361.
- Training golden scenes supply the integration smoke. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v43: 25,288,432 bytes, SHA-256
  `b83c6d37131e0e88de51a4c02d163c93c2db4bc93a1d4bc905b049e33e84098f`.
  Only UV bytes change: 567 vertices in 30 meshes. All 1,881 textures
  match the prior v43; strict check reports zero unresolved.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Acid root writes, Castle ground, clouds and scales, per-object material
  animation and the match stage loader remain (TODO).
- Sector Z Arwing, bonus stages and stage items remain; Twister/TaruCann
  have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Guard, teeter, pipe, item and hammer statuses keep the previous pose.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
