# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** fighter fidelity in How to Play (RE-472). Its
  gameplay now matches the N64 on every frame (1–4448): the catch frame,
  the held fighter's hand and rotation, the throw's release from joint 4,
  Mario's Fireball, the Fire Flower's held pose and flame clock, and a hit
  status posed through its hitlag.
- **Next batch:** measure RE-470 and RE-471 on the PSP-2000 when it is
  available: RE-469's six scenes, then RE-471's per-frame worst cases
  (`profile` build, `stage=`/`hold` lines). Until then, How to Play's
  effect draws of the generator from frame 3250 (shield grab, item on a
  shield, `DamageFlyTop`; RE-472, `TODO.md`), then the setter first-frame
  audit against the opening and VS traces.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| How to Play fidelity | Gameplay matches the N64 to the scene's end; host replay to frame 3590 with grabs, KO, rebirth and weapons | RE-472 |
| CPU worst frames | 333 MHz, word `memcpy`, per-texel magnifier mask, decoded map surfaces; no gameplay frame over 12 ms under PPSSPP | RE-471 |
| CPU performance | Indexed pack lookups, compile-time sine table, unblocked present; 60 FPS under PPSSPP | RE-470 |
| Physical PSP | KO crash fixed; 512 KiB stack; first hardware profile | RE-469 |

## Verification baseline

- 2,159 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v106: 46,908,848 bytes, SHA-256 `a319e657…d341` (unchanged).
  `ssb64-menus.pak` (28 scenes): 13,163,676 bytes. No ROM assets are
  committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. Both link with
  `--wrap=memcpy,memmove,memset` from their `.cargo/config.toml`; a set
  `RUSTFLAGS` hides it. The production game runs the opening at 60 FPS
  under PPSSPP software.
- Golden matrix: 198 of 198 match (six rebaselined for RE-472, each
  traced to one fix); deepest game stack 239,488 of 524,288 bytes.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, pack v106 (RE-469):
  battles 8–15 FPS before RE-470. RE-470's and RE-471's hardware numbers
  are pending.

## Blockers and remaining scope

- The PSP was unavailable for RE-470 to RE-472: their frame times are
  PPSSPP's. Whether a PSP started from the XMB traps FPU exceptions as
  PSPLink does is unchecked (RE-469).
- Scene loads still take one long frame (up to 39 ms in `sceIoRead` under
  PPSSPP); a battle's first frame takes 14–18 ms (`TODO.md`, `P5`).
- How to Play's effect draws, a figatree's end resetting `anim_frame`, the
  setter first-frame audit, Fox's Blaster spawn point, the opening's
  rendering differences (RE-467), the CPUs' special effects, Sound Test,
  rumble and all audio remain (`TODO.md`).
