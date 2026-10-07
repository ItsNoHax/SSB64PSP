# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** CPU worst frames (`P5`, RE-471). The game runs at
  333 MHz, links word-wise `memcpy`/`memset`/`memmove`, samples the
  magnifier mask per texel and decodes map surfaces once. Under PPSSPP
  every scene averages under 5 ms of CPU a frame; a four-fighter battle's
  worst live frame fell from 56 to 9 ms.
- **Next batch:** measure RE-470 and RE-471 on the PSP-2000 when it is
  available: RE-469's six scenes, then RE-471's per-frame worst cases
  (`profile` build, `stage=`/`hold` lines). Until then, fighter fidelity
  from How to Play frame 3274: a caught fighter is placed a frame late and
  a thrown one is released from the hand, not its joint 4 less 300
  (RE-468, `TODO.md`).

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| CPU worst frames | 333 MHz, word `memcpy`, per-texel magnifier mask, decoded map surfaces; no gameplay frame over 12 ms under PPSSPP | RE-471 |
| CPU performance | Indexed pack lookups, compile-time sine table, unblocked present; 60 FPS under PPSSPP | RE-470 |
| Physical PSP | KO crash fixed; 512 KiB stack; first hardware profile | RE-469 |
| Fighter fidelity | How to Play to frame 3273/4039; opening battles match | RE-468 |

## Verification baseline

- 2,155 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,908,848 bytes, SHA-256 `a319e657…d341`.
  `ssb64-menus.pak` (28 scenes): 13,163,676 bytes. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. Both link with
  `--wrap=memcpy,memmove,memset` from their `.cargo/config.toml`; a set
  `RUSTFLAGS` hides it. The production game runs the opening at 60 FPS
  under PPSSPP software.
- Golden matrix: 198 of 198 match; deepest game stack 239,168 of 524,288
  bytes.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, pack v106 (RE-469):
  battles 8–15 FPS before RE-470. RE-470's and RE-471's hardware numbers
  are pending.

## Blockers and remaining scope

- The PSP was unavailable for RE-470 and RE-471: their frame times are
  PPSSPP's. Whether a PSP started from the XMB traps FPU exceptions as
  PSPLink does is unchecked (RE-469).
- Scene loads still take one long frame (up to 39 ms in `sceIoRead` under
  PPSSPP); a battle's first frame takes 14–18 ms (`TODO.md`, `P5`).
- The setter first-frame audit, looping figatrees' `anim_frame`, the
  opening's rendering differences (RE-467), the CPUs' special effects,
  Sound Test, rumble and all audio remain (`TODO.md`).
