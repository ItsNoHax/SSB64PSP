# Visual Regression Methodology (R0.17)

This is the deterministic, repeatable visual-regression procedure `PLAN.md`
R0.17 requires, superseding `TODO.md` Phase H's "Reference renderer" /
"Screenshot regression" items. Screenshots taken ad hoc during individual
`RE-` investigations remain valid evidence for the specific claims they were
taken for, but they are not a substitute for this: a fixed scene that can be
re-captured and diffed automatically as the renderer changes.

## The deterministic test scene

**Stage:** Dream Land, `stage_index` 0 (confirmed against the decomp
repeatedly throughout `docs/reverse-engineering.md`, e.g. RE-031/RE-034:
"stage 0/41 file 255 @0x14").

**Fighter:** Mario, placed at the stage's own spawn 0
(`psp/src/play.rs::Play::at_spawn` hardcodes `FighterKind::Mario` and
`pack.spawn(stage, 0)` — no debug-viewer state affects this choice).

**Camera:** the debug viewer's own boot defaults — `cam_distance =
CAM_FIT`, `spin = 0.0`, no button input. Dream Land's stage-view camera is
"face-on, always" or `sim_fighter`-follow only past a zoom threshold this
scene never crosses at default zoom (`psp/src/main.rs`, stage-view camera
comment), so no additional pinning was needed.

**Animation/frame:** pinned by a new `regression_capture` Cargo feature on
the `ssb64-psp` crate (`psp/Cargo.toml`), off by default. When enabled, once
240 simulation ticks have run (`DETERMINISTIC_CAPTURE_TICKS`, 4 real seconds at
the sim's fixed 60 Hz — comfortably past Mario's fall from Dream Land's
spawn height), every per-frame mutation freezes: the fighter physics tick
(`Play::tick`), the object/animation-viewer skeleton tick, the stage
scenery animator (`StageAnimator::tick`), and the material animator
(`MaterialAnimator::tick`). Nothing in the simulation is randomised, so a
frozen state never changes again regardless of how long the capture script
keeps the emulator running afterward — a screenshot at tick 240 and one at
tick 900 are the same PNG (measured, see Evidence below).

**Game state:** none of the interactive toggles are touched (`show_collision`
stays at its default `true`, `sim_fighter` stays `true`, no button is
pressed) — the debug viewer boots directly into this state with a pack
loaded and no input.

**The on-screen debug HUD is never drawn at all under `regression_capture`,
from frame 0.** `gpu.debug_text` normally prints live perf counters
(`cpu`, `frame`, `tick`) that are meaningless once the sim is frozen and
would otherwise be the only non-deterministic content left in the frame.
Three narrower fixes were tried before this one and each failed for a
different reason specific to `sceGuDebugPrint` (a PPSSPP-only debug
overlay hook, not real GE drawing, that does not fully clear between
calls): skipping the call once frozen left a stuck, partial-width redraw
behind; pinning the three fields to `0` once frozen left old, wider digits
ghosted behind a shorter string; pinning them to their real last-seen
values fixed the width but not the content, since `cpu`/`frame` are
genuine wall-clock timing measurements that legitimately differ between
runs; and even a hardcoded, safely-wide sentinel value still ghosted,
meaning the corruption was never simply about string width. Never calling
`sceGuDebugPrint` in a `regression_capture` build sidesteps whatever
PPSSPP-internal state causes this rather than trying to out-guess it — and
a developer diagnostic overlay was never actually part of the golden scene
this task wants captured. See `docs/reverse-engineering.md` RE-123 and
RE-125 for the full account.

## The second deterministic test scene (RE-199)

`regression_capture`'s Dream Land scene is `stage_view`; it never exercises
the object-view code path at all, so it cannot cover assets that only exist
outside Dream Land's own files. RE-198 tied three uncovered test-matrix rows
to concrete files (CI8 texture and untextured/vertex-coloured geometry to
file 52, clamp texture mode to file 22) but did not build anything to put
them on screen.

RE-199 adds a second Cargo feature, `regression_capture_scene2`, on the same
`ssb64-psp` crate, off by default like `regression_capture`. It:

* disables the default Dream Land `stage_view` boot (added to the same
  `cfg!(any(...))` list `animation_audit_capture`/`effect_audit_capture`/etc.
  already use to do this);
* overrides the object viewer's normal "deepest hierarchy" boot heuristic
  (`psp/src/main.rs`, object_index selection) to file 52's own graph
  (`mvopeningroom.c`'s "MVCommon" scene) by matching `ObjectDesc.source_file
  == 52` — the same graph the depth/triangle heuristic already found and
  rejected for the *first* golden scene, because its 38 flat cutscene panels
  "look exactly like a rendering bug" in that context. That is precisely
  what makes it the concrete carrier of RE-198's CI8 (offset `0x2ee8`) and
  untextured/vertex-coloured (mesh index 4, primitive 0) examples;
* reuses `deterministic_capture_frozen`'s existing tick-240 freeze
  (extended to recognise this feature too) and the existing HUD-suppression
  `cfg!(any(...))` list (same extension) for a clean, exact-match frame;
* additionally freezes the object viewer's own idle model spin (`spin +=
  0.02` per frame), which `regression_capture`'s `stage_view` path never
  exercises and which is not covered by `deterministic_capture_frozen` --
  every other object-view-based audit tolerates spin drift because it only
  checks a captured frame is non-blank, not that two captures are pixel
  identical. Measured: without this, two captures of the same build 24
  real seconds apart differed by 126,693 pixels; with it, byte-identical
  (see Evidence).

Build and capture it the same way as the first scene, substituting the
feature name:

```
cd psp && cargo psp --release --features regression_capture_scene2
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r1-mvopeningroom.png ~/ppsspp-test/screenshot.png
```

Follow with a plain `cargo psp --release` before resuming normal work, for
the same reason `regression_capture` requires it.

## Third and fourth deterministic test scenes (RE-200)

RE-200's graph-backed archive census found that no one scene graph covers all
four remaining matrix rows. Two exact object keys cover them with the smallest
additional suite:

* `regression_capture_scene3` selects file 109 (`StageSectorFile2`) graph
  `0x44C8`. Its converted graph contains 7 `combiner_texture_blend`
  primitives, 5 classified translucent primitives, and 8 clean clamped
  textures with neither mirror axis set.
* `regression_capture_scene4` selects file 84 (`EFCommonEffects2`) graph
  `0x2760`, the decomp-named `CatchSwirlDObjDesc`. Its four rendered quads
  use the classified flat-constant-colour path.

Both features reuse scene 2's tick-240 freeze, idle-spin freeze, object-view
boot, and HUD suppression. Selection matches both `source_file` and
`source_offset`; files 84 and 109 each contain several unrelated graphs.

Build and compare each scene with:

```
cd psp && cargo psp --release --features regression_capture_scene3
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r1-stage-sector.png ~/ppsspp-test/screenshot.png

