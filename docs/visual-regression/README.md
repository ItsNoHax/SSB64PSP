# Visual Regression Methodology

Deterministic visual-regression methodology (`PLAN.md` R0.17). Per-scene procedures live under `docs/visual-regression/scenes/`; load only the scene you need.

This is the deterministic, repeatable visual-regression procedure `PLAN.md`
R0.17 requires, superseding `TODO.md` Phase H's "Reference renderer" /
"Screenshot regression" items. Screenshots taken ad hoc during individual
`RE-` investigations remain valid evidence for the specific claims they were
taken for, but they are not a substitute for this: a fixed scene that can be
re-captured and diffed automatically as the renderer changes. The automated
capture runner is PPSSPPHeadless; the windowed `tools/run-ppsspp.sh` helper is
reserved for interactive inspection and hardware-backend experiments.

## PPSSPPHeadless setup

Build the headless target from the local PPSSPP checkout:

```
cd ~/.local/src/ppsspp
cmake -DHEADLESS=ON -DCMAKE_BUILD_TYPE=Release -B build-headless
cmake --build build-headless --target PPSSPPHeadless
```

If the checkout or build directory is elsewhere, set `PPSSPP_HEADLESS_BIN` to
the resulting executable. The project wrapper defaults to
`~/.local/src/ppsspp/build-headless/PPSSPPHeadless` and uses the software GPU
backend for deterministic output. It captures through PPSSPP's emulator
`sceIoDevctl` hook after the PSP scene reaches its frozen tick; real PSPs
ignore that emulator-only request.

## Texgen and renderer-corrective matrix

Scenes 6–9 prove the current PSP lowering is deterministic and responsive;
they do not by themselves prove original-N64 equivalence. `PLAN.md`
R2.1/T1–T10's source, ROM, PPSSPP, original-ROM and physical-PSP matrix is
complete (RE-225–239); the remaining cross-node reuse item is tracked as a
non-blocking follow-up rather than reopened T1–T10 work.
Record PSP model, firmware, commit, pack hash, EBOOT identity, scene and
capture hash for each hardware run; regenerate any golden after a semantic
change.

`PLAN.md` R2.2/C1–C7 is complete (RE-240–261). The 22-scene suite covers
primitive colour ownership, load-time lighting provenance, independent depth
writes, adjacent-only primitive merging, PSP GE cache invalidation and fighter
costume-light propagation. Every semantic golden change was explained before
acceptance; RE-264 records the subsequent focused fighter-light and Ness-face
refresh while leaving the corrective gate closed.

## Capture procedure

### 1. PPSSPPHeadless software rendering (current golden source)

```
tools/run-ppsspp-headless.sh --feature regression_capture
```

The wrapper builds the EBOOT, stages the pack, runs PPSSPPHeadless, and writes
`$PPSSPP_HEADLESS_TEST_DIR/screenshot.png` (default
`~/ppsspp-headless-test/screenshot.png`). `--seconds` is only a safety timeout;
the capture itself is requested at the frozen tick, so it is not tied to host
window timing. New headless captures are the visual reference from this point
forward. The committed 960x544 images predate this switch and contain the
windowed PPSSPP FPS overlay, so an exact pixel comparison against those legacy
images includes that expected presentation difference. Compare when working
against a matching headless golden with:

```
tools/compare-screenshot.sh tests/golden/r0-dream-land-default.png ~/ppsspp-headless-test/screenshot.png
```

Exits 0 and prints `PASS` on a match; nonzero and the differing-pixel count
otherwise. The threshold defaults to 0 (exact match) because the scene is
measured byte-identical run to run — see Evidence.

**Rebuilding without the feature.** `regression_capture` is off by default
and must not be left enabled for normal interactive debug-viewer use (it
would freeze the fighter and hide the live perf counters after 4 seconds of
any session). Always follow a regression-capture run with a plain `cargo psp
--release` before resuming normal work. The wrapper deliberately leaves the
deterministic feature out of ordinary builds.

### 2. PPSSPP hardware rendering (documented, not yet executed)

Same procedure with `tools/run-ppsspp.sh --no-build --backend opengl
--seconds 6`. Not yet run as part of this task — the software-vs-hardware
comparison is meaningful once there is a second golden image to diff
against, which is future work, not a blocker for this task's "at least one
deterministic scene" acceptance item.

### 3. Physical PSP hardware (documented, not yet executed)

Connect the PSP in USB mode and mount its Memory Stick. Stage a freshly rebuilt
pack and deterministic EBOOT with:

```
tools/stage-psp-regression.sh /path/to/psp-mount
```

