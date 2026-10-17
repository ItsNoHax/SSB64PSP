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
| PSP | 60 FPS under PPSSPP; PSP-2000 baselines in RE-497, RE-498, RE-479, RE-493. Opening on a PSP-2000 (RE-530, RE-532): room 12.9 ms busy at 59.5 FPS, Run 40.5 FPS, Yoster 38, Clash 35; `vs4@2400` 16.24 ms busy, 58.6 FPS (120-frame reports; RE-535). PSP-1000 unsupported |
| Tests | Workspace tests with the ROM, stub tests, clippy, rustfmt pass; 222 of 222 goldens match by pixel hash |

## Current Work

Closed: CPU special effects (RE-536). Every port draws Magnet, Sing, Spin Attack, Falcon Punch/Kick, the Fox reflector (four tables), Charge Shot, the held egg and per-weapon visuals; Kirby's copied Charge Shot and Falcon Punch flame; Charge Shot spin starts at 1 (N64 trace); Link's entry Wave and Beam eject at frame 0 of their animation. `MAX_WEAPONS` 32; pack v118; peak memory 20.43 MB. Goldens: `f1-training-samus` rebaselined, four `f1-cpufx-*` rows added (222).

Next: stall batching, approved. Conditions: capture the 7 changed PPSSPP VS rows repeatedly (hashes must be stable, else drop); psp-runtime writes the draw words itself (like the RE-534 bind cache), no rust-psp fork; record a new D-NNN that PPSSPP goldens move only when a hardware framebuffer comparison proves identical output, hashes are repeat-stable and the changed rows are listed; first widen the hardware comparison to the opening and 1P scenes (`PLAN.md` MS8, RE-535).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- PSPLink restarts need a person at the PSP; builds without `profile` or `frame_timer` let an unattended PSP auto-sleep (RE-481).

## Verification

Baseline after RE-536:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`, and with `SSB64_STUB_TABLES=1` and no ROM: pass. Clippy (`cargo +1.98.0`, `-D warnings`) and `cargo fmt --all --check` pass.
- Both PSP release builds compile.
- Goldens: 222 of 222 match after the rebaseline.
- PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` exits 0 with a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B in RE-530 and RE-532; `stack_poison` sweep in RE-531.
- ROMs: `rom/Super Smash Bros. (USA).z64` and `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
