# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Link in `psp-game` Training (RE-374). Pack v53
  adds `AnimDesc::WEAPON` with the Boomerang's spin. The Boomerang draws
  as its three-node tree, and its return hides node 2. The Spin Attack
  swirl follows TopN (battle matrix function 80) and plays its transform
  and material animations until the status ends.
- **Next gameplay batch:** bring Yoshi into `psp-game` Training, with his
  Egg Throw egg and Egg Lay/Bomb stars (TODO). Stage and fighter select
  remain deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Link in Training | Pack v53 adds the Boomerang animation; new `f1-training-link` and `-link-spin` goldens; 80 of 80 goldens match twice | RE-374 |
| Samus in Training | Pack v52 adds Charge Shot and Bomb meshes; three new `f1-training-samus*` goldens; 78 of 78 goldens match twice | RE-373 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-374 (psp-crate rustfmt still flags two older, untouched spots);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 80 goldens match twice (RE-374), including the
  two Link scenes. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v53: 27,089,392 bytes, SHA-256
  `290e996db16ede28c31d798c56e8aa97fa7215eed7e5f2f39105da85e4a5a264`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only Mario, Fox, Luigi, Samus and Link draw in `psp-game`, and only
  capture scenes pick the others. Link's Bomb item is not drawn. The Fireball does not spin (RE-372;
  its kind 71 is `func_ovl0_800CA5C8`, RE-373). A released Charge Shot
  restarts its spin (RE-373). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
