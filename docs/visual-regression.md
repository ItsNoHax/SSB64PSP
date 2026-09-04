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
240 simulation ticks have run (`regression::TARGET_TICKS`, 4 real seconds at
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

**The one thing left unpinned:** the on-screen debug HUD
(`gpu.debug_text`) prints live perf counters (`cpu`, `frame`, `tick`) that
are meaningless once the sim is frozen and would otherwise be the only
non-deterministic content left in the frame. `regression_capture` pins
those three fields to `0` once frozen rather than skipping the
`sceGuDebugPrint` call outright — an earlier version of this skipped the
call, and that confused PPSSPP's own debug-overlay HLE hook (which is not
real GE drawing) into a stuck, partial-width redraw. See
`docs/reverse-engineering.md` RE-123 for the full account of both the
freeze mechanism and that specific pitfall.

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

Copy the same EBOOT (`psp/target/mipsel-sony-psp/release/EBOOT.PBP`, built
with `--features regression_capture`) and the asset pack
(`assets/generated/ssb64.pak`) to a PSP's `PSP/GAME/` directory, run it,
wait past 4 seconds, and photograph or capture the screen. Not yet executed
in this task; this project's existing device-verification precedent (e.g.
RE-098, RE-114) is the model to follow when it is.

### 4. Original SSB64 (N64 ROM/emulator reference, documented, not yet executed)

Boot the ROM in an N64 emulator, select a match on Dream Land with Mario,
and let it reach the same idle-on-spawn state. No frame-perfect frame
count is expected to line up with the PSP port's own tick 240 (different
engines, different boot sequences) — the comparison is visual (does the
platform look the same, does Mario's idle pose match), not pixel-exact
across engines. Not yet executed in this task.

## Test matrix

Each row names a concrete asset or display list, not a hypothetical
example. "Covered by golden scene" means the current single Dream Land
capture actually exercises it; other rows need a dedicated scene (a second
`regression_capture`-style frozen state) that this task does not yet add.

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
| Fighter model + skeleton | Mario, idle pose, spawn 0 | Yes |
| CI8 texture | Not yet identified to a specific file/offset in this task | No — needs identification |
| `combiner_texture_blend` shape | RE-074's PRIM/ENV-blended primitives; not present in Dream Land's default camera framing | No — needs a dedicated scene |
| `combiner_flat_color` shape | RE-080's flat-constant-colour primitives; not confirmed present in this scene | No — needs a dedicated scene |
| Transparency / translucency | RE-083's billboards are the known concrete case, not on-screen in this framing | No — needs a dedicated scene |
| Clamp texture mode | Not yet identified to a specific file/offset in this task | No — needs identification |
| Untextured / vertex-coloured geometry | The fallback tetrahedron (`TRIANGLE` in `psp/src/main.rs`) when no pack loads; no known textured-pack example identified | No — needs identification |
| Particles | No confirmed particle system exists yet; file 48's "particle-like" node layout (per `docs/reverse-engineering.md`) is unconfirmed, not a named system | Blocked — system not confirmed to exist |
| Shadows | `FighterDesc`'s shadow fields are parsed but "no subsystem reads them yet" (`docs/reverse-engineering.md`) | Blocked — not yet implemented |
| UI / HUD | No in-game menu/HUD system exists yet (Layer C's debug viewer is a developer tool, not the game's own UI) | Blocked — not yet implemented |

Rows marked "needs identification" or "needs a dedicated scene" are real,
named gaps for follow-up work, not silently dropped: extending this matrix
is scoped as ongoing work under this same document rather than a new
`PLAN.md` task, since the methodology (frozen `regression_capture` scene +
`compare-screenshot.sh`) already generalises to any of them once a
suitable frame is identified.

## Evidence

Executed once end-to-end for capture source 1 (PPSSPP software rendering).
See `docs/reverse-engineering.md` RE-123 for the full account: two captures
of the same `regression_capture` build, taken 9 real seconds apart
(`--seconds 6` and `--seconds 15`, both past the tick-240 freeze point),
compared with `cmp` and found byte-identical, and separately with
`tools/compare-screenshot.sh` (0 differing pixels). The golden image is
committed at `tests/golden/r0-dream-land-default.png`.

This satisfies `PLAN.md` R0.17's "at least one deterministic test scene",
"methodology is actually run at least once end-to-end", and "captured
reference images are compared automatically" acceptance items. The 4-source
capture procedure is fully documented; only source 1 has been executed.
The test matrix exists with named, concrete rows; a minority are confirmed
covered by the single golden scene, the remainder are honestly tracked as
not yet covered rather than assumed.
