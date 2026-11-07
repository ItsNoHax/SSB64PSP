# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v148 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 7.58 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 14.43 ms busy (Zebes 10.52 at batch 180; batch 181) | RE-585, RE-589 |
| PSP-2000 1P intro | 13.81 ms (timer build); 1P battle 12.83 ms; `vs4@2170` electric no-hit r18 16.02, shock r12 16.84 (batch 181) | RE-479, RE-585, RE-589 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 186): first-launch installer: separate `INSTALL.PBP`, started from the game by LoadExec; PSP-2000 at the PSP-1000 start budget (22.4 MB) 8.78 min, pack identical, lowest free 9.78 MB; 64-byte-aligned sink (writes 129 to 16.5 s) (D-053, RE-588 item 15).
- Landed (batch 185): on-PSP pack build at PSP-1000 memory: paged ROM, blob spill (RE-588 item 14).
- Landed (batch 181): tables batch E: 10 motion tables (scripts, motion descs, combat attributes, demo scripts) from the pack (v147, 456 KB section); every generator `no_std`; motion lookups out of line (inlined: +0.15 ms from `.text` pins); PSP mean +0.04 ms (RE-589 item 8). On-PSP pack build 6.20 min, pack identical, lowest free 9.0 MB, after an FPU-trap fix and no file cache on the PSP (RE-588 item 13).
- Measured (batch 157): Media Engine probe: ssb-engine DSP bit-exact on the ME at SC speed, SC slowdown < 2% (RE-479).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Electric `vs4@2170` r12 16.84 ms, over the 16.7 target: 0.33 ms from batch 172 pack content, cause not found (RE-589 items 5, 7, 8). Media Engine sleep/wake and PSP-1000 need a person (PLAN "Audio cost").
2. Owner hardware tests: XMB hand-off EBOOT -> INSTALL.PBP, HOME mid-build, NoSpace screen, PSP-1000 run (RE-588 item 15 pending list).

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 186 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Three PSP release builds (game, installer, viewer) | compile |
| Goldens | full matrix 286 of 286, verify --twice |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