cd psp && cargo psp --release --features regression_capture_scene4
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r1-catch-swirl-flat-color.png ~/ppsspp-test/screenshot.png
```

These images are deterministic port-regression baselines. Source-classified
primitive membership proves each path is exercised; the images do not claim
pixel equivalence to an original N64 capture.

## Fifth deterministic test scene (RE-205)

`PLAN.md` R2's "stage animation works" row needed a scene that isolates
*visibly* animated stage geometry within the frozen tick-240 window — none
of the first four did (Dream Land, scene 1's stage, has no `anim_joints` at
all per RE-051; scenes 2-4 use the object viewer on a single static graph).

`regression_capture_scene5` overrides `stage_index` to 9, Saffron City
(file 112), whose gate RE-142/RE-143 already proved moves under real joint
animation (an independent 836-pixel PPSSPP before/after diff). Unlike
scenes 2-4 it stays in the default `stage_view` — the same whole-stage,
face-on framing scene 1 uses already fits Saffron City's full geometry,
gate included, at the debug viewer's default zoom, so no new camera code
was needed. Reuses the existing tick-240 freeze (extended to recognise this
feature) and HUD suppression.

Finding this candidate used a new persistent example,
`crates/ssb-rom/examples/stage_animation_amplitude.rs`, which ranks every
animated, mesh-bearing, non-billboard stage node by translation/rotation
amplitude over 240 frames — run it to see the full ranking across all 35
animated stages.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene5
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-saffron-city-gate.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-205): built and `ldstart`ed under
PSPLink the same way as the other four scenes (`docs/psplink.md`). This
surfaced a real hardware-only fault — a third `1.0 / payload` speculative-
division FPU trap in `objanim.rs`'s `StageJoint::apply`, the same class
`f111892` already fixed in `figatree.rs`/`matanim.rs` — fixed with the same
`reciprocal_or_one` guard. After the fix: zero exceptions, native capture
matches this golden (upscaled 2x nearest-neighbour) with only the expected
edge-antialiasing band and known overlay regions differing, the same result
shape RE-203 found for the other four scenes.

## Sixth deterministic test scene (RE-207)

Every prior fighter-bearing scene used Mario. `regression_capture_scene6`
puts Fox on screen instead, reusing scenes 2–4's object-viewer pattern:
selects file 313 offset `0x2938`, Fox's own model graph, and the same tick-240
freeze, idle-spin freeze, `stage_view` disable and HUD suppression. The graph
was chosen deliberately, not arbitrarily: it is the exact one RE-152 found
and fixed a real bug on (a clamp-window coordinate bug that painted Fox's
lower face solid black), so this scene doubles as a regression check for
that fix rather than an untested pick.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene6
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-fox-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-207): built and `ldstart`ed under
PSPLink the same way as the other five scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band and PPSSPP's own FPS-counter overlay differing — no solid interior
region of the model differs, confirming RE-152's fix holds on real hardware.

## Seventh deterministic test scene (RE-208)

`regression_capture_scene7` puts Captain Falcon on screen, the same
object-viewer pattern as scene 6: selects file 332 offset `0x3BE0`, Falcon's
own model graph (the lower-offset of the file's two symmetric 26-node
graphs, matching the convention scene 6 used for Fox). Falcon was chosen
over the other untested fighters because RE-102 (R0.5) already named him,
alongside Fox and Kirby specifically, as one of three fighters with a real
UV-scale/clamp bug on face/torso/head textures — this scene extends the
same regression lineage RE-207 started for Fox to a second fighter.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene7
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-falcon-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-208): built and `ldstart`ed under
PSPLink the same way as the other six scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band, PSPLink's status text, and PPSSPP's FPS-counter overlay differing — no
solid interior region of the model differs.

## Eighth deterministic test scene (RE-209)

`regression_capture_scene8` puts Kirby on screen, the same object-viewer
pattern as scenes 6-7: selects file 328 offset `0x1448`, Kirby's own model
graph (the lower-offset of the file's symmetric 27-node graph pair, matching
the convention scenes 6-7 used for Fox and Falcon). Kirby was chosen because
RE-102 (R0.5) named him, alongside Fox and Falcon, as the third of three
fighters with a real UV-scale/clamp bug on face/torso/head textures — this
scene completes the regression lineage RE-207/RE-208 started for that bug
class.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene8
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-kirby-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-209): built and `ldstart`ed under
PSPLink the same way as the other seven scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band, PSPLink's status text, and PPSSPP's FPS-counter overlay differing — no
solid interior region of the model differs.

## Ninth deterministic test scene (RE-210)

`regression_capture_scene9` puts Ness on screen, the same object-viewer
pattern as scenes 6-8: selects file 335 offset `0x26B0`, Ness's own model
graph (the lower-offset of the file's symmetric 27-node graph pair, matching
the convention scenes 6-8 used). Ness was chosen because RE-103 named him,
alongside Fox, Captain Falcon and Kirby, as a fighter whose surface
"melted" into rainbow noise under the old per-primitive majority-vote
lit-vs-literal heuristic — a different bug class than scenes 6-8's
UV-scale/clamp fix, and the one fighter from RE-103's set still
hardware-untested.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene9
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-ness-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-210): built and `ldstart`ed under
PSPLink the same way as the other eight scenes. Zero exceptions (`exlist`
empty, `main_thread` alive in `thlist`). Native capture matches this golden
(upscaled 2x nearest-neighbour) with only the expected edge-antialiasing
band and PPSSPP's FPS-counter overlay differing — no solid interior region
of the model differs.

## Tenth deterministic test scene (RE-212)

`regression_capture_scene10` puts Donkey Kong on screen, the same
object-viewer pattern as scenes 6-9: selects file 317 offset `0x39A8`,
DK's own model graph (the lower-offset of the file's symmetric 26-node
graph pair, matching the convention scenes 6-9 used). DK was chosen as the
next untested fighter in `FIGHTER_COSTUME_COUNTS`
(`tools/romtool/src/main.rs`) order once RE-102's and RE-103's own named
fighter sets were both exhausted (RE-207–210).

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene10
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-dk-fighter.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-212): built and `ldstart`ed under
PSPLink the same way as the other nine scenes. The first two attempts
produced the built-in fallback tetrahedron instead of DK despite `exlist`
being empty and `main_thread` alive in `thlist` — a silently failed
asset-pack open, invisible to this project's usual health checks. `pspsh -e
reset` before the next `ldstart` resolved it; see RE-212 and
`docs/psplink.md` for the methodology finding. Once resolved: zero
exceptions, native capture matches this golden (upscaled 2x
nearest-neighbour) with only the expected edge-antialiasing band, PSPLink's
status text, and PPSSPP's FPS-counter overlay differing — no solid interior
region of the model differs.

## Eleventh and twelfth deterministic test scenes (RE-214)

`regression_capture_scene11` selects `StageMetalFile2`'s first graph (file
117, offset `0x1B10`, two nodes) in the same object-viewer pattern as scenes
2-4 and 6-10. It is the only content this port can currently show that draws
real `G_TEXTURE_GEN` material state.

`regression_capture_scene12` is the **same** graph under the **same** camera,
frozen a quarter turn further round. It exists because one frozen reflection
is not evidence: a correct coordinate generator, a stuck basis and a constant
both produce a single plausible frame. Two deterministic captures at two known
rotations distinguish them — they differ by RMSE 0.058 here — and a future
regression that silently freezes the generator fails the pair even though it
would pass either frame alone.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene11
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen.png ~/ppsspp-test/screenshot.png

cd psp && cargo psp --release --features regression_capture_scene12
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen-rotated.png ~/ppsspp-test/screenshot.png
```

