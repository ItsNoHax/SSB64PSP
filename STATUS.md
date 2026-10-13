# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v108 (`38ed322c…`) with the audio section |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | The N64 audio system ported whole, every call site wired, Sound Test; sample-exact against N64 captures of the opening, menus and How to Play ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md), RE-477) |
| Frontend | N64 logo, opening, title and attract loop; every menu including Sound Test; per-scene loading with background reads (RE-475, RE-476) |
| PSP | 60 FPS under PPSSPP. PSP-2000 (RE-479): the production build plays the attract loop clean; the opening averages 55.8 FPS (58.7 without audio); four-player VS 39.0 (40.6 without audio), held under 60 by the game thread; no underruns. PSP-1000 unsupported |
| Tests | 2,275 workspace tests with the ROM (2,077 with stub tables, as CI, before RE-484), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

Allocation is off the kernel (RE-484): a size-class heap serves blocks up
to 32 KiB, and VS's per-frame lists are `ArrayVec`s. VS makes one
allocation a frame instead of 8–11; on the PSP-2000 `vs4@2400` goes from
40.1 to 41.0 FPS (CPU 18.54 → 17.77 ms) for 0.3–0.7 MB of free memory.
rust-psp is pinned at fork commit `3ecf457` (`external-allocator`).
Next: the
uncached list, then compacting the per-mesh hot path
([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after the allocator batch (RE-484):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,275
  pass (stub-table count below is from the magnifier-mask batch). With `SSB64_STUB_TABLES=1` and no ROM: 2,077 pass, 191 ignored.
  `cargo +1.98.0 clippy` (warnings denied) in both modes and rustfmt pass.
- Goldens: `tools/golden.sh verify`: 198 of 198 hashes match; deepest
  game stack 310,944 bytes (`f1-vs-sudden-death`).
- How to Play's `rng_trace` (`explain@4460`): identical to `06becb0`'s.
- PPSSPP with `audio_dump`: title, mode select, `vs4@2400` and Sound Test
  play, no underrun; the attract loop at the XMB budget, lowest free
  23.1 MB.
- Production EBOOT: `target/release-audio/EBOOT.PBP`, 4,496,282 bytes
  (`d9bd863a…39f2`), no debug info, with `target/release-audio/ssb64.pak`
  (64,174,736 bytes, `38ed322c…559c`); installed in
  `ms0:/PSP/GAME/ssb64/` (verified byte-identical).
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the production PRX
  ran the attract loop for four minutes without an exception; `profile`
  A/B runs with and without `no_audio` (RE-479); a `profile_hw` run of
  `vs4@2400`, 19 reports (RE-481); function-ordered and `opt-level s`
  `profile_hw` runs of `vs4@2400` (RE-482).
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
