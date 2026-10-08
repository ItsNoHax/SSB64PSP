# Visual Regression

Deterministic golden captures of the PSP renderer, taken with
PPSSPPHeadless's software GPU ([D-041](../decisions/D-041.md)). A golden is a
pixel hash, not an image: [`tests/golden/hashes.tsv`](../../tests/golden/hashes.tsv)
records each scene's size and the SHA-256 of its decoded pixels. The captures
show the game's characters and stages, so no golden PNG is committed; local
copies live in the gitignored `tests/golden/local/`.

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
tools/golden.sh baseline [--filter REGEX] [-j N] [--no-build]
```

- [`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv) lists every
  golden: crate, scene spec, `pass` or `known-failing`, and evidence.
- The driver builds each crate once with `golden_capture`, then captures
  every selected scene from that EBOOT in parallel (default `nproc` jobs).
  The manifest has 198 scenes; a full run at `-j 16` captures
  in about 110 s.
- Comparison is exact. A capture matches when the SHA-256 of its decoded
  pixels (8-bit RGB, row-major from the top-left, no header;
  `tools/lib/pixel-hash.sh`) equals the recorded hash, which is the same
  test as a 0-pixel difference. Hashing pixels rather than PNG bytes keeps
  the result independent of the encoder. There is no tolerance: captures
  freeze every animator at a fixed tick and are pixel-identical run to run.
- Output goes to `target/golden-run/<timestamp>/`: `candidates/`, difference
  masks in `masks/`, `summary.tsv`, and `index.html`, a side-by-side review
  of golden, candidate and mask with changed scenes first. A hash only says
  that a capture differs; the pixel count and mask need the scene's local
  golden PNG in `tests/golden/local/` (used only when its own hash still
  matches the manifest). Without one the pixel count reads `?`.
- `verify` fails on a `pass` row whose hash differs, a row with no hash, a
  `known-failing` row that now matches (set it to `pass`), a failed capture,
  and with `--twice` on two captures of one scene that differ.
- Each `psp-game` capture logs its game thread's deepest stack use;
  `verify` also fails a scene past seven eighths of the stack
  (`stack-near-limit`) or with no stack line. PPSSPP does not enforce the
  stack bound a PSP does (RE-469). `tools/stack-check.sh` runs the same
  check on the 1P Game and opening scenes outside the manifest.
- `rebaseline` always captures twice. It writes only changed candidates'
  hashes into `hashes.tsv` (and their PNGs into `tests/golden/local/`) and
  prints a Markdown table (golden, pixel count, reason) for the commit
  message. It skips `known-failing` rows unless `--filter` is given. A new
  manifest row has no hash until its first rebaseline.
- `baseline` (re)creates the local PNGs: it captures the selected scenes and
  stores each capture whose hash equals the manifest's, without changing
  the manifest. To get masks for a failing change, check out a commit whose
  goldens pass (for example `git stash` or `git worktree add`), run
  `tools/golden.sh baseline`, then return and run `verify`.
- `tools/verify-fighter-goldens.sh` runs `verify --filter
  'fighter|link-costume'`.
- `tools/golden-reference.sh` captures every scene with the per-feature
  pipeline (one build and one 8 s run each, about 15 minutes) into
  `~/golden-reference/`. Use it as the byte-identity reference when changing
  the capture pipeline itself.
- Serve `index.html` over HTTP from the repository root (for example
  `python3 -m http.server`); it loads goldens from `tests/golden/local/`.

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

### Campaign diagnostic capture

`onepgame` scripts 1P select START, intro and the real Link battle (RE-450).
Tick 600 shows GO and the two-player HUD; `onepgame@65` checks the
select outline, `@275` the red countdown, and `@700`/`@1200` the live
timer. These are diagnostics; they are not in the golden manifest.
Two tick-600 captures are byte-identical. See
[RE-450](../evidence/re/RE-450.md) for verification limits.

