# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v111 with the audio section |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | The N64 audio system ported whole, every call site wired, Sound Test; sample-exact against N64 captures of the opening, menus and How to Play ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md), RE-477); per-node model matrices compose with a raw VFPU `vmmul.q` (RE-497) |
| Frontend | N64 logo, opening, title and attract loop; every menu including Sound Test; per-scene loading with background reads (RE-475, RE-476) |
| PSP | 60 FPS under PPSSPP. PSP-2000 `vs4@2400` (`profile` builds, own searched order each): 57.6 FPS from a VFPU matrix compose and a `sqrt.s` skin loop (RE-497), then 55.3 → 56.5 FPS from a scalar billboard matrix (RE-498, separate baseline). Production build (RE-479): attract loop clean, opening averages 55.8 FPS (58.7 without audio), four-player VS 48.27 without audio (RE-493). PSP-1000 unsupported |
| Tests | workspace tests with the ROM (stub tables pass separately, as CI), clippy and rustfmt pass; 207 of 207 goldens pass by pixel hash |

## Current Work

RE-517: 2D sprite edge fringe fixed by a one-ring RGB bleed baked at pack time (pack v111, filters unchanged). RE-516's Samus charge also ran on the PSP-2000 from the XMB without a crash. Next, per [`PLAN.md`](PLAN.md#remaining-work): the swallowed-B audit (RE-516), the XMB FCR31 readout, then wind-reaction effects and the slope follow-ups.

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` and `frame_timer` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after RE-517:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: pass.
  `SSB64_STUB_TABLES=1` with no ROM: pass (tests that need ROM tables, including
  the joint masks, are ignored there). Clippy (`cargo
  +1.98.0`, workspace, `-D warnings`) and `cargo fmt --all --check` pass. Parallel `ssb-game` runs fail global-state tests (PLAN.md).
- Both PSP release builds compile.
- Goldens: `tools/golden.sh verify`: 207 of 207 match (pack `ssb64.pak` rebuilt as v111).
- PPSSPP smoke: `run-ppsspp-headless.sh --crate psp-game --scene vs4@2400`
  exits 0 and writes a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; RE-506 matrix cost
  1.46 us against 2.61 us for the float path. Frame rates: RE-499 (`vs4@2400`
  +0.89 FPS toggle). The `ms0:` pack on the PSP is v108-sized; hardware
  profile runs need a local pack symlink beside the host0 PRX.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
