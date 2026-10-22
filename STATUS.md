# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v131 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 247 of 247 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed (batch 40): 1P fighter shade/fog (pack v131), ending room matanim, CP/heart tags, bonus pause (N64-traced), intro RSP camera and modelview lights, loss/win host traces; new goldens `f1-bonus-pause`, `f1-onep-yoshi-shade`, none moved (RE-453, RE-456, RE-458, RE-459).
- Landed (batch 39): 1P stage 6 and 8 CPU replays bit-exact; facing-velocity audit (`set_lr`), same-frame interrupts after `proc_update` status sets, dead-chain order, Kirby-copy escape; red silhouette confirmed N64 (colanim 39); rebaselined `f1-vs-four` (98583), `f1-vs-team` (96145), `f1-vs-shield-damage` (92245 native px, set-off tick 1274) (RE-413, RE-418, RE-464, RE-568).
- Landed (batch 38): 1P/CPU traces: stage-4 CPU replay bit-exact, Link Hyrule wake, wall arithmetic, friction sign, CPU over-stage/detect-size/try_roll fixes (RE-503, RE-510, RE-512, RE-566, RE-568).
- Landed (batch 37): VS two-pad harness and traces; PK reflect fixes, link-order weapon search (RE-566).
- Landed (batch 36): PK Fire/Thunder on deferred procs; CPU cliff scan on live groups (RE-566, RE-568).
- Open: Mew N64 read (costly, RE-568). Countdown RNG seed vs a real N64 VS boot. Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). PK Fire flame route (RE-352); Bomb fuse sparkle (RE-444). Gaps in `PLAN.md`.

## Next

1. Further playtest reports; open gameplay rows in `PLAN.md`; then MS8.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 40; deepest game stack 392400 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix verified (247 of 247) |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
