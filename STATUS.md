# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v109 (`d3efddb2…`) with the audio section |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | The N64 audio system ported whole, every call site wired, Sound Test; sample-exact against N64 captures of the opening, menus and How to Play ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md), RE-477) |
| Frontend | N64 logo, opening, title and attract loop; every menu including Sound Test; per-scene loading with background reads (RE-475, RE-476) |
| PSP | 60 FPS under PPSSPP. PSP-2000 (RE-479): the production build plays the attract loop clean; the opening averages 55.8 FPS (58.7 without audio); four-player VS 47.3 without audio (RE-491; 40.6 at RE-479), held under 60 by the game thread; no underruns. PSP-1000 unsupported |
| Tests | 2,277 workspace tests with the ROM (2,086 with stub tables, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

Stored every direct-colour texture with at most 256 colours as CLUT4/CLUT8
with a palette of its own colours (RE-491). The change is lossless: all 198
goldens are unchanged. 1,433 of 1,761 8888 textures and 126 of 156 5551
textures qualify, and the pack shrinks from 64.2 to 47.0 MB. On a PSP-2000,
`vs4@2400` without audio goes from 45.7 to 47.3 FPS (CPU frame
19.3 → 18.9 ms, all memory stall), with 4 MiB more RAM free in battle. No
other large textured rectangle is drawn in the battle loop. About 1.15 ms
of texel stall remains: the wallpaper (0.5 ms, 3,031 colours) and stage
textures that are already CLUT4. Next: wallpaper residency, then the next
`MS8` lever ([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after lossless palettization (RE-491):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,277
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 2,086 pass, 191 ignored;
  `cargo +1.98.0 clippy` (warnings denied) in both modes and rustfmt
  pass.
- Both PSP builds (`psp-game`, `psp-asset-viewer`) compile clean in
  release.
- Goldens: `tools/golden.sh verify`: 198 of 198 hashes match, none
  rebaselined; deepest game stack 312,584 bytes (`f1-vs-sudden-death`).
- PPSSPP smoke test (`tools/run-ppsspp.sh --crate psp-game`): runs clean.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: the production PRX
  ran the attract loop for four minutes without an exception; `profile`
  A/B runs with and without `no_audio` (RE-479); a `profile_hw` run of
  `vs4@2400`, 19 reports (RE-481); function-ordered and `opt-level s`
  `profile_hw` runs of `vs4@2400` (RE-482); cached display-list
  builds against `91323e8` (RE-485); stall-interval builds against
  `2960b58` (RE-486); compacted-hot-path builds, both link orders,
  against RE-487; outlined `draw_stage_links` against `f0f4ed6`
  (RE-488); textures-off diagnostic against `3dca012` (RE-489);
  per-category `texture_probe` and wallpaper-strip builds against
  `8650b0b` (RE-490); palettized-texture builds against the strips
  (RE-491). The installed EBOOT and its `ms0:` pack are still v108.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
