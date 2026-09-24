# Visual Regression

Deterministic golden screenshots of the PSP renderer, captured with
PPSSPPHeadless's software GPU ([D-041](../decisions/D-041.md)). Goldens live in
`tests/golden/`.

A golden pins current PSP output. It is not proof of original-N64 equivalence
or of physical-PSP behavior.

## Setup

```bash
cd ~/.local/src/ppsspp
cmake -DHEADLESS=ON -DCMAKE_BUILD_TYPE=Release -B build-headless
cmake --build build-headless --target PPSSPPHeadless
```

Set `PPSSPP_HEADLESS_BIN` if the binary is elsewhere. `ffmpeg` is required.

## Capture and compare

```bash
tools/run-ppsspp-headless.sh [--crate psp-game] --feature <feature>
tools/compare-screenshot.sh tests/golden/<golden>.png ~/ppsspp-headless-test/screenshot.png
```

- The runner builds the EBOOT with `<feature>,headless_capture`, stages it
  and the pack under PPSSPP's memstick (required for `MEMSIZE`), and writes a
  960×544 PNG to `$PPSSPP_HEADLESS_TEST_DIR` (default `~/ppsspp-headless-test`).
- Capture features freeze all simulation and animation at a fixed tick and
  hide the debug HUD, so the capture is independent of wall-clock timing.
- `compare-screenshot.sh` defaults to an exact match (0 differing pixels).
- `tools/verify-fighter-goldens.sh` runs every fighter scene.
- Capture features must not ship in interactive builds; rebuild without them
  afterwards.

## Rules

- Explain every golden change (pixel count, cause) before accepting it.
- Confirm a new golden is deterministic with two captures.
- PPSSPP software is the only exact tier. PPSSPP hardware backends and
  physical PSP captures are separate, qualitative tiers.
- Never pass a camera photo of a PSP screen to the pixel comparator.

## Scenes

`psp-asset-viewer` unless noted.

| Feature | Golden | Covers | Evidence |
|---|---|---|---|
| `regression_capture` | `r0-dream-land-default` | Dream Land stage view, Mario at spawn 0: textured geometry, CI4 + CLUT, mirror wrap, depth, culling, lighting | RE-123, RE-125 |
| `regression_capture_scene2` | `r1-mvopeningroom` | File 52 opening-movie graph: CI8, untextured vertex colour | RE-198, RE-199 |
| `regression_capture_scene3` | `r1-stage-sector` | File 109 graph `0x44C8`: texture blend, translucency, clamp | RE-200 |
| `regression_capture_scene4` | `r1-catch-swirl-flat-color` | File 84 graph `0x2760`: flat constant colour | RE-200 |
| `regression_capture_scene5` | `r2-saffron-city-gate` | Stage 9, animated gate | RE-205 |
| `regression_capture_scene6` / `scene7` | `r2-metal-texgen` / `-rotated` | File 117 graph `0x1B10`, ordinary texgen at two object rotations | RE-214 |
| `regression_capture_scene8` | `r2-metal-texgen-linear` | File 117 graph `0x2EE0`, linear texgen (crystal cluster) | RE-215, RE-259 |
| `regression_capture_scene9` | `r2-metal-texgen-camera-rotated` | Ordinary texgen under a rotated camera basis | RE-237 |
| `regression_capture_scene10` | `r2-peach-castle` | Stage 4, two animated joint layers | RE-286 |
| `regression_capture_stage_index` | `r2-stage-<name>` (38) | Whole-stage view; stage chosen at build time by `SSB64_STAGE_INDEX` (1–3, 5–8, 10–40) | RE-286, RE-312 |
| `depth_mask_diagnostic` | `r2-depth-mask-diagnostic` | Synthetic quads: depth write ON → OFF → ON | RE-251 |
| `regression_capture_<fighter>` | `r2-<fighter>-fighter` | High-detail model in `Wait` pose with stage fighter light; all 12 playable fighters | RE-265 |
| `regression_capture_link_costume_1` | `r2-link-costume-1` | Non-zero costume palette, colour and light | RE-301 |
| `regression_capture_metal_mario` | `r2-metal-mario-fighter` | Texgen on a posed fighter; compensated body texture | RE-311 |
| `regression_capture_mario_entry` | `r2-mario-entry-pipe` | File 356 graph `0x608`, dense compensation variant | RE-312 |
| `regression_capture_bonus_platform` | `r2-bonus-platform-small` | File 136 graph `0x3DA8`, UV phase variant | RE-312 |
| `regression_capture_fireball` (`psp-game`) | `f1-training-fireball` | Translucent Fireball weapon in Training | RE-300 |
| `regression_capture_shadows` (`psp-game`) | `f1-training-shadows` | Grounded and airborne fighter shadows | RE-302 |

Stage sweep example:

```bash
SSB64_STAGE_INDEX=5 tools/run-ppsspp-headless.sh --feature regression_capture_stage_index
```

Not covered by a golden: dynamic particles (device audits RE-180–189) and UI
(not implemented).

### Known failing goldens

These eight already differed before RE-313 and are identical between the
RE-313 and prior packs. They have not been re-investigated or rebaselined:

`f1-training-fireball`, `f1-training-shadows`, `r1-mvopeningroom`,
`r2-depth-mask-diagnostic`, `r2-metal-texgen`, `r2-metal-texgen-rotated`,
`r2-metal-texgen-linear`, `r2-metal-texgen-camera-rotated`.

The 2026-09-24 stage rebaseline (42 goldens) pinned content changes from
RE-300 to RE-311 without bisecting them, for example Zebes' walls and Kongo
Jungle's backdrop. Those goldens record current output, not verified
correctness.

## Audits

Broad PPSSPP smoke audits with the windowed runner. They check that every
item renders, not exact pixels. Output stays outside Git under
`$PPSSPP_TEST_DIR`.

| Command | Scope | Evidence |
|---|---|---|
| `tools/run-ppsspp.sh --no-build --seconds 3 --audit-stages 41` | All 41 stages | RE-170 |
| `tools/run-ppsspp.sh --seconds 2 --audit-animations 532` | All 532 fighter animations | RE-171 |
| `--audit-effects`, `--audit-effect-animations`, `--audit-effect-materials`, `--audit-particles` | Effects and particles | RE-173–189 |

## Physical PSP

1. Mount the PSP over USB and run
   `tools/stage-psp-regression.sh /path/to/psp-mount [rom]`. It rebuilds the
   pack, builds the capture EBOOT, installs both under `PSP/GAME/ssb64/` and
   writes `regression-manifest.txt` (commit, build mode, hashes).
2. Launch, wait past the freeze tick, then capture (PSPLink `scrshot` or a
   square-on photo with locked focus and exposure).
3. Record model, firmware, commit, manifest hashes, capture method and
   observations.

See the `psp-hardware` skill for PSPLink. Existing hardware captures (PSP
Slim, 6.61) predate pack v31 and several golden refreshes.

## Original game

RE-151 drives the original ROM through Mupen64Plus with frame-indexed inputs
and compares camera state numerically from RDRAM. See the `n64-emulator`
skill.
