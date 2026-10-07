# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** CPU performance (`P5`, RE-470). Every scene of
  RE-469's profile runs at about 60 FPS under PPSSPP's cycle model, from
  8–20; a four-fighter battle's CPU time fell from 114 to 9.3 ms a frame.
- **Next batch:** measure RE-470 on the PSP-2000 (every scene of the
  RE-469 table, before and after), then fighter fidelity from How to
  Play frame 3274: a caught fighter is placed a frame late and a thrown
  one is released from the hand, not its joint 4 less 300 (RE-468,
  `TODO.md`). If the PSP misses 60 FPS, the remaining hotspots come first
  (`TODO.md`, `P5`).

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| CPU performance | Indexed pack lookups, compile-time sine table, unblocked present; 60 FPS under PPSSPP | RE-470 |
| Physical PSP | KO crash fixed; 512 KiB stack; first hardware profile | RE-469 |
| Fighter fidelity | How to Play to frame 3273/4039; opening battles match | RE-468 |
| Opening movie | N64 logo, 19 opening scenes, title opening layout; pack v106 | RE-467 |

## Verification baseline

- 2,149 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,908,848 bytes, SHA-256 `a319e657…d341`.
  `ssb64-menus.pak` (28 scenes): 13,163,676 bytes. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game boots into
  the N64 logo and the opening at 60 FPS under PPSSPP software (RE-470).
- Golden matrix: 198 of 198 match (`f1-vs-four-magnifier` is new);
  deepest game stack 239,344 of 524,288 bytes.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, pack v106 (RE-469):
  battles 8–15 FPS before RE-470. RE-470's hardware numbers are pending.

## Blockers and remaining scope

- The PSP was unavailable for RE-470: its frame rates are PPSSPP's.
  Whether a PSP started from the XMB traps FPU exceptions as PSPLink does
  is unchecked (RE-469).
- The setter first-frame audit, looping figatrees' `anim_frame`, the
  opening's rendering differences (RE-467), the CPUs' special effects,
  Sound Test, rumble and all audio remain (`TODO.md`).
