# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the entry effects (RE-403): the pipe, barrel,
  capsule, Link's wave and beam, egg and leftward star play at each entry.
  Before it: the VS battle entry (RE-402).
- **Next gameplay batch:** `mnPlayersVS`. The Arwing, car and Poké Ball
  entries (their own updates) stay in `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Entry effects | `EntryEffect`, `effect_ticks`, `ENTRY_EFFECT_KEYS`, `draw_entry_effects`; 1 golden rebaselined | RE-403 |
| VS entry | `ssb_game::appear`, 32-bit skeleton mode, entry focus; 6 goldens rebaselined | RE-402 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-403; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 106 goldens match twice (RE-403). Scene captures
  time out at 60 s. No golden passes through a platform.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v63: 28,254,080 bytes, SHA-256
  `8bfe1acc108ec0305fd7ab8b96c06077c2961d6d244bcb11d0bdfafef1130197`.
- `run` is 107,232 bytes in release (RE-403); the
  MIPS branch range is 128 KB.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO). Converted
  sprites: damage digits, emblems, stock icons, files 37, 82, 165, 197.
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
