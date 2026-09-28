# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Jigglypuff in `psp-game` Training (RE-380). The
  Sing notes start on Sing's first update. Their nodes draw as billboards
  under TopN's matrix, with the ring spun by its animation.
- **Next gameplay batch:** bring Donkey Kong, then Ness, into `psp-game`
  Training (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Jigglypuff in Training | New `f1-training-purin` golden; 88 of 88 goldens match twice | RE-380 |
| Pikachu in Training | Pack v56; draw order and cutout gate rebaselined six goldens; two new Pikachu goldens; 87 of 87 match twice | RE-379 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-380 (psp-crate rustfmt flags only older, untouched spots);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 88 goldens match twice (RE-380), including the
  new Jigglypuff scene. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v56: 27,184,192 bytes, SHA-256
  `8c65430a69b1cafd149bccecaa852e79b2a7ef795c4698a732ae72eaea8072ba`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Donkey Kong and Ness do not draw in `psp-game` yet; capture scenes pick
  the other ten.
  Kirby's copy hats and Pikachu's Thunder are not drawn. Link's Bomb item, the Egg Lay
  victim's egg and the egg/star hit effects are not drawn. The Fireball does not spin (RE-372;
  its kind 71 is `func_ovl0_800CA5C8`, RE-373). A released Charge Shot
  restarts its spin (RE-373). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
