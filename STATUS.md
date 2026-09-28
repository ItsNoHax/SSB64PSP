# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** scripted Training shield scene (RE-369). PPSSPP reaches
  `Guard` at tick 60 with a tilted pose and a shield collision transform from
  the posed `YRotN`. A new golden pins the frame.
- **Next gameplay batch:** investigate the Training grab whiff in PPSSPP
  against the host catch route (RE-351). Stage selection remains deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Live Training shield | Raised and tilted at tick 60; `YRotN` collision center logged; new golden and jab control match twice | RE-369 |
| Luigi translation scales | 29 ROM vectors; clip and shield endpoints checked; Luigi golden updated, Mario control unchanged | RE-368 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied, rustfmt and the
  `thumbv7em-none-eabi` `no_std` builds pass.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: Training shield and jab goldens match twice; the shield
  capture logs `Guard`, a present `YRotN`, and center (75, 255, 0). No golden
  passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v50: 27,085,696 bytes, SHA-256
  `d8394ea153d52bd24df32415ff6a2ec92fe8dcdc672a80d392627643e7b45902`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- The shield bubble is not drawn (RE-369).
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
