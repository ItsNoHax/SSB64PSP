# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v144 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 286 of 286 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 7.58 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579, RE-585 |
| PSP-2000 `vs4@2400` | 14.30 ms busy (Zebes 10.43; batch 177) | RE-585, RE-589 |
| PSP-2000 1P intro | 13.62 ms (timer build); 1P battle 12.75 ms; `vs4@2170` electric no-hit r18 15.91, shock r12 16.81 (batch 177) | RE-479, RE-585, RE-589 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 178): tables batch B: 24 cold `ssb-game` tables (HUD, menus, 1P, staff roll, opening) from the pack (v144, 188 KB section). Electric r12 regression bisected to pack content of batch 172, not code; cause below texture level (RE-589 item 5).
- Landed (batch 177): tables batch A: 469 `d_arrays` and 16 `ssb-rom` tables read from the pack `TABLES` section (176 KB section); PSP frame time unchanged or faster (r12 -0.15 ms; RE-589 item 4).
- Measured (batch 176): `ssb_pack::build` on the PSP-2000 (probe feature `pack_probe`): 5.93 min from ms0 ROM to ms0 pack, byte-identical to the host pack; meshes 4.5 min; lowest free 11.3 MB (RE-588 item 12).
- Landed (batch 175): pack build peak 198 to 37.5 MiB live heap (streaming sink, compressed blob, lazy file LRU), pack byte-identical; host 3.2 s, PSP estimate 6.5-9.3 min (RE-588 item 11).
- Landed (batch 174): pack build moved into the portable `no_std` crate `ssb-pack`; `romtool pack` wraps it; pack byte-identical; host peak 145 MB vs 51.4 MB free on the PSP (RE-588 item 10).
- Landed (batch 172): texture compensation only for the owner's 37 textures; coverage and fits skipped for the rest; host `romtool pack` 122 to 4.3 s; 55 goldens rebaselined (owner-approved; RE-588 item 8).
- Measured (batch 157): Media Engine probe: ssb-engine DSP bit-exact on the ME at SC speed, SC slowdown < 2% (RE-479).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Electric `vs4@2170` r12 16.81 ms, over the 16.7 target: 0.33 ms from batch 172 pack content, cause not found (RE-589 item 5). Media Engine sleep/wake and PSP-1000 need a person (PLAN "Audio cost").
2. ROM-free release (PLAN, RE-588): on-PSP build measured 5.93 min (limit 10); next tables batches C-F (warm, hot, motion scripts, cleanup), then the installer.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 178 (hardware: RE-575, RE-585); deepest game stack 127,752 B of 524288.

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
