# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the VS character select's presentation (RE-411):
  `mnplayersvs.c`'s wallpaper, top bar, sliding portraits and flash,
  panels with shutters, names, emblems, levels and arrows, glowing pucks,
  cursors, ready banner, and each slot's fighter turning and playing its
  selected clip.
- **Next gameplay batch:** the KO presentation (`ftCommonDead*` and
  `efManager`'s dead effects: the blast explosion, star KO sparkle, screen
  flash and top-out fade, and the rebirth halo). VS item spawning waits
  on the item kinds.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| VS select presentation | `players_vs::layer`, `players_screen`; sprite files 0, 17, 19–21 and eight gate LUTs packed; `f1-vs-players` rebaselined | RE-411 |
| Results layer | `results_layer`; sprite files 18, 34, 36, 38 packed | RE-410 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-411; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 110 goldens match (RE-411); captures time out at
  60 s.
- PSP-2000: scripted Training held 16,682 µs per frame (RE-360).
- Pack v64: 30,405,424 bytes, SHA-256
  `720bbc897adb249921a50ce9e2382d1ea75ecfcc1881308ed39c198e55e82e67`.
- `run` is 20,956 bytes in release (RE-411); the MIPS branch range is
  128 KB. Its stack frame is about 4 KB; the largest frame is
  `enter_training` at about 128 KB, within the 256 KB main-thread stack.
- Physical PSP last checked in RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the Training and stage selects draw plain
  slots (TODO). Converted sprites: damage digits, emblems, stock icons,
  files 0, 17–21, 34, 36, 37, 38, 82, 165, 197. The results' emblem and
  confetti and the VS select's spotlight are not drawn (RE-410, RE-411).
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
