# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The backup now has its menus: Option, Screen Adjust, Backup Clear, Data,
  VS Record and Characters run from the main menu. Shared machinery and PSP
  game integration remain before `P2` is complete.
- **Completed batch:** the options and data menus (`ssb_game::menu`,
  RE-461), each scene's sprites read from `ssb64-menus.pak` when it starts
  and freed when it ends.
- **Next gameplay batch:** the front end's remaining menus, replacing
  `psp-game`'s placeholder: `mnTitle`, `mnModeSelect`, `mn1PMode`, the VS
  mode menu's sprites, `mnVSOptions` and `mnVSItemSwitch` (with the Item
  Switch gate) and the 1P Bonus select's records (`mnPlayers1PBonus`; TODO
  "Remaining save users and the menus' leftovers").
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Options and data menus | Six menus, per-scene menu packs | RE-461 |
| Save data | `lbBackup`, memory-stick save, VS records and unlocks | RE-460, D-045 |
| 1P last scenes | Ending, staff roll, congratulations, challengers, messages | RE-459 |
| Special fighters | Metal Mario, Giant DK, Polygon Team stages; TopN scale | RE-458 |

## Verification baseline

- 2,011 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v103 (unchanged): 46,645,200 bytes, SHA-256
  `96b70bbaacdfe9cbe6f6af92238be8c8f6d848b60b4fb9e8726449cb5e48690c`.
  New `ssb64-menus.pak` beside it: 4,466,560 bytes, six scene packs of
  160 KB to 2.7 MB, SHA-256
  `10f52013d37cc190a96739a75eca946bba4d0754c17b8fc4a7f3e48d24c6407e`.
  No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game runs under
  PPSSPP software for 20 seconds without faults, loading and writing
  `ssb64.sav`.
- Capture scenes `option`, `screenadjust`, `backupclear`, `datamenu`,
  `vsrecord` and `characters` run under PPSSPPHeadless; the first five
  overlay warp-booted N64 references to within edge resampling.
- Golden manifest remains 195; full matrix not rerun. Physical PSP last
  checked RE-361 (PSP-2000, 6.61 ARK, pack v43); the save and the menu
  packs are not yet tried on hardware.

## Blockers and remaining scope

- No blocker. VS Options, Sound Test, the menus' audio, the Characters
  fighter's motion-script events and the anti-piracy validators are in
  `TODO.md`, with the Bonus 1 select after Luigi's challenge, scene audio
  and the 1P fidelity items.
- Rumble, select spotlight, fighter jostling and Yoshi's double-jump apex
  hang remain. Fidelity, performance and VRAM belong to P5.
