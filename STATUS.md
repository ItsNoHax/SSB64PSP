# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end now runs the title's whole attract loop but the N64 logo
  and opening movie.
- **Completed batch:** the title's attract modes (RE-465): How to Play
  (scripted Mario and Luigi, its 22-phase window, stick, spark and overlay)
  and the auto demo (four CPUs, focus camera, names), wired title → How to
  Play → Characters' demo → auto demo → title.
- **Next gameplay batch:** the fighter movement How to Play's scripts expose
  (TODO "Fighter movement against How to Play's scripts"): Turn's pivot
  gate, a dash's end into Walk, and the appear's rise and landing. Then the
  N64 logo and opening movie (`mnStartup`, `mvOpening*`), a separate scene
  subsystem left as the loop's boundary.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Attract modes | How to Play and auto demo ported; pack v105 (How to Play wallpaper), menu-pack scene `Explain`; N64 RDRAM trace finds three fighter divergences | RE-465 |
| VS results presentation | Wipe, fade-in, stock snaps, scores, team steal | RE-464 |
| Golden drift triage | 116 goldens rebaselined | RE-463 |
| Front end | Title, mode select, 1P/VS menus, Bonus Practice | RE-462 |

## Verification baseline

- 2,094 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v105: 46,795,968 bytes, SHA-256 `e26b63eb…85e3`.
  `ssb64-menus.pak` (14 scenes): 7,548,456 bytes, SHA-256
  `d164b41e…c52c`. No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. Idle at the title, the
  production game enters How to Play at 60 FPS under PPSSPP software.
- Golden matrix: 195 of 195 match. Diagnostics `explain[@N]` and
  `autodemo` (outside the manifest). Physical PSP last checked RE-361.

## Blockers and remaining scope

- No blocker. How to Play drifts from the N64 from explain frame 254 (Turn)
  and Luigi is knocked out at 1441 (RE-465).
- The CPUs' special effects, the select's spotlight, Sound Test, rumble and
  all audio remain (`TODO.md`).
