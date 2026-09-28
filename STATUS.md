# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Luigi in `psp-game` Training (RE-372). The `luigi`
  capture scene spawns Luigi; his Fireball draws Mario's mesh with
  `palettes[1]` (green), from pack v51.
- **Next gameplay batch:** bring Samus into `psp-game` Training, with her
  Charge Shot and Bomb meshes (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Luigi in Training | Pack v51 adds the palette-1 Fireball mesh; new `f1-training-luigi` golden; 75 of 75 goldens match twice | RE-372 |
| Held fighter orientation | Removed the composed facing yaw; held draw uses only the catcher joint; grab and jab goldens match twice | RE-371 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-372 (psp-crate rustfmt still flags two older, untouched spots);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 75 goldens match twice (RE-372), including
  `f1-training-luigi`. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v51: 27,086,256 bytes, SHA-256
  `9f8a292d7ce56d4c50c4919795163cc38c7c40e08dab17dd696a71ffdd40c891`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only Mario, Fox and Luigi draw in `psp-game`, and only capture scenes
  pick Fox or Luigi. The Fireball does not spin (RE-372). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
