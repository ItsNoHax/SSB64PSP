# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The 1P Game runs from its select through its ending into a challenger's
  battle and unlock, and the backup now survives a reboot. Shared machinery
  and PSP game integration remain before `P2` is complete.
- **Completed batch:** save data (`lbBackup`): the full backup in the N64
  layout, validated and defaulted as the source does, saved to
  `ssb64.sav` beside the pack (D-045) at every write; the selects, stage
  select, VS results, Mew and the screen flash read it (RE-460).
- **Next gameplay batch:** the options and data menus that show and edit
  the backup: `mnOption`, `mnScreenAdjust`, `mnBackupClear`, `mnData`,
  `mnVSRecord` and `mnCharacters` (TODO "Backup menus and remaining save
  users").
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Save data | `lbBackup`, memory-stick save, VS records and unlocks | RE-460, D-045 |
| 1P last scenes | Ending, staff roll, congratulations, challengers, messages | RE-459 |
| Special fighters | Metal Mario, Giant DK, Polygon Team stages; TopN scale | RE-458 |
| Master Hand | Boss fighter, bullets, wallpaper, cameras, defeat, GAME CLEAR | RE-457 |

## Verification baseline

- 1,993 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v103 (unchanged): 46,645,200 bytes, SHA-256
  `96b70bbaacdfe9cbe6f6af92238be8c8f6d848b60b4fb9e8726449cb5e48690c`.
  PPSSPP reports about 6 MB of user memory free with it loaded. No ROM
  assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game runs under
  PPSSPP software for 20 seconds without faults, and two runs from one
  directory write and then reload `ssb64.sav` (boot 1, then 2); corrupted
  copies fall back as `lbBackupIsSramValid` does.
- `saveunlock` then `saveplayers` show a written Ness unlock reloaded on
  the VS select. `vssuddendeath` differs from its golden identically
  before and after this batch (fighter goldens predate TopN's scale).
- Golden manifest remains 195; full matrix not rerun. Physical PSP last
  checked RE-361 (PSP-2000, 6.61 ARK, pack v43); the save is not yet tried
  on hardware.

## Blockers and remaining scope

- No blocker. The backup menus, VS Options' Item Switch, the 1P Bonus
  select's records, the anti-piracy validators and `lbBackupApplyOptions`
  are in `TODO.md`, with the Bonus 1 select after Luigi's challenge,
  scene audio and the 1P fidelity items.
- Rumble, select spotlight, fighter jostling and Yoshi's double-jump apex
  hang remain. Fidelity, performance and VRAM belong to P5.