RE-451 adds presentation fixtures after the same real select. At global
105 they seed the requested scene; these do not simulate a real win/loss.

| Spec | Captured display |
|---|---|
| `onepgame@145` | Real Link intro with authored fighter cards |
| `onepintro` (150) | Yoshi intro/team cards |
| `onepbonus` (150) | Break the Targets intro picture |
| `onepcontinue` (240) | Continue options and dropped figure |
| `onepretry` (320), `onepretry@470` | A at 270 accepts; stand-up clip, score halved |
| `onepcontinue@2600` | Automatic Game Over |
| `onepclear@150`, `onepclear` (270) | Timer/damage page, then bonus rows and dimmed snapshot |

Intro, Continue and stage-clear repeat captures are pixel-identical.
The stage-clear fixture copies the preceding intro frame. These remain
diagnostics outside the golden manifest; see
[RE-451](../evidence/re/RE-451.md) for source/ROM checks and limits.

RE-452 extends the seeded bonus intro into a real Break the Targets world.
The fixtures still bypass the first three campaign wins. Objective counts,
announcements and result handoffs run through the real scene controller.

| Spec | Captured display |
|---|---|
| `onepbonus@600` | Kirby's live course, ten objectives and the campaign's two-minute timer |
| `oneptargetclear@670`, `oneptargetclear` (850) | Mario's real jab breaks relocated targets; COMPLETE, then ten credited targets in RESULT |
| `oneptargetfall` (670), `oneptargetfall@750` | A seeded Fall below the blast zone; FAILURE, then zero credited targets |
| `onepbonus@7790`, `onepbonus@7860` | Unmodified timer reaches FAILURE, then zero credited targets |

The clear fixture stops target motion and offers one target at a time to
Mario's jab. It does not write counts or results. Campaign stock remains
two through all three handoffs. Repeated live-course native images are
pixel-identical. These diagnostics remain outside the golden manifest;
see [RE-452](../evidence/re/RE-452.md) for the remaining validation limits.

RE-454 adds Board the Platforms with ten real landing objectives. Its
fixtures seed the bonus intro after the real select and bypass preceding
campaign wins.

| Spec | Captured display |
|---|---|
| `onepplatforms` (600) | Kirby's live course, ten platform icons and timer 01:59 |
| `onepplatformclear@800` | Mario lands on ten real floors; COMPLETE |
| `onepplatformclear@1000`, `@1100` | Ten credited platforms in RESULT, then Perfect |
| `onepplatformfall@670`, `onepplatformfall` (750) | Seeded Fall below the blast zone; FAILURE, then zero credited platforms |
| `onepplatforms@7790` | Unmodified two-minute timer reaches FAILURE at 00:00 |

The clear fixture places Mario above each floor and lets normal swept
collision land him before the objective process credits it. It does not
write task counts or results. Repeated live-course native images are
pixel-identical. These diagnostics remain outside the golden manifest;
see [RE-454](../evidence/re/RE-454.md) for source/ROM checks and validation
limits.

RE-455 adds Race to the Finish. Fixtures seed its intro after the real
select and bypass preceding campaign wins.

| Spec | Captured display |
|---|---|
| `oneprace` (600) | Kirby, three Polygons, live course and timer 00:59; repeated native images match exactly |
| `onepraceclear@650`, `onepraceclear` (750), `@900` | Real DETECT landing gives COMPLETE, Timer RESULT, then No Damage and total 44,500 |
| `onepracefall` (750) | Real blast-zone fall consumes a stock and rebirths |
| `oneprace@4170`, `@4280` | Unmodified minute gives FAILURE, then zero-time RESULT |
| `onepracehazards` (660), `@800` | Human beneath the real dropper; smash pieces, then 26% damage |

Clear places the human above the actual DETECT segment and lets swept
collision land him. Fall and hazards move him to the authored blast/dropper
areas; fixtures never write completion, score, stock loss or damage.

