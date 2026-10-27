# Status

- Scope: current state only. Tasks: [`PLAN.md`](PLAN.md#remaining-work). History: git.

## Current figures

| Item | Value | Refs |
|---|---|---|
| Pack | v136 | `crates/ssb-rom/src/pack.rs` |
| Goldens | 251 of 251 goldens match by pixel hash | `tests/golden/hashes.tsv` |
| Toolchains | Host Rust 1.98.0; PSP crates pinned nightly | [D-026](docs/decisions/D-026.md) |
| PPSSPP | 60 FPS | RE-469–RE-471 |
| PSP-2000 opening (busy) | Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17 | RE-530, RE-532, RE-539 |
| PSP-2000 `vs4@2400` | 16.92 ms busy, BFPS 59.1 | RE-539 |
| PSP-2000 1P intro | ~34 ms (~21 FPS) | RE-537, RE-539 |

## Current work

- Landed (batch 56): Master Hand defeat draws one RNG per damage digit before the dead status (seeds match N64 t1..2491); boss wallpaper clips at the viewport like the game (tunnel MAE 6.6; `f1-onep-boss-defeat-zoom` rebaselined, MAE vs N64 26.9 to 17.9); RE-321 tile origins may differ (pack v136); Fall still opaque (RE-457, RE-519, RE-321).
- Landed (batch 55): Master Hand defeat matches N64 (owner handicap, Attack12 same-frame interrupt, defeat zoom tick, wallpaper proc render modes seeded in the pack, head-0 effects under head-1); items start with the make-time frame-0 parse (`StageAnimator::start_item`; Link Bomb sparkle phase now N64's, 4 goldens rebaselined); cpufx matches N64; 2 new goldens (RE-457, RE-444, RE-536).
- Landed (batch 54): capture-only scenes `TrainingLaser`, `TrainingReflect` force the Sector Z Arwing pattern; N64 ParaLLEl matches laser and Fox reflect (timing, damage, knockback; IoU laser 0.81-0.85, hexagon 0.84-0.90); no port bug; warp offset +7 sector.z64, +3 foxsector.z64; 2 new goldens (RE-428).
- Landed (batch 53, docs only): N64 ParaLLEl confirms title fire (random colour id; MAE 5.7 forced id 4) and opening room (pose, overlay, cone) (RE-529, RE-467).
- Landed (batch 52): weapon graphics parse frame 0 on their first counted play (`GraphicPlayers`; Spin band, Boomerang, Final Cutter wave, PK Thunder head); `MAX_EFFECT_MAT_ANIMS` 9 (Spin band has 9 scripts); N64 confirmed Zebes offset (PSP T = t + 8) and wipe (Camera Shutter, timing exact); 6 goldens rebaselined (RE-540, RE-364, RE-464).
- Landed (batch 51): Thunder Jolt burst area gap was RE-541's whole-image normalisation (2.3125 x 2.267 vs D-047 2.4727); re-measured within spread, no renderer change (RE-541).
- Open: movie camera kinds 3+8 basis; NOFOG under demo fade; 1P intro Metal Mario darkness. Mew N64 read (costly, RE-568). Countdown RNG seed vs a real N64 VS boot. Magnifier and FlyRoll re-takes (RE-561); `proc_physics`-set status with a pending accessory maker (RE-555). PK Fire flame route (RE-352); PK Fire item frame-0 rule N64 check. Gaps in `PLAN.md`.

## Next

1. Further playtest reports; open gameplay rows in `PLAN.md`; then MS8.

## Blockers

- Campaign past Race to the Finish: needs a person at the PSP.
- Builds without `profile`/`frame_timer`: unattended PSP auto-sleeps (RE-481).

## Verification

Baseline: batch 56; deepest game stack 395256 B (f1-bonus-pause) of 524288.

| Check | Result |
|---|---|
| `cargo test --workspace` with `SSB64_ROM` | pass |
| `cargo test --workspace`, `SSB64_STUB_TABLES=1`, no ROM | pass |
| `cargo +1.98.0 clippy -D warnings`, `cargo fmt --all --check` | pass |
| Both PSP release builds | compile |
| Goldens | full matrix verified (251 of 251) |
| PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` | exit 0, screenshot |
| Physical PSP | PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B RE-530, RE-532; `stack_poison` RE-531 |
| ROMs | `rom/Super Smash Bros. (USA).z64`, `refs/ssb-decomp-re/baserom.us.z64`; SHA-1 `e2929e10...` |
