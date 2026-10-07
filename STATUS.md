# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** scene loading by archive file (RE-475, D-046). The
  XMB black screen was the whole 47 MB pack in a 47.9 MB budget, and
  `psp`'s allocation-error handler spinning. The game now holds the
  pack's tables and shared region (5.5 MB) and loads each scene's N64
  files when it starts; the menus' pack and the opening's models pack are
  folded into `ssb64.pak`. A load failure ends on a text screen. A
  speculated NaN compare in `grab::nearest_catch` that trapped on the PSP
  is fixed.
- **Next batch:** the user's XMB check on the PSP-2000, then RE-470 and
  RE-471's hardware frame times. Then the CPUs' special effects drawn in
  `psp-game` and the status remainder (`TODO.md`).

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Scene loading | Every scene under 17 MB (was 48–51 MB); the XMB budget runs the attract loop on a PSP-2000 | RE-475, D-046 |
| Status machine remainder | `anim_frame` is the N64's figatree clock | RE-474 |
| Effects and setters | How to Play's RNG matches the N64 every frame | RE-473 |
| CPU performance | 60 FPS under PPSSPP | RE-470, RE-471 |

## Verification baseline

- 2,172 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Golden matrix 198 of 198, no rebaseline; 66 other capture scenes
  pixel-identical to the previous build (A/B). Deepest game stack
  239,808 of 524,288 bytes.
- Pack v107 (59.7 MB on the stick, 5.5 MB resident). No ROM assets are
  committed. `psp-game` ships without debug info (EBOOT 4.2 MB).
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the attract loop at
  the XMB's 47.9 MB and at 46 MB (`memory_ballast`), at least 23.8 MB
  free throughout (RE-475). The XMB launch itself is the user's check.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64` both SHA-1 `e2929e10…`.

## Blockers and remaining scope

- The XMB launch is untested by the agent: PSPLink cannot start a PBP.
- Scene loads take one long frame (0.1–0.6 s on the PSP); the opening's
  rendering differences (RE-467), the CPUs' special effects, Sound Test,
  rumble and all audio remain (`TODO.md`).
