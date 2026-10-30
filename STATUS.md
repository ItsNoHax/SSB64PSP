# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v140 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 275 of 275 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 9.08 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579 |
| PSP-2000 `vs4@2400` | 16.55 ms busy (Zebes 11.10) | RE-585 |
| PSP-2000 1P intro | 14.81 ms (timer build); 1P battle 15.56 ms; vs4 16.62; opening 8.89; Fall 19.44 | RE-585 |
| PSP-2000 boot | `psp_main` to frame 1 about 417 ms | RE-582 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 118): AObj track order closed: no ROM node keys an interp track beside its axis tracks (RE-508).
- Landed (batch 117): objanim/matanim linear rate divided (objanim.c:397, 966), payload 0 keeps the rate, SetTargetRate rate-only, step zeroes `rate_target`, `length_invert` cubic-only; pack v140 (camera/title bakes 1 ulp); goldens unchanged (RE-508).
- Landed (batch 116): ground ambient effects (`efground`): slot held until the bounds eject, cycle ends from a generated table, drawn in psp-game; Sector and Castle seeds and effect root N64-equal ticks 69-700; new golden `f1-vs-ground`; hidden-part order history closed (RE-474, RE-508).
- Landed (batch 115): status end decided by the highest bound scripted joint (pre-order with hidden parts, tablegen `END_BLOCKS`, 25 motions); Thunder trail copied rehit timers count down on its make frame; VS and cpufx traces N64-equal every tick (cpufx-s 430 was an N64 snapshot artifact); goldens unchanged (RE-474, RE-508).
- Landed (batch 114): spawn floor on moved map groups; status end by the last scripted joint's f32 countdown (fractional speeds); Kirby Stone end; Thunder trail first update; VS and cpufx traces N64-equal every tick but cpufx-s tick 430; 6 goldens rebaselined (RE-474, RE-508).
- Landed (batch 99): frozen 1P-intro models compose once; order s4; Slim intro 17.35 to 14.81 ms; new golden `f1-onep-intro-zako` (ballast low_free 6,801,920 B, equal to base; RE-585).
- Landed (batch 98): display effects skip the empty split head (shock 19.08 to 18.25 ms with order s3 re-searched after 5d1bf2ae shifted the layout; RE-584, RE-585).
- Open: real N64 VS menu boot. Gaps in `PLAN.md`.

## Next

1. Playtest performance: electric hit (17.48 no hit, 18.23 shock) and Master Hand windows still above 16.7 ms.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 118 (hardware: RE-585); deepest game stack 127,752 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix 274 of 274, verify --twice |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
