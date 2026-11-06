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

- Landed (batch 175): pack build peak 198 to 37.5 MiB live heap (streaming sink, compressed blob, lazy file LRU), pack byte-identical; host 3.2 s, PSP estimate 6.5-9.3 min (RE-588 item 11).
- Landed (batch 174): pack build moved into the portable `no_std` crate `ssb-pack`; `romtool pack` wraps it; pack byte-identical; host peak 145 MB vs 51.4 MB free on the PSP (RE-588 item 10).
- Landed (batch 173): exact pack-build cuts (plain-key lookup, coverage bit set and radix sort, palette memo): host `romtool pack` 4.3 to 2.6 s, pack byte-identical; PSP estimate 5.8-7.9 min (RE-588 item 9).
- Landed (batch 172): texture compensation only for the owner's 37 textures; coverage and fits skipped for the rest; host `romtool pack` 122 to 4.3 s; 55 goldens rebaselined (owner-approved; RE-588 item 8).
- Landed (batch 171): `ssb-engine` sine tables read from the pack's new `TABLES` section (`ssb-tables`, fixed `.bss` store on the PSP; pack v142): r12 +0.16 (16.55, under 16.7), battle +0.17 ms; host tests build the section from the ROM (RE-589).
- Measured (batch 157): Media Engine probe: ssb-engine DSP bit-exact on the ME at SC speed, SC slowdown < 2% (RE-479).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Electric `vs4@2170` (r12 16.55, r18 15.65 ms; target 16.7 met; budget headroom next). Media Engine sleep/wake and PSP-1000 need a person (PLAN "Audio cost").
2. ROM-free release (PLAN, RE-588): portable `ssb-pack` fits PSP memory (37.5 MiB peak); PSP estimate 6.5-9.3 min (limit 10); next measure the build on the PSP; then the other tables into the pack, installer.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 175 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

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
