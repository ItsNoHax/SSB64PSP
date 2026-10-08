# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** CRT overscan crop (D-047, RE-477, supersedes
  D-008).
  - The N64's visible (10,10)–(310,230) box fills the PSP's 272 lines at
    272/220 and is centred, about 371 px wide with black bars.
  - The 10-pixel strip is cropped. The N64 never draws there: libgc clamps
    every scissor 10 px inside the frame.
  - `ssb_engine::coord` is the only screen mapping.
  - The battle camera frames at its viewport's 15/11, as the N64 does.
- **Next batch:** the PSP-2000 checks of D-047's GE fill cost and RE-476's
  frozen picture, and installing the new EBOOT. After that: the opening's
  CPU-bound scenes on the PSP (`TODO.md`, CPU performance), the CPUs'
  special effects and the status remainder.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Overscan crop | Visible box fills the height; strip cropped; one mapping | RE-477, D-047 |
| Intro slowdowns | Background scene reads; no load hitch in the intro on a PSP-2000 | RE-476 |
| Scene loading | Every scene under 17 MB; XMB budget on a PSP-2000 | RE-475, D-046 |
| CPU performance | 60 FPS under PPSSPP | RE-470, RE-471 |

## Verification baseline

- Tests: 2,174 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0). Clippy (`cargo +1.98.0 clippy`, warnings denied) and rustfmt
  pass.
- Goldens: the matrix was rebaselined in one update; 197 of 198 changed
  and 198 of 198 now pass. Three captures were identical.
- How to Play's trace is identical to `e5b3b07`'s.
- PPSSPP CPU per frame is unchanged; `vs4@4000` averages 5.25 ms.
- Deepest game stack: 239,840 of 524,288 bytes.
- Pack v107 is unchanged (`487bfd8a…`). No ROM assets are committed.
- The production EBOOT has no debug info and no `boot_log`. It is
  4,195,114 bytes (`45de0375…40ef`), in `target/release-re477/`, not
  installed: the PSP was disconnected.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1 (RE-476). D-047 has not
  run on it.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64` both SHA-1 `e2929e10…`.

## Blockers and remaining scope

- The PSP-2000 is unchecked on two counts:
  - The GE fill cost of the crop: the battle viewport covers 19% more
    pixels.
  - The opening's CPU-bound scenes (Run, Yoster/Sector, Clash at 45–57
    FPS).
- Scenes outside the attract loop still load in their first frame
  (`TODO.md`).
- Still missing: the opening's rendering differences (RE-467), the CPUs'
  special effects, Sound Test, rumble and all audio.
