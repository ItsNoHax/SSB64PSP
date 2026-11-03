# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v140 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 8.89 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 16.97 ms busy (Zebes 11.27) | RE-585 |
| PSP-2000 1P intro | 14.85 ms (timer build); 1P battle 15.63 ms; Comets 15.83; Fall 16.94; `vs4@2170` electric no-hit r18 18.37, shock r12 20.09 | RE-585 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 146): Fall comets on the comet code, Fall-only tail reordered for the I-cache: Fall 17.18 -> 16.94 ms; others within +0.04; memory 0 (RE-585 item 41).
- Landed (batch 145): ShockSmall material replayed once per run: r12 -0.21, r14 -0.17 ms; others within +0.03; memory 0 (RE-585 item 40).
- Landed (batch 144): one fog-override search per fogged node (`ssb_all_costume_rows`): r12 -0.10, r14 -0.09 ms; others within +0.04; memory 0 (RE-585 item 39).
- Landed (batch 143): exact float-UV clipper for the Fall sheets: Fall 17.31 -> 17.21 ms; other windows within +0.01 ms; free memory -8 KiB (RE-585 item 38).
- Landed (batch 140): Fall pass 1 reuses pass 0's clip, texel plan hoisted: Fall 17.60 -> 17.29 ms; other windows within +0.07 ms (RE-585 item 37).
- Landed (batch 139): small shocks batched (`DisplayBatch`) and import stubs pinned (`STUB_SET`): r12 20.60 -> 20.34, r14 19.40 -> 19.07 ms; other windows within +0.06 ms (RE-585 item 36).
- Landed (batch 138): Fall's LOD prims batched through `draw_unbatched` (comet functions unchanged): Fall 18.41 -> 17.52 ms; Comets 15.79 -> 15.80; other windows within +0.06 ms (RE-585).
- Measured (batch 132): ShockSmall material cache leaves the shock window unchanged (r12 20.59/20.60 in place) and costs intro +0.5 ms; rejected (RE-585).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Playtest performance above 16.7 ms: Master Hand Fall (16.94 ms) and electric shock display effects (`vs4@2170` r12 20.09 vs no-hit r18 18.37 ms).

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 146 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

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
