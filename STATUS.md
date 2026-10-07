# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  The front end boots into the N64 logo and runs the whole attract loop.
- **Completed batch:** the fighter status machine's remainder (RE-474).
  `anim_frame` follows the figatree's clock (loops back to 0, stands at
  its end, carries through statuses without a figatree) beside the port's
  status clock; How to Play's `anim_frame` matches the N64 on all but 19
  of 8,896 samples. Donkey Kong's cargo and Spinning Kong, Kirby's swallow
  and copies, the rapid jab starts, Captain Falcon's, Pikachu's and Ness's
  specials and the capture victims play their setter's first frame, each
  against an N64 Training trace.
- **Next batch:** measure RE-470 and RE-471 on the PSP-2000 when it is
  available: RE-469's six scenes, then RE-471's per-frame worst cases
  (`profile` build, `stage=`/`hold` lines). Until then, the next gameplay
  subsystem in `PLAN.md`/`TODO.md`: the CPUs' special effects drawn in
  `psp-game` (Kirby's stars and copied effects), then the status
  remainder (`TODO.md`: the shield pose's clock, the untraced Barrel,
  Tornado and item-throw setters).

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Status machine remainder | `anim_frame` is the N64's figatree clock; the remaining setters traced and played | RE-474 |
| Effects and setters | How to Play's RNG matches the N64 every frame; 44 setters traced | RE-473 |
| How to Play fidelity | Gameplay matches the N64 to the scene's end | RE-472 |
| CPU performance | 60 FPS, worst gameplay frame under 12 ms under PPSSPP | RE-470, RE-471 |

## Verification baseline

- 2,169 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied (also `ssb-game` with `rng_trace`)
  and rustfmt pass.
- How to Play under PPSSPPHeadless: status, position, damage, facing and
  the random generator match the N64 on all 4,448 frames.
- Pack v106 unchanged. No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings; the production game runs the
  opening at 60 FPS under PPSSPP software.
- Golden matrix: 198 of 198 match (three rebaselined for RE-474, each
  traced); deepest game stack 239,760 of 524,288 bytes.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, pack v106 (RE-469).
  RE-470's to RE-474's hardware numbers are pending.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64` both SHA-1 `e2929e10…` (the earlier
  header damage is repaired).

## Blockers and remaining scope

- The PSP was unavailable for RE-470 to RE-474: their frame times are
  PPSSPP's.
- Scene loads still take one long frame; the opening's rendering
  differences (RE-467), the CPUs' special effects, Sound Test, rumble and
  all audio remain (`TODO.md`).
