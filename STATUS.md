# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v149 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 7.58 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 14.84 ms busy, Zebes 10.52 (batch 197) | RE-519, RE-585 |
| PSP-2000 1P intro | 14.01 ms (timer build); 1P battle 12.68 ms; `vs4@2170` electric no-hit r18 15.78, shock r12 16.65 (batch 187) | RE-479, RE-585, RE-589 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 197): clip path diet, bit-exact: branch-free corner encode, format resolved once per primitive. PSP-2000 busy vs4 14.90 to 14.84 ms, Zebes 10.70 to 10.52; Zebes clip walk 0.86 to 0.66 ms (RE-519).
- Landed (batch 194): all scene draws clip at the viewport (`FRUSTRATIO_1`); flat triangles crossing only x/y draw whole. PSP-2000 busy vs4 15.11 to 15.84 ms, Zebes 10.81 to 11.29, opening 9.60 to 9.46; 172 goldens rebaselined with hardware proof (RE-519).
- Landed (batch 193): fight first frame `update` 19-22 to 16-20 ms on PSP-2000 (figatree groups cached at boot); `enter` waits only for reads it wants (RE-476, RE-501). GE shade screen-linear (RE-519).
- Landed (batch 192): opening fight first frame `update` 24-28 to 19-22 ms on PSP-2000: one-field pack scans, in-place clears; vs4 15.50 to 15.45 ms; lazy item players rejected, vs4 +0.25 ms (RE-476; goldens unchanged).
- Landed (batch 191): pack stores texture level 0 only, the GE never samples mips (pack -2.85 MB, v149 unchanged, goldens unchanged, RE-491); training-select clip reads already hidden by the 1P menu read-ahead (RE-501).
- Measured (batch 157): Media Engine probe: ssb-engine DSP bit-exact on the ME at SC speed, SC slowdown < 2% (RE-479).
- Gaps in `PLAN.md`.

## Next

1. Owner hardware tests (XMB hand-off done): HOME mid-build, NoSpace screen, PSP-1000 run (RE-588 item 15 pending list).
2. PSP-1000 Media Engine audio needs a person (PLAN "Audio cost").

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.

## Verification

Baseline: batch 187 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

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