The script refuses a target without `PSP/GAME`, rebuilds the pack from the
local verified ROM, deletes the prior EBOOT before building with
`regression_capture`, and installs both files under
`PSP/GAME/ssb64/`. It also writes `regression-manifest.txt` with the Git
commit, build mode, UTC time, and SHA-256 hashes, preventing a stale binary or
pack from being mistaken for current evidence. An alternate legally obtained
ROM path may be supplied as the second argument.

Safely eject the PSP, launch SSB64PSP, wait at least 5 seconds (the scene
freezes at tick 240), and photograph or directly capture the screen. Frame the
entire LCD square-on with focus and exposure locked if using a camera; retain
the uncropped original. Record the PSP model, firmware or CFW, Memory Stick,
commit and the two hashes from the manifest, capture method, and observed FPS
or failures. A camera photograph is qualitative hardware evidence and must not
be passed to the exact PPSSPP pixel comparator; a direct 480x272 digital
capture may be compared only after documenting any capture-device scaling or
colour conversion.

This step has not yet been executed; this project's existing
device-verification precedent (e.g. RE-098, RE-114) is the model to follow.

### 4. Original SSB64 (repeatable camera comparison executed)

RE-151 drives the identified original ROM through Mupen64Plus's public core API
with frame-indexed inputs and screenshots. The sequence enters VS Mode, selects
Mario and Pikachu, selects Dream Land, and holds the stable match state. The
PSP audit build supplies the same positions, facings, camera offsets, fighter
count and settled Wait zoom. Direct original-RDRAM reads provide a numerical
oracle for `target_dist`, `at`, `eye`, and `fovy`; a 600×450 normalized
side-by-side then checks projected landmarks. The reproducible harness and
copyrighted screenshots remain outside Git under
`/home/alberto/ppsspp-test/re151/` and `/tmp/n64-camera-audit.*`.

## Test matrix

Each row names a concrete asset or display list, not a hypothetical example.
"Covered by golden scene" means one committed deterministic capture exercises
it. Source-classified primitive inventories identify the exact graph behind
scenes 3 and 4; this does not turn current PSP output into an original-N64
pixel oracle.

