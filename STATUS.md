# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the VS battle's game status, countdown, timer, KO
  credit and sudden death (RE-389), run from the menu's VS entry. Before
  it: KOs and rebirth (RE-388) and spawn facing (RE-387).
- **Next gameplay batch:** CPU AI, part 1 (`P4`): `ftkey.c` input
  playback and `ftcomputer.c`'s target and objective choice, host-tested,
  so the VS CPU moves. The Appear entry is parked with its findings in
  TODO (32-bit clips, three leading runtime joints).
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| VS battle | `ssb_game::battle`, `damage_player`; 9 host tests; new `f1-vs-countdown`, `f1-vs-sudden-death` | RE-389 |
| KO and rebirth | `ssb_game::dead` replaces the simplified port; 11 host tests; new `f1-training-rebirth` | RE-388 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread: all pass.
  Pinned 1.98.0 Clippy with warnings denied and workspace rustfmt passed in
  RE-389; the `thumbv7em-none-eabi` `no_std` builds last passed in RE-369.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: all 95 goldens match twice (RE-389), including the
  two VS scenes. No golden passes through a platform.
- `psp-game` reaches scripted Training in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v60: 27,262,288 bytes, SHA-256
  `81cf9a097c500d7019933f6bf5ec8d8b9545a68a78a7156742c3ed8a897525fc`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- No stage draws its wallpaper; the selects draw plain slots (TODO).
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
