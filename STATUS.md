# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md#remaining-work); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v118 |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | N64 audio system ported whole, sample-exact against N64 captures ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md)); fighter shadow is one mirrored ellipse with team colours (RE-302); CPU guard-band clipping for movie scenes (RE-519); shade-only combiners whitened (RE-520); prim-alpha untextured surfaces (RE-521); fighter entries match the N64: effects play at make time, Samus's lit glass tube, Yoshi hidden in the egg, focus-2 camera fallback (RE-522, RE-523); opening fighter scenes: movie-camera particles and billboards, Samus grapple glow, Yoshi cloud fade (RE-525); run dust from demo-script events (RE-528); two-cycle shade x texel-lerp baked into the texture, so Yoster clouds are tinted and soft (RE-527) |
| Frontend | N64 logo, opening, title (fire fade matches Rice within MAE 2-6); opening sector and Yoster models lit by display-list lights (RE-529), attract loop, every menu; opening room now matches the N64 closely (MAE vs Rice 5-11 at most tics) |
| PSP | 60 FPS under PPSSPP; PSP-2000 baselines in RE-497, RE-498, RE-479, RE-493. Opening on a PSP-2000 (RE-530, RE-532): room 12.4 ms busy at 59.5 FPS, Run 53 FPS, Yoster 50, Clash 52 (RE-537); `vs4@2400` 15.73 ms busy, 59.2 FPS (120-frame reports; RE-537). PSP-1000 unsupported |
| Tests | Workspace tests with the ROM, stub tests, clippy, rustfmt pass; 222 of 222 goldens match by pixel hash |

## Current Work

Closed: stall batching (RE-537, [D-051](docs/decisions/D-051.md)). `psp-runtime` writes the draw words itself, moves the GE stall address every 16 draws, carves list blocks from 8 KiB uncached chunks; link order re-searched (seed 2). PSP-2000: `vs4@2400` 17.61 to 15.73 ms busy (59.2 FPS), opening Run 53 FPS, Yoster 50; 26,040 hardware frames bit-identical. 44 PPSSPP goldens rebaselined (stable over 6 runs, listed in RE-537).

Next open `PLAN.md` item (MS8): the lit expand's memory traffic (~0.68 us a vertex), then the remaining opening CPU cost (`bind_texture`, 121 binds a frame at about 11 us).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink restarts need a person at the PSP; builds without `profile` or `frame_timer` let an unattended PSP auto-sleep (RE-481).

## Verification

Baseline after RE-537:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`, and with `SSB64_STUB_TABLES=1` and no ROM: pass. Clippy (`cargo +1.98.0`, `-D warnings`) and `cargo fmt --all --check` pass.
- Both PSP release builds compile.
- Goldens: 222 of 222 match after rebaselining 44 rows (D-051).
- PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` exits 0 with a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B in RE-530 and RE-532; `stack_poison` sweep in RE-531.
- ROMs: `rom/Super Smash Bros. (USA).z64` and `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
