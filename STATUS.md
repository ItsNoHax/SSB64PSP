# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v138 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 271 of 271 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 9.08 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579 |
| PSP-2000 `vs4@2400` | 16.55 ms busy (Zebes 11.10) | RE-585 |
| PSP-2000 1P intro | 14.81 ms (timer build); 1P battle 15.56 ms; vs4 16.62; opening 8.89; Fall 19.44 | RE-585 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 104): proc_map audit, 24 fixes (Ness air PSI Magnet ledge, Samus Screw Attack ledge, Yoshi/Link/Captain/Kirby ceiling stops; RE-586); Reflect Break effect made and drawn; stale module docs. Goldens unchanged.
- Landed (batch 99): frozen 1P-intro models compose once; order s4; Slim intro 17.35 to 14.81 ms; new golden `f1-onep-intro-zako` (ballast low_free 6,801,920 B, equal to base; RE-585).
- Landed (batch 98): display effects skip the empty split head (shock 19.08 to 18.25 ms with order s3 re-searched after 5d1bf2ae shifted the layout; RE-584, RE-585).
- Landed (batch 89): Master Hand Fall wallpaper translucent (two-pass texel alpha; N64 MAE 30.4 to 5.6 / 26.3 to 7.3; PSP-2000 matches PPSSPP); new goldens `f1-onep-boss-fall-fadein`, `-full`; pack v138; boss low_free -133,376 B; link order re-searched (s7), Slim vs4 -0.15, intro -1.43, battle -0.48 ms, Fall +0.44 (RE-321, RE-585).
- Landed (batches 93-94): anim-less display effects reuse one `StageAnimator` (electric hit window 23.1 to 19.1 ms, Fall 22.2 to 18.4 ms on the Slim); `.rodata` pinned at line offset 0 (RE-584).
- Landed (batch 90): boot audio start and first save at tick 2 (no sound call precedes it); PSP-2000 `psp_main` to frame 1 534 to about 417 ms, boot-to-opening unchanged; frame stack pinned at d-cache way offset 0x1600 (RE-583); `psp-link.py` warns on unmatched link-order lines; `boot_log` buffered to frame 60; pack of another version stops on a fatal screen naming both versions (RE-582).
- Landed (batch 84): in-place battle locals, order re-searched on the timed build, `.bss` pinned to a data-cache way. Slim ms0: vs4 17.16 to 16.76, intro 18.29 to 17.86, battle 16.41 to 15.90, Zebes 11.53 to 11.36; low_free -1,280 B (RE-580).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Reflect Break N64 trace + golden (`PLAN.md`).
2. Playtest performance: electric hit (17.48 no hit, 18.23 shock) and Master Hand windows still above 16.7 ms.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 104 (hardware: RE-585); deepest game stack 127,752 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix 271 of 271, verify --twice |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
