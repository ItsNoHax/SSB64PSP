# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Samus in `psp-game` Training (RE-373). Pack v52
  adds her Charge Shot quad and two Bomb blink meshes. The Charge Shot draws
  as a kind-46 camera-facing billboard. The Bomb takes battle matrix
  function 70 (spin about world Z).
- **Next gameplay batch:** bring Link into `psp-game` Training, with his
  Boomerang and Spin Attack visuals (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Samus in Training | Pack v52 adds Charge Shot and Bomb meshes; three new `f1-training-samus*` goldens; 78 of 78 goldens match twice | RE-373 |
| Luigi in Training | Pack v51 adds the palette-1 Fireball mesh; new `f1-training-luigi` golden; 75 of 75 goldens match twice | RE-372 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-373 (psp-crate rustfmt still flags two older, untouched spots);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 78 goldens match twice (RE-373), including the
  three Samus scenes. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v52: 27,087,568 bytes, SHA-256
  `e814f589d254e0a3d14b5081eb91326d36a24767b75cf5c41187009058f8759d`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only Mario, Fox, Luigi and Samus draw in `psp-game`, and only capture
  scenes pick Fox, Luigi or Samus. The Fireball does not spin (RE-372;
  its kind 71 is `func_ovl0_800CA5C8`, RE-373). A released Charge Shot
  restarts its spin (RE-373). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