| Category | Concrete asset | Covered by golden scene? |
|---|---|---|
| Textured geometry | Dream Land's stage geometry, file 104 | Yes |
| CI4 texture | Dream Land's ground texture, file 103 `+0x1BE0`, 32×32 CI4 (`docs/reverse-engineering.md`, RE-046) | Yes |
| Palette / CLUT | Same CI4 ground texture's palette load | Yes |
| Mirror wrap mode | Dream Land's canopy, `G_TX_MIRROR` on both axes, file 104 offset `0xE20` (`mirror_s=true, mirror_t=true`) and offset `0x5F0` (`mirror_s=true, mirror_t=false`) (RE-067) | Yes |
| Lighting | Dream Land's platform/canopy shading plus Link's costume `LIGHT1COLOR`/`LIGHT2COLOR` under the runtime fighter-light path (RE-164–167, RE-240–243, RE-261) | Yes — scenes 1 and 16 |
| `combiner_shade_scale` shape | Dream Land's lit, unlit-texture primitives (RE-073); exact per-primitive attribution not isolated in this task | Likely, unconfirmed |
| Depth testing | Dream Land's canopy occluding the platform behind it; independent compare/write state wired (RE-251) | Yes — plus `tests/golden/r2-depth-mask-diagnostic.png`'s dedicated ON→OFF→ON `sceGuDepthMask` regression |
| Back-face culling | Dream Land's stage geometry (`cull_back` default for non-object-view) | Yes |
| Fighter model + skeleton | All 12 playable fighters, each in its high-detail `FTCommonPart` graph and source `Wait` pose with runtime fighter lighting (RE-265) | Yes — 12 `tests/golden/r2-*-fighter.png` images |
| CI8 texture | RE-198: file 52 (`mvopeningroom.c`'s opening-movie scene), texel data offset `0x2ee8`, 16×32 — one of 75 CI8-bound primitives archive-wide | Yes — RE-199's second scene, `tests/golden/r1-mvopeningroom.png` |
| `combiner_texture_blend` shape | RE-200: file 109 (`StageSectorFile2`) graph `0x44C8`, 7 converted primitives | Yes — scene 3, `tests/golden/r1-stage-sector.png` |
| `combiner_flat_color` shape | RE-200: file 84 (`EFCommonEffects2`) graph `0x2760` (`CatchSwirlDObjDesc`), 4 converted primitives | Yes — scene 4, `tests/golden/r1-catch-swirl-flat-color.png` |
| Transparency / translucency | RE-200: file 109 graph `0x44C8`, 5 primitives carrying both `TRANSLUCENT` and a classified alpha formula | Yes — scene 3, `tests/golden/r1-stage-sector.png` |
| Clamp texture mode | RE-200: file 109 graph `0x44C8`, 8 clamp-bound primitives with neither mirror axis set. This replaces file 22 as the clean on-screen citation while retaining RE-198's archive-wide evidence | Yes — scene 3, `tests/golden/r1-stage-sector.png` |
| Untextured / vertex-coloured geometry | RE-198: file 52, mesh index 4, primitive 0 (14 triangles, unlit, opaque non-degenerate vertex colour `[145,213,213,255]`) | Yes — RE-199's second scene, `tests/golden/r1-mvopeningroom.png` |
| Particles | RE-180–189: all 160 real `LBParticle` scripts plus one live manager-effect `LBGenerator` spawn event | Audited separately on device; dynamic particle coverage is not one of the static golden scenes |
| Shadows | `FighterDesc`'s shadow fields are parsed but "no subsystem reads them yet" (`docs/reverse-engineering.md`) | Blocked — not yet implemented |
| UI / HUD | No in-game menu/HUD system exists yet (Layer C's debug viewer is a developer tool, not the game's own UI) | Blocked — not yet implemented |

Rows blocked on not-yet-implemented gameplay systems remain explicit future
coverage. Every currently implemented rendering category now has a committed
deterministic scene or cited coverage.

## R1 exhaustive stage-render audit

R1's "all stages render" row uses a broad smoke audit in addition to the
single deterministic golden above. Run:

```
tools/run-ppsspp.sh --no-build --seconds 3 --audit-stages 41
```

After the initial warm-up, the harness captures the normal stage viewer,
sends one right-D-pad input, waits one second for the next stage to settle,
and repeats in stable pack order. It uses `xdotool` when installed and the
in-repository Python X11 fallback otherwise. Output is written outside Git to
`$PPSSPP_TEST_DIR/stage-audit/`; `manifest.txt` records the Git revision,
backend, EBOOT/pack hashes, requested stage count and every PNG hash. This is
an exhaustive PPSSPP smoke audit, not a new golden suite: stage animations and
the live diagnostic HUD make exact cross-run pixel equality neither expected
nor required.

RE-170 executed this for all 41 packed stages under PPSSPP's software
rasterizer. All 41 captures were nonblank and had distinct SHA-256 hashes;
the HUD advanced from stage 0/file 255 through stage 40/file 295, every frame
showed 60 FPS, and the emulator log had no error/failure/panic/rejection
lines. EBOOT / pack SHA-256:
`2180df75b52b1f041a8d55ee280f02eb54db4e21c6f22383c1c225ec23dda236` /
`0477c7d3fb86378e08685545209f52d560d0d8a0a695135e9633eb46e5bde72c`.
This closes the software-side R1 stage row only; it is not physical PSP
evidence and does not resolve R0.5.

## R1 exhaustive fighter-animation audit

R1's fighter-motion coverage uses a dedicated build that boots directly into
the existing object/figatree viewer with animation zero playing. Run:

```
tools/run-ppsspp.sh --seconds 2 --audit-animations 532
```

The audit intentionally stops before the pack's appended stage and results
entries: those use the 32-bit `AObjEvent32` format and run through
`StageAnimator` / `ResultsTransition`, not the fighter `Skeleton`. Each right
D-pad input advances one sparse fighter animation entry and waits roughly 15
simulation ticks before capture. The audit HUD shows table index/count,
fighter, slot, source file, animation frame and submitted triangle count while
leaving the model unobscured.

The harness rejects a capture when the centred 60% x 76% crop has standard
deviation below `0.003`, which prevents the small identity HUD alone from
satisfying the check. It separately hashes the top identity strip and requires
all `N` headers to be unique, catching ignored navigation input without
requiring animation pictures themselves to differ (the original deliberately
shares motions). Captures and their SHA-256 manifest remain outside Git under
`$PPSSPP_TEST_DIR/animation-audit/`.

RE-171 executed all 532 entries under PPSSPP software. All centred crops
passed (minimum standard deviation `0.0454806`), all 532 identity strips were
unique, representative captures across the table showed 60 FPS, and the log
contained no error/failure/panic/rejection/desynchronisation lines. EBOOT /
pack SHA-256:
`8a4fc9ffea30a47fdafdb426af2b700e0720d791863432217e587ee4141ddc6f` /
`0477c7d3fb86378e08685545209f52d560d0d8a0a695135e9633eb46e5bde72c`.
The external manifest SHA-256 is
`9afee0e9614816f33d748609700258661e4af8499c99c127a24d976ed4371b8e`.
This is software render coverage, not original-pixel equivalence or physical
PSP evidence.

## Evidence

Executed once end-to-end for capture source 1 (PPSSPP software rendering).
See `docs/reverse-engineering.md` RE-123 and RE-125 for the full account:
after landing on "never draw the debug HUD under `regression_capture`" as
the robust fix (three narrower attempts each ghosted or corrupted in a
different way — see above), two captures of the same build were taken 39
real seconds apart (`--seconds 6` and `--seconds 45`, both comfortably
past the tick-240 freeze point), compared with `cmp` and found
byte-identical, and separately with `tools/compare-screenshot.sh` (0
differing pixels). The golden image is committed at
`tests/golden/r0-dream-land-default.png`. RE-150 refreshed it after RE-144's
source-proven billboard Z-scale correction deliberately changed 9,972 pixels.
RE-152 refreshes it again after the nonzero clamp-window correction changed
85 pixels, all within the small Mario model at `(479,337)..(486,354)`; stage
pixels are unchanged. Two independent captures of that build were
pixel-identical. RE-261 refreshes it once more for the source-authored
costume-light-track correction: 24 stable pixels on Mario's lower body, with
all stage pixels unchanged. RE-262 later corrects signed clamped UVs across
the canopy and Mario, changing 23,852 pixels. RE-281 refreshes it once more
after RE-280's `Ci4`/`PsmT4` nibble-order correction (every 4-bit paletted
texture the archive packs, including the CI4 ground texture this scene
draws), changing 26,100 pixels; its current SHA-256 is
`5da0908599d32f4a9a4da04a096bfcf2a94824196a09e8427a781c87297aa98f`.

