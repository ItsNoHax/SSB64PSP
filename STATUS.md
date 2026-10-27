# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v134 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 247 of 247 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed (batch 52): weapon graphics parse frame 0 on their first counted play (`GraphicPlayers`; Spin band, Boomerang, Final Cutter wave, PK Thunder head); `MAX_EFFECT_MAT_ANIMS` 9 (Spin band has 9 scripts); N64 confirmed Zebes offset (PSP T = t + 8) and wipe (Camera Shutter, timing exact); 6 goldens rebaselined (RE-540, RE-364, RE-464).
- Landed (batch 51): Thunder Jolt burst area gap was RE-541's whole-image normalisation (2.3125 x 2.267 vs D-047 2.4727); re-measured within spread, no renderer change (RE-541).
- Landed (batch 50): stage matanim clock restarts per ground creation and plays the creation-time `gcPlayAnimAll` once (`MaterialAnimator::start_stage`); 89 goldens rebaselined, pond water and Zebes acid only (RE-364, RE-321).
- Landed (batch 49): guard-band CPU clip for battle stages (Zebes acid, Yoshi cloud hill; hardware-proven, +1.21 ms vs4, +1.75 ms Zebes); stage-select preview uses `dMNMapsLights1`; 105 goldens rebaselined (RE-519, RE-570, RE-364).
- Landed (batch 48): battle and asset viewer use `set_scene_light`: lit stage/effect prims whose lists write both light colours shade under the stage direction with those colours (N64 RDRAM trace Meta Crystal, N64 framebuffer Board the Platforms beam 226 vs PSP 180 -> 221); Hyrule darker floor was a misaligned region script, not a gap; 31 goldens rebaselined (RE-065, RE-529, RE-570).
- Landed (batch 47): Zebes acid two-tile blend lowered with premultiplied shade alpha (worst error 3 steps, held +698,624 B, pack v134); Spin Attack band drawn with `wpDisplayDrawNormal` state (no Z, XLU); `f1-training-link-spin` 172 px, `f1-training-zebes` 22 px rebaselined (RE-321, RE-364, RE-540).
- Open: movie camera kinds 3+8 basis; NOFOG under demo fade; 1P intro Metal Mario darkness. Mew N64 read (costly, RE-568). Countdown RNG seed vs a real N64 VS boot. Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). PK Fire flame route (RE-352); Bomb fuse sparkle (RE-444). Gaps in `PLAN.md`.

## Next

1. Further playtest reports; open gameplay rows in `PLAN.md`; then MS8.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 52; deepest game stack 395032 B (f1-bonus-pause) of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix verified (247 of 247) |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