Physical PSP hardware verification (RE-214): both scenes `ldstart`ed under
PSPLink with `exlist` empty and `main_thread` alive. Diffed 2x-upscaled
against their goldens: 25,977 and 28,866 differing pixels, of which 2,652 and
8,443 differ by more than 6%. A reflection has fine internal gradients, so its
band boundaries move under any sub-texel difference in coordinate
interpolation — the right comparison is against this harness's own baseline,
and the *non*-texgen Dream Land scene diffs at 61,362 / 11,668 by the same
measurement. These scenes agree with PPSSPP better than the long-accepted
baseline does. The two hardware captures differ from each other by 57,076
pixels, so the reflection responds to model rotation on real hardware too.

These goldens do **not** establish agreement with the original N64 output;
see RE-214 §10 for why no such comparison exists yet.

## Thirteenth deterministic test scene (RE-215)

`regression_capture_scene13` selects `StageMetalFile2`'s **second** graph
(file 117, offset `0x2EE0`, two nodes) in the same object-viewer pattern as
scenes 11/12. Verifying RE-215's exact `G_TEXTURE_GEN_LINEAR` fix found that
scenes 11/12's graph (`0x1B10`) carries only *ordinary* texgen primitives —
the archive's one packed linear primitive (file 117, texture 499, 12
triangles) lives in this sibling graph instead. A reachability check (a debug
colour marker in the linear-texgen branch) and a direct read of
`assets/generated/ssb64.pak` both confirmed it before this scene was added;
see RE-215 for the full account, including the correction to an earlier
RE-214 note that had claimed scene 11 itself carried linear triangles.

