# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** the intro's slowdowns on a PSP-2000 (RE-476). The
  next scene's files are read in the background (`sceIoReadAsync`)
  through the opening and the attract loop, an opening scene's files are
  entered after the hold that still draws the last one, and the opening
  figures' figatrees and fights' battle files are in their lists: no
  load hitch is left in the intro (worst frame on the PSP 538 ms → 40 ms).
  The room's frozen picture is sampled from VRAM (the GE spent 27 ms a
  frame on it from main memory).
- **Next batch:** the PSP-2000 after-run of the frozen picture and the
  install of the RE-476 EBOOT (the PSP left USB mid-batch); then the
  opening's CPU-bound scenes on the PSP (`TODO.md`, CPU performance), the
  CPUs' special effects and the status remainder.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Intro slowdowns | Background scene reads; no load hitch in the intro on a PSP-2000; frozen picture from VRAM | RE-476 |
| Scene loading | Every scene under 17 MB; the XMB budget runs the attract loop on a PSP-2000 | RE-475, D-046 |
| Status machine remainder | `anim_frame` is the N64's figatree clock | RE-474 |
| CPU performance | 60 FPS under PPSSPP | RE-470, RE-471 |

## Verification baseline

- 2,173 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Golden matrix 198 of 198, no rebaseline; the attract loop's 11,602
  frames hash identical to `ee67b6a` under PPSSPP. Deepest game stack
  239,808 of 524,288 bytes.
- Pack v107 (unchanged, `487bfd8a…`). No ROM assets are committed.
  Production EBOOT without debug info or `boot_log`: 4,201,482 bytes,
  `511945e4…47a2`, in `target/release-re476/` (not installed).
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the opening at the
  XMB budget before and after the background reads (RE-476).
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64` both SHA-1 `e2929e10…`.

## Blockers and remaining scope

- On the PSP-2000, Run, Yoster/Sector and Clash run at 45–57 FPS (12–14 ms
  of CPU in the movie draw's mesh lists) and each opening fight's first
  frame takes 21–40 ms; scenes outside the attract loop still load in
  their first frame (`TODO.md`). The opening's rendering differences
  (RE-467), the CPUs' special effects, Sound Test, rumble and all audio
  remain.
