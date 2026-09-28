# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** item drawing (RE-382, RE-383). Items count their
  animation plays. The PK Fire flame and Link's Bomb draw; the Bomb is
  held in the item joint's frame and loose at the item position. `run` was
  split to get back inside MIPS branch range.
- **Next gameplay batch:** the shield bubble (RE-369), then stage
  selection (TODO). Stage and fighter select remain
  deferred.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Link's Bomb | Pack v59; right attributes record, held parent; new `f1-training-link-bomb` golden; 93 of 93 match twice | RE-383 |
| Items, part 1 | Pack v58; PK Fire flame draws (viewer-checked, not captured); 92 of 92 goldens match twice | RE-382 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-383 (psp-runtime rustfmt flags one older spot);
  the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 93 goldens match twice (RE-383), including the
  new Link's Bomb scene. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v59: 27,262,288 bytes, SHA-256
  `77df9d7631f6d10ee2ced8e8beb09c52b7b428f985f321d13792b79261e04de3`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The shield bubble is not drawn (RE-369).
- Only capture scenes pick fighters other than Mario in `psp-game`.
  Kirby's copy hats, Pikachu's Thunder, the Egg Lay victim's egg and most
  hit effects are not drawn. The Fireball does not spin (RE-372; kind 71
  is `func_ovl0_800CA5C8`, RE-373), and a released Charge Shot restarts
  its spin (RE-373). Other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
