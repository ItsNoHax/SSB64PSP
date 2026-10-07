# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** effects and setters (RE-473). How to Play's
  generator now matches the N64 on all 4,448 frames; the item hit paths
  make their set-offs and sparks; Fox's Blaster leaves joint 17 as the
  frame posed it; 44 special statuses of six fighters play their setter's
  first frame, each against an N64 Training trace.
- **Next batch:** measure RE-470 and RE-471 on the PSP-2000 when it is
  available: RE-469's six scenes, then RE-471's per-frame worst cases
  (`profile` build, `stage=`/`hold` lines). Until then, the fighter status
  machine's remainder: the untraced setters (Donkey Kong's cargo throw and
  walk, the Barrel and Tornado captures, the item throws, Kirby's copies
  and aerial Inhale end) with RE-473's warp traces, and a figatree's end
  resetting `anim_frame` (`TODO.md`).

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Effects and setters | How to Play's RNG matches the N64 every frame; item hit effects; Blaster spawn exact; 44 setters traced | RE-473 |
| How to Play fidelity | Gameplay matches the N64 to the scene's end | RE-472 |
| CPU worst frames | No gameplay frame over 12 ms under PPSSPP | RE-471 |
| CPU performance | 60 FPS under PPSSPP | RE-470 |

## Verification baseline

- 2,164 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,908,848 bytes, SHA-256 `a319e657…d341` (unchanged). No ROM
  assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game runs the
  opening at 61 FPS under PPSSPP software.
- Golden matrix: 198 of 198 match (four rebaselined for RE-473, each
  traced); deepest game stack 239,664 of 524,288 bytes.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, pack v106 (RE-469).
  RE-470's and RE-471's hardware numbers are pending.

## Blockers and remaining scope

- **`rom/Super Smash Bros. (USA).z64` is damaged:** a tool error in RE-473
  overwrote its header CRC words (bytes `0x10`–`0x17`, now
  `ab9253bf e265a8ef`; SHA-1 `ba145cee…`). Restore it from
  `refs/ssb-decomp-re/baserom.us.z64` (SHA-1 `e2929e10…`, the original
  words `916b8b5b 780b85a4`) before trusting `SSB64_ROM` runs or N64
  traces against it. RE-473's tests and traces used the baserom copy.
- The PSP was unavailable for RE-470 to RE-473: their frame times are
  PPSSPP's.
- Scene loads still take one long frame; the opening's rendering
  differences (RE-467), the CPUs' special effects, Sound Test, rumble and
  all audio remain (`TODO.md`).
