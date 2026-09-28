# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** items in `psp-game`, partly (RE-382). Items count
  their animation plays; pack v58 adds item animations, and the PK Fire
  flame draws. Link's Bomb does not draw yet. `run` was split to get back
  inside MIPS branch range.
- **Next gameplay batch:** Link's Bomb under its held-item hand parent
  (kind 0x52), with a capture that reaches the PK Fire flame (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Items, part 1 | Pack v58; PK Fire flame draws (viewer-checked, not captured); 92 of 92 goldens match twice | RE-382 |
| Ness in Training | Pack v57; three new Ness goldens; 92 of 92 match twice | RE-381 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-382 (psp-runtime rustfmt flags one older spot);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 92 goldens match twice (RE-381), including the
  three Ness scenes. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v58: 27,262,256 bytes, SHA-256
  `c4ae3c3e67a0b8c50aa9aa2a508d6b727facf5e2cccfc2537b8a49ee53736d71`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only capture scenes pick fighters other than Mario in `psp-game`.
  Kirby's copy hats and Pikachu's Thunder are not drawn. Link's Bomb item, the Egg Lay
  victim's egg and the egg/star hit effects are not drawn. The Fireball does not spin (RE-372;
  its kind 71 is `func_ovl0_800CA5C8`, RE-373). A released Charge Shot
  restarts its spin (RE-373). The other
  fighters remain host-only; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
