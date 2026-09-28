# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Kirby in `psp-game` Training (RE-378). Pack v55
  packs the Final Cutter wave under the weapon seed. `psp-game` now
  samples TransN for every status; Final Cutter never rose or landed
  before, and three goldens moved.
- **Next gameplay batch:** bring Pikachu and his Thunder Jolt into
  `psp-game` Training (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Kirby in Training | Pack v55; TransN for every status rebaselined three goldens; new `f1-training-kirby` golden; 85 of 85 goldens match twice | RE-378 |
| Falcon Kick flame | New `f1-training-captain-kick` golden; 84 of 84 goldens match twice | RE-377 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-378 (psp-crate rustfmt flags only older, untouched spots);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 85 goldens match twice (RE-378), including the
  new Kirby scene. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v55: 27,096,240 bytes, SHA-256
  `fc254d0b1275f1c7e56bf2533ea3a0574232df66aa02ee660a7a838ad92d95b8`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only Mario, Fox, Luigi, Samus, Link, Yoshi, Captain Falcon and Kirby
  draw in `psp-game`, and only capture scenes pick the others. Kirby's
  copy hats and stars are not drawn. Link's Bomb item, the Egg Lay
  victim's egg and the egg/star hit effects are not drawn. The Fireball does not spin (RE-372;
  its kind 71 is `func_ovl0_800CA5C8`, RE-373). A released Charge Shot
  restarts its spin (RE-373). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
