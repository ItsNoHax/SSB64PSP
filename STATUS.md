# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v125 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 243 of 243 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed: scripted subject scenes `f1-vs-magnify` (`vsmagnify@660`) and `f1-training-purin-throw` (`purinthrow@112`) replace `f1-vs-four-magnifier` and `f1-vs-four-fly-roll`; subjects survive RNG changes. RE-561. CPU-match rows `f1-vs-four`, `f1-vs-cpu`, `f1-vs-team` still move on gameplay changes.
- Open: Smash DI lag-update equivalence (RE-560); magnifier and FlyRoll measurement re-takes (RE-561); stage-clear zoom N64 trace (RE-554); `proc_physics`-set status with a pending accessory maker (RE-555). PK Fire flame route (RE-352); Bomb fuse sparkle (RE-444). Shield poke at `vsshield` t=610 awaits an N64 trace (RE-418). Gaps in `PLAN.md`.

## Next

1. Further playtest reports; open gameplay rows in `PLAN.md`; then MS8.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 26 (RE-561); deepest game stack 366896 B.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix verified (243 of 243) |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
