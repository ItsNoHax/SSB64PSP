# Visual Regression

Deterministic golden captures of the PSP renderer, PPSSPPHeadless software GPU ([D-041](../decisions/D-041.md)).

- Golden: pixel hash, not image. [`tests/golden/hashes.tsv`](../../tests/golden/hashes.tsv) holds scene size + SHA-256 of decoded pixels.
- No golden PNG committed (game characters and stages). Local copies: gitignored `tests/golden/local/`.
- Pins current PSP output only: not N64 equivalence, not physical-PSP behavior.

## Setup

```bash
cd ~/.local/src/ppsspp
cmake -DHEADLESS=ON -DCMAKE_BUILD_TYPE=Release -B build-headless
cmake --build build-headless --target PPSSPPHeadless
```

- Binary elsewhere: set `PPSSPP_HEADLESS_BIN`.
- Required tools: `ffmpeg`, ImageMagick (`magick`).

## Verify and rebaseline

```bash
tools/golden.sh verify [--filter REGEX] [-j N] [--twice] [--no-build]
tools/golden.sh rebaseline [--filter REGEX] [-j N] --reason TEXT
tools/golden.sh baseline [--filter REGEX] [-j N] [--no-build]
```

Driver
- [`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv): every golden (crate, scene spec, `pass`/`known-failing`, evidence).
- Builds each crate once with `golden_capture`; captures selected scenes from that EBOOT in parallel (default `nproc` jobs).

Comparison
- Exact, no tolerance: SHA-256 of decoded pixels equals recorded hash (= 0-pixel difference).
- Pixel format: 8-bit RGB, row-major from top-left, no header (`tools/lib/pixel-hash.sh`).
- why: pixels, not PNG bytes → independent of encoder.
- why no tolerance: captures freeze every animator at a fixed tick; pixel-identical run to run.

Output: `target/golden-run/<timestamp>/`

| Path | Content |
|---|---|
| `candidates/` | Captures |
| `masks/` | Difference masks |
| `summary.tsv` | Per-scene result |
| `index.html` | Side-by-side golden, candidate, mask; changed scenes first |
| `stack/<golden>-<pass>.holds` | `scene hold tick=N` lines: ticks that repeat the previous picture (scene exit); capture tick = hold tick means a held picture |

- Pixel count and mask need the scene's local PNG in `tests/golden/local/`, used only if its hash still matches the manifest.
- No local PNG → pixel count reads `?`.
- View `index.html`: serve over HTTP from repo root (`python3 -m http.server`); loads goldens from `tests/golden/local/`.

`verify` fails on
- `pass` row with differing hash
- row with no hash
- `known-failing` row that now matches (set to `pass`)
- failed capture
- `--twice`: two captures of one scene differ
- stack: past 7/8 of game-thread stack (`stack-near-limit`) or no stack line

Stack check
- Each `psp-game` capture logs its game thread's deepest stack use.
- PPSSPP does not enforce the PSP stack bound (RE-469).
- `tools/stack-check.sh`: same check on 1P Game and opening scenes, outside the manifest.

`rebaseline`
- Captures twice, always.
- Writes only changed candidates' hashes to `hashes.tsv` and PNGs to `tests/golden/local/`.
- Prints Markdown table (golden, pixel count, reason) for the commit message.
- Skips `known-failing` rows unless `--filter` given.
- New manifest row: no hash until first rebaseline.

`baseline`
- (Re)creates local PNGs: captures selected scenes, stores each whose hash equals the manifest's; manifest unchanged.
- Masks for a failing change:
  1. Check out a commit whose goldens pass (`git stash` or `git worktree add`).
  2. `tools/golden.sh baseline`.
  3. Return; run `verify`.

Helpers

| Tool | Does |
|---|---|
| `tools/verify-fighter-goldens.sh` | `verify --filter 'fighter\|link-costume'` |
| `tools/golden-reference.sh` | All scenes via per-feature pipeline (one build, one 8 s run each, ~15 min) into `~/golden-reference/`; byte-identity reference when changing the capture pipeline |

## Scene manifest

Source of which goldens exist: [`tests/golden/scenes.tsv`](../../tests/golden/scenes.tsv). This file states no count. Tab-separated columns:

| Column | Meaning |
|---|---|
| `golden` | Name; key into `hashes.tsv` and `tests/golden/local/<golden>.png` |
| `crate` | `psp-asset-viewer` or `psp-game` (the binary that renders the scene) |
| `scene_spec` | Line written to `capture_scene.txt` |
| `status` | `pass`, or `known-failing` (differs from current output, not yet explained) |
| `evidence` | RE records for the scene |

- `known-failing` row that starts matching fails `verify`: set to `pass`.
- Name prefixes carry no meaning except:
  - `r2-stage-N`: stage index N
  - `f1-training-*`, `f1-vs-*`, `f1-opening-*`, `f1-cpufx-*`: `psp-game` Training, VS, opening, CPU-effect scenes
- Look up a scene by name or `evidence` column.

## Scene specs

`golden_capture` EBOOT
- Reads first non-empty, non-comment line of `capture_scene.txt` beside it.
- Limit: 64 bytes (`SCENE_FILE`, `MAX_SPEC_LEN` in `crates/ssb-capture/src/lib.rs`).
- Freezes at scene tick (default 240), requests screenshot, exits two frames later.
- No file → Dream Land default.
- Interactive and physical-PSP builds read no scene file unless built with `capture_scene_file`.

| Binary | Spec grammar (parsers in `ssb-capture`) |
|---|---|
| `psp-asset-viewer` | `ViewerScene::parse`: `default`, `scene2`..`scene10`, `stage N`, `fighter NAME`, `skeleton NAME`, `link_costume_1`, `mario_entry`, `bonus_platform`, `depth_mask`, `dream_land_water` |
| `psp-game` | `GameSpec::parse`: `NAME` or `NAME@TICK`; `NAME` is a `GameScene` (`GameScene::ALL`, `name()`), `TICK` a positive decimal that overrides the capture tick |

- Fighter names: `Fighter::name` (`mario`, `donkey_kong`, `captain_falcon`, `metal_mario`, ...).
- Scripted pad input and capture tick per scene: `crates/ssb-capture/src/script.rs`.
- Scene behaviour (object, stage, light): in each binary, not `ssb-capture`.
- `scene@tick` also serves N64 frame comparisons (RE-425).

## Campaign diagnostic capture

- `onepgame`: scripts 1P select START, intro, real Link battle (RE-450).
- Other `onep*` specs: run the same real select, then seed their scene at a fixed global tick.
  - `onepintro`, `onepbonus`, `onepcontinue`, `onepretry`, `onepclear`
  - Break the Targets, Board the Platforms, Race to the Finish fixtures
  - `onepboss*`, `onepmetal`, `onepgiant`, `onepzako`
  - `onepending`, `onepstaffroll`, `onepcongra`, `onepchallenger`, `onepmessage`, `onepfinale`
- Fixtures simulate no preceding campaign wins and write no results.
- Objective, result and handoff logic after the seed: the port's own.
- Moment: `@TICK`; without `@`, scene default (`script.rs` `capture_tick`).
  - `onepgame@65`: select outline
  - `onepgame@275`: red countdown
  - `onepgame@600`: GO and two-player HUD
- Repeated captures of one spec are pixel-identical.
- In the manifest only if `scenes.tsv` lists it.
- Save scenes (RE-460): `saveunlock` then `saveplayers`.
  - Only captures that load or write the memory-stick backup.
  - Run in order in one `--job` directory; delete its `ssb64.sav` afterwards.
- Front-end and menu scenes (`title`, `modeselect`, `option`, `datamenu`, `characters`, `soundtest`, ...):
  - Start from the preceding scene with scripted pad input.
  - A few overlay N64 references taken from the N64 logo with START.
  - Detail: RE records cited in `scenes.tsv`, [RE-450](../evidence/re/RE-450.md) to [RE-462](../evidence/re/RE-462.md).
- Not proof of: unmodified campaign traversal, N64 equivalence, physical-PSP behavior.
- Ad-hoc run captures: `~/ppsspp-headless-test/`, outside Git.

## Determinism traps

- Every animator advances per simulation tick at a frozen capture tick → no dependence on load timing or pack size (RE-315).
- `r2-mario-entry-pipe`: hashes vary with zero differing RGB pixels after a pack change (RE-506; open item in `PLAN.md`).
  - When it flips: `verify --twice`, compare pixels, not only hashes.
- Goldens need the real ROM-generated tables, not `SSB64_STUB_TABLES`.

### Single captures

```bash
tools/run-ppsspp-headless.sh --scene 'stage 17' [--job NAME] [--no-build]
tools/run-ppsspp-headless.sh [--crate psp-game] --feature <feature>
tools/compare-screenshot.sh tests/golden/local/<golden>.png ~/ppsspp-headless-test/screenshot.png
```

Runner
- Builds EBOOT: `golden_capture` with `--scene`, else `<feature>,headless_capture`.
- Stages EBOOT and pack under PPSSPP's memstick (required for `MEMSIZE`).
- Writes 960×544 PNG to `$PPSSPP_HEADLESS_TEST_DIR` (default `~/ppsspp-headless-test`).
- `--scene`: timeout defaults to 30 s and is a failure.
  - PPSSPPHeadless exits 0 on timeout too; runner reads the `TIMEOUT` line in its log.
- `--job NAME`: stages into `PSP/GAME/ssb64_regression_<NAME>`, writes to `$PPSSPP_HEADLESS_TEST_DIR/<NAME>`.
  - Parallel runs share no game directory or `no-status-overlay.ini`.
  - PPSSPPHeadless memstick is `$HOME/.ppsspp`; it saves no `ppsspp.ini` → no other shared state.
  - `golden.sh` deletes each job's game directory afterwards.
- Pack: hard-linked into `--job` directories, copied into shared ones.
  - why: a hard link there would let an older checkout's `cp -f` overwrite it.
- Capture features freeze all simulation and animation at a fixed tick, hide the debug HUD (no load-timing or pack-size dependence, RE-315).
- `--pack PATH`: stages another pack (A/B captures); also writes native 480×272 `screenshot-native.png`.
  - `tools/residual-ab-capture.sh` uses both (RE-312 A/B review).
- `compare-screenshot.sh`: exact match default (0 differing pixels).
- Shared libs: `tools/lib/pixel-diff.sh` (`compare-screenshot.sh`, `golden.sh`); `tools/lib/pixel-hash.sh` (`golden.sh`, `golden-reference.sh`).
- Capture features must not ship in interactive builds; rebuild without them afterwards.

## Rules

- Explain every golden change (pixel count, cause) before accepting it.
- Never commit a capture, golden PNG or game screenshot; commit hashes only.
  - `tools/docs/validate_docs.py` rejects tracked images outside the original XMB artwork.
- New golden: confirm deterministic with two captures.
- Renderer-path change moves PPSSPP hashes only under D-051 (RE-537):
  - hardware `frame_hash` comparison proves PSP output identical
  - new hashes stable over ≥3 runs including `-j 1`
  - changed rows listed with pixel counts
- PPSSPP software: only exact tier. PPSSPP hardware backends, physical PSP captures: separate, qualitative tiers.
- Never pass a camera photo of a PSP screen to the pixel comparator.
- Differing, unexplained golden → status `known-failing` in `scenes.tsv`.
- Rebaselined goldens record current output, not verified correctness.
- Stage sweep: `tools/golden.sh verify --filter '^r2-stage-'`

## Audits

- Broad PPSSPP smoke audits, windowed runner.
- Check every item renders, not exact pixels.
- Output outside Git under `$PPSSPP_TEST_DIR`.

| Command | Scope | Evidence |
|---|---|---|
| `tools/run-ppsspp.sh --no-build --seconds 3 --audit-stages 41` | All 41 stages | RE-170 |
| `tools/run-ppsspp.sh --seconds 2 --audit-animations 532` | All 532 fighter animations | RE-171 |
| `--audit-effects`, `--audit-effect-animations`, `--audit-effect-materials`, `--audit-particles` | Effects and particles | RE-173–189 |

## Physical PSP

1. Mount PSP over USB; run `tools/stage-psp-regression.sh [--golden NAME] /path/to/psp-mount [rom]` (default `r0-dream-land-default`).
   - Rebuilds pack.
   - Builds manifest row's crate with `capture_scene_file`.
   - Installs EBOOT, pack, `capture_scene.txt` under `PSP/GAME/ssb64/`.
   - Writes `regression-manifest.txt` (commit, build, golden, scene, hashes).
2. Launch; wait past freeze tick; capture (PSPLink `scrshot`, or square-on photo with locked focus and exposure).
3. Record model, firmware, commit, manifest hashes, capture method, observations.

- PSPLink: `psp-hardware` skill.
- Hardware captures are qualitative, never exact evidence: RE-320 (PSP-2000 Slim, firmware 6.61), RE-341 (`psp-game` scenes).
- Viewer goldens: not recaptured on hardware.

## Original game

- RE-151: original ROM via Mupen64Plus, frame-indexed inputs, camera state compared numerically from RDRAM.
- See `n64-emulator` skill.
