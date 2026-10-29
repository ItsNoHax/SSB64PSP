# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v137 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 267 of 267 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy, ms0) | opening 1-39 9.05 ms (Room 12.80, Run 13.35, Yoster 11.69-11.73, Clash 10.98-11.00 at batch 82) | RE-578, RE-579 |
| PSP-2000 `vs4@2400` | 17.03 ms busy (Zebes 11.38) | RE-579 |
| PSP-2000 1P intro | 18.01 ms; 1P battle 16.36 ms | RE-579 |
| PSP-2000 free at `psp_main` | 51.4 MB under PSPLink | RE-574 |

## Current work

- Landed (batch 85): Master Hand TopN yaw turns with `invert_lr`, Move and Wait turns; 1P intro fighter cameras and the Mario Bros. Luigi clear depth; new golden `f1-onep-intro-mario`; low_free unchanged (RE-581).
- Landed (batch 83): memcpy/hash cuts, bit-exact `Mat4::mul` affine path, order re-searched. Slim ms0: vs4 17.62 to 17.03, 1P intro 18.44 to 18.01, battle 16.79 to 16.36, Zebes 11.94 to 11.38, opening 9.17 to 9.05 (RE-579). Batch 82: mat_anim range read, `safe_point` skip (RE-578).
- Landed (batch 80): frozen-model vertex replay in the 1P intro: 38.3 to 23.0 ms busy on the Slim; vs4, 1P battle, opening unchanged; low_free -6,656 B code size; goldens 266/266 (RE-576).
- Batch 77: results, menus, boss scene built in place; deepest stack 126,888 B; peak 14,615,040 B.
- Batch 75: battle allocations 0 after load (vs4@2400 except 2 FxPool grows; vsmagnify 0); eager battle slabs (46 training rows -76,800 B, f1-onep-zako-clear/-ko-flash -471,552 B); OOM reaches the fatal screen (ballast 14.5 MB: 23,124 B @4); rust-psp 6c3f2c33; peak 14,615,040 B.
- Batch 74: `golden.sh verify --ballast BYTES`; goldens 266/266 at 18.0 and 17.5 MB, min headroom 3.58 MB (f1-training-fighter-select); failures begin at 15.0 MB (RE-572). PPSSPP-side PSP-1000 acceptance passed.
- Open: cutout over a later surface (RE-424); NOFOG under demo fade; real N64 VS menu boot; Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). Gaps in `PLAN.md`.

## Next

1. MS8 vs4 to 16.7 ms (batch 84).
2. Playtest performance: boot time, electric hit drop, Master Hand stage.

PSP-1000: owner confirmed the game runs on one (2026-10-29 playtest).

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 83 (hardware: RE-579); deepest game stack 126,840 B of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix 267 of 267, verify --twice |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
