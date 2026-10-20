# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v121 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 230 of 230 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed: trig parity chunk 6 (RE-503): `topn_rotate_z` (Kirby Stone slope, Falcon Kick), joint-4 pitch and Pikachu scale (`model_pitch.rs`), CST/CLC closers.
- Rebaselined: `f1-vs-four` 99024 native px, `f1-vs-four-magnifier` 99606 native px (CPU Fox's pitched Fire Fox hits DK from tick ~619). New rows: `f1-training-fox-fire`, `f1-training-pikachu-quick`.
- Open: N64 pitch comparison needs analog-stick input in `tools/run-n64-headless.sh` / `n64_driver.py` (PLAN: Trig goldens).
- Open: shield poke at `vsshield` t=610 (0.7-unit margin) awaits an N64 trace (RE-418).

## Next

1. MS8 list in `PLAN.md`, 1P intro first.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: trig parity chunk 6.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix verified (230 of 230) |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