Build and compare:

```
cd psp && cargo psp --release --features regression_capture_scene13
tools/run-ppsspp.sh --no-build --seconds 6
tools/compare-screenshot.sh tests/golden/r2-metal-texgen-linear.png ~/ppsspp-test/screenshot.png
```

The fix's effect was confirmed against this exact scene before adopting the
golden: rebuilding with the pre-fix ordinary-mapping code (everything but the
scene's own wiring reverted) and comparing against the fixed build showed
10,766 differing pixels — not inert. Two captures of the fixed build are
byte-identical (deterministic). Scenes 11, 12, Dream Land, and the Fox
fighter golden (the only fighter-viewer check run here; no fighter golden
carries texgen) are all still byte-identical to their existing goldens —
this fix is isolated to the one graph that actually carries linear content.

Physical PSP hardware verification (RE-215): `ldstart`ed under PSPLink with
`exlist` empty and `main_thread` alive; native capture visually matches the
PPSSPP golden (pink/tan reflective facet, yellow flag panel, gold crystal
band). Hashes recorded in RE-215.

## Capture procedure

### 1. PPSSPP software rendering (executed; this is the current golden source)

```
cd psp && cargo psp --release --features regression_capture
tools/run-ppsspp.sh --no-build --seconds 6   # any --seconds past ~5 works; see below
```

The screenshot lands at `$PPSSPP_TEST_DIR/screenshot.png` (default
`~/ppsspp-test/screenshot.png`). `--seconds` no longer has to be tuned
precisely: because the scene freezes at tick 240 (4 real seconds in), any
value at or past ~5 seconds captures the identical frame. Compare it with:

```
tools/compare-screenshot.sh tests/golden/r0-dream-land-default.png ~/ppsspp-test/screenshot.png
```

Exits 0 and prints `PASS` on a match; nonzero and the differing-pixel count
otherwise. The threshold defaults to 0 (exact match) because the scene is
measured byte-identical run to run — see Evidence.

**Rebuilding without the feature.** `regression_capture` is off by default
and must not be left enabled for normal interactive debug-viewer use (it
would freeze the fighter and hide the live perf counters after 4 seconds of
any session). Always follow a regression-capture run with a plain `cargo
psp --release` before resuming normal work; `tools/run-ppsspp.sh --seconds
N` on its own (`--build`, the default) already does this since it never
passes `--features`.

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
| Lighting | Dream Land's platform/canopy shading (`G_LIGHTING`, key light baked at pack time, RE-065) | Yes |
| `combiner_shade_scale` shape | Dream Land's lit, unlit-texture primitives (RE-073); exact per-primitive attribution not isolated in this task | Likely, unconfirmed |
| Depth testing | Dream Land's canopy occluding the platform behind it | Yes |
| Back-face culling | Dream Land's stage geometry (`cull_back` default for non-object-view) | Yes |
| Fighter model + skeleton | Mario, idle pose, spawn 0; Fox, file 313 graph `0x2938` (RE-152/RE-207); Captain Falcon, file 332 graph `0x3BE0` (RE-208) | Yes — scene 1, `tests/golden/r0-dream-land-default.png`; scene 6, `tests/golden/r2-fox-fighter.png`; scene 7, `tests/golden/r2-falcon-fighter.png` |
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
pixels are unchanged. Two independent captures of the new build are
pixel-identical. Its SHA-256 is
`a1d9c22538d6f56ab0d850630c3649e4b7adede799d10f15d4cdd0ab6ced1194`.

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

This satisfies `PLAN.md` R0.17's "at least one deterministic test scene",
"methodology is actually run at least once end-to-end", and "captured
reference images are compared automatically" acceptance items. The 4-source
capture procedure is fully documented; source 1 has an exact golden and source
4 now has RE-151's same-input numerical camera trace and normalized
representative comparison. RE-200 extends deterministic PPSSPP coverage to
every specifically targeted open static row. Dynamic particles retain their
separate RE-180–189 device audits. Shadows and real UI remain blocked on later
subsystems rather than being treated as covered.
