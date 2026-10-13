# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v109 (`d3efddb2…`) with the audio section |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | The N64 audio system ported whole, every call site wired, Sound Test; sample-exact against N64 captures of the opening, menus and How to Play ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md), RE-477); per-node model matrices compose with a raw VFPU `vmmul.q` (RE-497) |
| Frontend | N64 logo, opening, title and attract loop; every menu including Sound Test; per-scene loading with background reads (RE-475, RE-476) |
| PSP | 60 FPS under PPSSPP. PSP-2000 `vs4@2400` (`profile` builds, own searched order each): 56.5 → 57.6 FPS from a VFPU matrix compose and a `sqrt.s` skin loop (RE-497). Production build (RE-479): attract loop clean, opening averages 55.8 FPS (58.7 without audio), four-player VS 48.27 without audio (RE-493). PSP-1000 unsupported |
| Tests | 2,278 workspace tests with the ROM (stub tables pass separately, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

RE-497 covered the `MS8` VFPU render-matrix item: `compose_model_matrix`
issues a raw `vmmul.q` with no `vfpu_context` save/restore (skipping the
per-call cost that made RE-487's scalar code win), +0.70 FPS on a
PSP-2000. The skinned-vertex loop's real cost was libm's software
`sqrtf`, not its matrix math; a correctly-rounded Allegrex `sqrt.s`
(bit-identical against 249,001 inputs) gave +0.45 FPS unchanged. A VFPU
`vtfm`-based rewrite of the same skin loop is a rejected negative result:
slower on hardware and it adds rounding PPSSPP's goldens cannot see —
PPSSPP emulates `vmmul.q`/`vtfm` as bit-identical scalar sums, the
PSP-2000 does not, a standing blind spot for future VFPU work. The link
order was re-searched for the new hot-function sizes (RE-494/496's
procedure, seed 8 of 12), verified byte-identical between a real
`cargo psp --release` build and a `relink.py` relink. Next: the next
`MS8` lever ([`PLAN.md`](PLAN.md#remaining-work)) — the billboard arm's
`load_billboard_matrix` is unmeasured, and the opening/movie draw path
still has no golden.

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after the VFPU compose, `sqrt.s` skin change and the link-order
re-search that followed it (RE-497; `pack.rs`, `psp-runtime/{lib,meshdraw}.rs`,
`psp-game/link-order.txt` changed):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,278
  pass, 0 failed. `SSB64_STUB_TABLES=1` with no ROM: passed. Clippy
  (`cargo +1.98.0`) and `cargo fmt --check` pass.
- Both PSP builds (`psp-game`, `psp-asset-viewer`) compile clean in release.
- Goldens: `tools/golden.sh verify`: 198 of 198 match (pack v109).
- PPSSPP smoke test (`vs4@2400`): exit 0.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1: `profile,
  capture_scene_file,no_audio` builds of `vs4@2400`, each relinked with
  its own searched link order, ran 56.5 → 57.6 FPS (RE-497). The
  installed production EBOOT and its `ms0:` pack are still v108.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
