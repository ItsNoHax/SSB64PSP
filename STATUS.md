# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v142 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 7.58 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 14.64 ms busy (Zebes 10.41) | RE-585, RE-589 |
| PSP-2000 1P intro | 13.62 ms (timer build); 1P battle 12.83 ms; `vs4@2170` electric no-hit r18 15.65, shock r12 16.55 | RE-479, RE-585, RE-589 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 172): texture compensation only for the owner's 37 textures; coverage and fits skipped for the rest; host `romtool pack` 122 to 4.3 s; 55 goldens rebaselined (owner-approved; RE-588 item 8).
- Landed (batch 171): `ssb-engine` sine tables read from the pack's new `TABLES` section (`ssb-tables`, fixed `.bss` store on the PSP; pack v142): r12 +0.16 (16.55, under 16.7), battle +0.17 ms; host tests build the section from the ROM (RE-589).
- Landed (batch 162): sprite lookups memoized (`SpriteIndex.memo`): r12 -0.12 (16.59, under 16.7), battle -0.35, vs4 -0.10 ms; no window slower; free memory -8 KiB, fighter select -256 KiB more (RE-585 item 52).
- Landed (batch 161): map surfaces bucketed by kind (`SurfaceTable`): r12 -0.14, r18 -0.17, battle -0.10, Fall -0.11 ms; no window slower; free memory +8 KiB (RE-585 item 51).
- Measured (batch 157): Media Engine probe: ssb-engine DSP bit-exact on the ME at SC speed, SC slowdown < 2% (RE-479).
- Rejected (batch 155): cross-fighter staged draw, r18 -0.16 ms only (RE-585 item 50).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Electric `vs4@2170` (r12 16.55, r18 15.65 ms; target 16.7 met; budget headroom next). Media Engine sleep/wake and PSP-1000 need a person (PLAN "Audio cost").
2. ROM-free release (PLAN, RE-588): host pack 4.3 s with the 37 compensated textures (PSP estimate 8-11.5 min, limit 10): break down the remaining time; then portable packer, the other tables into the pack, installer.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 172 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

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