RE-199 executed the same end-to-end procedure for the second scene
(`regression_capture_scene2`, file 52's `mvopeningroom.c` graph). The first
capture (`--seconds 6`) showed 126,693 differing pixels against a capture
taken 24 real seconds later (`--seconds 30`) before the object-view idle
spin was added to the freeze list; after that fix, the same two capture
times produced byte-identical PNGs (`cmp`) and `tools/compare-screenshot.sh`
reported 0 differing pixels. The rebuilt default (`regression_capture`,
no scene-2 feature) still matches the original Dream Land golden exactly (0
differing pixels), confirming the scene-2-only code paths (`object_index`
override, `stage_view` disable, spin freeze, HUD suppression) have no effect
on builds without that feature. The new golden is committed at
`tests/golden/r1-mvopeningroom.png`, SHA-256
`db3fd4bce8d3dbbed4534d53fdbea1c3708d708d19298149037676f2628f9ba1`, captured
against EBOOT SHA-256
`e0166727c26b78ea53cd97648790885d9897e5b17b0a0375db032508b90370f8` and pack
SHA-256
`7647db75dce032048e6ab69a1ada5b6990e8ccfd9c86d36a6c04fe612650b2f0`.

RE-200 executed scenes 3 and 4 under the same PPSSPP software renderer.
Captures at 6 and 30 seconds were byte-identical for each scene, and the
pixel comparator reported 0 differing pixels. Goldens and SHA-256 values:

* `tests/golden/r1-stage-sector.png`:
  `5aac523fd46ec969e96314d8f22c1b6da54251ce4fae061cc3fc8a04dbc5f4f8`
* `tests/golden/r1-catch-swirl-flat-color.png`:
  `3a7b7df27d18b7369abc785bd4fe9cc07e592017aaf110e313af243d5defac76`

After adding both feature-only paths, scene 2 and Dream Land were rebuilt and
still matched their committed goldens with 0 differing pixels.

RE-281 applied RE-280's `Ci4`/`PsmT4` nibble-order fix, rebuilt
`assets/generated/ssb64.pak`, and re-ran the full 22-scene matrix against it:
18 goldens changed (`r1-stage-sector.png`: 3,408 pixels; the 4 unchanged were
`r1-catch-swirl-flat-color.png`, `r2-depth-mask-diagnostic.png`,
`r2-dk-fighter.png`, `r2-kirby-fighter.png`), every changed pixel tracing to
the same single cause (corrected `Ci4` texel decode, not a rendering
regression). See RE-281 for the full per-scene differing-pixel table and the
Mario-face visual confirmation this fix was built to resolve (RE-272).

This satisfies `PLAN.md` R0.17's "at least one deterministic test scene",
"methodology is actually run at least once end-to-end", and "captured
reference images are compared automatically" acceptance items. The 4-source
capture procedure is fully documented; source 1 has an exact golden and source
4 now has RE-151's same-input numerical camera trace and normalized
representative comparison. RE-200 extends deterministic PPSSPP coverage to
every specifically targeted open static row. Dynamic particles retain their
separate RE-180–189 device audits. Shadows and real UI remain blocked on later
subsystems rather than being treated as covered.
