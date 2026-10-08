# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v107 (`487bfd8a…`) |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Rendering | CRT overscan crop done ([D-047](docs/decisions/D-047.md), RE-477): the N64's visible box fills the PSP's 272 lines |
| Frontend | N64 logo, opening, title and attract loop; per-scene loading with background reads (RE-475, RE-476) |
| PSP | 60 FPS under PPSSPP; on a PSP-2000 three opening scenes run at 45–57 FPS (RE-476); PSP-1000 unsupported |
| Tests | 2,189 workspace tests with the ROM (1,999 with stub tables, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

RE-478 fixed the crash after Race to the Finish (the Polygon Team's intro
read a name for the Race's Polygons). Next: the user's PSP check of the
RE-478 EBOOT, then RE-476/RE-477's hardware checks (D-047's GE fill cost,
the frozen picture), the opening's CPU-bound scenes on the PSP and the
CPUs' special effects ([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The PSP was in USB mass-storage mode, not PSPLink: RE-478's fix and
  D-047 have not run on a PSP.

## Verification

Baseline after RE-478:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,189
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 1,999 pass, 190 ignored.
  `cargo +1.98.0 clippy` (warnings denied) and rustfmt pass.
- Goldens: `tools/golden.sh verify`: 198 of 198 hashes match; deepest
  game stack 239,856 bytes.
- How to Play's `rng_trace` (`explain@4460`): identical to `a440575`'s.
- PPSSPP at the XMB budget: the campaign from each fixture-cleared stage
  through the next stages, the ending and staff roll, no panic; deepest
  stack 295,232 bytes, lowest free 32.4 MB (RE-478).
- Production EBOOT: `target/release-re478/EBOOT.PBP`, 4,195,190 bytes
  (`e2f9d44a…3199`), no debug info, not installed.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1 (RE-476).
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
