# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v137 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 266 of 266 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed (batch 77): results, menus and boss scene built in place (`InlineOption`); deepest stack 163,860 to 126,888 B; low_free +3,584 B (code size); peak 14,615,040 B unchanged.
- Batch 76: scene state built in place (`enter_training` 176,248 to 24,672 B).
- Batch 75: battle allocations 0 after load (vs4@2400 except 2 FxPool grows; vsmagnify 0); eager battle slabs (46 training rows -76,800 B, f1-onep-zako-clear/-ko-flash -471,552 B); OOM reaches the fatal screen (ballast 14.5 MB: 23,124 B @4); rust-psp 6c3f2c33; peak 14,615,040 B.
- Batch 74: `golden.sh verify --ballast BYTES`; goldens 266/266 at 18.0 and 17.5 MB, min headroom 3.58 MB (f1-training-fighter-select); failures begin at 15.0 MB (RE-572). PPSSPP-side PSP-1000 acceptance passed.
- Open: cutout over a later surface (RE-424); NOFOG under demo fade; real N64 VS menu boot; Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). Gaps in `PLAN.md`.

## Next

1. MS8 performance.
2. Hardware: audio streamer smoke (misses=0, under=0), loader load times with async prio 0x1E, figatree miss latency (RE-573).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 77; deepest game stack 126,888 B of 524288.

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
