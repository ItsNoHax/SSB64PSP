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
| Frontend | N64 logo, opening, title (fire fade matches Rice within MAE 2-6); opening sector and Yoster models lit by display-list lights (RE-529), attract loop, every menu; opening room now matches the N64 closely (MAE vs Rice 5-11 at most tics) |
| PSP | 60 FPS under PPSSPP; PSP-2000 baselines in RE-497, RE-498, RE-479, RE-493. Opening on a PSP-2000 (RE-530, RE-532): room 12.9 ms busy at 59.5 FPS, Run 40.5 FPS, Yoster 38, Clash 35; `vs4@2400` 16.24 ms busy, 58.6 FPS (120-frame reports; RE-535). PSP-1000 unsupported |
| Tests | Workspace tests with the ROM, stub tests, clippy, rustfmt pass; 218 of 218 goldens match by pixel hash |

## Current Work

Closed: VS GE batch 2 (RE-535): descriptor prelookup before the bind cache, a texture-mapping cache, link order seed 3, `room_time()` fixed under `regression_capture` so PSPLink opening runs repeat. `vs4@2400` 16.53 -> 16.24 ms busy (57.7 -> 58.6 FPS) on a PSP-2000; opening Run 49 -> 53 FPS; bit-identical (2402-frame RGBA, 218 of 218 goldens). XMB budget checked: lowest free 29.16 MB. Stall batching is bit-identical on hardware (-0.39 ms) but changes 7 PPSSPP VS goldens: needs a decision and a production route (`PLAN.md` MS8).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink restarts need a person at the PSP; builds without `profile` or `frame_timer` let an unattended PSP auto-sleep (RE-481).

## Verification

Baseline after RE-535:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`, and with `SSB64_STUB_TABLES=1` and no ROM: pass. Clippy (`cargo +1.98.0`, `-D warnings`) and `cargo fmt --all --check` pass.
- Both PSP release builds compile.
- Goldens: 218 of 218 unchanged.
- PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` exits 0 with a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B in RE-530 and RE-532; `stack_poison` sweep in RE-531.
- ROMs: `rom/Super Smash Bros. (USA).z64` and `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
