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

Set `PPSSPP_HEADLESS_BIN` if the binary is elsewhere. `ffmpeg` and
ImageMagick (`magick`) are required.

## Verify and rebaseline

```bash
tools/golden.sh verify [--filter REGEX] [-j N] [--twice] [--no-build]
tools/golden.sh rebaseline [--filter REGEX] [-j N] --reason TEXT
```

- [`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv) lists every
  golden: crate, scene spec, `pass` or `known-failing`, and evidence.
- The driver builds each crate once with `golden_capture`, then captures
  every selected scene from that EBOOT in parallel (default `nproc` jobs).
  A full run of all 68 scenes takes about 17 s (RE-316).
- Output goes to `target/golden-run/<timestamp>/`: `candidates/`, difference
  masks in `masks/`, `summary.tsv`, and `index.html`, a side-by-side review
  of golden, candidate and mask with changed scenes first.
- `verify` fails on a `pass` row that differs, a `known-failing` row that now
  matches (set it to `pass`), a failed capture, and with `--twice` on two
  captures of one scene that differ.
- `rebaseline` always captures twice. It copies only changed candidates over
  their goldens and prints a Markdown table (golden, pixel count, reason)
  for the evidence record. It skips `known-failing` rows unless `--filter`
  is given.
- `tools/verify-fighter-goldens.sh` runs `verify --filter
  'fighter|link-costume'`.
- `tools/golden-reference.sh` captures every scene with the per-feature
  pipeline (one build and one 8 s run each, about 15 minutes) into
  `~/golden-reference/`. Use it as the byte-identity reference when changing
  the capture pipeline itself.
- Serve `index.html` over HTTP from the repository root (for example
  `python3 -m http.server`); it loads goldens from `tests/golden/`.

### How a scene is chosen

A `golden_capture` EBOOT reads one line from `capture_scene.txt` beside it
(`stage 17`, `fighter fox`, `scene3`, `depth_mask`; format in
`crates/ssb-capture`). It freezes at tick 240, requests the screenshot, and
exits two frames later. Without the file it renders the Dream Land default.

The old per-scene features (`regression_capture_scene3`,
`regression_capture_fox`, `regression_capture_stage_index` with
`SSB64_STAGE_INDEX`, ...) still build. Each only sets a default scene; these
builds never read the file and keep running until the runner's timeout, as
before. Interactive and physical-PSP builds read no scene file.

### Single captures

```bash
tools/run-ppsspp-headless.sh --scene 'stage 17' [--job NAME] [--no-build]
tools/run-ppsspp-headless.sh [--crate psp-game] --feature <feature>
tools/compare-screenshot.sh tests/golden/<golden>.png ~/ppsspp-headless-test/screenshot.png
```

- The runner builds the EBOOT (`golden_capture` with `--scene`, otherwise
  `<feature>,headless_capture`), stages it and the pack under PPSSPP's
  memstick (required for `MEMSIZE`), and writes a 960×544 PNG to
  `$PPSSPP_HEADLESS_TEST_DIR` (default `~/ppsspp-headless-test`).
- With `--scene` the timeout defaults to 30 s and is a failure. PPSSPPHeadless
  exits 0 on a timeout too; the runner reads the `TIMEOUT` line in its log.
- `--job NAME` stages into `PSP/GAME/ssb64_regression_<NAME>` and writes to
  `$PPSSPP_HEADLESS_TEST_DIR/<NAME>`, so parallel runs never share a game
  directory or `no-status-overlay.ini`. PPSSPPHeadless keeps its memstick at
  `$HOME/.ppsspp` and does not save `ppsspp.ini`, so no other state is
  shared. `golden.sh` deletes each job's game directory afterwards.
- The pack is hard-linked into `--job` game directories and copied into the
  shared ones, and not restaged when it is already the same file. A hard
  link in a shared directory would let an older checkout's runner, which
  stages with `cp -f`, overwrite this repository's pack.
- Capture features freeze all simulation and animation at a fixed tick and
  hide the debug HUD. Every animator advances per simulation tick, so a
  capture does not depend on load timing or pack size (RE-315).
- `--pack PATH` stages another pack (A/B captures); the native 480×272 frame
  is also written as `screenshot-native.png`. `tools/residual-ab-capture.sh`
  uses both for the RE-312 visual review
  ([three-point-visual-review.md](../rendering/three-point-visual-review.md)).
- `compare-screenshot.sh` defaults to an exact match (0 differing pixels).
  It and `golden.sh` share `tools/lib/pixel-diff.sh`.
- Capture features must not ship in interactive builds; rebuild without them
  afterwards.

## Rules

- Explain every golden change (pixel count, cause) before accepting it.
- Confirm a new golden is deterministic with two captures.
- PPSSPP software is the only exact tier. PPSSPP hardware backends and
  physical PSP captures are separate, qualitative tiers.
- Never pass a camera photo of a PSP screen to the pixel comparator.

## Scenes

`psp-asset-viewer` unless noted. The scene spec for each golden is in
[`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv); the feature
builds the same scene as its default.

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
| `regression_capture_stage_index` | `r2-stage-<name>` (38) | Whole-stage view; spec `stage N`, or `SSB64_STAGE_INDEX` at build time (1–3, 5–8, 10–40). Index → golden map in the manifest (RE-316) | RE-286, RE-312 |
| `depth_mask_diagnostic` | `r2-depth-mask-diagnostic` | Synthetic quads: depth write ON → OFF → ON | RE-251 |
| `regression_capture_<fighter>` | `r2-<fighter>-fighter` | High-detail model in `Wait` pose with stage fighter light; all 12 playable fighters | RE-265 |
| `regression_capture_link_costume_1` | `r2-link-costume-1` | Non-zero costume palette, colour and light | RE-301 |
| `regression_capture_metal_mario` | `r2-metal-mario-fighter` | Texgen on a posed fighter; compensated body texture | RE-311 |
| `regression_capture_mario_entry` | `r2-mario-entry-pipe` | File 356 graph `0x608`, dense compensation variant | RE-312 |
| `regression_capture_bonus_platform` | `r2-bonus-platform-small` | File 136 graph `0x3DA8`, UV phase variant | RE-312 |
| — (`dream_land_water` spec only) | `r2-dream-land-water` | File 104 graph `0x2450` from above: both two-tile fractional blend ponds | RE-321 |
| `regression_capture_object` | none (A/B only) | Any graph, chosen by `SSB64_CAPTURE_OBJECT=<file>:<hex graph>` | RE-312 |
| `regression_capture_fireball` (`psp-game`) | `f1-training-fireball` | Translucent Fireball weapon in Training | RE-300 |
| `regression_capture_shadows` (`psp-game`) | `f1-training-shadows` | Grounded and airborne fighter shadows | RE-302 |

