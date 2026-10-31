# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v140 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 8.86 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 16.96 ms busy (Zebes 11.24) | RE-585 |
| PSP-2000 1P intro | 14.89 ms (timer build); 1P battle 15.70 ms; Comets 17.46; Fall 19.23; `vs4@2170` electric no-hit r18 18.33, shock r12 20.62 | RE-585 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Measured (batch 130): pack-side mesh merge for Master Hand saves 0 of 72 meshes: Comets meshes have 3 textures on 3 animated nodes, Fall meshes 8 matanims; no data-only lever left (RE-585).
- Landed (batch 129): link order h5, trained on a real Fall trace (earlier "Fall" traces ended before Fall): Comets -0.46, Fall -0.22 ms, every window faster (-0.02 to -0.09) (RE-585).
- Measured (batch 128): deferred prefetch alone fixes Room tic 57 (24.9 -> 16.3 ms busy, waits behind I/O 6.1 -> <=2.2 ms) but its 164-function layout shift costs +0.06-0.27 ms; rejected pending a link-order re-search (RE-575).
- Measured (batch 127): loader miss-alone at 64 KiB alone rejected: intro +0.99 ms, vs4/battle within noise; Room tic 57 still waits 8.3 ms (the deferred prefetch fixed it in batch 126) (RE-575).
- Measured (batch 126): 16 KiB background chunks, miss-alone, deferred prefetch rejected: opening waits behind I/O 6.8 -> 1.7 ms, but timer windows +0.13-0.27 ms, intro +1.14 ms (one round; USB link dropped) (RE-575).
- Measured (batch 125): shared mesh-path footprint cut (`round_i16_sat`, `clip_mvp_column` outlined) rejected: Comets/Fall -0.40 ms but vs4/intro/battle +0.17-0.19, electric +0.2 ms with a re-searched order; code shrink shifts the trained layout (RE-585).
- Measured (batch 124): Master Hand batched effect path and ShockSmall material cache rejected: Comets/Fall -1.4/-1.6 ms but vs4 +0.3, intro +0.45, battle +0.3 ms under every re-searched order; cache alone +0.1-0.2 ms (RE-585).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Playtest performance above 16.7 ms: Master Hand `BossEffects::draw` I-cache refill (Comets 17.46, Fall 19.23 ms) and electric shock display effects (`vs4@2170` r12 20.62 vs no-hit r18 18.33 ms).

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 129 (hardware: RE-585); deepest game stack 127,752 B of 524288.

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
