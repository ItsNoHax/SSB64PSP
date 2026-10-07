# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** physical-PSP pass (RE-469). The KO crash was a
  0/0 in the KO blast's particles, trapped by the PSP's FPU; a 0 × inf in
  the CPU's input and a stack overflow of the 256 KiB main thread were
  found with it. All three are fixed and KOs run on a PSP-2000.
- **Next batch:** fighter fidelity from How to Play frame 3274: a caught
  fighter is placed a frame late and a thrown one is released from the
  hand, not its joint 4 less 300 (RE-468, `TODO.md`).
- **Parallel track:** rendering and CPU performance (`P5`): a PSP runs
  battles at 8–15 FPS (RE-469).

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Physical PSP | KO crash fixed; 512 KiB stack; first hardware profile | RE-469 |
| Fighter fidelity | How to Play to frame 3273/4039; opening battles match | RE-468 |
| Opening movie | N64 logo, 19 opening scenes, title opening layout; pack v106 | RE-467 |
| Fighter movement | How to Play matches the N64 to frame 735 | RE-466 |

## Verification baseline

- 2,144 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,908,848 bytes, SHA-256 `a319e657…d341`.
  `ssb64-menus.pak` (28 scenes): 13,163,676 bytes. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game boots into
  the N64 logo and the opening at 60 FPS under PPSSPP software.
- Golden matrix: 197 of 197 match, twice; deepest game stack 239,016 of
  524,288 bytes. `tools/stack-check.sh` covers the 1P Game and opening
  (peak 294,304).
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, pack v106 (RE-469):
  pushed-off, smashed-off and run-off KOs and seven scenes run without
  exceptions. Battles 8–15 FPS, CPU-bound; lowest free memory 1,346,816
  bytes in the opening's Jungle scene.

## Blockers and remaining scope

- No blocker. Whether a PSP started from the XMB traps FPU exceptions as
  PSPLink does is unchecked (RE-469).
- The setter first-frame audit, looping figatrees' `anim_frame`, the
  opening's rendering differences (RE-467), the CPUs' special effects,
  Sound Test, rumble and all audio remain (`TODO.md`).
