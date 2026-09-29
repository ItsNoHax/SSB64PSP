# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the VS results screen's fighters (RE-409):
  `mnVSResultsInitFightersAll`'s placement, scale, facing and Win/Lose
  pick, posed with the demo clips under the results camera. Before it:
  the demo clips in pack v64 (RE-408).
- **Next gameplay batch:** the results screen's text, numbers and
  wallpaper sprites.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Results fighters | `ssb_game::results_scene`, `psp-game` `results_screen`; new `f1-vs-results` golden, `f1-vs-no-contest` rebaselined | RE-409 |
| Demo clips | `anim::SLOT_WIN1`..`SLOT_LOSE` from `dFT<Name>SubMotionDescs`; pack v64; goldens unchanged | RE-408 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-409; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 110 goldens match twice (RE-409). Scene captures
  time out at 60 s. No golden passes through a platform.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v64: 28,510,000 bytes, SHA-256
  `33fd70558a870a81d9e6ff283127739f5d49bccdb98c1ae3073f2836056e0e13`.
- `run` is 20,800 bytes in release (RE-409); the MIPS branch range is
  128 KB. Its stack frame is about 4 KB; the largest frame is
  `enter_training` at about 128 KB, within the 256 KB main-thread stack.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO). Converted
  sprites: damage digits, emblems, stock icons, files 37, 82, 165, 197.
  With no save data, Mushroom Kingdom and the four unlockable fighters
  stay locked.
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The one-frame grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder, the Egg Lay victim's egg and most
  hit effects are not drawn. The Fireball does not spin (RE-372; kind 71
  is `func_ovl0_800CA5C8`, RE-373), and a released Charge Shot restarts
  its spin (RE-373). Other item kinds remain.
- Four-fighter VS exceeds the ~700 KiB VRAM texture pool (TODO). The
  stage draw costs 4.5 ms per frame on hardware (RE-360); performance is
  `P5`.