RE-457 adds Master Hand's stage. Both fixtures seed stage 13's intro after
the real select (Kirby).

| Spec | Captured display |
|---|---|
| `onepboss@600` | Intro camera, Master Hand appearing, comets, boss stock icon and emblem; no magnifiers before "Go" |
| `onepboss@1200`, `onepboss` (1500) | Poke and finger gun on the idle Kirby, then his fall KO |
| `onepbossdefeat@1250`, `@1500`, `@2000` | Defeat zoom with broken H.P, closing effect and white fade, black fade |
| `onepbossdefeat@2250` | GAME CLEAR stage clear with the boss bonuses |

The defeat fixture writes Master Hand's damage to 300 at battle clock 701
and calls the real `ftBossCommonUpdateDamageStats`; everything after is the
port's own sequence. Captures live under `~/ppsspp-headless-test/re457-*`
and stay outside the manifest.

RE-458 adds the special fighters' stages. Each fixture seeds its stage's
intro after the real select (Kirby) and runs the manager's own stage setup
(Giant Donkey Kong's random allies).

| Spec | Captured display |
|---|---|
| `onepmetal@200`, `onepgiant@200`, `onepzako@200` | Intro cards: Metal Mario, Giant DK with both allies, the Polygons' card frames; every fighter under the banners |
| `onepmetal@1000` | Metal Mario on Meta Crystal's truss after his pipe entry, his emblem and stock icon |
| `onepgiant@780` | Giant DK on the tree platform, his two random allies after their entries |
| `onepzako@1000`, `@1700` | Three Polygons, their emblems and the 30-icon team stock; after fixture KOs, 23 left |
| `onepmetal@1700`, `onepgiant@1700` | STAGE CLEAR after the enemy's fixture KO |

From battle clock 1000 the fixture moves every enemy below the stage's
bottom bound every 150 ticks; the real blast check, replacement and
stage-clear logic do the rest. Captures live under
`~/ppsspp-headless-test/re458-*`.

RE-459 adds the campaign's last scenes. Fixtures seed each scene after the
real select (Kirby), with the stage stepped past Master Hand where the
manager expects it.

| Spec | Captured display |
|---|---|
| `onepending@200`, `onepending` (400) | The figure dropped on the desk; the room props and window as the light rises |
| `onepstaffroll@400`, `onepstaffroll` (900) | Director and Chief Programmer rolling; A every 20 ticks highlights a name with its role (C. Falcon hidden) and company |
| `onepcongra` (200), `onepchallenger` (150), `onepmessage` (150) | Kirby's picture; Ness's silhouette under WARNING; Ness's unlock message |
| `onepfinale@1520`, `@1700`, `onepfinale` (2200) | One run from the ending through the fast staff roll and congratulations into Ness's challenge and battle |

Captures live under `~/ppsspp-headless-test/re459/`, outside the manifest.

Two save scenes (RE-460) are the only captures that load or write the
memory-stick backup; every other scene boots with the defaults. Run them in
order in one `--job` directory and delete its `ssb64.sav` afterwards:

| Spec | Captured display |
|---|---|
| `saveunlock` (260) | `onepmessage` closed with A at tick 240, writing Ness's unlock; the Intro after it |
| `saveplayers` (85) | `vsplayers` after loading that save: Ness's portrait instead of "?" |

The options and data menus (RE-461) start in their scene (the last two on
Data, so the capture loads one scene's files after another, RE-475) with
the default backup and neutral sticks:

| Spec | Captured display |
|---|---|
| `option` (40) | Option on Sound, stereo underlined |
| `screenadjust` (30) | Screen Adjust's guide, instruction and frame |
| `backupclear` (50) | Down to VS Record, A: "Is it okay to clear this data?" with No circled |
| `datamenu` (40) | Data on Characters, without Sound Test |
| `vsrecord` (40) | From Data (down, A) into VS Record, A at tick 30: the Ranking page, Mario highlighted |
| `characters` (90) | From Data (A at tick 15): Mario's page; the fighter's motion comes from a fixed byte sequence, not the clock |

The first five match warp-booted N64 references of the same states to
within edge resampling (`~/ppsspp-test/n64menus/`).

The front end's scenes (RE-462) start the same way, from the scene before
them; `bonusselect` and `bonuspractice` stick the hand up from tick 12 to
32 and place the puck with A at 40:

| Spec | Captured display |
|---|---|
| `title` (140) | The title at rest, its fire colour from a fixed byte sequence; "Press Start" in its hidden half |
| `modeselect` (40) | Down at 15: VS Mode lit, the labels drawn before the options |
| `onepmode` (40) | Down at 15 and 30: Bonus 1 Practice highlighted |
| `vsoptions` (40) | Down three rows, right twice: Damage 102 % highlighted |
| `itemswitch` (40) | Down, A: the Beam Sword off |
| `bonusselect` (70) | Yoshi placed on Bonus 1's select, his targets record and "Press Start" |
| `bonuspractice` (300) | START at 50: Yoshi's Break the Targets with the practice timer |

`title`, `modeselect`, `vsoptions`, `onepmode` and `bonusselect` overlay
N64 references reached from the N64 logo with START
(`~/ppsspp-test/n64front/`), as does `vsmode` (whose golden
`f1-vs-mode-menu` now shows the menu's sprites).

These diagnostics remain outside the golden manifest. They do not prove
unmodified traversal, a full campaign, N64 equivalence or physical PSP;
see [RE-455](../evidence/re/RE-455.md) for source checks and capture limits.

### Single captures

```bash
tools/run-ppsspp-headless.sh --scene 'stage 17' [--job NAME] [--no-build]
tools/run-ppsspp-headless.sh [--crate psp-game] --feature <feature>
tools/compare-screenshot.sh tests/golden/local/<golden>.png ~/ppsspp-headless-test/screenshot.png
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
  It and `golden.sh` share `tools/lib/pixel-diff.sh`; `golden.sh` and
  `golden-reference.sh` share `tools/lib/pixel-hash.sh`.
- Capture features must not ship in interactive builds; rebuild without them
  afterwards.

## Rules

- Explain every golden change (pixel count, cause) before accepting it.
- Never commit a capture, golden PNG or other screenshot of the game; commit
  hashes only (`tools/docs/validate_docs.py` rejects tracked images outside
  the original XMB artwork).
- Confirm a new golden is deterministic with two captures.
- PPSSPP software is the only exact tier. PPSSPP hardware backends and
  physical PSP captures are separate, qualitative tiers.
- Never pass a camera photo of a PSP screen to the pixel comparator.

## Scenes

`psp-asset-viewer` unless noted. The scene spec for each golden is in
[`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv); the feature
builds the same scene as its default. The `r0-`, `r1-`, `r2-` and `f1-`
name prefixes are historical and carry no meaning.

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
| `golden_capture` (`psp-game`, scene `trainingcpuitem`) | `f1-training-cpu-item` | A level-9 Training CPU on the default behaviour at tick 200, holding the Bat it tracked and picked up (TrackItem) | RE-437 |
| `golden_capture` (`psp-game`, scene `trainingmenu`) | `f1-training-menu` | Training's menu, open from tick 20 to 34, sets the CPU to Walk and drops a Maxim Tomato above Mario; at tick 150 the CPU has walked onto the left platform. The closed-menu stat interface draws | RE-438, RE-439 |
| `golden_capture` (`psp-game`, scene `trainingmenu@30`) | `f1-training-menu-open` | Open Training menu on Item/Maxim Tomato with CP Walk, Normal view, panel, border, cursor, arrows and underline; battle HUD and stats hidden | RE-439 |
| `golden_capture` (`psp-game`, scene `traininginterface@50/67/75/100/280`) | `f1-training-interface-normal/menu/close/wait/restored` | Tags and off-screen miniatures, open-menu visibility, Close-Up, Normal's delay and restoration | RE-440 |
| `golden_capture` (`psp-game`, scene `vs`) | `f1-vs-countdown` | A VS battle's countdown on Dream Land, both fighters locked until "Go" | RE-389 |
| `golden_capture` (`psp-game`, scene `vstimeup`) | `f1-vs-sudden-death` | A one-minute VS battle timed out in a tie, in sudden death with both fighters at 300% | RE-389 |
| `golden_capture` (`psp-game`, scene `vscpu`) | `f1-vs-cpu` | A VS battle's CPU landing a hit on the idle player at tick 900 (7%); `vstimeup` pins its CPU to Stand to keep its tie | RE-391, RE-438 |
| `golden_capture` (`psp-game`, scene `vstimeupsign`) | `f1-vs-time-up` | `vstimeup` at tick 4,040: 00:00 and "TIME UP" during the end wait | RE-395 |
| `golden_capture` (`psp-game`, scene `vssuddendeath`) | `f1-vs-sudden-death-sign` | `vstimeup` at tick 4,150: "SUDDEN DEATH!" before its "GO!", emblems and stock icons without digits | RE-397 |
| `golden_capture` (`psp-game`, scene `vspause`) | `f1-vs-pause` | START at tick 500: the pause menu's zoom on Mario, border, "1P PAUSE" and decals at 560 | RE-398 |
| `golden_capture` (`psp-game`, scene `vsmode`) | `f1-vs-mode-menu` | The VS mode menu's sprites after Rule → Stock and one more stock | RE-399, RE-462 |
| `golden_capture` (`psp-game`, scene `vsnocontest`) | `f1-vs-no-contest` | A VS battle reset from the pause menu, at results tic 117 (tick 640): the random blue wallpaper, the KOs and TKO rows, both Marios one row back clapping under their tags, and "NO CONTEST" | RE-400, RE-409, RE-410 |
| `golden_capture` (`psp-game`, scene `vsresults`) | `f1-vs-results` | A one-stock battle Luigi loses by running off Dream Land, at tick 1100: Kirby in front holding his last Win frame, Luigi behind turned to him and clapping, over the blue wallpaper, with the tags, the Place and KOs rows, the bar, the header and "KIRBY WINS!" | RE-409, RE-410 |
| `golden_capture` (`psp-game`, scene `vsplayers`) | `f1-vs-players` | The VS character select at select tic 65: Yoshi placed on port 1's red card, a CPU (Donkey Kong, in his Win1 clip) opened in port 2 with its CP level, the NA doors shut, the stone wallpaper, portraits, pucks, hand and the "Ready to fight" banner | RE-404, RE-411 |
| `golden_capture` (`psp-game`, scene `vs4`) | `f1-vs-four` | Mario against three CPUs (Fox, Donkey Kong, Kirby) on Dream Land at tick 870, four damage displays | RE-405 |
| `golden_capture` (`psp-game`, scene `vsteam`) | `f1-vs-team` | A team battle on Dream Land at tick 940: Mario and Kirby (red) against Fox and Donkey Kong (blue) in team costumes and emblem colours | RE-407, RE-438 |
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
| `golden_capture` (`psp-game`, scene `yoshiegg`) | `f1-training-yoshi-egg` | Yoshi lands beside the dummy, backs off, turns and tongues it at tick 330; at 460 the laid egg rests on the top platform in its Wait wobble | RE-417, RE-432 |
| `golden_capture` (`psp-game`, scene `yoshishield`) | `f1-training-yoshi-shield` | Yoshi holds Z from tick 40 to 600: his model is hidden inside the egg shield, darkened by the worn shield (health 21, ENV (107, 132, 132)): the egg's white texels (231) draw (134, 111, 111) and its green spots (57, 214, 57) draw (33, 103, 27) | RE-418 |
| `golden_capture` (`psp-game`, scene `vsshield`) | `f1-vs-shield-damage` | A VS battle whose player shields from "Go"; frozen after tick 884's update, where the CPU's hit sets the shield off and the bubble draws the grey damage row | RE-418, RE-438 |
| `golden_capture` (`psp-game`, scene `stageselectview`) | `f1-training-stage-select-view` | Training's stage select left on Hyrule Castle at tick 60: the stone, icons, red cursor, plaque, name plate and emblem, and the castle's preview model over its blue Training wallpaper | RE-419 |
| `golden_capture` (`psp-game`, scene `stageselectyoshi`) | `f1-training-stage-select-yoshi` | The same select moved down to Yoshi's Island: the yellow Training wallpaper, and the preview hiding the two cloud nodes `mnMapsMakeModel` hides | RE-419 |
| `golden_capture` (`psp-game`, scene `vssector`) | `f1-vs-sector` | `vs`'s countdown on Sector Z: the wallpaper scaled about its centre by the camera's distance | RE-419 |
| `golden_capture` (`psp-game`, scene `trainingarwing`) | `f1-training-arwing` | Training on Sector Z at tick 1800: the Arwing's first pattern passing low, its wing in the top right | RE-428 |
| `golden_capture` (`psp-game`, scene `vsyoshi`) | `f1-vs-yoshi` | `vs`'s countdown on Yoshi's Island: the static wallpaper | RE-419 |
| `golden_capture` (`psp-game`, scene `trainingselect`) | `f1-training-select` | The Training character select at its tick 60: the stone, "Training Mode", BACK, the portraits with the four locked shadows, the red and grey cards, the hand on the player's card, and the CPU's Mario turning on his card under his name, emblem and puck | — |
| `golden_capture` (`psp-game`, scene `trainingselectpicked`) | `f1-training-select-picked` | The same select after Kirby is placed in his C-Down (cyan) costume and the CPU's puck is picked up and placed again in Mario's C-Right costume: both fighters in their Win3 clips, the "Ready to fight" banner and "Press Start" | — |
| `golden_capture` (`psp-game`, scene `starko`) | `f1-training-starko` | `rebirth`'s Mario put in `DeadUpStar` on Dream Land's floor at tick 60, 150 ticks into the flight: a few pixels over the treetop, past the old 10,000-unit far plane | RE-420 |
| `golden_capture` (`psp-game`, scene `vsresultsemblem`) | `f1-vs-results-emblem` | `vsresults` at results tic 100: the winner's (Kirby, port 2) blue series emblem shrinking and rising over the fading wallpaper, before the text and confetti | RE-420 |
| `golden_capture` (`psp-game`, scene `shield`) | `f1-training-shield` | Mario's tilted Guard pose on Dream Land; `PPSSPPHeadless --log` confirms the raised shield and posed `YRotN` collision center, and the red player-1 shield bubble around him | RE-367, RE-369, RE-384 |
| `golden_capture` (`psp-game`, scene `trainingpokemonc@60`) | `f1-training-pokemon-c` | Onix rising and Beedrill's animated wings | RE-443 |
| `golden_capture` (`psp-game`, scene `trainingpokemonc@210`) | `f1-training-pokemon-fall` | Snorlax's enlarged fall on link 18 under its translucent callback state | RE-443 |
| `golden_capture` (`psp-game`, scene `trainingpokemonc@240`) | `f1-training-pokemon-rocks` | Onix's textured rock shower and Beedrill swarm | RE-443 |
| `golden_capture` (`psp-game`, scene `trainingpokemond@120`) | `f1-training-pokemon-d` | Goldeen's material frames, Hitmonlee and Clefairy | RE-443 |

Stage sweep example:

```bash
tools/golden.sh verify --filter '^r2-stage-'
```

Particle and effect audits (RE-180–189) are smoke runs, not goldens; see
[Audits](#audits).

### Known failing goldens

None. A golden that is known to differ from current output gets status
`known-failing` in [`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv)
until it is explained and rebaselined.

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
