# Status

Where the project is now. The roadmap and every open task are in
[`PLAN.md`](PLAN.md#remaining-work); history is in git.

## Current

| Area | State |
|---|---|
| Build | Tables read from the ROM are generated at build time ([D-048](docs/decisions/D-048.md)); host workspace on Rust 1.98.0; `psp-game` and `psp-asset-viewer` on the pinned nightly; pack v121 |
| Gameplay | All fighters, VS, Training, 1P campaign through the ending; How to Play matches the N64 on every frame |
| Audio | N64 audio system ported whole, sample-exact against N64 captures ([D-049](docs/decisions/D-049.md), RE-479) |
| Rendering | CRT overscan crop ([D-047](docs/decisions/D-047.md)); fighter shadow is one mirrored ellipse with team colours (RE-302); CPU guard-band clipping for movie scenes (RE-519); shade-only combiners whitened (RE-520); prim-alpha untextured surfaces (RE-521); fighter entries match the N64: effects play at make time, Samus's lit glass tube, Yoshi hidden in the egg, focus-2 camera fallback (RE-522, RE-523); opening fighter scenes: movie-camera particles and billboards, Samus grapple glow, Yoshi cloud fade (RE-525); run dust from demo-script events (RE-528); two-cycle shade x texel-lerp baked into the texture, so Yoster clouds are tinted and soft (RE-527) |
| Frontend | N64 logo, opening, title (fire fade matches Rice within MAE 2-6); opening sector and Yoster models lit by display-list lights (RE-529), attract loop, every menu; opening room now matches the N64 closely (MAE vs Rice 5-11 at most tics) |
| PSP | 60 FPS under PPSSPP; PSP-2000 baselines in RE-497, RE-498, RE-479. Opening on a PSP-2000 (RE-530, RE-532): room 13.4 ms busy at 58.8 FPS, Run 54 FPS, Yoster 51, Clash 51; `vs4@2400` 16.9 ms busy, BFPS 59.1 (RE-539). PSP-1000 unsupported |
| Tests | Workspace tests with the ROM, stub tests, clippy, rustfmt pass; 225 of 225 goldens match by pixel hash |

## Current Work

Landed (RE-539, MS8): VRAM lit-expand vertex blocks, a photo-region texture pool, the 1P stage-override fix and link order s7b. On a PSP-2000 output is bit-identical (14,280 frames per arm; the two viewer rows by screenshot). Opening Room 13.39 ms, Run 14.98, Yoster 15.58, Clash 13.17; `vs4@2400` 16.92 ms (BFPS 59.1); 1P intro ~34 ms (~21 FPS), 1P battle 59.7 FPS. 16 PPSSPP goldens rebaselined (D-051, 707 native pixels). RE-537's 1P numbers were default-stage numbers.

Earlier: RE-547 (D-048 part 4) generates the small `d*` arrays from the ROM.

Open user question: function-local arrays inside decompiled functions are still committed; should they be generated too?

Next: D-048 leftovers in `PLAN.md` (function-local arrays answer, check 10 blind spots, sprite file-ID rows), Kirby's roster-A2 entry camera, Thunder Jolt burst spikes, effects process order, then the MS8 list in `PLAN.md` (1P intro at ~21 FPS first).

## Blockers

- The campaign past Race to the Finish needs a person at the PSP.
- Builds without `profile` or `frame_timer` let an unattended PSP auto-sleep (RE-481).

## Verification

Baseline after RE-539:

- `cargo test --workspace -- --test-threads=1` with `SSB64_ROM`, and with `SSB64_STUB_TABLES=1` and no ROM: pass. Clippy (`cargo +1.98.0`, `-D warnings`) and `cargo fmt --all --check` pass.
- Both PSP release builds compile.
- Goldens: 225 of 225 match after the 16-row rebaseline (r2-mario-entry-pipe is nondeterministic, RE-506).
- PPSSPP smoke: `tools/run-ppsspp.sh --crate psp-game --seconds 20` exits 0 with a screenshot.
- Physical PSP: PSP-2000, 6.61 ARK, PSPLink v3.2.1; opening A/B in RE-530 and RE-532; `stack_poison` sweep in RE-531.
- ROMs: `rom/Super Smash Bros. (USA).z64` and `refs/ssb-decomp-re/baserom.us.z64`, SHA-1 `e2929e10...`.
