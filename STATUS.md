# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end now runs as the original's: the production build boots the
  title, and the mode select, 1P and VS menus, VS Options, the Item Switch
  and Bonus Practice reach every ported mode. `P3`'s results presentation
  remains.
- **Completed batch:** the front end's menus (`ssb_game::menu`,
  `players_1p_bonus`, RE-462), replacing `psp-game`'s placeholder main menu.
- **Next gameplay batch:** the VS results' presentation (TODO "VS battle
  presentation and players"): the results' wipe, the fighters' fade-in and
  the remaining VS HUD pieces, from `mnvsresults.c` and `ifcommon.c`.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Front end | Title, mode select, 1P/VS menus, VS Options, Item Switch, Bonus Practice | RE-462 |
| Options and data menus | Six menus, per-scene menu packs | RE-461 |
| Save data | `lbBackup`, memory-stick save, VS records and unlocks | RE-460, D-045 |
| 1P last scenes | Ending, staff roll, congratulations, challengers, messages | RE-459 |

## Verification baseline

- 2,065 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v103 unchanged (46,645,200 bytes, SHA-256 `96b70bba…690c`).
  `ssb64-menus.pak`: 13 scene packs, 6,821,296 bytes, SHA-256
  `3568d3d65fe488b4bab905ad82e343914fafe6df9421b476aa5c0f7fec882287`.
  No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game boots the
  title under PPSSPP software and runs its attract loop (title,
  Characters' demo, title) for 25 seconds without faults.
- Capture scenes `title`, `modeselect`, `onepmode`, `vsoptions`,
  `itemswitch`, `bonusselect` and `bonuspractice` run under
  PPSSPPHeadless; six front-end scenes overlay N64 references.
- Golden matrix: 78 of 195 match. The previous commit already differs in
  116 scenes (small fighter-pose drift in Training and VS scenes); this
  batch changes only `f1-vs-mode-menu`, rebaselined. The drift needs
  triage before the next rebaseline. Physical PSP last checked RE-361.

## Blockers and remaining scope

- No blocker. The opening movie, How to Play and the auto demo are not
  ported (the title skips them), nor Sound Test, the anti-piracy flags and
  the menus' audio (`TODO.md`).
- Rumble, select spotlight, fighter jostling and Yoshi's double-jump apex
  hang remain. Fidelity, performance and VRAM belong to P5.
