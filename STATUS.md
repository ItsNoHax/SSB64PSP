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

- Landed (batch 73): fighter figatrees stream per status (RE-572 C). Battles and the opening hold 20 pinned statuses per kind plus a 64 KiB LRU slab per kind; presentation scenes do not stream. Held bytes -0.36..-4.0 MB; `low_free` up to +3.66 MB on 183 of 197 rows; 14 menu rows -4,864 B (code size, accepted). Peak 14,615,040 B (f1-vs-results-link). Max 4 misses a frame in battle, 8 on the opening's first frame (PPSSPP).
- Open: cutout over a later surface (RE-424); NOFOG under demo fade; Mew catcher flag read (RE-568); real N64 VS menu boot; Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). Gaps in `PLAN.md`.

## Next

1. PSP-1000 acceptance: goldens under `memory_ballast` 18 MB, then MS8.
2. Hardware: audio streamer smoke (misses=0, under=0), loader load times with async prio 0x1E, figatree miss latency (RE-573).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 73; deepest game stack 367,616 B of 524288.

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
