# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md#remaining-work); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v117 |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | N64 audio system ported whole, sample-exact against N64 captures ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md)); fighter shadow is one mirrored ellipse with team colours (RE-302); CPU guard-band clipping for movie scenes (RE-519); shade-only combiners whitened (RE-520); prim-alpha untextured surfaces (RE-521); fighter entries match the N64: effects play at make time, Samus's lit glass tube, Yoshi hidden in the egg, focus-2 camera fallback (RE-522, RE-523); opening fighter scenes: movie-camera particles and billboards, Samus grapple glow, Yoshi cloud fade (RE-525); run dust from demo-script events (RE-528); two-cycle shade x texel-lerp baked into the texture, so Yoster clouds are tinted and soft (RE-527) |
| Frontend | N64 logo, opening, title (fire fade matches Rice within MAE 2-6), attract loop, every menu; opening room now matches the N64 closely (MAE vs Rice 5-11 at most tics) |
| PSP | 60 FPS under PPSSPP; PSP-2000 baselines in RE-497, RE-498, RE-479, RE-493. Frame-time cost of the guard clip is unmeasured (MS8). PSP-1000 unsupported |
| Tests | Workspace tests with the ROM, stub tests, clippy, rustfmt pass; 218 of 218 goldens match by pixel hash |

## Current Work

Closed: intro batch 4 (title fire fade via prim-alpha sprites, logo alpha, `op-title` capture scene). Next:

1. Intro batch 5: the title slash line, stage floor contrast (F3), sector details, then physical-PSP validation. The grade (R4) waits for an accurate N64 reference. Follow-ups are in `PLAN.md`.

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink restarts need a person at the PSP; builds without `profile` or `frame_timer` let an unattended PSP auto-sleep (RE-481).

## Verification

Baseline after intro batch 4:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`, and with `SSB64_STUB_TABLES=1` and no ROM: pass. Clippy (`cargo +1.98.0`, `-D warnings`) and `cargo fmt --all --check` pass.
- Both PSP release builds compile.
- Goldens: 218 of 218 after adding `f1-opening-title-sparks`, `-fire`, `-logo`; no existing row changed.
- PPSSPP smoke: `tools/run-ppsspp.sh --no-build --crate psp-game --seconds 20` exits 0 with a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; the `ms0:` pack there is v108-sized.
- ROMs: `rom/Super Smash Bros. (USA).z64` and `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
