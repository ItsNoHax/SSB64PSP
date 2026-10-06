# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The 1P Game now runs from its select through GAME CLEAR, the ending,
  the staff roll and congratulations into a challenger's battle and its
  unlock message. Shared machinery and PSP game integration remain before
  `P2` is complete.
- **Completed batch:** 1P ending, staff roll, congratulations,
  Challenger Approaching and unlock messages, with the credits tables,
  letters, room and camera packed from the ROM (RE-459).
- **Next gameplay batch:** save data (`lbBackup`): persist the backup's
  unlocks, records and settings to the memory stick, so unlocks and 1P
  records survive a reboot (TODO "Save data and unlocks").
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| 1P last scenes | Ending, staff roll, congratulations, challengers, messages | RE-459 |
| Special fighters | Metal Mario, Giant DK, Polygon Team stages; TopN scale | RE-458 |
| Master Hand | Boss fighter, bullets, wallpaper, cameras, defeat, GAME CLEAR | RE-457 |
| Bonus pause and entry fade | Source map zoom, L: Retry and black 12-tick fade | RE-456 |

## Verification baseline

- 1,977 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v103: 46,645,200 bytes, SHA-256
  `96b70bbaacdfe9cbe6f6af92238be8c8f6d848b60b4fb9e8726449cb5e48690c`.
  PPSSPP reports about 6 MB of user memory free with it loaded. No ROM
  assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game runs under
  PPSSPP software for 20 seconds without faults.
- PPSSPP `onepending`/`onepstaffroll`/`onepcongra`/`onepchallenger`/
  `onepmessage`/`onepfinale` captures cover every new scene and one run
  into Ness's battle; no N64 reference of these scenes yet.
- Golden manifest remains 195; full matrix not rerun, and its fighter
  goldens predate TopN's scale. Physical PSP last checked RE-361 (PSP-2000,
  6.61 ARK, pack v43).

## Blockers and remaining scope

- No blocker. Session-only unlocks, the Bonus 1 select after Luigi's
  challenge, scene audio, the room's material animation and N64
  comparisons of the last scenes are in `TODO.md`, with select
  reconciliation, shade, 1P tags and the boss's and special fighters'
  fidelity items.
- Rumble, select spotlight, fighter jostling and Yoshi's double-jump apex
  hang remain. Fidelity, performance and VRAM belong to P5.
