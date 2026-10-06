# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  `P3`'s results presentation is done: the VS results draw in full and the
  battle's falls and team steals show on the interface.
- **Completed batch:** the VS results' wipe and fade and the remaining VS
  interface pieces (RE-464), after triaging the golden matrix's drift
  (RE-463).
- **Next gameplay batch:** the title's attract modes, which the title now
  skips: How to Play (`scexplain.c`) and the auto demo (`scautodemo.c`),
  scripted battles on the existing VS machinery (TODO "Remaining save
  users and the menus' leftovers").
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| VS results presentation | Wipe over the battle's last frame (whole photo, pack v104), fighters' fade-in, stock snaps, "SCORE ±1", team stock steal | RE-464 |
| Golden drift triage | 116 goldens traced to RE-458, RE-449 and RE-454 and rebaselined | RE-463 |
| Front end | Title, mode select, 1P/VS menus, VS Options, Item Switch, Bonus Practice | RE-462 |
| Options and data menus | Six menus, per-scene menu packs | RE-461 |

## Verification baseline

- 2,079 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v104: 46,533,744 bytes, SHA-256 `136e8b3b…21d3`.
  `ssb64-menus.pak` (v104 header): 6,821,296 bytes, SHA-256
  `f3e2d072…03c2`. Two builds are byte-identical. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game boots to the
  title under PPSSPP software and runs 25 seconds without faults.
- Golden matrix: 195 of 195 match. Diagnostic `vsteamsteal` (outside the
  manifest) shows a steal's arc. Physical PSP last checked RE-361.

## Blockers and remaining scope

- No blocker. No N64 view of a wipe: Mupen64Plus's Rice plugin never shows
  the CPU-copied photo and GLideN64 crashes in the harness (RE-464).
- The CPUs' special effects (Magnet, Sing, Spin Attack swirl, Falcon
  Punch/Kick, reflector, Charge Shot, held egg), the select's spotlight,
  rumble and all audio remain (`TODO.md`).
