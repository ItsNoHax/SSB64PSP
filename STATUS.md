# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the Training stage select (RE-385). `mnMaps`'s
  cursor, lock, random pick and idle return are ported; Training loads any
  VS stage. The shield bubble draws (RE-384).
- **Next gameplay batch:** the Training fighter select (TODO): real pad
  input still always spawns Mario.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Stage select | `ssb_game::stage_select`; Training on the picked stage; new `f1-training-stage-select` (Hyrule Castle); `run` at 111 KB | RE-385 |
| Shield bubble | Pack v60; shield ENV seed and colour override; `f1-training-shield` rebaselined; 93 of 93 match twice | RE-384 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-385; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 94 goldens match twice (RE-385), including the
  new stage-select scene. No golden passes through a platform or respawns.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v60: 27,262,288 bytes, SHA-256
  `81cf9a097c500d7019933f6bf5ec8d8b9545a68a78a7156742c3ed8a897525fc`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper, and the stage select has no names or
  previews (TODO). Mushroom Kingdom stays locked: there is no save data.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The one-frame grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Only capture scenes pick fighters other than Mario in `psp-game`.
  Kirby's copy hats, Pikachu's Thunder, the Egg Lay victim's egg and most
  hit effects are not drawn. The Fireball does not spin (RE-372; kind 71
  is `func_ovl0_800CA5C8`, RE-373), and a released Charge Shot restarts
  its spin (RE-373). Other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