Stage sweep example:

```bash
tools/golden.sh verify --filter '^r2-stage-'
```

Not covered by a golden: dynamic particles (device audits RE-180–189) and UI
(not implemented).

### Known failing goldens

None. A golden that is known to differ from current output gets status
`known-failing` in [`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv)
until it is explained and rebaselined. The last eight were rebaselined in
RE-317.

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
   `tools/stage-psp-regression.sh [--golden NAME] /path/to/psp-mount [rom]`
   (default `r0-dream-land-default`). It rebuilds the pack, builds the
   manifest row's crate with `capture_scene_file`, installs the EBOOT, pack
   and `capture_scene.txt` under `PSP/GAME/ssb64/` and writes
   `regression-manifest.txt` (commit, build, golden, scene, hashes).
2. Launch, wait past the freeze tick, then capture (PSPLink `scrshot` or a
   square-on photo with locked focus and exposure).
3. Record model, firmware, commit, manifest hashes, capture method and
   observations.

See the `psp-hardware` skill for PSPLink. RE-320 captured the v32 stripe
diagnostic and stock pack on a PSP-2000 Slim, firmware 6.61. The current
golden matrix has not been recaptured on hardware.

## Original game

RE-151 drives the original ROM through Mupen64Plus with frame-indexed inputs
and compares camera state numerically from RDRAM. See the `n64-emulator`
skill.

## 2026-09-24 RE-312 v766 override

Pack SHA-256 `de9f6c64679b7d42e7b9d35dace9115a13f1cd96c00923748b994e3f825acc08` differs from the previous pack only in texture 766 (Board the Platforms, Mario). Every stage, scene and fighter golden was recaptured: only `r2-stage-bonus2-mario` changed, by 308 pixels at 2x (77 native pixels, all inside the v766 site, maximum 5/255). It was refreshed; a repeat capture differs by 0 pixels. The 14 fighter goldens pass. The eight known-failing goldens bind no changed texture.

## 2026-09-24 RE-314 texture size cap

Runtime only; the pack is unchanged. Textures whose padded width or height
exceeds 512 are now declared to the GE as 512 instead of overflowing
`TSIZE`. Eleven stage goldens changed and were refreshed:
`r2-stage-kongo-jungle`, `r2-stage-hyrule-castle`,
`r2-stage-bonus1-{link,kirby,ness}` and
`r2-stage-bonus2-{fox,link,captain-falcon,kirby,pikachu,ness}` (2,184 to
53,200 pixels at 2x). With the 88 primitives that bind an over-512 texture
hidden, old and new captures of all eleven are byte-identical, so every
change is inside those primitives. Each refreshed golden matched a repeat
capture. Stage 1 (control) is unchanged.

## 2026-09-24 RE-315 animation ticks per simulation tick

Runtime only; the pack is unchanged. `psp-asset-viewer` now ticks stage,
material and effect-spawn animation once per simulation tick instead of once
per render frame. All 67 goldens were recaptured with the shipped pack, a
repeat capture, and two zero-padded packs (+0x4A940 and +1,223,104 bytes):
every repeat and padded capture matches its shipped capture. 27 goldens
changed and were refreshed: 25 stage-index goldens, `r2-saffron-city-gate`
and `r2-peach-castle` (132 to 15,664 pixels at 2x). With the stage and
material animator ticks disabled in both runtimes, old and new captures are
byte-identical, so every change is animation timing. The six known-failing
viewer goldens are identical between old and new code.

## 2026-09-24 RE-317 known-failing rebaseline

Runtime and pack unchanged. The eight goldens that had differed since before
RE-313 were attributed commit by commit and rebaselined to current output:
`r2-metal-texgen`, `-rotated`, `-linear`, `-camera-rotated`,
`r2-depth-mask-diagnostic`, `r1-mvopeningroom`, `f1-training-fireball` and
`f1-training-shadows` (1,000 to 393,384 pixels at 2x). Causes: RE-304
sampling alignment, RE-305 to RE-310 compensation packs, and the
pillarbox clear that RE-307's commit made cover both swap buffers. All 67
goldens now pass.

## 2026-09-24 RE-318 wide-tile lowering

Pack changed (SHA-256 `af3236384d34fed25d8a441cdd700c555bb637791b2a6dbaebc6df8b11bcd835`,
22,010,384 bytes). The 88 primitives whose tile baked past 512 texels now
draw a repeating period plus a clamped far window. With the shipped pack all
67 goldens matched; with the new pack exactly ten changed and were
rebaselined: `r2-stage-kongo-jungle`, `r2-stage-hyrule-castle`,
`r2-stage-bonus1-{kirby,ness}` and
`r2-stage-bonus2-{fox,link,captain-falcon,kirby,pikachu,ness}` (516 to
22,772 pixels at 2x). With the affected primitives hidden in both packs, all
eleven affected stages are byte-identical; stage 35 needs the 2,048 bytes
after one 8-byte-row T4 texture made equal, because PPSSPP reads that
texture with a 16-byte pitch. `verify --twice` passes 67/67.

## 2026-09-24 RE-319 short texture rows

Pack v32 widens 304 short `PsmT4` texture rows to the GE's 16-byte minimum
without changing their declared size. Against the RE-318 baseline, 44 of 67
goldens changed (56 to 17,092 pixels at 2×), including Dream Land, Captain
Falcon, and Bonus 1/2 stage surfaces. Each changed golden was rebaselined
after two matching captures; `verify --twice` passes 67/67. See RE-319 for
the texture census, stripe diagnostic, and visual attribution.

## 2026-09-25 RE-322 colour tracks and animator capacity

Pack v34 (`ff5166dd…`). Eight goldens changed and were refreshed; 68 of 68
then match twice. `r2-stage-bonus3-race-to-the-finish` (10,656 pixels at 2x):
Race to the Finish's animated glows blend and its lit fixture follows its
light track. `r2-stage-final-destination`, `r2-stage-metal-mario`,
`r2-stage-bonus2-fox` and the four `r2-metal-texgen*` scenes (24,612 to 44,596
pixels): `MaterialAnimator` now ticks `MatAnimDesc` entries 64-102, which a
fixed 64-slot array had left static. `SSB64_CAPTURE_TICKS=<n>` at build time
moves a diagnostic build's freeze tick; goldens never set it.

## 2026-09-25 RE-323 task-list-1 XLU reset

Pack `a4072881…`. Four goldens changed and were refreshed; 68 of 68 then
match twice. Stage render-layer-1 task-list-1 primitives now blend under
`G_RM_AA_ZB_XLU_SURF`: `r2-stage-bonus3-race-to-the-finish` (8,764 pixels
at 2x, nodes 8/9 glows), `r2-stage-bonus2-mario` (1,156, hazard-bar glow),
`r2-stage-zebes` (236) and `r2-stage-sector-z` (224, engine glows). N64
references in RE-323.

## 2026-09-25 RE-324 material animation phase

Pack unchanged (`a4072881…`). `MaterialJoint` tick `n` is now the decomp's
frame `n`, two frames later than before. Fourteen goldens changed and were
refreshed; 68 of 68 then match twice. Changes range from 144 pixels
(`r2-stage-yoshis-island`) to 81,856 (`r2-dream-land-water`) at 2x.
Control: a build that pre-ticks every material joint twice reproduces all
68 old goldens exactly. Per-golden counts are in RE-324. Correction
(RE-325): the pack was not rebuilt, and RE-324's packer changes it.

## 2026-09-25 RE-325 material resolvers

Pack v34 rebuilt (`59cc6d36…`, +5,424 bytes: entry 91's sprite table gains
its second texture). `resolved_palette` accepts every live kind and
truncates. No golden changed; 68 of 68 match twice. No golden covers the
frames either change affects (RE-325).
