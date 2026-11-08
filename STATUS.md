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
| PSP-2000 `vs4@2400` | 14.43 ms busy (Zebes 10.52 at batch 180; batch 181) | RE-585, RE-589 |
| PSP-2000 1P intro | 14.01 ms (timer build); 1P battle 12.68 ms; `vs4@2170` electric no-hit r18 15.78, shock r12 16.65 (batch 187) | RE-479, RE-585, RE-589 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 190): sleep with ME audio: `k_me_reset` unregisters the system's `SceMeRpc` suspend handler, whose save handshake our ME program never answered; owner: sleeps, sound after wake (RE-479).
- Landed (batch 189): static-CLUT textures keep the ROM's indices (`matcolors` 0 texel diffs, RE-364; same pack size, goldens unchanged); viewer 512 KiB stack ends its PSP kill, GE HUD shown on hardware (RE-202); D-029 text current.
- Landed (batch 188): vs-four RE-511 change attributed (release z plus TopN display); Yoshi egg holds a forced effect struct, no-room break 16 frames (RE-552); real-menu VS boot seed matches the N64, idle title demo at tic 1190 (RE-568); stage material scripts packed as per-script windows (pack v149, -1.23 MB pack and resident, RE-010); release builds tick `scePowerTick` about once a second (`timing::keep_awake`; PSP vs4 14.35 to 14.35 ms, 1P intro 13.99 to 14.00); viewer HUD drawn by the GE (`gu/hud_text.rs`), `sceGuDebugFlush` gone; shows in PPSSPP OpenGL.
- Landed (batch 187): Electric `vs4@2170` r12 16.75 to 16.65 ms (under 16.7): the 0.33 ms pack gap was `BIND_CACHE` slot collisions moving with texture ids; 64 victim slots; intro +0.31 ms from hot-caller codegen, other scenes ±0.05 (RE-589 item 10).
- Landed (batch 186): first-launch installer: separate `INSTALL.PBP`, started from the game by LoadExec; PSP-2000 at the PSP-1000 start budget (22.4 MB) 8.78 min, pack identical, lowest free 9.78 MB; 64-byte-aligned sink (writes 129 to 16.5 s) (D-053, RE-588 item 15).
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
