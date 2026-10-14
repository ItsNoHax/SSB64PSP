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
| PSP | 60 FPS under PPSSPP. PSP-2000 `vs4@2400` (`profile` builds, own searched order each): 57.6 FPS from a VFPU matrix compose and a `sqrt.s` skin loop (RE-497), then 55.3 → 56.5 FPS from a scalar billboard matrix (RE-498, separate baseline). Production build (RE-479): attract loop clean, opening averages 55.8 FPS (58.7 without audio), four-player VS 48.27 without audio (RE-493). PSP-1000 unsupported |
| Tests | workspace tests with the ROM (stub tables pass separately, as CI), clippy and rustfmt pass; 201 of 201 goldens pass by pixel hash |

## Current Work

RE-504: gameplay `__sinf`/`__cosf` equal the ROM's own routines bit for bit
(oracle `tools/r4300`; f64 port plus a double-single fast path, exhaustively
checked on the host, PSP-2000 hashes match). 4 goldens rebaselined. Next: the
`gSYSinTable` third family (`syGetSinCosUShort`: camera `project`, staffroll
fovy, `from_trs`, item root matrix), then the audit follow-ups in
[`PLAN.md`](PLAN.md#remaining-work).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink falls over now and then (a `kill`, an `opt-level s` build):
  restarting it needs a person at the PSP. `profile` and `frame_timer` builds tick power
  every frame; other builds still let an unattended PSP auto-sleep
  (RE-481).

## Verification

Baseline after the trig fidelity pass 2 (RE-504):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: pass.
  `SSB64_STUB_TABLES=1` with no ROM: pass. Clippy (`cargo +1.98.0`, workspace,
  `-D warnings`) passes; changed files are rustfmt-clean (`psp-game/src`
  was already not). Parallel `ssb-game` runs fail global-state tests, also at
  the previous commit (PLAN.md).
- Both PSP builds (`psp-game`, `psp-asset-viewer`) compile in release.
- Goldens: `tools/golden.sh verify`: 201 of 201 match (pack v109; 4 rebaselined, RE-504).
- PPSSPP smoke: `run-ppsspp-headless.sh --crate psp-game --scene vs4@2400`
  exits 0 and writes a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1, `profile,
  capture_scene_file,no_audio` builds of `vs4@2400`: +0.89 FPS same-binary toggle
  (RE-499; RE-498 55.31 to 56.45). The `ms0:` pack on the PSP is v108-sized (64 MB); hardware
  profile runs need a local pack symlink beside the host0 PRX, else
  nothing draws. The installed production EBOOT is v108 too.
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
