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
| Tests | 2,275 workspace tests with the ROM (2,084 with stub tables, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

Measured where four-player VS's CPU time goes next (RE-489). The GE's
texel reads from main RAM cost the CPU 2.2 ms of memory stall a frame
on a PSP-2000 (`vs4@2400` without audio, textures off: 42.0 → 48.5
FPS). That is the largest measured lever. The rest of the shared draw
pass costs under 1% of its I-cache misses, and `#[optimize(size)]` on
the per-mesh path is worse. Next: which textures carry the texel
traffic, and what VRAM can hold them: the frozen picture's 512 KiB
photo outside its scenes; 16-bit colour buffers would change every
golden and need a decision ([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after outlining `draw_stage_links` (RE-488); RE-489 changed
no code:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,275
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 2,084 pass, 191 ignored;
  `cargo +1.98.0 clippy` (warnings denied) in both modes and rustfmt
  pass (both as of RE-487; no workspace crate changed since).
- Both PSP builds (`psp-game`, `psp-asset-viewer`) compile clean in
  release.
- Goldens: `tools/golden.sh verify`: 198 of 198 hashes match; deepest
  game stack 312,584 bytes (`f1-vs-sudden-death`).
- PPSSPP smoke test (`tools/run-ppsspp.sh`): runs clean.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the production PRX
  ran the attract loop for four minutes without an exception; `profile`
  A/B runs with and without `no_audio` (RE-479); a `profile_hw` run of
  `vs4@2400`, 19 reports (RE-481); function-ordered and `opt-level s`
  `profile_hw` runs of `vs4@2400` (RE-482); cached display-list
  builds against `91323e8` (RE-485); stall-interval builds against
  `2960b58` (RE-486); compacted-hot-path builds, both link orders,
  against RE-487; outlined `draw_stage_links` against `f0f4ed6`
  (RE-488); textures-off diagnostic against `3dca012` (RE-489).
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
