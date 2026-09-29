# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the VS results screen's 2D layer (RE-410): the
  wallpaper, fades, player tags, winner text, tint and table from
  `mnvsresults.c`, with their tics, positions and colours. Before it: the
  results' fighters (RE-409).
- **Next gameplay batch:** the VS character select's presentation
  (`mn/mnplayers/mnplayersvs.c`: portraits, panels, names and the
  fighters on the select), the last VS screen still drawn as plain slots.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Results layer | `ssb_game::results_layer`; sprite files 18, 34, 36, 38 packed; two goldens rebaselined | RE-410 |
| Results fighters | `ssb_game::results_scene`, `psp-game` `results_screen`; new `f1-vs-results` golden, `f1-vs-no-contest` rebaselined | RE-409 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-410; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 110 goldens match (RE-410). Scene captures
  time out at 60 s. No golden passes through a platform.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v64: 29,127,792 bytes, SHA-256
  `abb0b38a5e472408faadb64511613e5a6f03ca3e1e826cea3c58b2219946eb66`.
- `run` is 20,800 bytes in release (RE-410); the MIPS branch range is
  128 KB. Its stack frame is about 4 KB; the largest frame is
  `enter_training` at about 128 KB, within the 256 KB main-thread stack.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO). Converted
  sprites: damage digits, emblems, stock icons, files 18, 34, 36, 37, 38,
  82, 165, 197. The results' emblem and confetti are not drawn (RE-410).
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
