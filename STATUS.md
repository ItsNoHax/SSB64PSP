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

- Landed: camera rig (`camera_pass`), `CpuSight`, `explain_movement` on `matchtick::step`, stale dedup keyed `(attack_id, motion_count)`; goldens unchanged. Before: match-loop frame in decomp order, per-battle `ManagerCounts`. RE-556, [D-052](docs/decisions/D-052.md).
- Open: stage-clear zoom N64 trace (RE-554); `proc_physics`-set status with a pending accessory maker (RE-555 deviation, no current move). PK Fire flame route (RE-352); Bomb fuse sparkle (RE-444). Shield poke at `vsshield` t=610 awaits an N64 trace (RE-418). Gaps in `PLAN.md`.

## Next

1. Further playtest reports; open gameplay rows in `PLAN.md`; then MS8.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 21 (D-052); deepest game stack 366880 B.

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
