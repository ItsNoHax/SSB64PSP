# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v140 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 8.75 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 15.82 ms busy (Zebes 10.67) | RE-585 |
| PSP-2000 1P intro | 14.56 ms (timer build); 1P battle 14.53 ms; Comets 15.23; Fall 16.06; `vs4@2170` electric no-hit r18 16.93, shock r12 18.78 | RE-479, RE-585 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Measured (batch 157): Media Engine probe: ssb-engine DSP bit-exact on the ME at SC speed, SC slowdown < 2% (RE-479).
- Landed (batch 156): exact VFPU reverb: audio thread -16 to -19 ms/s; every window faster (intro -0.23, Fall -0.15, Comets -0.14, r18 -0.11 ms); free memory -40 KiB PSP, -64 KiB goldens (RE-479).
- Rejected (batch 155): cross-fighter staged draw, r18 -0.16 ms only (RE-585 item 50).
- Landed (batch 154): plain compose for unflagged skeletons: r12 -0.36, r18 -0.31, vs4 -0.24 ms; every window faster; free memory -7.75 KiB (RE-585 item 49).
- Landed (batch 153): `.text` pinned per function (`link-pins.txt`), map surfaces walked once per fighter tick: r18 -0.22, vs4 -0.26, Zebes -0.22 ms; Comets +0.03; free memory -176 KiB (RE-585 item 48).
- Landed (batch 152): lean gameplay joints sample in the tail: r12 -0.05, battle -0.11 ms; intro +0.02; free memory -7.75 KiB (RE-585 item 47).
- Measured (batch 151): `vs4@2170` update-side profile; layout shift rejected a joints cut (RE-585 item 46).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Broad MS8 cuts first (owner): Media Engine audio integration (PLAN "Audio cost"; probe passed), then memory rows; electric `vs4@2170` (r12 18.78, r18 16.93 ms) after.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 156 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

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
