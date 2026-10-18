# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md#remaining-work); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v119 |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | N64 audio system ported whole, sample-exact against N64 captures ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md)); fighter shadow is one mirrored ellipse with team colours (RE-302); CPU guard-band clipping for movie scenes (RE-519); shade-only combiners whitened (RE-520); prim-alpha untextured surfaces (RE-521); fighter entries match the N64: effects play at make time, Samus's lit glass tube, Yoshi hidden in the egg, focus-2 camera fallback (RE-522, RE-523); opening fighter scenes: movie-camera particles and billboards, Samus grapple glow, Yoshi cloud fade (RE-525); run dust from demo-script events (RE-528); two-cycle shade x texel-lerp baked into the texture, so Yoster clouds are tinted and soft (RE-527) |
| Frontend | N64 logo, opening, title (fire fade matches Rice within MAE 2-6); opening sector and Yoster models lit by display-list lights (RE-529), attract loop, every menu; opening room now matches the N64 closely (MAE vs Rice 5-11 at most tics) |
| PSP | 60 FPS under PPSSPP; PSP-2000 baselines in RE-497, RE-498, RE-479, RE-493. Opening on a PSP-2000 (RE-530, RE-532): room 12.4 ms busy at 59.5 FPS, Run 53 FPS, Yoster 50, Clash 52 (RE-537); `vs4@2400` 15.73 ms busy, 59.2 FPS (120-frame reports; RE-537). PSP-1000 unsupported |
| Tests | Workspace tests with the ROM, stub tests, clippy, rustfmt pass; 224 of 224 goldens match by pixel hash |

## Current Work

Landed: fx-order (RE-540): anim-joint effects play from frame 0 on the creation frame, and fighter link-15 effects draw in one creation order. fx-remaining (RE-541): Kirby Stone bank scripts, the ground Thunder Jolt burst and timing, and the other effect gaps; `f1-vs-four` was rebaselined because the Kirby Stone scripts draw from the shared RNG, and `f1-vs-four-magnifier` moved to `vs4@3000`. Pack v119 is unchanged.

Uncommitted in the working tree: the perf batch (VRAM lit-expand vertex blocks, photo texture pool, 1P stage-override fix, link order s7b). It is blocked on the hung PSPLink: D-051 hardware proof of r2-stage-final-destination and r2-stage-bonus2-captain-falcon, and the s7b link-order re-check.

Next: fx-remaining's open items 4 (link-15 merge), 8 (node 4 texture, pack 120) and 9 (process order), then the MS8 list in `PLAN.md`.

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink is hung and needs a person at the PSP; builds without `profile` or `frame_timer` let an unattended PSP auto-sleep (RE-481).

## Verification

Baseline after RE-541:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`, and with `SSB64_STUB_TABLES=1` and no ROM: pass. Clippy (`cargo +1.98.0`, `-D warnings`) and `cargo fmt --all --check` pass.
- Both PSP release builds compile.
- Goldens: 222 of 222 match (8 rows rebaselined by RE-538).
- PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` exits 0 with a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B in RE-530 and RE-532; `stack_poison` sweep in RE-531.
- ROMs: `rom/Super Smash Bros. (USA).z64` and `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
