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
| Tests | 2,183 workspace tests with the ROM (1,993 with stub tables, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

Hardware check of the RE-476/RE-477 builds on the PSP-2000: install the
production EBOOT, measure D-047's GE fill cost and confirm RE-476's frozen
picture. Then the opening's CPU-bound scenes on the PSP and the CPUs'
special effects ([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The PSP was disconnected: the production EBOOT is built but not installed.
  D-047 has not run on a PSP.

## Verification

Baseline after D-048 (build-time tables, golden hashes):

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,183
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 1,993 pass, 190 ignored.
  `cargo +1.98.0 clippy` (warnings denied) in both modes and rustfmt pass.
- Goldens: `tools/golden.sh verify`: 198 of 198 hashes match.
- How to Play's `rng_trace` (`explain@4460`): every value on all 13,347
  lines identical to `de8793c`'s; ten call sites in `motion/mod.rs` moved
  five lines.
- Both PSP builds; PPSSPP smoke of `psp-game` at 60 FPS.
- PPSSPP CPU per frame: `vs4@4000` averages 5.25 ms. Deepest game stack:
  239,840 of 524,288 bytes.
- Production EBOOT: `target/release-re477/EBOOT.PBP`, 4,195,114 bytes
  (`45de0375…40ef`), no debug info, not installed.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1 (RE-476).
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
