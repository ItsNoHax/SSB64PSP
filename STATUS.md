# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Captain Falcon in `psp-game` Training (RE-376).
  The Falcon Punch flame lives from script flag 0's first set (frame 42)
  to its second (frame 55), survives the ground/air switch, and draws at
  joint 16 with the `lr * -90` yaw and its material animation.
- **Next gameplay batch:** draw the Falcon Kick flame, then bring Kirby
  into `psp-game` Training (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Captain Falcon in Training | Falcon Punch flame from the host effect clock; new `f1-training-captain` golden; 83 of 83 goldens match twice | RE-376 |
| Yoshi in Training | Pack v54 adds the weapon-seeded star; new `f1-training-yoshi` and `-yoshi-bomb` goldens; 82 of 82 goldens match twice | RE-375 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-376 (psp-crate rustfmt flags only older, untouched spots);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 83 goldens match twice (RE-376), including the
  new Captain Falcon scene. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v54: 27,096,032 bytes, SHA-256
  `8580875d40d2203de7d4762316f770ccad58df0885d586403fab2e6d265f3392`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only Mario, Fox, Luigi, Samus, Link, Yoshi and Captain Falcon draw in
  `psp-game`, and only capture scenes pick the others. The Falcon Kick
  flame is not drawn. Link's Bomb item, the Egg Lay
  victim's egg and the egg/star hit effects are not drawn. The Fireball does not spin (RE-372;
  its kind 71 is `func_ovl0_800CA5C8`, RE-373). A released Charge Shot
  restarts its spin (RE-373). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
