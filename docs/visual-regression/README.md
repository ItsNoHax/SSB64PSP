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
  The manifest currently has 74 scenes; RE-316 measured about 17 s for an
  earlier full run.
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

RE-357 refreshes the seven Training goldens for Whispy's face and flower
beds, plus DK and the two Link goldens for previously ignored cached ST
writes. All ten changes were visually reviewed and captured twice with
identical pixels; the full 73-scene matrix passes. Pixel counts and source
attribution are in [RE-357](../evidence/re/RE-357.md).

## Scenes

RE-368 refreshes only `r2-luigi-fighter`: the ROM's translation scales
retarget his shared Mario animation, changing 57,056 pixels at 2×. The
new capture was visually reviewed and byte-identical twice. The Mario
fighter control remains unchanged; both pass after rebaseline.

RE-359 refreshes 41 goldens for tile-relative repeating/mirrored UVs.
Mario and Luigi regain their overall buttons; DK's head texture changes
slightly. Other changes are on Mario in stage and Training views. All
textures are unchanged, and the old pack with the new runtime reproduces
all 73 previous goldens twice. Counts and controls: [RE-359](../evidence/re/RE-359.md).

RE-358 refreshes 40 goldens for animated shared-joint geometry. Mario's
knees and Pikachu's torso/limb seams close; stage-view differences are on
the small Mario model. Disabling only the new vertex reconstruction with
pack v43 reproduces all 73 previous goldens twice. Textures are unchanged.
Counts and source attribution: [RE-358](../evidence/re/RE-358.md).

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
| `golden_capture` (`psp-game`, scene `grab`) | `f1-training-grab` | Mario turns toward and catches the Training dummy on Dream Land's left platform; the held fighter's TopN takes only the catcher's hand-joint rotation, with no extra facing yaw | RE-330–332, RE-370–371 |
| `golden_capture` (`psp-game`, scenes `costume1`–`costume3`) | `f1-training-costume-1`–`3` | Fox after a C-Right, C-Down or C-Left pick on the Training entry: costumes 1–3 in `psp-game` | RE-341 |
| `golden_capture` (`psp-game`, scene `jab`) | `f1-training-jab` | Mario's jab from the grab route landing on the Training dummy; the hit is in the capture log (`dummy_damage=2 dummy_status=Common(DamageN1)`, `PPSSPPHeadless --log`), not the image | RE-351 |
| `golden_capture` (`psp-game`, scene `luigi`) | `f1-training-luigi` | Luigi in Training after one B tap: his Fireball in flight with `palettes[1]` (green) | RE-372 |
| `golden_capture` (`psp-game`, scenes `samus`, `samusshot`, `samusbomb`) | `f1-training-samus`, `-samus-shot`, `-samus-bomb` | Samus's level-2 charging shot on her arm cannon; a released level-1 shot; her Bomb on the floor after she walks off it | RE-373 |
| `golden_capture` (`psp-game`, scenes `link`, `linkspin`) | `f1-training-link`, `-link-spin` | Link's Boomerang in flight; 13 frames into a ground Spin Attack, whose swirl shows edge-on as a thin arc | RE-374 |
| `golden_capture` (`psp-game`, scenes `yoshi`, `yoshibomb`) | `f1-training-yoshi`, `-yoshi-bomb` | Yoshi's Egg Throw egg 16 frames into its flight; the Yoshi Bomb's two stars 6 frames after they appear (retimed for the hop in RE-378) | RE-375, RE-378 |
| `golden_capture` (`psp-game`, scenes `ness`, `nessthunder`, `nessmagnet`) | `f1-training-ness`, `-ness-thunder`, `-ness-magnet` | Ness's PK Fire spark in flight; PK Thunder's head with four trails; PSI Magnet held | RE-381 |
| `golden_capture` (`psp-game`, scene `linkbomb`) | `f1-training-link-bomb` | Link holding a freshly pulled Bomb | RE-383 |
| `golden_capture` (`psp-game`, scenes `cpuwalk`, `cpujump`) | `f1-training-cpu-walk`, `f1-training-cpu-jump` | The Training dummy under the CPU's Walk and Jump behaviours (in `WalkMiddle` and `JumpF` at the freeze) | RE-390 |
| `golden_capture` (`psp-game`, scene `vs`) | `f1-vs-countdown` | A VS battle's countdown on Dream Land, both fighters locked until "Go" | RE-389 |
| `golden_capture` (`psp-game`, scene `vstimeup`) | `f1-vs-sudden-death` | A one-minute VS battle timed out in a tie, in sudden death with both fighters at 300% | RE-389 |
| `golden_capture` (`psp-game`, scene `vscpu`) | `f1-vs-cpu` | A VS battle's CPU forward-throwing the idle player at tick 690, after a down air; `vstimeup` pins its CPU to Stand to keep its tie | RE-391 |
| `golden_capture` (`psp-game`, scene `vstimeupsign`) | `f1-vs-time-up` | `vstimeup` at tick 4,040: 00:00 and "TIME UP" during the end wait | RE-395 |
| `golden_capture` (`psp-game`, scene `vssuddendeath`) | `f1-vs-sudden-death-sign` | `vstimeup` at tick 4,150: "SUDDEN DEATH!" before its "GO!", emblems and stock icons without digits | RE-397 |
| `golden_capture` (`psp-game`, scene `vspause`) | `f1-vs-pause` | START at tick 500: the pause menu's zoom on Mario, border, "1P PAUSE" and decals at 560 | RE-398 |
| `golden_capture` (`psp-game`, scene `vsmode`) | `f1-vs-mode-menu` | The VS mode menu as plain slots after Rule → Stock and one more stock | RE-399 |
| `golden_capture` (`psp-game`, scene `vsnocontest`) | `f1-vs-no-contest` | A VS battle reset from the pause menu, at results tic 117 (tick 640): the random blue wallpaper, the KOs and TKO rows, both Marios one row back clapping under their tags, and "NO CONTEST" | RE-400, RE-409, RE-410 |
| `golden_capture` (`psp-game`, scene `vsresults`) | `f1-vs-results` | A one-stock battle Luigi loses by running off Dream Land, at tick 1100: Kirby in front holding his last Win frame, Luigi behind turned to him and clapping, over the blue wallpaper, with the tags, the Place and KOs rows, the bar, the header and "KIRBY WINS!" | RE-409, RE-410 |
| `golden_capture` (`psp-game`, scene `vsplayers`) | `f1-vs-players` | The VS character select at select tic 65: Yoshi placed on port 1's red card, a CPU (Donkey Kong, in his Win1 clip) opened in port 2 with its CP level, the NA doors shut, the stone wallpaper, portraits, pucks, hand and the "Ready to fight" banner | RE-404, RE-411 |
| `golden_capture` (`psp-game`, scene `vs4`) | `f1-vs-four` | Mario against three CPUs (Fox, Donkey Kong, Kirby) on Dream Land at tick 870, four damage displays | RE-405 |
| `golden_capture` (`psp-game`, scene `vsteam`) | `f1-vs-team` | A team battle on Dream Land at tick 940: Mario and Kirby (red) against Fox and Donkey Kong (blue) in team costumes and emblem colours, Kirby's attack passing through Mario | RE-407 |
| `golden_capture` (`psp-game`, scene `rebirth`) | `f1-training-rebirth` | Mario after a KO below Dream Land, crouched in `RebirthStand` on the rebirth halo, faintly lit white by the rebirth glow | RE-388, RE-412 |
| `golden_capture` (`psp-game`, scene `rebirthblast`) | `f1-training-rebirthblast` | `rebirth` at tick 168, eight ticks after the KO: the blast column rising from the bottom of the screen and the screen flash inside the (10, 10)–(310, 230) border | RE-412 |
| `golden_capture` (`psp-game`, scene `fighterselect`) | `f1-training-fighter-select` | Kirby placed on the character select, then Peach's Castle on the stage select; Kirby and a Mario dummy standing on the castle | RE-386 |
| `golden_capture` (`psp-game`, scene `stageselect`) | `f1-training-stage-select` | Mario and the dummy standing on Hyrule Castle, picked on the stage select; every other `psp-game` scene skips the select and loads Dream Land | RE-385 |
| `golden_capture` (`psp-game`, scene `donkey`) | `f1-training-donkey` | Donkey Kong winding up a Giant Punch (`SpecialNLoop`) | — |
| `golden_capture` (`psp-game`, scene `purin`) | `f1-training-purin` | Jigglypuff 30 frames into Sing, with its rings and three notes | RE-380 |
| `golden_capture` (`psp-game`, scenes `pikachu`, `pikachuair`) | `f1-training-pikachu`, `-pikachu-air` | Pikachu's ground Thunder Jolt 8 plays into its first push cycle; an aerial jolt 11 frames into its flight | RE-379 |
| `golden_capture` (`psp-game`, scene `kirby`) | `f1-training-kirby` | Kirby after a grounded Final Cutter lands on the top platform; its wave 6 frames old | RE-378 |
| `golden_capture` (`psp-game`, scenes `captain`, `captainkick`) | `f1-training-captain`, `-captain-kick` | Captain Falcon's ground Falcon Punch, some 7 frames after its flame appears at joint 16; a ground Falcon Kick some 12 frames into its flame | RE-376–377 |
| `golden_capture` (`psp-game`, scene `pikachuthunder`) | `f1-training-pikachu-thunder` | Pikachu dashes out from under the top platform; Thunder's head reaches him (tick 92, the first `SpecialLwHit` frame) with its trails and fading segments above | RE-417 |
| `golden_capture` (`psp-game`, scene `kirbyhat`) | `f1-training-kirby-hat` | Kirby inhales and copies the Mario dummy on Dream Land's left platform and wears Mario's cap (joint 6's model part 12) | RE-417 |
| `golden_capture` (`psp-game`, scene `yoshiegg`) | `f1-training-yoshi-egg` | Yoshi's aerial Egg Lay of the dummy; the egg rests on the top platform in its Wait wobble | RE-417 |
| `golden_capture` (`psp-game`, scene `yoshishield`) | `f1-training-yoshi-shield` | Yoshi holds Z from tick 40 to 600: his model is hidden inside the egg shield, darkened by the worn shield (health 21, ENV (107, 132, 132)): the egg's white texels (231) draw (134, 111, 111) and its green spots (57, 214, 57) draw (33, 103, 27) | RE-418 |
| `golden_capture` (`psp-game`, scene `vsshield`) | `f1-vs-shield-damage` | A VS battle whose player shields from "Go"; frozen after tick 684's update, where the CPU's Mario Tornado sets the shield off and the bubble draws the grey damage row | RE-418 |
| `golden_capture` (`psp-game`, scene `stageselectview`) | `f1-training-stage-select-view` | Training's stage select left on Hyrule Castle at tick 60: the stone, icons, red cursor, plaque, name plate and emblem, and the castle's preview model over its blue Training wallpaper | RE-419 |
| `golden_capture` (`psp-game`, scene `stageselectyoshi`) | `f1-training-stage-select-yoshi` | The same select moved down to Yoshi's Island: the yellow Training wallpaper, and the preview hiding the two cloud nodes `mnMapsMakeModel` hides | RE-419 |
| `golden_capture` (`psp-game`, scene `vssector`) | `f1-vs-sector` | `vs`'s countdown on Sector Z: the wallpaper scaled about its centre by the camera's distance | RE-419 |
| `golden_capture` (`psp-game`, scene `vsyoshi`) | `f1-vs-yoshi` | `vs`'s countdown on Yoshi's Island: the static wallpaper | RE-419 |
| `golden_capture` (`psp-game`, scene `trainingselect`) | `f1-training-select` | The Training character select at its tick 60: the stone, "Training Mode", BACK, the portraits with the four locked shadows, the red and grey cards, the hand on the player's card, and the CPU's Mario turning on his card under his name, emblem and puck | — |
| `golden_capture` (`psp-game`, scene `trainingselectpicked`) | `f1-training-select-picked` | The same select after Kirby is placed in his C-Down (cyan) costume and the CPU's puck is picked up and placed again in Mario's C-Right costume: both fighters in their Win3 clips, the "Ready to fight" banner and "Press Start" | — |
| `golden_capture` (`psp-game`, scene `starko`) | `f1-training-starko` | `rebirth`'s Mario put in `DeadUpStar` on Dream Land's floor at tick 60, 150 ticks into the flight: a few pixels over the treetop, past the old 10,000-unit far plane | RE-420 |
| `golden_capture` (`psp-game`, scene `vsresultsemblem`) | `f1-vs-results-emblem` | `vsresults` at results tic 100: the winner's (Kirby, port 2) blue series emblem shrinking and rising over the fading wallpaper, before the text and confetti | RE-420 |
| `golden_capture` (`psp-game`, scene `shield`) | `f1-training-shield` | Mario's tilted Guard pose on Dream Land; `PPSSPPHeadless --log` confirms the raised shield and posed `YRotN` collision center, and the red player-1 shield bubble around him | RE-367, RE-369, RE-384 |

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
diagnostic and stock pack on a PSP-2000 Slim, firmware 6.61. RE-341
captured the six `psp-game` scenes; the viewer goldens have not been
recaptured on hardware.

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

## 2026-09-25 RE-326 material sampling

Pack v35 (`5e529bbf…`, same size). Texture, palette and window tracks apply
only where the primitive's `MObj` still owns that state; palettes resolve per
primitive; the material UV affine is decomp-exact. 13 goldens changed and
were rebaselined; 68 of 68 then match twice. Restoring the old affine alone
reproduces `r0-dream-land-default`, `r2-dream-land-water`,
`f1-training-fireball` and `f1-training-shadows`, and leaves Zebes, Mushroom
Kingdom, Final Destination and Bonus 2 Fox unchanged; Meta Crystal and the
four Metal texgen scenes change under both. Counts in RE-326.

PSP-2000 Slim hardware captures of Mushroom Kingdom, Meta Crystal and Final
Destination agree with the new renders (100–512 native pixels over 24 against
1,716–5,698 against the old goldens). The rest of the matrix is not
recaptured.

## 2026-09-25 RE-328 camera head-1 reset

Pack `a5eacfa2…` seeds every packed graph's head-1 stream from the camera's
`G_RM_AA_ZB_XLU_SURF` reset. Five goldens changed and were rebaselined after
two identical captures: `r1-mvopeningroom` (85,248 pixels at 2x),
`r1-stage-sector` (580), `r1-catch-swirl-flat-color` (30,892),
`r2-bonus-platform-small` (1,628) and `r2-stage-mushroom-kingdom` (2,012).
The other 63 goldens were unchanged. The ROM graph census and pack diff are
in RE-328.

## 2026-09-26 RE-341 costumes and dummy costume

Pack v37 unchanged (`2629e02d…`). After the RE-339/RE-340 merge all 69
goldens matched: no `psp-game` scene lands a hit, so handicaps and staling
cannot move them. Three new goldens, `f1-training-costume-1`–`3`, show Fox
after each C-button pick; each differs from a costume-0 control by 916
pixels at 2x. The Training dummy then took the source's free costume
(Mario costume 1 beside a Mario player), and exactly the three Mario-dummy
goldens changed: `f1-training-fireball` 2,124, `f1-training-shadows` 432 and
`f1-training-grab` 2,296 pixels at 2x, all on the dummy. 72 of 72 then match
twice. The PSP-2000 ran the six `psp-game` scenes; the captures agree with
PPSSPP apart from edge noise (counts in RE-341).

## 2026-09-27 RE-351 jab scene and bonus-stage rebaseline

Pack v39 (`2e0ce44b…`). The five scripted Training scenes still land no hit:
their jab route misses the dummy's platform, and the `grab` scene's catch
whiffs (capture log). New `f1-training-jab` takes the grab route and taps A;
the capture log reads `dummy_damage=2 dummy_status=Common(DamageN1)`. The
image cannot show the hit (no damage or jab clips). `r2-stage-bonus1-luigi`
was rebaselined (160 pixels at 2x): it had been stale since RE-350, and a
build of `c97ed46` renders the same pixels. 73 of 73 then match twice.

## 2026-09-27 RE-354 Fireball free flight

Pack v40 unchanged (`a3e1a831…`). The Fireball had moved only on a map
contact. It now moves by `vel_air` every frame. `f1-training-fireball`
changed by 2,476 pixels at 2x and was rebaselined: the shot leaves Mario's
hand and rebounds. The other six `f1-training` goldens match.

## 2026-09-28 RE-372 Luigi Training scene

Pack v51 (`9f8a292d…`) adds Luigi's Fireball mesh. New `f1-training-luigi`
shows Luigi after one B tap with a green Fireball in flight; its two
captures were identical. 75 of 75 goldens match twice, and
`f1-training-fireball` (Mario's red palette) is unchanged.

## 2026-09-28 RE-373 Samus Training scenes

Pack v52 (`e814f589…`) adds Samus's Charge Shot mesh and two Bomb blink
meshes. The new `f1-training-samus`, `f1-training-samus-shot` and
`f1-training-samus-bomb` goldens were each identical in both captures.
78 of 78 goldens match twice.

## 2026-09-28 RE-374 Link Training scenes

Pack v53 (`290e996d…`) adds Link's Boomerang weapon animation. The new
`f1-training-link` and `f1-training-link-spin` goldens were each identical
in both captures. 80 of 80 goldens match twice.

## 2026-09-28 RE-375 Yoshi Training scenes

Pack v54 (`8580875d…`) adds Yoshi's Bomb star under the weapon seed. The
new `f1-training-yoshi` and `f1-training-yoshi-bomb` goldens were each
identical in both captures. 82 of 82 goldens match twice.

## 2026-09-28 RE-376 Captain Falcon Training scene

Pack v54 is unchanged. The new `f1-training-captain` golden was identical
in both captures. 83 of 83 goldens match twice.

## 2026-09-28 RE-377 Captain Falcon Kick scene

Pack v54 is unchanged. The new `f1-training-captain-kick` golden was
identical in both captures. 84 of 84 goldens match twice.

## 2026-09-28 RE-378 Kirby scene and TransN for every status

Pack v55 (`fc254d0b…`) converts Kirby's Final Cutter wave under the weapon
seed. `psp-game` now samples TransN for every status, which changed
`f1-training-captain`, `-captain-kick` and `-yoshi-bomb` (the Yoshi Bomb
capture moved from tick 60 to 68). Those three and the new
`f1-training-kirby` were each identical in both captures. 85 of 85 goldens
match twice.

## 2026-09-28 RE-379 Pikachu scenes, weapon pass order and cutout gate

Pack v56 (`8c65430a…`). Weapons and effects now draw after the fighters,
and the cutout alpha gate is issued as `>= 1` to sidestep PPSSPP's
`GREATER 0` shortcut. That changed `f1-training-fireball`, `-samus`,
`-link-spin`, `-captain`, `-captain-kick` and `r2-saffron-city-gate`
(RE-379 lists why). They and the new `f1-training-pikachu` and
`-pikachu-air` were each identical in both captures. 87 of 87 goldens
match twice.

## 2026-09-28 RE-380 Jigglypuff scene

Pack v56 is unchanged. The new `f1-training-purin` golden was identical in
both captures. 88 of 88 goldens match twice.

## 2026-09-28 Donkey Kong scene

Pack v56 is unchanged. The new `f1-training-donkey` golden was identical in
both captures. 89 of 89 goldens match twice.

## 2026-09-28 RE-381 Ness scenes

Pack v57 (`f0fbc6a7…`). The new `f1-training-ness`, `-ness-thunder` and
`-ness-magnet` goldens were each identical in both captures. 92 of 92
goldens match twice.

## 2026-09-28 RE-382/RE-383 items

Pack v59 (`77df9d76…`). `psp-game`'s `run` was split for
MIPS branch range with no golden change. The new `f1-training-link-bomb`
golden was identical in both captures. 93 of 93 goldens match twice.

## 2026-09-28 RE-384/RE-385 shield bubble and stage select

Pack v60 (`81cf9a09…`). `f1-training-shield` was rebaselined for the
shield bubble; no other golden changed. The new `f1-training-stage-select`
golden was identical in both captures. 94 of 94 goldens match twice.

## 2026-09-28 RE-386 character select

Pack v60. The costume scenes' picks moved from a menu C-button tap to preset
scene data with no golden change. The new `f1-training-fighter-select`
golden was identical in both captures. 95 of 95 goldens match twice.

## 2026-09-28 RE-387 spawn facing

Pack v60. Fighters now face the stage centre from their spawn, so 47
goldens were rebaselined: 28 `psp-game` Training scenes and 19 asset-viewer
scenes whose only change is Mario's facing. `f1-training-grab` lost its
tick-98 turn tap. 95 of 95 goldens match twice.

## 2026-09-28 RE-388 KO and rebirth

Pack v60. The new `f1-training-rebirth` golden was identical in both
captures. `r2-stage-bonus2-fox` was rebaselined: the viewer's Mario used to
fall out of frame and is now KO'd and respawns. 96 of 96 goldens match
twice.

## 2026-09-29 RE-389 VS battle

Pack v60. The new `f1-vs-countdown` and `f1-vs-sudden-death` goldens were
each identical in both captures; `vstimeup` runs 4,200 ticks. 98 of 98
goldens match twice.

## 2026-09-29 RE-390 CPU Training behaviours

Pack v60. The dummy now runs the CPU's Stand behaviour with no golden
change. The new `f1-training-cpu-walk` and `f1-training-cpu-jump` goldens
were each identical in both captures. 100 of 100 goldens match twice.

## 2026-09-29 RE-391 VS CPU

Pack v60. The VS CPU fights; `vstimeup` pins its CPU to Stand so its tie
holds, and matches unchanged. The new `f1-vs-cpu` golden was identical in
both captures. 101 of 101 goldens match twice.

## 2026-09-29 RE-392 damage display

Pack v61 (`47f8869d…`) adds the `SObj` sprite table. The damage display
now draws under the stage in Training and, from "Go", in VS, so 34
`psp-game` goldens were rebaselined, each captured twice. Every diff lies in
the HUD band (y 454–496 of the 960×544 capture). `f1-vs-countdown` is
unchanged. A scene capture's timeout rose from 30 s to 60 s, since
`vstimeup` exceeds 30 s under a full `-j`. 101 of 101 goldens match twice.

## 2026-09-29 RE-393 fighter emblems

Pack v62 (`f072a92c…`). Each fighter's series emblem now draws behind its
damage display, so 35 `psp-game` goldens were rebaselined, each captured
twice; every diff lies in the HUD band (y 438–498). `f1-vs-countdown` shows
the emblems without digits, as `ifCommonPlayerDamageProcDisplay` draws the
emblem before its show check. 101 of 101 goldens match twice.

## 2026-09-29 RE-394 countdown

Pack v62 (`4c1c7045…`) adds file 82's sprites. The VS countdown's traffic
light and "GO!" draw, and the entry focus's `syUtilsRandIntRange(3)` now
draws from the seed, which moved the VS CPU's choices: `f1-vs-cpu` now
catches the player in `CatchPull` at 10%. Sprites tint through `Blend`,
which shifts the Training HUD by at most one level per channel. 35
`psp-game` goldens were rebaselined, each captured twice. 101 of 101
goldens match twice.

## 2026-09-29 RE-395 timer

Pack v62 (`2e1289b8…`) adds file 165. A timed battle draws its timer, so
`f1-vs-countdown` (03:00) and `f1-vs-cpu` (02:56) were rebaselined, each
captured twice. The new `f1-vs-time-up` golden was identical in both
captures. 102 of 102 goldens match twice.

## 2026-09-29 RE-396 stock icons

Pack v62. The four VS goldens gained each fighter's stock icon (636 pixels
each) and were rebaselined, each captured twice. 102 of 102 goldens match
twice.

## 2026-09-29 RE-397 sudden death text

Pack v62 (`0c2075ed…`) adds file 37. The new `f1-vs-sudden-death-sign`
golden was identical in both captures. 103 of 103 goldens match twice.

## 2026-09-29 RE-398 pause menu

Pack v62 (`b41c1951…`) adds file 197. The new `f1-vs-pause` golden was
identical in both captures; `vssuddendeath` got an explicitly neutral stick
with no golden change. 104 of 104 goldens match twice.

## 2026-09-29 RE-399 VS mode menu

Pack v62. The new `f1-vs-mode-menu` golden was identical in both captures;
the direct-route VS scenes skip the menu and are unchanged. 105 of 105
goldens match twice.

## 2026-09-29 RE-400 VS results

Pack v62. The new `f1-vs-no-contest` golden was identical in both
captures. 106 of 106 goldens match twice.

## 2026-09-29 RE-401 entry clips

Pack v63 (`8bfe1acc…`) adds the battle-entry clips, which nothing plays
yet. 106 of 106 goldens match twice.

## 2026-09-29 RE-402 VS entry

Pack v63. VS fighters now enter through their Appear clips, so six VS
goldens were rebaselined, each captured twice: `f1-vs-countdown` shows
Mario mid-entry and the rest follow from the entry's timing. 106 of 106
goldens match twice.

## 2026-09-29 RE-403 entry effects

Pack v63. The entry effects draw, so `f1-vs-countdown` was rebaselined,
captured twice: it shows Mario's pipe under each spawn. 106 of 106
goldens match twice.

## 2026-09-29 RE-404 VS character select

Pack v63. The new `f1-vs-players` golden was identical in both captures;
the other 106 are unchanged. 107 of 107 goldens match twice.

## 2026-09-29 RE-405 four-fighter VS

Pack v63. The pair-to-loop refactor left all 107 goldens unchanged. The
new `f1-vs-four` golden was identical in both captures. 108 of 108
goldens match twice.

## 2026-09-29 RE-406 battle camera interests

Pack v63. The battle camera now frames every fighter, so 39 Training and
VS goldens were rebaselined, each captured twice; scenes whose dummy sat
inside the player's box are unchanged. 108 of 108 goldens match twice.

## 2026-09-29 RE-407 team battles

Pack v63. The team rule is inert in a free-for-all and in Training, so
all 108 goldens are unchanged. The new `f1-vs-team` golden was identical
in both captures. 109 of 109 goldens match twice.

## 2026-09-29 RE-408 demo clips

Pack v64 (`33fd7055…`) adds the fighters' Win and Lose demo clips, which
nothing plays yet. All 109 goldens are unchanged. 109 of 109 goldens match
twice.

## 2026-09-29 RE-409 results fighters

Pack v64. The results screen now draws its fighters under the results
camera. `f1-vs-no-contest` was rebaselined (522,232 pixels at 2×,
identical in both captures). The new `f1-vs-results` golden was seeded
from its first verify run. The other 108 goldens are unchanged. 110 of 110
goldens match twice.

## 2026-09-29 RE-410 results wallpaper, text and table

Pack v64, now 29,127,792 bytes with the results screen's sprites. The
results draw their wallpaper, fades, player tags, winner text, tint and
table. The `vsnocontest` capture moved from tick 560 to 640, so the table
has its rows. `f1-vs-results` (337,000 pixels at 2×) and
`f1-vs-no-contest` (338,384) were rebaselined, identical in both captures.
The other 108 goldens are unchanged by the new pack. 110 of 110 goldens
match.

## 2026-09-29 RE-411 VS character select

Pack v64, now 30,405,424 bytes (SHA-256 `720bbc89…`) with the select's
sprites. The VS character select draws its wallpaper, top bar, portraits,
panels, pucks, cursors, banner and fighters. `f1-vs-players` (521,684
pixels at 2×) was rebaselined, identical in both captures; its capture
stays at tick 85. The other 109 goldens are unchanged by the new pack.
110 of 110 goldens match.

## 2026-09-29 RE-412 KO presentation

Pack v65, 30,405,584 bytes (SHA-256 `f0e8c31f…`), with each stage's fog
colour. KOs now draw the blast and the screen flash, and a respawn draws
its halo and the rebirth glow. `f1-training-rebirth` was rebaselined
(3,536 pixels at 2×, identical in both captures): Mario crouches on the
halo. The new `f1-training-rebirthblast` golden (`rebirthblast`, tick 168)
was seeded from its first verify run. `f1-vs-results` is unchanged, as its
battle is not captured. The other 109 goldens are unchanged. 111 of 111
goldens match.

## 2026-09-29 RE-413 particle runtime

No pack change. Matches now run the particle runtime: hits draw their
sparks and a KO its streaks. Six goldens were rebaselined, identical in
both captures: `f1-training-jab` (2,436 pixels at 2×, a light spark at
Mario's fist), `f1-training-rebirthblast` (12,184, stars rising along the
blast), `f1-vs-cpu` (416, a small spark), and `f1-vs-four` (337,444),
`f1-vs-team` (377,560) and `f1-vs-results` (11,592), whose battles and
win pose follow the source's random draws in the particle scripts and
makers. A build restoring the seed around every particle call left those
three matching their old goldens. The other 105 are unchanged. 111 of 111
goldens match twice.

## 2026-09-29 RE-414 colour animations

Pack v66, 30,491,056 bytes (SHA-256 `d5e4ba74…`), with the electric-damage
skeletons. Fighters now run their colour animations. Eleven goldens were
rebaselined, identical in both captures: `f1-training-fireball` (2,508
pixels at 2×, Mario red on the Fireball's first frame), `-luigi` (2,292)
and `-samus-shot` (2,976), lit from the side by their specials' scripts,
`-link-spin` (1,116), `-captain` (2,988), `-pikachu` (2,588),
`-pikachu-air` (3,360), `-ness-thunder` (1,732) and `-ness-magnet`
(1,544) with their specials' flashes, `f1-vs-cpu` (2,028, Mario white with
`DamageCommon`) and `f1-vs-team` (1,984, Mario dark red with the
shield-break flicker). Only fighter pixels changed; colour animations draw
no random numbers. The new viewer goldens `r2-mario-skeleton`,
`r2-samus-skeleton` and `r2-kirby-skeleton` (`skeleton NAME`) draw each
fighter's skeleton set 1 and were seeded from their first capture. The
other 100 are unchanged. 114 of 114 goldens match twice.

## 2026-09-29 RE-415 fighter effects

No pack change. Fighters now make their motion-script, colour-animation
and status effects, and hits their slash, orbs and sparks. 27 goldens were
rebaselined, identical in both captures: 21 Training scenes (for example
`f1-training-fireball`, 4,556 pixels at 2×, a sparkle at Mario's hand and
dust at his feet; `-captain`, 9,692, the Falcon Punch's dust and flames;
`-kirby`, 9,628, landing dust and a star; `-samus-shot`, 7,732, the dash
dust behind Samus), `f1-training-rebirthblast` (23,380), and the battles
`f1-vs-cpu`, `-pause`, `-four`, `-team`, `-results` with
`f1-training-cpu-walk` and `-cpu-jump`, which follow the effects' random
draws: a seed-isolated build matched `HEAD` isolated the same way in all
but `f1-vs-cpu`'s spark. The other 87 are unchanged. 114 of 114 match.

## 2026-09-29 RE-416 weapon effects, clashes and the quake

Pack v67, 30,491,216 bytes (SHA-256 `954ce580…`), with the quakes'
animations. Weapons now make their own effects and clash, and a quake
shakes the camera. Four goldens were rebaselined, identical in both
captures: `f1-training-yoshi-bomb` (292,304 pixels at 2×), `-captain`
(305,800) and `-kirby` (309,084), whose Bomb landing, Falcon Punch and
Final Cutter landing quakes now shift the view, and `-ness-thunder`
(1,320), where PK Thunder's trails draw their frames and a flash beside
Ness takes other random numbers. A build restoring the seed around every
weapon-effect flush, with the shake off, matched the old goldens in all
four; with the shake on, the three quake scenes matched this build's
captures exactly. No capture catches a weapon ending, so the new
particles show in none. The other 110 are unchanged. 114 of 114 match.

## 2026-09-30 RE-418 damage shield, egg shield and Fireball spin

Pack v69, 30,710,256 bytes (SHA-256 `a768f845…`), marking the egg list's
`(SHADE - ENV) * TEXEL0` combiner. Two goldens were rebaselined, identical
in both captures: `f1-training-fireball` (2,176 pixels at 2×) and
`f1-training-luigi` (1,712), whose Fireballs now spin in the screen plane
by `rotate_speed` per update (battle matrix function 0x47); only the
Fireball's pixels changed. `f1-training-yoshi-shield` and
`f1-vs-shield-damage` are new. The other 115 are unchanged. 119 of 119
match twice.

## 2026-09-30 RE-419 stage wallpapers and the stage select

Pack v70, 34,283,632 bytes (SHA-256 `51af4130…`), with the nine stage
wallpapers, Training's three (5551) and the stage select's sprites. 46
goldens were rebaselined, identical in both captures: every Training and
VS battle scene now draws its wallpaper behind the stage (Training's blue
on Dream Land, VS Dream Land's zoomed and panned with the camera and the
pause camera), from `f1-vs-pause` (63,204 pixels at 2×) to
`f1-training-rebirthblast` (273,888); `f1-training-stage-select` and
`-fighter-select` also pass through the new select. Four are new:
`f1-training-stage-select-view`, `-stage-select-yoshi`, `f1-vs-sector`
and `f1-vs-yoshi`. A build without the wallpaper draw or the preview's
hidden nodes changed 49 of the 54 `psp-game` goldens (the Yoshi's Island
select by the 320 pixels of an unhidden cloud). The four menu scenes and
the 69 viewer goldens are unchanged. 123 of 123 match twice.

## 2026-09-30 Training character select

Pack v71, 34,423,136 bytes (SHA-256 `daefb176…`), adds the Training
select's card (file 23, through its two LUTs) and "Training Mode". Two
goldens are new, identical in both captures: `f1-training-select` and
`f1-training-select-picked`. A build that drew the old plain slots changed
every pixel of both (522,240 at 2×); one without the CPU's
`nGMColAnimFighterComPlayer` blend changed 3,148 and 3,016, the CPU
fighter's. `f1-training-fighter-select` goes through the select but is
captured in battle, so it is unchanged, as are the other 122. 125 of 125
match twice, in two runs.

## 2026-09-30 RE-420 KO halo rays, quake, star-KO far plane; results emblem and confetti

Pack v72, 34,455,072 bytes (SHA-256 `527e07c5…`). Two goldens are new,
identical in both captures: `f1-training-starko` and
`f1-vs-results-emblem`. Thirteen were rebaselined:
- `r1-catch-swirl-flat-color` (27,980 pixels at 2×): the swirl keeps its
  texture.
- `f1-training-rebirth` (2,044): the halo's rays.
- `f1-training-rebirthblast` (185,644): the KO's quake.
- `f1-vs-results` (18,628): the emblem and confetti.
- Nine battle scenes by 4 to 76 pixels at depth ties: the far plane is now
  39,936.

The GE near plane stays 1. With `dGMCameraPerspDefault`'s 256 the
fighter drew over its shield bubble in `f1-training-shield`,
`-ness-magnet` and `f1-vs-shield-damage`, which an N64 capture does not
show (RE-420).

Negative tests:
- A build without the emblem, rays, quake and far plane changed
  `vsresultsemblem`, `vsresults`, `rebirth`, `rebirthblast` and `starko`.
- One without the confetti changed only `vsresults` (2,284).

127 of 127 match twice, in two runs.

## 2026-09-30 RE-421 shield bubble with no depth test; near plane 256

Pack v73, 34,455,072 bytes (SHA-256 `f07a105b…`). The battle projection
now uses `dGMCameraPerspDefault`'s near plane, 256. The shield bubble and
the other link-15/18 effect lists draw under `efDisplayCLDProcDisplay`'s
state, with no depth test. 49 goldens were rebaselined, identical in both
captures:
- `f1-training-purin` (2,160 pixels at 2×) and `f1-training-ness-magnet`
  (1,128): Sing's rings and the PSI Magnet field draw over the ground, as
  on the N64.
- `f1-training-shield` (552) and `f1-vs-shield-damage` (696): fighter
  self-occlusion; the bubble still covers the whole fighter, as on the
  N64.
- `f1-training-stage-select`, `-fighter-select` and `f1-vs-yoshi` (436,
  1,572, 2,252): stage-edge and foot depth ties.
- 42 other battle scenes (120 to 1,564): fighter self-occlusion and
  overlaps that near 1's coarse depth tied.

Negative test: a pack that seeds those lists with the camera's
Z-buffered mode changed `shield` (1,652), `vsshield` (2,420), `purin`
(2,464) and `nessmagnet` (2,188), with the fighter crisp over the bubble.
127 of 127 match twice.

## 2026-09-30 RE-422 battle draw order by display link and task head

Pack v74, 34,455,072 bytes (SHA-256 `2dbc2a68…`). The battle draws in the
original's display-link passes: Dream Land's front flowers and layer 3
after the fighters and the link-15 effects, layer 1's head-1 lists after
the fighters. Unlit `G_CC_SHADE` glows blend, and layers 0, 2 and 3's
head-1 lists have no depth test. The pause border is a true fill. 25
goldens were rebaselined, identical in both captures:
- 13 Training scenes on Dream Land (16 to 1,996 pixels at 2×): dust,
  sparks, the Fireball, Sing and the Falcon Kick behind the front flower
  beds; `f1-training-yoshi-bomb` (6,952) also draws the impact wave (link
  10) under the Bomb's stars (link 14).
- `f1-vs-time-up`, `f1-vs-four`, `f1-vs-team` (32 to 1,660): layer 3's
  fence and rim over a fighter.
- `f1-training-stage-select`, `-stage-select-view`, `r2-stage-hyrule-castle`
  (344 to 7,732): Hyrule Castle's ledge shadows blend.
- `r2-saffron-city-gate` (89,460), `r2-stage-zebes` and three bonus stages
  (1,140 to 2,128): the head-1 glows and Saffron City's haze blend.

Four goldens were added, `f1-training-jungle`, `-zebes`, `-saffron` and
`-inishie`: Training on those stages at tick 400, compared with the N64
warp-boot captures.

Negative tests: the old order changed the 17 draw-order goldens and
`jungle` (3,036) and `zebes` (1,684); the pack without the blend and seed
changed `zebes`, `saffron`, `stage-select` and `r2-saffron-city-gate`.
131 of 131 match twice.
