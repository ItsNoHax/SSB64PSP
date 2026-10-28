# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v137 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 266 of 266 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | Room 13.08 ms, Run 13.52-13.59, Yoster 11.94-11.97, Clash 11.15-11.19 (120-frame reports) | RE-577 |
| PSP-2000 `vs4@2400` | 18.20 ms busy (Zebes 12.46) | RE-577 |
| PSP-2000 1P intro | 19.6 ms (30.9 FPS); 1P battle 17.5 ms; opening all 1-39 9.48 ms | RE-577 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 81): vs4 link order re-searched on HEAD code; `tools/psp-link.py` matches names without hash-dependent back-references. Slim ms0: vs4 19.87 to 18.20 ms, 1P intro 24.95 to 19.58, 1P battle 19.45 to 17.51, opening 10.20 to 9.48. Bounding-box guard test rejected (slower). Goldens unchanged (RE-577).
- Landed (batch 80): frozen-model vertex replay in the 1P intro: 38.3 to 23.0 ms busy on the Slim; vs4, 1P battle, opening unchanged; low_free -6,656 B code size; goldens 266/266 (RE-576).
- Landed (batch 79): loader background reads run on the sample streamer's thread, up to 4 x 64 KiB per frame; a game-thread read holds them after the chunk in flight. Opening scenes read their `FuncStart` clips with the scene. Slim ms0: prefetch 1.15 to 3.2 MB/s; opening figatree misses 71 to 47 (max 8 to 2 per frame); audio misses 0; low_free -3,328 B code size, opening scenes up to +494,848 B (RE-575).
- Batch 77: results, menus, boss scene built in place; deepest stack 126,888 B; peak 14,615,040 B.
- Batch 75: battle allocations 0 after load (vs4@2400 except 2 FxPool grows; vsmagnify 0); eager battle slabs (46 training rows -76,800 B, f1-onep-zako-clear/-ko-flash -471,552 B); OOM reaches the fatal screen (ballast 14.5 MB: 23,124 B @4); rust-psp 6c3f2c33; peak 14,615,040 B.
- Batch 74: `golden.sh verify --ballast BYTES`; goldens 266/266 at 18.0 and 17.5 MB, min headroom 3.58 MB (f1-training-fighter-select); failures begin at 15.0 MB (RE-572). PPSSPP-side PSP-1000 acceptance passed.
- Open: cutout over a later surface (RE-424); NOFOG under demo fade; real N64 VS menu boot; Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). Gaps in `PLAN.md`.

## Next

1. MS8 performance.
2. Hardware: free memory on a PSP-1000 and an XMB launch (needs a person).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 81 (hardware: RE-577); deepest game stack 126,888 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix 266 of 266, verify --twice |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
