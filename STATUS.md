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
| PSP | 60 FPS under PPSSPP; on a PSP-2000 the audio thread costs 9% in a four-player VS and 16% in the opening, no underruns (RE-479); three opening scenes ran at 45–57 FPS before audio (RE-476); PSP-1000 unsupported |
| Tests | 2,264 workspace tests with the ROM (2,073 with stub tables, as CI), clippy and rustfmt pass; 198 of 198 goldens pass by pixel hash |

## Current Work

The audio batch is done (RE-479). Next: run its production EBOOT on the
PSP-2000 and compare a `no_audio` build's frame times, then RE-476/RE-477's
hardware checks and the opening's CPU-bound scenes ([`PLAN.md`](PLAN.md#remaining-work)).

## Blockers

- The PSP was disconnected at the end of the audio batch: the merged
  production EBOOT and pack v108 are built but not installed.

## Verification

Baseline after the audio batch:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`: 2,264
  pass. With `SSB64_STUB_TABLES=1` and no ROM: 2,073 pass, 191 ignored.
  `cargo +1.98.0 clippy` (warnings denied) in both modes and rustfmt pass.
- Goldens: `tools/golden.sh verify`: 198 of 198 hashes match; deepest
  game stack 310,912 bytes (`f1-vs-sudden-death`).
- How to Play's `rng_trace` (`explain@4460`): identical to `06becb0`'s.
- PPSSPP with `audio_dump`: title, mode select, `vs4@2400` and Sound Test
  play, no underrun; the attract loop at the XMB budget, lowest free
  23.1 MB.
- Production EBOOT: `target/release-audio/EBOOT.PBP`, 4,496,282 bytes
  (`d9bd863a…39f2`), no debug info, with `target/release-audio/ssb64.pak`
  (64,174,736 bytes, `38ed322c…559c`); not installed.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1 (RE-479's
  measurements, before the final merge).
- ROMs: `rom/Super Smash Bros. (USA).z64` and
  `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10…`.
