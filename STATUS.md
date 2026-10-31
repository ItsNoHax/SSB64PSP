# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v140 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 8.93 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 17.01 ms busy (Zebes 11.25) | RE-585 |
| PSP-2000 1P intro | 14.90 ms (timer build); 1P battle 15.73 ms; Comets 17.88; Fall 19.44; `vs4@2170` electric no-hit r18 18.36, shock r12 20.65 | RE-585 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Measured (batch 125): shared mesh-path footprint cut (`round_i16_sat`, `clip_mvp_column` outlined) rejected: Comets/Fall -0.40 ms but vs4/intro/battle +0.17-0.19, electric +0.2 ms with a re-searched order; code shrink shifts the trained layout (RE-585).
- Measured (batch 124): Master Hand batched effect path and ShockSmall material cache rejected: Comets/Fall -1.4/-1.6 ms but vs4 +0.3, intro +0.45, battle +0.3 ms under every re-searched order; cache alone +0.1-0.2 ms (RE-585).
- Landed (batch 123): link order c13 (boss trace in training; timer-variant fmt functions in the tail): -0.05 to -0.36 ms in every window; battle-draw group spans name the electric +2.2 ms (display effects +1.34, effects +0.50, particles +0.11) (RE-585).
- Landed (batch 122): electric windows re-mapped to `vs4@2170` (r18 no-hit, r12 shock); Master Hand cost is `BossEffects::draw` per-mesh fixed cost, not skinning; no code change (RE-585).
- Landed (batch 121): link order re-searched (s8b) after batches 100-120 moved 24-443 of 462 ordered functions; HEAD order cost +0.9 to +5.0 ms busy; `.rodata` line 0x00 kept (RE-585).
- Landed (batch 120): fighter particle bank IDs per battle scene (`ftManagerSetupFilesPlayablesAll`); a -1 bank makes nothing (1P Kirby Team Yoshi-copy egg explode); goldens unchanged (RE-475).
- Landed (batch 119): Kirby copy residency audited, no gap (copy specials in the swallowed fighter's closure or KirbySpecial1); VS stage select N64-equal on all 10 slots; 11 new goldens (RE-475).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Playtest performance above 16.7 ms: Master Hand `BossEffects::draw` I-cache refill (Comets 17.88, Fall 19.44 ms) and electric shock display effects (`vs4@2170` r12 20.65 vs no-hit r18 18.36 ms).

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 123 (hardware: RE-585); deepest game stack 127,752 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix 286 of 286, verify --twice |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
