# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** libultra sprites and the damage display (RE-392):
  `ssb_rom::sprite`, pack v61's sprite table, a 2D `SObj` draw and the
  battle damage display in Training and VS. Before it: the VS CPU fights
  (RE-391).
- **Next gameplay batch:** the rest of the VS HUD on the sprite path: the
  fighter emblems behind the damage (`ftSprites`), stock icons, the
  countdown lamps and "Go", and the timer (`ifCommon*`). The CPU's item
  objectives wait for items in VS. The Appear entry is parked in TODO.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Damage display | `ssb_rom::sprite`, pack v61, `meshdraw::draw_sprite`, `ssb_game::hud`; 34 goldens rebaselined | RE-392 |
| VS CPU fights | `computer::attack`, 10 host tests; VS CPU on trait Default; new `f1-vs-cpu` | RE-391 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-392; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 101 goldens match twice (RE-392). Scene captures
  time out at 60 s. No golden passes through a platform.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v61: 27,298,032 bytes, SHA-256
  `47f8869d4cf4f90d564b329c5a3779ee7a55c4a4edcc1a1dd5ac253f355bc3df`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO). Only
  the damage digits (file 164) are converted sprites so far.
  With no save data, Mushroom Kingdom and the four unlockable fighters
  stay locked.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- The one-frame grey damage shield and Yoshi's egg shield are not drawn (RE-384).
- Kirby's copy hats, Pikachu's Thunder, the Egg Lay victim's egg and most
  hit effects are not drawn. The Fireball does not spin (RE-372; kind 71
  is `func_ovl0_800CA5C8`, RE-373), and a released Charge Shot restarts
  its spin (RE-373). Other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
