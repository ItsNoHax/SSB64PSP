# Project Status

**Last updated:** 2026-09-05 (RE-131)

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

Three sessions in a row (RE-126/128/129/130) converged on the same
conclusion: every remaining independently-fixable rendering gap was
exhausted, and `R0.12`/`R0.13`/`R0.14` all ultimately need an actual
game camera this project did not have. Asked the user how to proceed;
the answer was to build a minimal real camera system. RE-131 (this
session) did that.

**Researched `gm/gmcamera.c`'s `gmCameraDefaultFuncCamera`** (the
single-fighter, normal-stage camera) and independently verified every
load-bearing formula directly against the source before writing any
Rust — a delegated research pass had already gotten one formula subtly
wrong (smoothed away two genuine discontinuities in the real pan-scale
function), caught by reading the C directly. Ported it call for call as
`ssb_game::camera`: interest-box framing from the fighter's position and
facing, FOV lerp, viewport-fit distance with damping, look-at panning,
and an eye-direction angle derived from the look-at point and the
stage's own light-angle nudge. `gmCameraSetBoundsPosition`'s clamp and
both hand-rolled diff/normalise/scale/add sequences reduce to this
project's own existing, tested `Vec3::lerp` and `Bounds::clamp` — no new
math primitives needed.

**A second, independent finding along the way.** `light_angle.z`
(`crates/ssb-rom/src/stage.rs`) was recorded as having no known reader —
wrong: the real camera reads it too, for its own eye-direction angle,
not lighting. Measured its real archive-wide values before assuming a
unit convention: unlike `.x`/`.y` (degrees), `.z` is stored
**pre-converted to radians** (confirmed both by the measured values and
by the camera's own C code adding it with no conversion of its own).
Extended `light_angle` to `[f32; 3]` and `StageDesc` to carry it
(`pack::VERSION` 16 → 17).

**Wired into the render path without disturbing the existing debug
camera.** A new `Gpu::set_view` loads a real `Mat4::look_at` (already
implemented, previously unused anywhere in this codebase) into the GE's
own View matrix — every mesh-drawing function already loads its own
baked model matrix per node, so the separate View matrix composes for
free; zero `meshdraw.rs` changes needed. The debug viewer's whole-stage
overview mode is **completely unchanged** (confirmed: zero differing
pixels against the `regression_capture` golden capture); only the
zoomed-in "follow the fighter" mode now uses the real camera instead of
a fixed, face-on placeholder — verified stable and sane over a 20-second
run with real perspective and depth, not just "does not crash."

**Deliberately scoped down, each simplification documented, not
silent**: no weapons, no multiplayer, no per-move camera zoom
(`camera_zoom_frame`/`camera_zoom_range`), no idle-zoom-out, no
entry/explain/dead-up modes, no pause-camera offset. Does not yet wire
the camera's own angle into `R0.12`'s billboard code, or drive `R0.13`'s
screen-wipe triggers (no match-transition state machine exists), or
close `R0.14`'s "camera transforms verified" (needs checking against
real footage, not just plausibility) — but all three now have a real,
tested camera to build on instead of nothing. See
`docs/reverse-engineering.md` RE-131 for the full account.

`cargo test --workspace`: 413 → 418 passing (5 new). `cargo clippy
--release --workspace` and `cargo psp --release`/`--features
regression_capture`: clean, same pre-existing 6-warning set.

**Task-selection note for the next session.** The concrete next steps
this unlocks: (1) decompose the camera's `eye`/`at` into pitch/yaw and
wire it into `Kind48`'s billboard transform (R0.12's own next step per
RE-126); (2) extend `ssb_game::camera` past the single-fighter case if
multiplayer ever becomes relevant; (3) `R0.5`'s one remaining item and
`R0.6`'s remaining three items are still blocked on `R2`/`R0.7`
respectively, unchanged by this session.

`R0.12 — Billboard Correctness` remains `VERIFYING`. RE-126 (an earlier
session) investigated its open "orientation verified"/"scale verified"
items by reading the real `gcPrepDObjMatrix` algorithm directly rather
than trusting the enum names `Kind46`/`Kind48`/`Kind50` already in use.
**Found a real, measured, previously-uncounted gap**: kinds 47/48 and
49/50 build their MVP from camera-axis-*locked* LookAt matrices
(pitch-locked and yaw-locked respectively), not the same fully
screen-aligned transform kinds 44/45/46 use — a materially different
algorithm this project currently approximates with one shared,
screen-aligned `billboard_place` for all four kinds. A temporary,
reverted census through the real `romtool pack` build found **47 real
`Kind48` nodes archive-wide, including Dream Land's own file 104** — the
*largest* individual billboard category (43% of all 109 flagged nodes,
splitting RE-049's original 81 into 34 `Kind46` + 47 `Kind48`; `Kind50`
remains the only confirmed-unused one at 0/3117, RE-063). Not yet
visibly wrong on screen because doing so requires a camera that
yaws/pitches relative to the object — neither RE-049's one forced test
rotation nor the current, still-face-on debug/gameplay camera ever
varies along the axis the two transforms disagree on. Also found a
related, smaller, **unconfirmed** lead: the real per-axis scale formula
multiplies a node's own Y-scale by the ancestor chain's cumulative
*X*-scale, not this project's composed-basis-column-length approach —
identical only if every ancestor's own scale is uniform, not measured
either way this pass.

**This does not close R0.12's open items** — it replaces two unexamined
gaps with measured, understood, and honestly-still-open ones. Fixing
`Kind48` properly needs: (1) a pack-format change preserving which kind
a node had, not just one collapsed `FLAG_BILLBOARD` bit, and (2) the
render call knowing the camera's own eye/at position decomposed into
pitch/yaw, which `PLAN.md` R0.14 (this task's own second dependency)
does not have yet ("an actual game camera" is still an open R0.14 item)
— this finding gives that item a concrete, quantified reason to need
doing, rather than a hypothetical one.

**Per `PLAN.md`'s task ordering, this session's investigation found every
currently `IN_PROGRESS`/`VERIFYING` R0.x rendering-correctness task is
now blocked on one of two things this project does not have yet, not on
more `romtool`-side investigation:** `R0.7`/`R0.4`/`R0.6` on upstream
decomp typing (RE-125, exhausted again); `R0.12`/`R0.14` on an actual
game camera system; `R0.13` on a game-state system (per an earlier
session's own finding). None of these is a "smallest appropriate fix"
away from closing without either new upstream decomp data or a genuine
architectural addition (a real camera/game-state layer) — which is
larger-scoped work this session did not start unprompted. The next
session should read this note and `PLAN.md`'s own dependency graph
before picking a task, rather than assume an easy next item remains.

`R0.7 — Missing Material Tables` advanced significantly in an earlier
session this same day (RE-125: paired 70 → 90 archive-wide, unpaired
57 → 37, via a systematic re-application of RE-078's own search-plus-
decomp-cross-check method; also found and fixed a real determinism bug
in R0.17's own debug-HUD-freezing logic while re-verifying it).
`R0.18 — Reference-Port Comparative Audit` closed `COMPLETE` in an
earlier session (RE-124: `oot-PSP` cloned and compared against `sf64-psp`
and this project's own choices; closed R0.5's filtering item, added a
new lead to R0.6's blending item, recorded an R3 performance lead).
`R0.17 — Visual Regression Methodology` closed `COMPLETE` in an earlier
session (RE-123, refined by RE-125: a `regression_capture` Cargo feature
freezes every per-frame mutation once 240 simulation ticks have run and
never draws the debug HUD, producing a byte-identical golden capture
regardless of real-world timing).

## Task Status

RE-131 (this session) built a real, minimal battle camera
(`gm/gmcamera.c`'s `gmCameraDefaultFuncCamera`, ported and verified) to
unblock `R0.12`/`R0.13`/`R0.14`, per the user's own explicit direction
after three sessions found every smaller independently-fixable rendering
gap exhausted. See `docs/reverse-engineering.md` RE-131 for the full
account; summary:

* **Researched before implementing, then independently re-verified.** A
  delegated research pass summarized `gmCameraDefaultFuncCamera`'s real
  algorithm; reading the actual C directly caught one real error in that
  summary (a smoothed paraphrase of the pan-scale formula that dropped
  its two genuine discontinuities — the decompilation's own comment on
  that function, "Needs to be two different 0.05s lol," already flags
  them as an original-game oddity, reproduced exactly rather than fixed).
* **Ported call for call**: interest-box framing (fighter position +
  facing-dependent asymmetric offset + single-player zoom), FOV lerp,
  viewport-fit distance with 7.5%-per-frame damping, look-at panning, and
  an eye-direction angle derived from the look-at point plus the stage's
  own light-angle nudge. Both real functions' own hand-rolled
  diff/normalise/scale/add sequences reduce to this project's existing,
  tested `Vec3::lerp`; the bounds clamp mirrors
  `gmCameraSetBoundsPosition`'s own one-axis-at-a-time loop exactly.
* **A second, independent finding while researching**: `light_angle.z`
  (`stage.rs`) was recorded as having no known reader — wrong, the real
  camera reads it too. Measured its real archive-wide values before
  assuming a unit convention: unlike `.x`/`.y` (degrees), `.z` is stored
  pre-converted to *radians* (confirmed by both the measured values and
  the camera's own code adding it with no conversion of its own).
  Extended `light_angle` to `[f32; 3]`, `StageDesc` to carry it
  (`pack::VERSION` 16 → 17).
* **Implemented as `ssb_game::camera`** — portable, `no_std`,
  platform-free, matching Layer A's own rules. Five new unit tests cover
  the bounds clamp, the pan-scale formula's own discontinuities, a
  stationary fighter settling and staying settled, and a moving fighter
  converging on the mathematically-predicted (not screenshot-derived)
  interest-box centre.
* **Wired in without disturbing the existing debug camera.** A new
  `Gpu::set_view` loads a real `Mat4::look_at` (already implemented,
  previously unused anywhere) into the GE's own View matrix slot —
  composes for free with the existing per-node model-matrix machinery,
  zero `meshdraw.rs` changes needed. The debug viewer's whole-stage
  overview mode is untouched (confirmed: zero differing pixels against
  the `regression_capture` golden capture); only the zoomed-in
  "follow the fighter" mode now uses it, verified stable and sane with
  real perspective/depth over a 20-second on-device run.
* **Deliberately scoped down, each simplification documented**: no
  weapons, multiplayer, per-move camera zoom, idle-zoom-out,
  entry/explain/dead-up modes, or pause offset.
* **What this does and does not close.** Does not itself close any
  acceptance item on `R0.12`/`R0.13`/`R0.14` — `R0.12`'s billboard code
  does not yet consume this camera's angle, `R0.13` still has no
  match-transition trigger, and `R0.14`'s "camera transforms verified"
  needs checking against real footage, not just plausibility. What it
  removes is the shared blocker on all three: a real camera to build on,
  where there was none.
* `cargo test --workspace`: 413 → 418 passing (5 new). `cargo clippy
  --release --workspace` and `cargo psp --release`/`--features
  regression_capture`: clean, same pre-existing 6-warning set. All
  temporary code (a `romtool stages` light-angle census, a `cam_distance`
  override used to force the zoomed-in mode for a screenshot) fully
  reverted.

## Previous Task Status

RE-130 (an earlier session) measured every real alpha-blend formula
archive-wide and shipped a fix, closing `R0.6`'s "blending verified"
item that has been open since RE-069. See
`docs/reverse-engineering.md` RE-130 for the full account; summary:

* **Measured every real `TRANSLUCENT` primitive's alpha formula, not
  just the canopy highlight RE-129 already decoded.** A temporary,
  reverted census through the real `romtool pack` build found 9 distinct
  explicit combiner values archive-wide. Classified them by hand
  (`gbi.h`'s macros, RE-129's method): the majority (~5,950 of ~8,800
  real, textured, single-cycle `TRANSLUCENT` primitives) is `TEXEL0_ALPHA`
  alone — Dream Land's flowers' own shape, the exact one RE-129's naive
  experiment broke. A smaller set (1,820) is `TEXEL0_ALPHA * SHADE_ALPHA`
  — the canopy highlight. A rare `PRIM_ALPHA` multiply (~43) and
  two-cycle mode (~93, <1%) are both deliberately declined rather than
  guessed at (two-cycle's own D-slot code `0` means `LOD_FRACTION`, not
  `COMBINED_ALPHA`, the identical per-slot-context trap RE-129 already
  hit once for the colour table — and this ROM never engages real RDP
  LOD at all, RE-127).
* **Implemented the classification as a new, independent axis.**
  `mesh.rs` gained `AlphaBlend` (`TexelOnly`/`Shade`) and
  `combiner_alpha_blend`, mirroring `combiner_shade_scale`/
  `combiner_texture_blend`/`combiner_flat_color`'s existing pattern but
  operating on the alpha sub-fields those never touch. Four new unit
  tests pin the exact archive-measured words (canopy, flowers, the
  declined `PRIM_ALPHA` case, two-cycle-always-declines).
* **Baked the correct vertex alpha per shape** in `push_vertex`, applied
  after the existing `prim_color`/`texture_blend`/`flat_color` branches
  since it is an independent axis: `TexelOnly` forces the vertex alpha to
  `255` (most vertex alpha bytes are not a coverage value at all —
  confirmed catastrophically by RE-129's own experiment), `Shade` leaves
  the raw byte untouched (captured before any RGB branch could overwrite
  it — `prim_color`'s own branch sets alpha too, for an unrelated reason,
  RE-106).
* **A new pack flag gated on both axes agreeing.**
  `pack::flags::ALPHA_BLEND` (`PrimDesc::flags` bit 9, `VERSION` 15 → 16,
  additive, no struct growth but bumped anyway per RE-049/RE-069's own
  precedent) sets only when a primitive is both `TRANSLUCENT` and its
  alpha formula was classified. A `TRANSLUCENT` primitive without it is
  completely unchanged from before this session.
* **Wired up real blending on the device**, replacing
  `psp/src/meshdraw.rs`'s long-standing "deliberately not wired" comment
  with `sceGuEnable(Blend)` plus the standard `SrcAlpha`/
  `OneMinusSrcAlpha` equation, gated on `TRANSLUCENT | ALPHA_BLEND` both
  being set.
* **Verified on-device, not just by pixel-statistic.** Rebuilt the
  shipped pack (`VERSION` 16, 5368.2 KiB — unchanged size) and
  screenshotted Dream Land: the canopy body is pixel-identical from the
  default framing (the highlight is not visible from that angle), the
  flowers survive intact, and two previously fully-invisible decorative
  props (matching benches flanking the tree) now render correctly —
  confirmed via a clean diff against the `regression_capture` golden
  capture: only those two symmetric prop regions and a few 1-pixel
  flower-tip antialiasing edges differ, nothing else in the scene moves.
  Re-verified deterministic across a 9-second timing spread (`--seconds
  6` vs `15`, 0 differing pixels), matching R0.17's own methodology.
  `tests/golden/r0-dream-land-default.png` updated to this new,
  more-correct capture — a deliberate content change, not drift.
* **What this closes.** `PLAN.md` R0.6's "blending verified" acceptance
  item, open since RE-069. `R0.6` overall stays `IN_PROGRESS` — "material
  tables resolved"/"primitive color verified"/"environment color
  verified"/"lighting verified" remain open, mostly on `R0.7`'s own
  material-table pairing gaps or the still-missing per-object lighting
  system.
* `cargo test --workspace`: 409 → 413 passing (4 new). `cargo clippy
  --release --workspace` and `cargo psp --release` (both feature
  states): clean, same pre-existing warning set. All temporary census
  code fully reverted; `git diff --stat` shows only the permanent
  classification/baking/flag/wiring changes plus the golden image
  update.

## Earlier Task Status

RE-129 (an earlier session) decoded the real alpha combiner for Dream Land's
long-open canopy-highlight blend failure and tested the direct
implication, narrowing the mystery further without fully resolving it.
See `docs/reverse-engineering.md` RE-129 for the full account; summary:

* **This project's combiner model never decoded the alpha formula, only
  the colour one.** `mesh.rs`'s `evaluate_combiner`/`cycle` resolve
  `G_SETCOMBINE`'s RGB multiplexers; the alpha formula's own four slots
  live at different bit positions in the same 64-bit word and have no
  code path at all.
* **Hand-derived the real bit layout from `gbi.h`'s macros**, catching a
  real trap along the way: the macros' own internal parameter names
  (`aRGB0`, `mA0`, etc.) do not correspond to which call-site slot
  (`a0`/`b0`/`c0`/`d0`) they actually receive — had to read the
  invocation (`gsDPSetCombineLERP`), not just the macro body, to get it
  right.
* **Applied it to the real ROM word** (a temporary `romtool` subcommand
  wrapping `dl::decode_list_at` against file 104 offsets `0x708`/`0xA78`,
  the exact list RE-069 already identified): the alpha formula is
  `TEXEL0_ALPHA * SHADE_ALPHA` in **both** cycles. Real hardware
  multiplies the texture's own alpha by the vertex's shade alpha; this
  project's renderer currently does neither, since `TRANSLUCENT` was
  never wired to `GuState::Blend` at all (RE-069's own deferral).
* **Tested the direct implication — narrows the problem, does not solve
  it.** The existing default texture function is already `Modulate`, so
  turning blend on with a standard equation should compute the
  newly-decoded formula for free. A temporary, reversible experiment
  doing exactly that produced neither RE-069's checkerboard nor RE-071's
  blowout — it made Dream Land's decorative flower triangles vanish
  instead, a different `TRANSLUCENT`-flagged primitive whose own vertex
  alpha is not a meaningful coverage value (confirmed via a clean
  before/after pixel diff: the whole canopy body is pixel-identical,
  only the five flower triangles go from opaque to absent).
* **What this establishes.** `TRANSLUCENT` alone cannot safely gate real
  blending — this project has no per-primitive record of *which* alpha
  formula a translucent primitive resolves to, unlike the colour
  formula's already-classified shapes
  (`combiner_shade_scale`/`combiner_texture_blend`/`combiner_flat_color`).
  The real fix needs that same classification done for alpha. Not
  attempted this session — a concrete, scoped, well-evidenced next step,
  not a vague "investigate more."
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. All temporary code (`psp/src/meshdraw.rs`'s
  blend-enable experiment, `romtool`'s `re129decodelist` subcommand)
  fully reverted; `git diff --stat` against the pre-session baseline is
  empty for both files. Default (Dream Land) build re-screenshotted
  clean after reverting (pixel-normal, 60 FPS, no panics, flowers
  present).

## Earlier Task Status

RE-128 (an earlier session) closed `R0.5`'s last mipmapping-adjacent item
("texture coordinate behavior verified") and found a real, only-partly
explained fighter rendering defect while doing it. See
`docs/reverse-engineering.md` RE-128 for the full account; summary:

* **Directly confirmed RE-101/RE-102 on two real fighters.** No
  "select fighter" control exists in the debug viewer, so a temporary,
  reverted patch forced `object_view` to a specific packed object index,
  cycling Fox (file 313), Captain Falcon (file 332), Kirby (file 328) —
  the three fighters RE-102's own fix names. `TEXVIEW` mode (direct
  texture display, bypassing lighting/geometry) showed Fox's real face
  texture (index 550) and Kirby's real face texture (index 734) both
  matching `romtool texdump`'s independent reference decode exactly —
  correct colours, no melting, no clamp-boundary seam.
* **What this closes.** `R0.5`'s "texture coordinate behavior verified"
  item, the one both RE-101 and RE-102 explicitly left open pending a
  real fighter screenshot. `R0.5` now has exactly one open item left
  ("Dream Land canopy discrepancy resolved"), already known to need
  `R2`'s real-hardware validation.
* **A real, separate defect found while verifying, not chased into a
  fix.** Fox's *full lit* render (not `TEXVIEW`) shows a large, solid,
  confirmed-`(0,0,0)` black patch on his face — confirmed via direct
  per-pixel sampling, confirmed still present with the debug HUD forced
  fully off (rules out the overlay's own text background), confirmed
  still present under `romtool pack --no-swizzle` (rules out a
  swizzle/deswizzle bug).
* **Two concrete hypotheses tested and eliminated, not guessed away.**
  (1) Texture corruption: decoded every texture on Fox's head straight
  out of the `.pak` bytes, independent of the PSP GE — none is black.
  (2) A double-application of `prim_color`'s scale (`mesh.rs::push_vertex`
  folds it once, `pack.rs::add_mesh`'s own `prim_scale` step multiplies
  again) crushing `shade_normal`'s ambient-floored grey (RE-065: floor
  `0.35`, mathematically never `0` on its own) to exact zero via integer
  truncation — decoding the real display list at the candidate
  primitives' own source offsets (a temporary `romtool` subcommand
  wrapping `dl::decode_list_at`) found no `G_SETPRIMCOLOR` anywhere near
  zero nearby; their exported `prim_color` reads `0x00000000` only
  because `add_object` cannot distinguish `None` from `Some(black)` in
  that inspection-only field, and tracing to source confirms it really
  is `None` there. Eliminated for the primitives checked.
* **Not resolved, and said so plainly.** Which exact primitive draws the
  visible black pixels was inferred from screen position, not confirmed
  geometrically — an explicit gap in this session's own rigor. Filed as
  a concrete lead on `R0.6`'s already-open "primitive color verified"/
  "lighting verified" items (which already anticipated this shape of gap
  in the abstract), not as a new acceptance item.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. All temporary code (`psp/src/main.rs`'s
  forced `object_index`/`stage_view`/`tex_view`/disabled-HUD overrides;
  `romtool`'s `re127findobj`/`re127dumpobj`/`re127dumptex`/
  `re128decodelist` subcommands) fully reverted; `git diff --stat`
  against the pre-session baseline is empty for both files. Default
  (Dream Land) build re-screenshotted clean after every revert.

## Earlier Task Status

RE-127 (an earlier session) re-checked RE-126's own "every `IN_PROGRESS`/
`VERIFYING` task is blocked" claim and found `R0.5` had three open
LOD/mipmapping items that were actually actionable. See
`docs/reverse-engineering.md` RE-127 for the full account; summary:

* **Applied RE-124's exact method to the two other fields
  `G_SETOTHERMODE_H` carries alongside `G_MDSFT_TEXTFILT`.**
  `G_MDSFT_TEXTLOD` (shift 16) and `G_MDSFT_TEXTDETAIL` (shift 17) had
  never been decoded by `mesh.rs` — only the cycle-type field (shift 20)
  had a match arm.
* **A temporary, reverted census through the real `romtool pack` build
  found zero real requests for either non-default mode.** 131/131 real
  `G_MDSFT_TEXTLOD` commands request `G_TL_TILE` (never `G_TL_LOD`);
  121/121 real `G_MDSFT_TEXTDETAIL` commands request `G_TD_CLAMP` — both
  exactly the RDP's own per-frame reset default, the identical shape
  RE-124 already found for `TEXTFILT`. Real N64 hardware never engages
  LOD-blended mipmapping for any content in this ROM.
* **A third field looked like a real, missed signal at first, then
  resolved to inert.** `G_TEXTURE`'s own `level` (decoded by `dl.rs`
  since early on, never read by `mesh.rs`) is nonzero in 241 real asset
  display lists (236×1, 2×2, 3×3) — but every hand-authored `gSPTexture`
  call in the decomp's own engine C code passes `level = 0`, and `level`
  only has an observable effect once `G_TL_LOD` or
  `G_TD_SHARPEN`/`G_TD_DETAIL` is active, both confirmed zero
  archive-wide. The nonzero data is real but unreachable.
* **What this closes.** All three of `R0.5`'s remaining LOD/mipmapping
  acceptance items. This project's own PSP-side `pack_mipped`/
  `sceGuTexLevelMode(Auto)` mip chains are confirmed to be a deliberate
  anti-aliasing technique for dithered CI4 textures (RE-053/070), not an
  attempted reproduction of a real N64 mechanic — there is none to
  reproduce. `R0.5`'s remaining two items ("texture coordinate behavior
  verified", "Dream Land canopy discrepancy resolved") both already need
  independent screenshot verification or `R2`'s real-hardware
  validation, not more `romtool`-side work — `R0.5` stays `IN_PROGRESS`.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. All temporary census code
  (`mesh.rs`'s two `SetOtherModeH` arms and `Cmd::Texture`'s `level`
  check) fully reverted; documentation-only session.
* **Broader task-selection note.** RE-126's "every task is blocked"
  conclusion only covered the tasks it happened to investigate, not
  literally every open `PLAN.md` row — a generalization this session
  caught and corrected for `R0.5` specifically. The next session should
  still re-check the remaining rows before assuming a camera/game-state
  system is the only way forward, per "Current Task" above.

## Earlier Task Status

RE-126 (an earlier session) investigated R0.12's open "orientation verified"/
"scale verified" items and found a real, measured, previously-uncounted
billboard-transform gap. See `docs/reverse-engineering.md` RE-126 for
the full account; summary:

* **Read the real `gcPrepDObjMatrix` algorithm directly**, not trusting
  the enum names already in use. Kinds 44/45/46 build their MVP from the
  pure projection matrix, fully screen-aligned — matching this project's
  existing `billboard_place`. Kinds 47/48 and 49/50 instead build theirs
  from `sGCMatrixMod1F`/`Mod2F`, camera-axis-*locked* LookAt matrices
  (pitch-locked and yaw-locked respectively) — a materially different
  transform this project does not implement separately.
* **Measured `Kind48`'s real archive-wide impact**, which RE-063 had
  only done for `Kind50` (0/3117, confirmed unused) before folding both
  into the same `FLAG_BILLBOARD` path as `Kind46`. A temporary, reverted
  census through the real `romtool pack` build found **47 real `Kind48`
  nodes archive-wide, including Dream Land's own file 104** — the
  *largest* individual billboard category, splitting RE-049's original
  81 into 34 `Kind46` + 47 `Kind48` (43% of all 109 flagged nodes).
* **Not yet visibly wrong on screen, for an identifiable reason.**
  Telling `Kind46`'s screen-aligned approximation apart from `Kind48`'s
  real pitch-locked transform needs a camera that yaws/pitches relative
  to the object; neither RE-049's one forced test rotation nor the
  current, still-face-on debug/gameplay camera ever varies along the
  axis the two transforms disagree on — the same "no actual game camera
  yet" gap `PLAN.md` R0.14 already tracks as open, now with a concrete,
  quantified dependent rather than a hypothetical one.
* **A related, smaller, unconfirmed lead found reading the same code:**
  the real per-axis scale formula multiplies a node's own Y-scale by the
  ancestor chain's cumulative *X*-scale, not this project's composed-
  basis-column-length approach — identical only if every ancestor's own
  scale is uniform, not measured either way this pass.
* **This does not close either open acceptance item** — it replaces two
  unexamined gaps with measured, understood, honestly-still-open ones.
  Fixing `Kind48` needs a pack-format change (preserve which kind a node
  had, not one collapsed bit) plus camera eye/at data this project's
  render call does not have yet. `cargo test --workspace`: 405 passing,
  unaffected (no code changed, only a temporary, fully-reverted census).
* **Broader conclusion for task selection:** every currently
  `IN_PROGRESS`/`VERIFYING` R0.x task (`R0.7`/`R0.4`/`R0.6`, `R0.12`/
  `R0.14`, `R0.13`) is now blocked on either upstream decomp typing or a
  foundational camera/game-state system this project does not have yet,
  not on more `romtool`-side investigation. See "Current Task" above for
  the full reasoning — the next session should read it before picking a
  task rather than assume an easy item remains.

## Earlier Task Status

RE-125 (an earlier session) advanced R0.7's material-table pairing and
fixed a real determinism bug in R0.17's own claim found while
re-verifying it. See `docs/reverse-engineering.md` RE-125 for the full
account; summary:

* **R0.7: systematically re-applied RE-078's own method to the
  "several candidates" bucket, not just already-unique candidates.**
  `tools/mobjtable-ground-truth.py` (existing, previously unused this
  way) emits every decomp-typed `MObjSub **name[N]` table with its real
  address. Cross-referencing all 50 archive-wide ambiguous graphs against
  it (within 8 bytes' slack, the shape RE-078 already found once for file
  84's `PAD(8)`) found 23 candidates landing near a real symbol.
* **Proximity alone is not evidence — independently confirmed each of
  the 23 with `read_table`:** does the graph's own demand vector match at
  the search's reported candidate, and does it also match at the
  decomp's own labeled address? All 23 showed the same shape (candidate
  matches, decomp's own address does not), ruling out coincidence.
* **3 of the 23 rejected on the same evidence standard already in use.**
  File 86 is the identical 27-way-ambiguous NBumper graph RE-061 already
  declined — a match against 1 of 27 is exactly the near-chance
  fingerprint already rejected for this graph, not new evidence. Files
  108/152's one candidate land inside a texture's own trailing pixel
  bytes with no real gap in the decomp at all.
* **The other 20, across 8 files, each have a decomp-documented reason**
  for the gap: explicit `PAD(4)` before a 1-entry table (105, 111, 112,
  157), explicit `NULL` entries the decomp itself declares (104, 152's
  other candidate, 342), or an explicit "combined chain" comment naming
  the exact sub-range (328's `JointVerts_Vtx`, a second real table in
  RE-077's own Kirby file). Inserted via `PartTables::insert()`.
* **Verified.** `romtool mobj`: paired 70 → 90, unpaired 57 → 37,
  mismatches held at **0** across all 407 nodes (up from 383) — every
  inserted pairing self-consistent across its whole node list, not just
  the checked subset. `romtool textures`: packed 646 → 657. Rebuilt the
  shipped pack (5348.1 → 5368.2 KiB); Dream Land's own default screenshot
  is pixel-identical except the HUD's own `tex 0/935` → `tex 0/949`
  readout (file 104 is one of the 20 newly-paired graphs, not previously
  visible from the default camera framing).
* **A real, separate bug found while re-verifying R0.17's own
  determinism claim** against the rebuilt pack. RE-123's "two captures 9
  seconds apart are byte-identical" turned out true but fragile: a hidden
  bug in the debug HUD's own `cpu`/`frame`/`tick` freezing logic (not
  simulation state) only surfaced at different capture timings. Three fix
  attempts each failed differently: pinning to `0` ghosted old, wider
  digits behind a shorter string (`sceGuDebugPrint` does not fully clear
  between calls); freezing the real last-seen values fixed the width but
  not the content (`cpu`/`frame` are genuine wall-clock measurements that
  differ between runs by design, confirmed directly: `cpu 8603us` vs
  `cpu 2603us`, same width, different real value); even a hardcoded,
  safely-wide sentinel (`999999`) still showed ghosted/corrupted digits.
* **Fixed by never drawing the debug HUD at all under
  `regression_capture`**, from frame 0, rather than continuing to
  out-guess `sceGuDebugPrint`'s own internal state — a developer overlay
  was never actually part of the golden scene anyway. Verified
  byte-identical across a 39-second timing spread (`--seconds 6` vs `45`).
  `docs/visual-regression.md` and the committed golden image are both
  updated to this cleaner, HUD-free capture.
* **What this closes:** 20 more of `PLAN.md` R0.7's unpaired graphs
  (70 → 90 archive-wide); strengthens R0.17's "methodology is actually
  run at least once end-to-end" evidence from true-by-luck to true-by-
  construction. `cargo test --workspace`: 405 passing throughout.
  `cargo clippy --release` (both `psp` feature states, and `romtool`):
  no new warnings. **`R0.7` stays `IN_PROGRESS`** (37 unpaired graphs
  remain, an accepted long tail); **`R0.17` stays `COMPLETE`** with a
  stronger evidence base.

## Earlier Task Status

RE-124 (an earlier session) performed R0.18's systematic reference-port
comparison and closed all of its acceptance items. See
`docs/reverse-engineering.md` RE-124 for the full account; summary:

* **Cloned `oot-PSP`** (`https://github.com/z2442/oot-PSP`) into
  `refs/` — previously only `sf64-psp`/`BattleShip`/`n64psp` had been.
  Read both PSP-targeting reference ports' actual graphics-backend
  source directly (`sf64-psp`'s PSPGL/OpenGL-ES wrapper over `sceGu`;
  `oot-PSP`'s direct `sceGu`/`sceGum` calls), not their documentation.
* **Render architecture and culling — (3), explained by D-001, not a
  gap.** Both references are runtime F3DEX2 interpreters that translate
  live N64 display lists every frame; this project converts offline
  (`romtool pack`) per `DECISIONS.md` D-001. Neither reference enables
  GPU-side culling — both do it in software as a byproduct of the
  runtime frustum clipping their architecture needs anyway, which this
  project's baked geometry never requires.
* **Texture filtering — (2), measured, not assumed.** Both references
  conditionally use point vs. bilinear filtering per N64's own bit; this
  project hardcodes `Linear` always, and `mesh.rs` never even decoded
  `G_MDSFT_TEXTFILT`. A temporary, reverted census through the real
  `romtool pack` build found **151/151 real `G_MDSFT_TEXTFILT` commands
  archive-wide request `G_TF_BILERP`** — zero `G_TF_POINT`/`G_TF_AVERAGE`
  — matching the RDP's own per-frame reset default
  (`sSYRdpResetDisplayList`, the same reset RE-068 already found for
  `Z_BUFFER`/`CULL_BACK`/`SHADE`). This project's existing unconditional
  `Linear` filtering is already correct; closes `PLAN.md` R0.5's open
  "filtering modes identified" item in this project's favor.
* **Texture mirroring — (3)/confirmed, independent validation of
  RE-067.** `oot-PSP` independently reached the identical fix this
  project's own RE-067 already shipped (pre-bake a doubled, mirrored
  texture, sample with plain repeat, since the PSP GE has no hardware
  mirror mode). `sf64-psp` took the opposite, cheaper tradeoff (plain
  repeat, visible seam) — confirming RE-067 was a real tradeoff between
  two valid answers, not an obvious gap.
* **Blending — new lead for R0.6, not resolved.** Both references
  successfully ship standard `SRC_ALPHA`/`ONE_MINUS_SRC_ALPHA` blending
  for their own translucent surfaces on the same PSP hardware, ruling
  out "the GE can't do this" as an explanation for R0.6's still-open
  canopy-dithering blend failure (RE-069/071). Whatever is wrong is
  specific to that one dithered CI4-to-RGBA texture's interaction with
  blending, not a platform limitation — added as a lead, no new
  experiment run against the actual texture yet.
* **Lighting/CLUT/combiner approximation — (2)/confirmed, no gap.** All
  three projects pre-bake lighting into vertex colors, load palettes via
  `sceGuClutMode`/`sceGuClutLoad` (or the GL equivalent), and approximate
  the RDP combiner with a small fixed set of recognized shapes rather
  than general per-pixel evaluation — matching this project's own
  RE-065/RE-079/RE-080 approach.
* **Performance technique — lead recorded for `R3`, not implemented**
  (R3 is `BLOCKED_BY_R2`): both references have materially more
  sophisticated state batching (`sf64-psp`'s FNV-1a material-hash batch/
  replay pool measured 177→89 draws/frame; `oot-PSP`'s sampler/shader
  hash caches) than this project's current "skip a redundant `sceGu`
  call" approach — concrete precedent for the state-sorting
  `DECISIONS.md` D-036 already anticipates needing once state fidelity
  (`R0.15`/`R0.16`, both `COMPLETE`) is no longer the open question.
* **What this closes:** all 5 of `PLAN.md` R0.18's acceptance items;
  conclusions cross-referenced into `R0.5` (closed), `R0.6` (new lead)
  and `R3` (new lead). `cargo test --workspace`: 405 passing,
  unaffected — this task touched no shipped code, only a temporary,
  fully-reverted census instrumentation. **`R0.18 — Reference-Port
  Comparative Audit` moves `TODO` → `COMPLETE`.**

## Earlier Task Status

RE-123 (an earlier session) built R0.17's deterministic capture mode
and closed all of its acceptance items. See `docs/reverse-engineering.md`
RE-123 and `docs/visual-regression.md` for the full account; summary:

* **The existing screenshot harness was not actually deterministic.**
  `tools/run-ppsspp.sh --seconds N` waits N *real* seconds, not N
  *simulation ticks*. Mario's idle animation loops forever once he lands
  (`Play::tick_animation`'s own doc comment says as much), and
  `MaterialAnimator`/`StageAnimator` tick once per *rendered* frame, not
  per simulation tick — so both are tied to however fast PPSSPP's
  software rasteriser happens to run on the host at capture time.
  Neither had been checked pixel-for-pixel before.
* **Fixed with a `regression_capture` Cargo feature** (off by default)
  on `ssb64-psp`. A `sim_frame_index` counter tracks simulation ticks
  since boot; once it passes 240 (4 real seconds — comfortably past
  Mario's fall from Dream Land's spawn height), `Play::tick`, the
  object-view skeleton tick, `StageAnimator::tick`, and
  `MaterialAnimator::tick` all freeze. Nothing in the sim is random, so
  a frozen state never changes again regardless of capture timing.
* **A real pitfall found and fixed along the way.** The first version
  also skipped the on-screen debug HUD's `gpu.debug_text` call once
  frozen, to hide its live `cpu`/`frame`/`tick` perf counters (the one
  remaining nondeterministic content). That corrupted PPSSPP's own
  `sceGuDebugPrint` overlay into a stuck, truncated partial-width
  redraw — it is a PPSSPP-only debug HLE hook, not real GE drawing, and
  evidently does not tolerate being called on some frames and never
  again. Fixed by always calling it, every frame, and pinning the three
  volatile values it prints to `0` once frozen instead.
* **Verified.** Two captures 9 real seconds apart
  (`--seconds 6` / `--seconds 15`, both past the freeze point) are
  byte-identical via `cmp` and 0 differing pixels via the new
  `tools/compare-screenshot.sh` (wraps `magick compare -metric AE`,
  threshold documented as 0 given the measured exactness). Golden image
  committed at `tests/golden/r0-dream-land-default.png`.
* **Also produced:** a 17-row test matrix in `docs/visual-regression.md`
  naming concrete assets/files per rendering category (CI4, mirror wrap,
  lighting, depth, culling, etc.), 8 confirmed covered by this one
  scene, the rest honestly tracked as needing further work rather than
  assumed; a documented (not yet fully executed) 4-source capture
  procedure covering PPSSPP software/hardware, physical PSP, and N64
  reference.
* `cargo test --workspace`: 405 passing, unaffected. `cargo psp
  --release` (feature off) and `--release --features
  regression_capture` both build clean; `cargo clippy --release` shows
  the same pre-existing 6-warning set under both.
* **What this closes:** all 6 of `PLAN.md` R0.17's acceptance items,
  including consolidating `TODO.md` Phase H's "Reference renderer /
  Screenshot regression" item and giving R1's "golden/reference renders"
  item a concrete owner. **`R0.17 — Visual Regression Methodology`
  moves `TODO` → `COMPLETE`.**

## Earlier Task Status

RE-122 (an earlier session) closed R0.16's last acceptance item:
checking D-036's ordering rule (state fidelity before batching/dedup)
against every shipped optimization. See `docs/reverse-engineering.md`
RE-122 for the full account; summary:

* **Vertex dedup and material merge are both safe by construction.**
  `Builder::push_vertex` keys on the full post-bake `MeshVertex`;
  `merge_by_material` keys on the full, `derive(Ord)` `MeshMaterial`.
  Neither can silently ignore a field, since the key *is* the whole
  struct, not a hand-picked subset.
* **`tools/romtool`'s `TexKey` was different, and had a real bug.** It
  was a hand-picked 4-tuple `(image_file, image_offset, palette_file,
  palette_offset)` — no wrap/mirror/clamp mode. But `convert_texture`
  pre-bakes a genuinely different, mirrored copy of a texture's bytes
  when `mirror_s`/`mirror_t` is set (RE-067), so two bindings of the
  same image+palette with different wrap modes need different cache
  entries. A temporary, reverted census recording each `TexKey`'s first
  wrap mode and flagging later mismatches found **126 archive-wide
  occurrences** across 19+ files — two different-wrap bindings silently
  sharing one entry, one of them getting the wrong pre-baked bytes/wrap
  flags.
* **Fixed by widening `TexKey` to an 8-tuple** including `mirror_s`/
  `mirror_t`/`clamp_s`/`clamp_t`. The framebuffer-role texture cache's
  own separate sentinel key needed the same widening to stay valid.
* **Verified archive-wide.** Textures `899 → 935` (+36, un-merging
  previously-collapsed variants — not new discovery, same image/palette
  identities just no longer wrongly shared), pack size `5253.2 → 5348.1
  KiB` (+1.8%). `cargo test --workspace`: 405 passing (fix lives in
  `romtool`, not the library crate). `cargo clippy --release
  --workspace`: clean. `cargo psp --release` + `tools/run-ppsspp.sh`:
  Dream Land re-screenshotted clean (pixel-normal, 60 FPS, no panics;
  overlay's own `tex 0/935` confirms the pack regenerated). Not
  independently re-verified against one specific newly-fixed
  fighter/stage screenshot this session — the same caveat RE-102
  recorded for its own structurally similar fix.
* **What this closes:** `PLAN.md` R0.16's D-036 acceptance item, the
  last of its 5 items, is now satisfied. **`R0.16 — N64 Render-State
  Model Fidelity` moves `IN_PROGRESS` → `COMPLETE`.**

## Earlier Task Status

RE-121 (earlier in this session) closed R0.16's second acceptance item:
is any state category silently dropped between `mesh.rs`'s
`MeshMaterial` and `pack.rs`'s on-disk record without a documented
reason? See `docs/reverse-engineering.md` RE-121 for the full account;
summary:

* **Went field by field through all 14 `MeshMaterial` fields** against
  `pack.rs`/`psp/src/meshdraw.rs`'s real consumers, rather than assuming
  the existing fields were already exhaustive.
* **Found one real gap: `blend_color` (`G_SETBLENDCOLOR`, 366
  archive-wide occurrences) is never packed into `PrimDesc`/
  `TextureDesc` at all** — no field, no flag, no documented reason,
  unlike every other field in the struct.
* **Measured whether it matters, rather than assuming either way.** A
  temporary, reverted census checked every real blend equation
  archive-wide for the blend-color register (`G_BL_CLR_BL`) as a colour
  source: **zero occurrences** — the identical shape RE-072 already
  found for fog. `blend_color`'s absence is correct, now measured and
  documented instead of merely unaddressed.
* **Also found and fixed two stale "not yet consumed on the device
  side" doc comments** (`MeshMaterial::texture_blend`,
  `pack::flags::TEXTURE_BLEND`), both contradicted by RE-074 (several
  sessions earlier) having already wired `TEXTURE_BLEND` up — neither
  comment was ever updated afterward. Added missing "inspection only,
  not read back by the device" clarifications to `PrimDesc::prim_color`/
  `env_color` and `pack::flags::LIT`, matching `flat_color`'s own
  existing note.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. Pack rebuild confirmed byte-identical
  to the pre-session baseline. All temporary census code fully reverted.
* **What this closes:** `PLAN.md` R0.16's "no state category is
  silently dropped... without a documented reason" acceptance item is
  now satisfied.

## Earlier Task Status

RE-120 (a previous session) closed RE-119's own open item: does any real
primitive combine `G_SHADE` cleared with a combiner that still reads
`SHADE` — the one scenario that would actually render wrong. See
`docs/reverse-engineering.md` RE-120 for the full account; summary:

* **Confirmed archive-wide, with real file attribution.** A temporary,
  reverted census threaded a `shade: bool` field through `mesh.rs`'s
  `State` (mirroring the existing `cull_back`/`lit`/`smooth`/`z_buffer`
  handling) and checked, at every triangle, whether `!state.shade` while
  `combiner_shade_scale` still resolves. Getting file attribution needed
  a temporary `Source::file_id` field — the first attempt used `romtool
  scan`'s own discovery mechanism and found nothing, because (per
  RE-112) that heuristic differs from the real graph-based
  `convert_sequence` pipeline `romtool pack` actually uses; moving the
  census into the real pipeline found matches immediately.
* **Result: 31 occurrences, concentrated almost entirely in content this
  project does not render at all yet.** 29 of 31 (files 86
  `ITCommonObject`/items, 350 `CaptainSpecial2`, 85
  `EFCommonEffects3`, 353 `LinkSpecial2`) are items, fighter
  special-move effects, and general effects — correctly gated behind
  the combat/item systems `AGENTS.md` §5 blocks. These cannot produce a
  visible defect today since nothing calls into that content at all.
* **2 occurrences affect content already rendered**: one primitive each
  in `StageYosterFile2`/`StageYosterSmallFile2` (Yoshi's Island, both
  variants) — real, currently-live, but extremely narrow (a single
  primitive per stage, not core platform geometry; neither stage
  previously flagged as visually wrong).
* **Not fixed.** The correct real-hardware behavior for `G_SHADE` off
  with a shade-reading combiner isn't well-defined by `gbi.h`'s own
  documentation ("use primcolor to see anything" describes what the
  author should have done, not what hardware actually displays) —
  implementing a fix risks guessing at undefined behavior, the exact
  failure mode `AGENTS.md` §9 warns against. Recorded as a concrete,
  narrow, low-priority open lead.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. Pack rebuild confirmed byte-identical
  to the pre-session baseline after reverting. All temporary code
  (`mesh.rs`'s `shade` field/census, `Source::file_id`,
  `combiner_shade_scale`'s brief `pub` bump) fully reverted; `git diff
  --stat` is empty — this session is documentation-only.
* **What this closes:** `PLAN.md` R0.16's "every state category has an
  explicit field or documented reason" acceptance item is now satisfied
  for `G_SHADE` — its real impact is measured, attributed by file, and
  classified, not left as an unexamined risk.

## Earlier Task Status

RE-119 (a previous session) started R0.16 from its own first acceptance
item — R0.2's opcode inventory — rather than guessing which state
categories needed auditing. See `docs/reverse-engineering.md` RE-119 for
the full account; summary:

* **Fixed a real bug in `romtool`'s own diagnostic tooling.** A fresh
  `romtool scan` showed geometry-mode bit `0x00000004` occurring 60
  times with no name printed. `geometry_mode_name` had `G_SHADE` mapped
  to `0x2`, disagreeing with `refs/ssb-decomp-re`'s real `gbi.h`
  (`0x4`). Fixed the one-line constant.
* **The whole opcode table in `docs/rendering.md` had gone stale, not
  just `G_SHADE`'s label.** Every count had drifted since R0.2's
  original measurement (e.g. `G_TRI2` 10954 → 13523) — the same 135
  files/1,864 lists now parse into more triangles (22,515 → 28,089)
  thanks to later conversion-fidelity fixes. More significantly,
  `G_MOVEWORD` was still listed under "Never emitted", flatly
  contradicted by its current count (3,722) and by RE-105 (a much
  earlier session) already relying on real `G_MW_LIGHTCOL` usage.
  Refreshed the whole table.
* **Found two geometry-mode categories genuinely used by SSB64 with
  zero handling in `mesh.rs`, previously absent from the docs
  entirely.** `G_SHADE` (60 occurrences) — always cleared together with
  `G_LIGHTING`/`G_SHADING_SMOOTH` in the same command, never re-set in
  that command (checked archive-wide via a temporary, reverted census),
  consistent with a deliberate flat/unlit switch this project's existing
  `combiner_flat_color`/`combiner_texture_blend` detection likely
  already reproduces correctly — but not yet cross-referenced
  per-primitive against combiner shape, the one scenario that would
  actually render wrong. `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR`
  (156/13 occurrences) — used by Metal Mario's stage and
  `MMarioModel`/`NMarioModel`/`NFoxModel`: the "Metal [Character]"
  transformation's environment-mapped shiny effect. Genuinely needed,
  but correctly deferred (an item-pickup effect, downstream of the
  combat/item systems this project's own rendering gate blocks) — not
  an `ACCEPTED_DEVIATION` since it's technically reproducible on the PSP
  GE, just out of scope until items exist.
* `cargo test --workspace`: 405 passing, unaffected (diagnostic label
  and documentation only, no `ssb-rom`/`psp` logic changed). `cargo
  clippy --release --workspace`: clean. No pack rebuild needed
  (`geometry_mode_name` is display-only, not used by pack-building). All
  temporary census code fully reverted; `git diff --stat` shows only the
  permanent one-line bit-value fix.
* **What this closes:** `PLAN.md` R0.16's "docs/rendering.md's state-mapping
  table is complete against this audit's findings" item is satisfied —
  every category the refreshed scan surfaced now has handling, a
  documented deferral reason, or a named open cross-reference; nothing
  is silently missing. The "every state category has an explicit field
  or documented reason" item stays open pending `G_SHADE`'s own
  per-primitive cross-reference. `R0.16` moves `TODO` → `IN_PROGRESS`.

## Earlier Task Status

RE-118 (a previous session) picked up R0.15's remaining thread: audit
`psp/src/meshdraw.rs::DrawState`'s own device-side GE cache, the layer
`mesh.rs`'s decode-time state (RE-117) doesn't cover. See
`docs/reverse-engineering.md` RE-118 for the full account; summary:

* **Read `apply_material`/`bind_texture` end to end against every
  category.** Culling, shading, depth test and alpha test are each set
  inside an explicit if/else with no skipped branch — no leak possible.
  A stale `sceGuTexLevelMode` or CLUT left from a previous texture are
  both inert (the GE clamps LOD to the mip count `sceGuTexMode` just
  declared; non-indexed formats never consult CLUT). `GuState::Blend` is
  confirmed never enabled anywhere in the crate (grepped, not assumed).
* **Found one real, new gap.** `Gpu::draw_triangles`/`draw_line_strip`
  disable `Texture2D` directly, bypassing `DrawState` entirely.
  `draw_collision`/`draw_fighter` (the collision-line and
  simulated-fighter-marker overlays, both calling `draw_line_strip`) run
  *between* two cached mesh draws whenever `show_collision`/`sim_fighter`
  are on — which is the default (`main.rs`'s own initial values). A
  primitive drawn afterward that happens to share a texture index with
  whatever was bound before the overlay (the pack dedups textures by
  content, so this is plausible, not guaranteed) would wrongly stay
  untextured.
* **Checked whether this manifests visibly before fixing it — it does
  not, and that's not the same as the bug being fake.** Zoomed into the
  simulated fighter model in Dream Land's default view (the exact
  triggering code path): renders fully textured. This only shows Dream
  Land's own last texture and the fighter's own first texture don't
  happen to coincide — the underlying cache-invariant violation is a
  structural fact independent of today's specific scene.
* **Fixed by invalidating the cache, not restoring prior state.** Added
  `DrawState::forget_texture()` (clears `last_texture` only), called at
  the end of `draw_collision` (if it drew anything) and `draw_fighter`
  (always). Forces the next primitive to always rebind for real.
* `cargo test --workspace`: 405 passing, unaffected (fix lives entirely
  in the `psp` crate). `cargo clippy --release --workspace`: clean.
  `cargo psp --release`: builds clean. `tools/run-ppsspp.sh`: Dream Land
  re-screenshotted pixel-identical to the pre-fix baseline (same overlay
  counts, same fighter-model crop) — confirms the fix is inert for the
  currently-non-triggering case.
* **What this closes:** `PLAN.md` R0.15's "state leakage tests added"
  item is now satisfied for both layers. **`R0.15 — Render-State
  Isolation` moves `IN_PROGRESS` → `COMPLETE`.**

## Earlier Task Status

RE-117 (a previous session) surveyed before writing any test, rather than
guessing which R0.15 categories needed coverage. See
`docs/reverse-engineering.md` RE-117 for the full account; summary:

* **One shared mechanism, one existing test.** Every render-state
  category R0.15 names lives in a single `State` struct `convert_sequence`
  constructs exactly once per scene graph, then mutates in place across
  every node — by construction, nothing resets between nodes except
  `State::forget_texture`'s narrow, intentional, image-only clear. Only
  texture image binding (RE-064) had a direct cross-node persistence
  test; the other nine categories had only single-list "sets correctly"
  tests, not "survives/resets correctly across a node boundary" tests.
* **Texture addressing was already covered, just undocumented.**
  `TextureRef` (`derive(PartialEq, Eq)`) bundles `mirror_s`/`mirror_t`/
  `clamp_s`/`clamp_t`/dimensions/palette fields together, and RE-064's
  own assertion already compares whole `TextureRef` structs with
  non-default `mask`/`cm` values set in its own test — confirmed this
  precisely (checked the derive and full field list) rather than assumed.
* **Four new tests close the remaining categories:**
  `a_palette_binding_survives_a_new_image_bind_without_a_new_tlut_load`
  (TLUT — the direction RE-093 never covered: image changes, palette
  must persist), `combiner_and_colour_constants_persist_into_a_node_
  that_sets_none_of_them` (combiner + primitive + environment color, via
  Link's own real combiner word), `render_mode_persists_into_a_node_
  that_sets_no_new_render_mode` (blend/alpha, via a real translucent
  render-mode word), `geometry_mode_persists_into_a_node_that_sets_no_
  new_geometry_mode` (depth + culling + geometry/lighting mode, all four
  bits in one test).
* **Verified capable of failing.** A temporary, reverted change to
  `convert_sequence` (rebuilding `State::new()` every loop iteration
  instead of reusing one) made all four new tests fail with the expected
  mismatch, plus two pre-existing tests (the vertex cache and RE-064's
  own texture test) — confirming this is one shared mechanism, not
  independent per-category logic. Reverted before committing.
* `cargo test --workspace`: 266 `ssb-rom` (405 total workspace, was
  401). `cargo clippy --release --workspace`: clean. Rebuilt pack:
  byte-identical to baseline (test-only change, no conversion-logic
  change). `cargo psp --release` + `tools/run-ppsspp.sh`: Dream Land
  re-screenshotted clean (pixel-normal, 60 FPS, no panics).
* **What remains:** the PSP-side `psp/src/meshdraw.rs::DrawState`'s own
  GE draw-state cache (`last_texture`/`last_flags`/`last_texture_blend`)
  is a second, distinct layer this task's objective also covers — RE-074
  already found and fixed one real bug there incidentally (not from a
  dedicated audit). `PLAN.md` R0.15 moves `TODO` → `IN_PROGRESS`; its
  "state leakage tests added" item stays open pending that second
  layer's own audit.

## Earlier Task Status

RE-116 (a previous session) picked up R0.13's one remaining thread: root-cause
file 46's diagonal black banding. See `docs/reverse-engineering.md`
RE-116 for the full account; summary:

* **A close pixel scanline showed a smooth blend, not a hard cut.**
  Sampling across a "black" band pixel-by-pixel found values like
  `(74, 32, 93)`, `(157, 17, 168)` — each one sits exactly on the linear
  interpolation between the real background `(32, 40, 56)` and the real
  magenta capture `(255, 0, 255)` on all three channels simultaneously.
  This is anti-aliased polygon-edge blending, not a sampling/capture
  error (a real defect would not reproduce two independent channels'
  worth of exact linear interpolation).
* **An exhaustive, bounding-box-restricted census found zero pure
  `(0, 0, 0)` pixels** in either of file 46's two rendered squares —
  every pixel checked, not sampled.
* **RE-113's own "116,152 genuine black pixels" figure was itself the
  bug.** Recomputing it found it came from a whole-image `Counter` scan
  with no bounding-box restriction at all — the exact window-decoration
  confound RE-111 had already identified and documented. RE-113 asserted
  the opposite ("not the window-border artifact RE-111 already
  identified") without re-deriving the bounding box for file 46's own
  screen position.
* **Confirmed RE-115's culling fix is unrelated.** Temporarily set
  `force_no_cull = false` (the pre-RE-115 behaviour) and re-rendered:
  pixel-identical to the fixed behaviour. File 46's primitives were never
  being culled either way.
* **The diagonal `U`-shifting pattern is real, correct ROM data.** RE-113's
  own finding (an 11-step shifting/full-width cycle per strip) is very
  likely file 46's own authored diagonal-wipe shear; the diagonal magenta
  bands with soft anti-aliased gaps are the correct rendered result for
  that shape, not a bug in how RE-113 characterized the underlying data —
  only in treating the *rendered result* as defective.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. Default build re-screenshotted clean
  (Dream Land pixel-normal, 60 FPS, no panics). All temporary code fully
  reverted; `git diff --stat` is empty — this session is
  documentation-only.
* **What this closes:** `PLAN.md` R0.13's "visual verification
  completed" item is now fully satisfied — **all 13 LB-transition files
  are confirmed correct on the real device.** R0.13's only remaining open
  items (`screen wipes implemented`, `framebuffer synchronization
  verified`) are both blocked on this project having no real
  match-transition/game-state system yet, not on further rendering
  investigation — R0.13 cannot progress further until upstream game-state
  work exists, which is itself gated behind the rendering-correctness
  milestone this task belongs to.

## Earlier Task Status

RE-115 (a previous session) picked up the camera-framing gap RE-109 first
recorded and RE-113/114 left as the remaining blocker for files 41, 43,
50. See `docs/reverse-engineering.md` RE-115 for the full account;
summary:

* **The camera was never the problem.** File 41's `cam`/`r` overlay
  readout was always sane and non-degenerate (`cam 5301 r 1860`), and
  `draws`/`tris` were non-zero every attempt. Disabling
  `GuState::CullFace` entirely (a temporary, reverted test) made it
  visible immediately, first try.
* **Root cause: one-sided authored planes, and an inspection camera with
  no guarantee of viewing them from their front side.** Real gameplay
  always looks at a `CULL_BACK` surface from its authored front (a real
  camera has a fixed relationship to what it's pointed at); the debug
  viewer's free-roaming `object_view` auto-framing camera does not. Files
  39/40/42/44/45/47/48/49/51 happen to have their front face toward the
  default angle; 41/43/50 do not. Never a material/UV/capture bug — the
  same shape of conclusion RE-108 reached for the "backing quad", this
  time for a correctly-identified different cause.
* **Fixed narrowly.** Added `DrawState::force_no_cull`
  (`psp/src/meshdraw.rs`), checked in `apply_material`'s existing
  per-primitive cull decision. `psp/src/main.rs` sets it to `object_view`
  once per frame, right after `draw_state.begin_frame()` — active only
  during the debug viewer's own inspection mode. Real gameplay rendering
  (`stage_view`, fighter simulation) never sets it; `apply_material`'s
  ordinary culling (RE-068's verified `CULL_BACK`/`CULL_FRONT`
  reproduction) is unchanged for everything else.
* **Verified on all three previously-blocked files.** File 41 (object
  13) and file 43 (object 15, both of its two quads): clean, uniform
  magenta, zero `(0, 0, 0)` pixels by direct pixel scan. File 50 (object
  22): confirmed correct by direct on-device observation in the live
  PPSSPP window — a series of automated screenshot attempts could not
  reliably catch this specific file's correct frame (a screenshot-timing
  tooling limitation, not a rendering defect); not chased further once
  directly confirmed working.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. Default (non-transition) build
  re-screenshotted clean (Dream Land pixel-normal, 60 FPS, no panics) —
  correctly unaffected, since Dream Land uses `stage_view`, never
  `object_view`. `git diff --stat` against the previous commit shows only
  the permanent `force_no_cull` mechanism.
* **What this closes:** `PLAN.md` R0.13's "visual verification
  completed" item now has only one remaining open item: file 46's
  diagonal-banding defect (RE-113). **12 of 13 transition files are now
  fully verified correct on the real device.** The camera-framing gap,
  open since RE-109, is closed for good — structurally, for any future
  one-sided object browsed via `object_view`, not just these three files.

## Earlier Task Status

RE-114 (a previous session) finished the file-by-file screenshotting RE-113
left unaddressed (`39, 48, 50, 51`), using the same recipe. See
`docs/reverse-engineering.md` RE-114 for the full account; summary:

* **Three more files confirmed fully correct.** File 39 (object 11,
  `spin = 0`, the same 8-node "sudare" shape as file 45) rendered clean,
  uniform magenta, zero black. File 51 (object 23) rendered as an
  8-pointed radial "starburst" matching its circular node layout
  (`--nodes` census: 8 nodes on a circle), zero black. File 48 (object
  20, the one structurally distinct 30-node/29-dl outlier) rendered as a
  scattered ~29-panel cluster matching its particle-like layout, zero
  black.
* **File 50 (object 22) hits the same camera-framing gap as 41 and 43.**
  Tried `spin = 0` (file 45's working value) and `spin = π`; neither
  brought it into view despite drawing (`draws 352`, `tris 704`, matching
  its 8-tower structure). Not investigated further — same
  already-tracked, separate limitation.
* **All 13 transition files are now accounted for**, not just partially
  screenshotted: 9 confirmed clean (`39, 40, 42, 44, 45, 47, 48, 49, 51`),
  3 blocked on the camera-framing gap (`41, 43, 50`), 1 with RE-113's
  still-open diagonal-banding defect (`46`).
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. Default build re-screenshotted clean
  (Dream Land pixel-normal, 60 FPS, no panics) after reverting. All
  temporary code (`psp/src/main.rs`'s forced object/spin/capture patch,
  cycled across objects 11/22/23/20) fully reverted; `git diff --stat`
  is empty — this session is documentation-only.
* **What this closes:** `PLAN.md` R0.13's "visual verification
  completed" item now has a complete accounting of every one of the 13
  files. The item itself cannot close yet — the camera-framing gap and
  file 46's defect are both still open — but no file remains unexamined.
  The two concrete remaining threads are independent and well-scoped:
  fix the debug viewer's auto-framing camera (unblocks 41/43/50), and
  root-cause file 46's diagonal banding.

## Earlier Task Status

RE-113 (a previous session) picked up RE-112's own handoff — screenshot the
remaining 12 transition files — starting with the six structurally
simple ones (1–2 nodes: `42, 43, 44, 46, 47, 49`). See
`docs/reverse-engineering.md` RE-113 for the full account; summary:

* **Four files fully confirmed correct, first time for any of them.**
  Files `44` (object 16), `42` (object 14), `47` (object 19) and `49`
  (object 21), tested with the same `spin = 0` /
  magenta-clear-and-capture-at-frame-30 recipe RE-110/RE-111 established,
  each rendered a clean, uniform magenta shape with zero `(0, 0, 0)`
  pixels in its own screen region (direct pixel scan). File 42 renders as
  a diamond — an authored shape difference, not a defect. **6 of 13 files
  now have real on-device evidence** (`40, 42, 44, 45, 47, 49`), up from 2
  at session start.
* **File 43 (object 15) hit RE-109's already-documented camera-framing
  limitation**, not a new issue: two widely-separated nodes drew
  (non-zero draw count) but nothing appeared on screen at `spin = 0`.
* **File 46 (object 18) is a real, new, distinct defect.** Both its nodes
  rendered as visible squares, but each showed alternating
  magenta/pure-black *diagonal* stripes instead of a uniform capture
  colour — 116,152 genuinely pure-`(0, 0, 0)` pixels by full-resolution
  pixel census, not the window-border artifact RE-111 already identified
  and ruled out as unrelated screenshot-tooling noise (that artifact sits
  at the image's outer edges, not inside an object's own silhouette). A
  temporary, reverted `romtool` census of its baked UV data ruled out the
  V-axis/pillarbox mechanism (every primitive's `V` range is identical to
  file 45's already-fixed shape); the difference is in `U`, which cycles
  through an 11-step shifting/full-width pattern per strip as `origin_t`
  advances — very likely the ROM's own authored diagonal-wipe shape, not
  a decode error. What produces solid black at the narrowed edge of each
  shifted band was not isolated this session — recorded as a concrete,
  characterized, reproducible lead.
* `cargo test --workspace`: 405 passing, unaffected. `cargo clippy
  --release --workspace`: clean. Default (non-transition) build
  re-screenshotted clean (Dream Land pixel-normal, 60 FPS, no panics)
  after every revert. All temporary code (`tools/romtool/src/main.rs`'s
  object-index lookups and file-46 UV census, `psp/src/main.rs`'s forced
  object/spin/capture patch cycled across objects 14/15/16/18/19/21)
  fully reverted; `git diff --stat` is empty — this session is
  documentation-only.
* **What this closes:** `PLAN.md` R0.13's "visual verification completed"
  item: 6 of 13 files now have real on-device evidence. One new,
  distinct, characterized defect (file 46) is open, separate from
  RE-111's already-fixed pillarbox bug and RE-109's camera-framing gap
  (which file 43 also hits). 5 files remain unscreenshotted
  (`39, 41, 48, 50, 51`; 41 also blocked on the camera-framing gap).

## Earlier Task Status

RE-112 (a previous session) checked whether the backing quad is even reachable
by the renderer at all, instead of eliminating a fifth GE state. See
`docs/reverse-engineering.md` RE-112 for the full account; summary:

* **`romtool scene --file 45 --list --nodes` settled it directly.** File
  45's one scene graph has 9 nodes, 8 with a display list, and all 8 are
  the photo towers already confirmed correct. None of the "backing"
  offsets appear in this object's node list, and `add_object` folds
  extra-leaf pre/post-pair lists into the same `node_count` the debug
  overlay reads (`nodes 9 placed 8`, unchanged all along) — 17 would show
  if they were attached extra-leaf siblings. They are attached to nothing.
* **A scan-inventory census explained where they come from.**
  `crates/ssb-rom/src/scan.rs::find_root_display_lists`'s "outermost
  list" dedup advances its coverage boundary using a kept list's own
  literal decoded byte span, not the larger range it actually renders via
  an inlined `Call`. File 45's tiny 9-word dispatch list (`0x1950`) calls
  into each tower's real 310-word body (`0x1998` etc.) — `mesh::convert`
  correctly inlines that when asked to convert from `0x1950` (the real,
  704-triangle mesh RE-111 verified), but the scan's *independent* attempt
  to decode starting at `0x1998` itself fails (no `G_VTX` in its own
  window, since that lived in `0x1950`'s body before the `Call`) and is
  correctly rejected — except the *tail* ~40 bytes of that same body
  (`0x2320`, matching RE-108's already-identified "300×5, drawn once"
  sub-tile — the *same* triangles as primitive 0 of the real mesh) happens
  to be its own self-contained `G_VTX`+`G_TRI`+`G_ENDDL`, so it
  independently re-decodes as a second, spurious "root" list — untextured,
  because the real `G_SETTIMG` lived earlier in the true list, outside
  this tail window.
* **Impact: pack-time waste, not a rendering bug.** These duplicates get
  packed by `pack()`'s own "discovery" fallback loop (meant to catch
  genuinely un-named lists), inflating mesh/texture/triangle counts by a
  small, unquantified amount archive-wide — not measured this session.
  They are never attached to any object's node list, so
  `draw_object`/`draw_object_posed` can never emit them; no on-device fix
  applies, and none was needed.
* **This retracts RE-107/108/110/111's entire "backing quad" line of
  questioning, not just RE-110's specific attempt.** There was never a
  second, real backing primitive rendering (in)correctly on file 45's
  object. Every prior session's elimination (colour, culling, depth test,
  texture-state caching, shade model, alpha test) correctly proved the
  primitive is never visible — for the right reason (never submitted to
  the GE when drawing object 17), just not the reason assumed until the
  node-list check above.
* **Not fixed this session, deliberately.** `find_root_display_lists` is
  shared well beyond R0.13 (opcode inventory, stage-animation discovery,
  others); extending its containment check to follow `Call`/`Branch`
  targets transitively is a real, scoped fix, but touches every file's
  discovered-list inventory archive-wide and needs the same
  archive-wide before/after measurement this project always requires for
  a shared-function change. Recorded as a concrete, reproducible lead.
* `cargo test --workspace`: 405 passing, unaffected (investigation-only).
  `cargo clippy --release --workspace`: clean. Default build
  re-screenshotted clean (Dream Land pixel-normal, 60 FPS, no panics)
  after reverting. All temporary code (`meshdraw.rs`'s forced-off
  depth-test/cull/alpha-test overrides, `psp/src/main.rs`'s forced
  object-view patch, `tools/romtool/src/main.rs`'s position and
  scan-inventory censuses) fully reverted; `git diff --stat` is empty
  relative to RE-111's commit — this session is documentation-only.
* **What this closes:** `PLAN.md` R0.13's "visual verification completed"
  item no longer carries an open defect for file 45 at all — its only
  reachable geometry (8 photo towers) is fully confirmed correct. The
  remaining work is unchanged in kind (screenshot the other 12 files) but
  no longer has an asterisk next to file 45.
* **Addendum:** `romtool scene --list --nodes` across all 13 files found
  file 45's exact 9-node/8-dl shape in files 39, 41, 50, 51 too (files 40,
  48 and 42/43/44/46/47/49 all have genuinely different structures).
  Attempted a second on-device file (41, object 13) to extend visual
  verification beyond file 45 — object selection and draws worked, but
  nothing was visible at `spin = 0` or `π/2`, the same debug-viewer
  camera-framing limitation RE-109 already documented for screen-covering
  objects, not a new issue. Not chased further this session (fixing the
  viewer's camera is RE-109's own separate, already-recorded lead). All
  temporary code reverted; `git diff --stat` empty.

## Earlier Task Status

RE-111 (a previous session) picked up RE-110's own fresh lead directly: the
backing quad (raw colour `[255,255,255,0]`) reproducing RE-107's original
"renders solid black" finding now that RE-109's fix made the photo tile
correct. See `docs/reverse-engineering.md` RE-111 for the full account;
summary:

* **The backing quad still never painted a single visible pixel.** A
  targeted, reverted `pack.rs` hack recoloured only untextured primitives
  with the exact raw colour to screaming green (narrower than RE-108's
  archive-wide version, since a `romtool` census found the *photo*
  primitive's own vertices share that identical raw colour as a modulate
  identity — recolouring those too would have corrupted the texture
  instead of isolating the backing quad). No visible change. RE-110's own
  attribution was wrong, the same way RE-108 once corrected RE-107's.
* **A `romtool` census found the real structure.** File 45's object is 8
  side-by-side vertical strips, each an independent 44-primitive "photo
  tower" (all `ROLE_FRAMEBUFFER`) plus a separate 1-primitive backing
  strip below it — not "one photo primitive plus one backing primitive"
  as earlier sessions described. All 8 towers' baked UVs are
  byte-for-byte identical post-RE-109-rebase, ruling out the
  material/UV/vertex-colour pipeline entirely.
* **Two decisive tests isolated the real cause.** (1) Pre-filling
  `TRANSITION_PHOTO` with solid magenta and skipping the real capture:
  the object rendered 100% uniform magenta, zero black — proves the
  rendering/sampling pipeline is correct. (2) Restoring the real capture
  and disabling `ScissorTest` around the debug clear, correctly timed
  *inside* `Gpu::begin_frame`'s open display list (an earlier,
  incorrectly-timed attempt outside any open list had no effect — itself
  worth knowing): same result, 100% uniform magenta.
* **Root cause: the permanent 4:3 pillarbox scissor.** `Gpu::new` scopes
  every draw, including `sceGuClear`, to the pillarboxed viewport
  (`vx = 59`, `vw = 362` of the raw 480-wide buffer) and enables
  `ScissorTest` permanently. Columns `0..59` are never drawn to by
  anything, ever, and sit at their power-on-zeroed (black) value for the
  program's whole life, by design (the setup code's own comment: "nothing
  bleeds into the black bars"). `capture_transition_photo` read
  `TRANSITION_PHOTO_WIDTH` (300) columns starting at absolute column 0,
  not the pillarbox's own left edge — 4 of the 8 towers' `u` ranges fall
  in that permanently-black slice. This is a real bug independent of the
  debug recipe: a real transition capture in real gameplay would hit the
  same bar, since nothing this project draws ever reaches those columns.
* **Fixed with a one-line offset**
  (`BUF_WIDTH * y + pillarboxed_viewport().0`), not a re-tuned capture
  size — `TRANSITION_PHOTO_WIDTH` (300) already fits inside the
  pillarboxed width (362) from that edge. Re-verified with the real
  capture and no diagnostic overrides: a direct pixel scan of the
  object's own screen region found zero `(0, 0, 0)` pixels.
* **Also tested and eliminated:** whether `sceGuDebugFlush`'s HUD-text
  paint (which currently runs before the capture in `end_frame`)
  contaminates the captured corner — temporarily reordered, no measurable
  difference, reverted to the original order.
* `cargo test --workspace`: 405 passing, unaffected (fix lives entirely
  in the `psp` crate, no host-runnable unit tests there).
  `cargo clippy --release --workspace`: clean. Default (non-transition)
  build re-screenshotted clean (Dream Land pixel-normal, 60 FPS, no
  panics) after every revert and again after the final fix. All temporary
  code (`pack.rs`'s targeted recolour hack, `tools/romtool/src/main.rs`'s
  file-45 census, `psp/src/main.rs`'s forced object/spin/capture patch,
  `gu.rs`'s synthetic-buffer and scissor-toggle experiments) fully
  reverted; `git diff --stat` shows only the permanent fix in
  `psp/src/gu.rs`.
* **What this closes:** `PLAN.md` R0.13's "framebuffer texture paths
  implemented" item now covers a second, independent, real bug in the
  same mechanism, fixed and device-verified. File 45's photo towers are
  fully confirmed correct now (all 8, not just some). "Visual
  verification completed" stays open — 11 of 13 files remain
  unscreenshotted, and the backing quad's own on-screen appearance is
  still genuinely unobserved after four sessions, not merely unresolved.

## Earlier Task Status

RE-110 (a previous session) picked up RE-109's own addendum lead directly:
force a small fixed set of exact `spin` values across separate runs
instead of relying on elapsed real time. `spin = 0.0` (a temporary,
reverted constant in `psp/src/main.rs`, alongside the same `object_view`/
`object_index = 17`/magenta-capture recipe RE-100/RE-107/RE-108/RE-109
all used) was the first value tried and was decisive immediately — no
sweep needed. See `docs/reverse-engineering.md` RE-110 for the full
account; summary:

* **The fix works, confirmed by direct pixel measurement.** File 45's
  transition object at `spin = 0` shows a large solid magenta region
  (`(255, 0, 255)`, 25,778 sampled pixels) exactly where RE-108's own
  investigation found solid black before.
* **A second, real, spatially distinct region measured, not
  background.** Sampling the screenshot broadly found three real colour
  populations: background `(32, 40, 56)` (57,097 px), pure black
  `(0, 0, 0)` (34,584 px, genuinely rendered — measurably different from
  the clear colour), and the magenta capture (25,778 px).
* **This reopens, not resolves, RE-107's original mystery.** RE-107
  first found the backing primitive (raw colour `[255,255,255,0]`,
  white) rendering solid black with every known colour mechanism
  (`prim_color`, `flat_color`, RE-103's per-vertex lit-normal fallback)
  ruled out by direct evidence, and left it unexplained. RE-108 then
  retracted the *attribution* — proving via a green-forcing hack that
  the backing quad "never painted a single visible pixel" in RE-108's
  own tests, and that the black region everyone had been looking at was
  actually the (now-fixed) photo tile. With that fixed, this session is
  the first time the backing quad's own on-screen appearance has
  actually been isolated — and it independently reproduces RE-107's
  original finding.
* **Deliberately not chased further this session.** A different root
  cause from the UV/capture-origin gap RE-108/RE-109 addressed, and
  already a multi-session investigation once (RE-107→RE-108) before
  being retracted as a misattribution. Recorded as a fresh, concrete,
  reproducible lead for a dedicated future investigation, not guessed at
  same-session.
* All temporary code (`psp/src/main.rs`'s frame counter, magenta clear,
  forced object/view-mode override, fixed-`spin` constant) was fully
  reverted; `git diff --stat` on `psp/src/main.rs` is empty. Default
  build re-screenshotted clean (Dream Land pixel-normal, 60 FPS, no
  panics) after reverting.
* **What this closes:** `PLAN.md` R0.13's "framebuffer texture paths
  implemented" item now has real device evidence for RE-109's fix, not
  only a unit test and packed-byte diff. "Visual verification completed"
  stays open — 11 of 13 files remain unscreenshotted, and the newly
  re-isolated backing-quad defect is itself a new, unresolved gap.

  **Retracted by RE-111 (a later session): this attribution was wrong
  too.** The "second, real, spatially distinct" black region named here
  was never actually the backing quad — see "Task Status" above for the
  real cause (the pillarbox scissor leaving part of the raw framebuffer
  permanently black, unrelated to material/vertex-colour handling).

## Earlier Task Status

RE-109 (a previous session) picked up RE-108's own two recorded fix candidates
and shipped the one RE-108 itself judged more general: rebasing each
framebuffer-role primitive's baked UV by its own tile's `uls`/`ult` origin
at pack time, rather than trying to guess a second capture band. See
`docs/reverse-engineering.md` RE-109 for the full account; summary:

* **Root cause confirmed before implementing.** `crates/ssb-rom/src/
  mesh.rs`'s `Cmd::SetTileSize` handler decoded `uls`/`ult` but only ever
  used them to compute `tile_dims` (width/height), discarding the origin
  itself — exactly the gap RE-108 named. Ordinary textures never needed
  it (pack-time extraction already starts at the tile's own origin);
  framebuffer-role textures do, because their synthetic small capture
  always starts at its own row/column 0 regardless of the tile's real
  absolute position in the conceptual 300×220 image.
* **Implemented via the existing per-vertex bake mechanism**, not a new
  one: `State::tile0_origin` → `TextureRef::origin_s`/`origin_t` →
  subtracted from the vertex UV in `Builder::push_vertex`, the same place
  `prim_color`/`texture_blend`/`flat_color` already bake adjustments
  before the content-keyed vertex dedup runs.
* **New unit test** (`a_framebuffer_role_tile_not_at_the_origin_has_its_
  uv_rebased`) reproduces RE-108's own exact real numbers (file 45's
  300×5 tile, `ult = 860`) and was confirmed capable of failing (removed
  the fix, reran, confirmed the exact `860*8` discrepancy, restored it).
* **Verified real archive-wide effect**, not just the unit fixture: built
  the real pack twice (with/without the fix) and diffed the two `.pak`
  files directly — 3,572,132 bytes differ, size 5165.9 → 5253.2 KiB
  (+87.3 KiB, an expected dedup-correctness side effect: two framebuffer-
  role vertices that previously collided in the content-keyed dedup by
  sharing an absolute UV now correctly diverge once rebased per tile).
* `cargo test --workspace`: 262 `ssb-rom` (405 total workspace), all
  passing. `cargo clippy --release --workspace`: clean. Default
  (non-transition) build re-screenshotted clean (Dream Land pixel-normal,
  60 FPS, no panics) both before and after.
* **On-device visual re-verification attempted, not achieved.** Followed
  RE-100/RE-107/RE-108's own recipe (temporary, fully reverted
  `psp/src/main.rs` patch: force `object_view`, `object_index = 17`
  [file 45], magenta-clear + `request_transition_capture()`). Object
  selection worked (overlay confirmed `file 45 ... tris 704`, no panic,
  60 FPS), but the debug viewer's generic `object_view` auto-framing
  camera — built for ordinary models, not a screen-covering "transition
  wipe" plane — never brought the primitive into visible frame across two
  attempts (3s and 8s, well past enough rotation to rule out a momentary
  edge-on angle). RE-107/RE-108 used the identical mechanism successfully
  before on this exact object, so this is a this-session-specific viewer
  gap, not evidence against the fix. `git diff --stat` on
  `psp/src/main.rs` is empty (fully reverted).
* **What remains:** `visual verification completed` stays unchecked — a
  fix backed by a unit test and a packed-byte diff is not the same as a
  device screenshot showing the previously-black region now correct. The
  concrete next step is fixing the debug viewer's camera framing for
  screen-covering objects (or writing a bespoke close-in test camera),
  not re-attempting the same generic path unchanged. The other 11 (of
  13) transition files also remain unscreenshotted.
* **Addendum, same session:** measured *why* the camera framing fails
  instead of leaving it a mystery. A temporary, reverted `romtool`
  subcommand dumped file 45's real vertex bounds directly: every one of
  its 9 display lists has `z 0..0` exactly — a flat plane in its own
  local `XY` plane, confirming "screen-covering transition wipe" as data,
  not inference. This rules out a gross framing/bounding-sphere bug but
  does not fully explain the invisibility (backface culling on a
  `z`-normal plane is invisible across a full 180° hemisphere, and a
  baked node rotation `romtool scene --nodes` doesn't print could also be
  involved). Concrete next step recorded in `docs/reverse-engineering.md`
  RE-109's addendum: force a small fixed set of exact `spin` values
  (`0`, `π/2`, `π`, `3π/2`) across separate runs rather than relying on
  elapsed real time. `git diff --stat` on `tools/romtool/src/main.rs` is
  empty (temporary dump subcommand fully reverted).

## Earlier Task Status

RE-108 (a previous session) picked up exactly where RE-107 left off: a
backing primitive supposedly rendering solid black on the real device
despite genuinely non-black raw vertex colour, with `prim_color`,
`flat_color`, and RE-103's per-vertex lit-normal fallback already ruled
out by direct evidence. Continued eliminating on the real device rather
than guessing, one isolated variable at a time, each change temporary
and reverted before the next: forced the backing primitive's vertex
colour to screaming green (confirmed present in the built `.pak` by
grepping for the packed byte pattern) — no visible change. Ruled out
backface culling, depth testing, stale texture-state caching, and shade
model the same way, each individually, each with the green-forcing hack
still active. None of the seven eliminations changed anything on screen.

**The premise itself was wrong, and a single decisive test exposed it.**
Forcing `crate::gu::TRANSITION_PHOTO` (the framebuffer capture buffer
RE-099/RE-100 built) to a uniform green *before any capture ever
runs* turned the **entire** visible shape green — not just the region
RE-107 called "backing". The untextured backing quad, exhaustively
tested above, never painted a single visible pixel in any test; the
region everyone had been calling "the black backing panel" was actually
one of the object's two `ROLE_FRAMEBUFFER` *photo* texture entries the
whole time. Comparing the two entries directly (a 300×5 "drawn once"
tile and a 300×6 "tiles vertically" tile) isolated which one is broken:
nudging the 300×6 entry's own wrap mode broke its previously-correct
magenta render, proving it samples correctly by default and occupies the
region that already worked. A raw-UV dump of the 300×5 entry then
explained why it fails: its baked `V` range is `214.97..219.97` texels
— the *bottom* edge of the real N64 `sLBTransitionPhotoHeap` (300×220),
not the top. RE-100's capture only ever stores the buffer's top 6–8
rows (exactly right for the 300×6 entry, whose own `V` starts at 0), so
the 300×5 entry wraps into memory the capture never populates with
anything relevant to it.

**This is a scope gap in RE-100's own original measurement, not a bug
in anything this session or RE-107 tested and eliminated.** RE-100's
write-up recorded the 300×5 entry's *span* ("always exactly 5.0
texels") correctly, but never checked its *absolute position* within
the real 220-texel-tall buffer — a genuinely reasonable oversight, since
nothing about "span" alone would have flagged it, and the assumption
that "top 6–8 rows suffice" held for every other measurement RE-100 made.

`cargo test --workspace`: 261 `ssb-rom` tests passing throughout.
`cargo clippy --release --workspace`: clean. All temporary patches
(`pack.rs` colour-force, `meshdraw.rs` cull/depth/texture-cache/shade-model
overrides, `gu.rs`'s `TRANSITION_PHOTO` initial value, `main.rs`'s
forced object view, `romtool`'s material/UV census) were reverted
individually after use; `git diff --stat` was empty after each. The
default (non-transition) build was rebuilt and re-screenshotted clean
(Dream Land pixel-normal, 60 FPS) after every revert, not just once at
the end.

**Not fixed this session.** Two candidate fixes are recorded in
`docs/reverse-engineering.md` RE-108 (capturing a second band near the
real buffer's bottom edge, or rebasing each framebuffer-role primitive's
UV by its own tile's origin at pack time) but neither was attempted —
this session's goal was root-causing an already very long investigation,
not shipping a fix on top of it. The other 12 transition files may well
have the same shape (a "drawn once" tile sampling somewhere other than
the captured top band) and are not yet checked.

## Earlier Task Status

RE-107 (a previous session) started from "continue with the plan" and found
the working tree already held a large, fully implemented, fully tested,
but almost entirely undocumented diff — `STATUS.md`'s own narrative
(below) described only RE-099/RE-100, but the code already contained
RE-101 through RE-106 (`pack::VERSION` was already `15`, not the `14`
this file claimed). Committed that diff first, since it was real, tested
(`cargo test --workspace`, `cargo clippy --release --workspace` both
clean), regression-checked (Dream Land pixel-normal at 60 FPS) code, not
something to discard for being undocumented — then read it in full and
wrote up RE-101 (`G_TEXTURE` UV-scale, `PLAN.md` R0.5), RE-102
(`G_TX_CLAMP` wrap, `PLAN.md` R0.5), RE-103 (per-vertex lit decision,
`PLAN.md` R0.6), RE-105 (`G_MW_LIGHTCOL` as the real "this is lit"
signal, `PLAN.md` R0.6) and RE-106 (`prim_color` was resolved but never
consumed, `PLAN.md` R0.6) in `docs/reverse-engineering.md`, with
`PLAN.md`'s own acceptance items updated to match. RE-104's number is
skipped — nothing in the diff corresponds to it, and no entry was
fabricated to fill the numbering gap.

With the record caught up, continued R0.13 itself rather than stopping
at documentation. A temporary, reverted `romtool` census across all 13
LB-transition files (39–51) found the two-primitive shape RE-100
verified on file 40 alone (one framebuffer-textured primitive, one
untextured "backing" primitive) generalizes archive-wide — but file 40
is **not** representative of the backing primitive's own colour: 12 of
13 files' backing primitives are white (`[255,255,255,0]`), only file 40
is navy (`[0,0,127/128,0]`), and file 40 is the only one of the 13 whose
primitives are `lit`. Chose a second file to verify on-device precisely
because the census flagged it as different on both axes: file 45 (white
backing, unlit). A temporary, reverted `psp/src/main.rs` patch,
following RE-100's own recipe exactly (magenta clear + capture at frame
30, forced object switch to file 45's object from frame 35), produced a
screenshot showing the framebuffer-textured primitive rendering the
correct magenta test colour — real, independent confirmation of the
capture/bind mechanism on a second, deliberately different file, not an
inference from the shape census alone.

**The black-rectangle question RE-100 left unlooked-at turned out to be
real, and stranger than expected.** Both file 40's navy backing and file
45's white backing render **pure black** (`0,0,0`, confirmed by direct
pixel sampling, not eyeballing) on the real device, despite neither raw
vertex colour being anywhere close to black. Checked rather than
guessed: a temporary, reverted census confirmed file 45's backing
primitive has `prim_color = None` and `flat_color = None` at the exact
point `pack_mesh` builds it (ruling out RE-106's shade-scale bake and
RE-080's flat-colour bake), and its raw bytes fail RE-103's
`looks_like_unit_normal` check by a wide margin (length² `3` against an
`11,000..=21,000` window, computed by hand — ruling out the per-vertex
lit fallback shading it as a normal). All three of this project's known
vertex-colour-overriding mechanisms are eliminated by direct evidence;
nothing in the material pipeline as currently understood explains the
result. Left genuinely open, not swept under the two files' own
otherwise-successful verification — see `docs/reverse-engineering.md`
RE-107 for the full account, and RE-106's own closing note for where the
same finding is recorded against the mechanism it first looked most
likely to implicate.

**Retracted by RE-108 (the next session): this attribution was wrong.**
The "backing primitive" named here was never actually visible on screen
in any test above — the black region is one of the object's *photo*
(framebuffer-role) texture entries, not the untextured backing quad. See
"Task Status" above for the real cause (a capture-scope gap in RE-100's
own design, not a material-pipeline bug).

`cargo test --workspace`: 261 `ssb-rom` tests passing throughout (no
regressions from either the temporary census or the temporary device
patch — both fully reverted, `git diff --stat` empty after each).
`cargo clippy --release --workspace`: clean.

**What this does and does not close.** `PLAN.md` R0.13 stays
`IN_PROGRESS`. Visual verification now covers 2 of 13 files (up from 1),
chosen to span the two known backing-colour/lit variants rather than an
arbitrary second pick; 11 files remain unscreenshotted. The archive-wide
census is real, permanent, structural evidence the other 11 share the
same primitive shape — not proof they render correctly, since the
black-rectangle defect demonstrates structural similarity alone does not
guarantee visual correctness. The concrete next lead for whoever picks
this back up is the black-rectangle defect itself: three plausible
causes are now eliminated with direct evidence, which narrows but does
not yet answer where a genuinely white, unlit, untextured vertex colour
is actually turning black between the pack and the screen.

## Earlier Task Status

RE-100 (an earlier session) picked up exactly where RE-099 (the session before that)
left off: RE-099 scoped the LB transition mechanism but explicitly left
one design question unverified — does a PSP port need the N64's own full
`300×220` capture with strip-by-strip TMEM addressing reproduced, or does
a smaller capture with unmodified UVs suffice? Measured it directly
(`romtool textures --file <id>` across all 13 files, not just one) before
writing any implementation.

**RE-099's own favoured hypothesis was wrong.** Every file's 300×5 tile
draws once (V span always exactly 5.0 texels); every file's 300×6 tile
tiles vertically by ordinary wrap addressing (V span 22.5–215 texels,
3.75×–35.83× repeat depending on the file). U never wraps in any of the
13 files. The real ROM shows a **repeating 6-row colour smear**, not a
crisp photo — the correct PSP capture is a **300×6 top-left corner**
(3,600 texels), far smaller than RE-099's own "maybe the full 300×220"
guess.

**Implemented the whole pipeline that session, verified at every layer:**

* `crates/ssb-rom`: `mobj::LB_TRANSITION_SEGMENT`, `mesh::State::
  framebuffer_capture` (set by a segment-`0x1` `G_SETTIMG`, cleared by
  any real one), `mesh::TextureRef::framebuffer`, `pack::TextureDesc::role`
  (`pack::VERSION` 13 → 14, `TextureDesc::SIZE` 32 → 36 — the first growth
  since `mat_anim`, RE-091, exhausted the struct's spare tail padding).
* `tools/romtool`: `pack_mesh` dedups the 13 files' 26 segment-`0x1`
  binds down to the 2 distinct shapes that actually exist (`(u32::MAX,
  u32::MAX, width, height)`, not `texture_cache_key` — a framebuffer ref's
  placeholder `(data_file: None, data_offset: 0, palette: None)` would
  otherwise collide with a real unpaletted texture at a file's own
  offset 0).
* `psp/src/gu.rs`: `Gpu::request_transition_capture()`, a CPU-side VRAM
  readback (`VramMemChunk::as_mut_ptr_direct_to_vram()`, not the
  GE-relative `as_mut_ptr_from_zero()` pointers `sceGuDrawBuffer` already
  uses) into a small `Psm8888` buffer, timed against a manually-tracked
  `draw_is_fbp0` flag kept in lockstep with the PSP SDK's own internal
  `sceGuSwapBuffers` buffer-role swap. Rows 6–7 of the padded 8-row buffer
  are filled with a wrapped copy of rows 0–1 so the GE's own padded-height
  wrap period (8, not the real 6) doesn't introduce two unintended extra
  rows into the repeat.
* `psp/src/meshdraw.rs`: `bind_texture` intercepts `ROLE_FRAMEBUFFER`
  before `pack.texture_data` (which returns `Some(&[])`, not `None`, for
  a zero-length entry) and sources pixels from the new capture buffer
  instead, using `t.stride`/`t.height` the same way the general path
  already does.

**Verified at every layer, not just built.** `cargo test --workspace`:
401 passing (was 398; 3 new tests: two in `mesh.rs` proving the segment
marker sets and clears correctly, one in `pack.rs` proving `role`/`stride`
round-trip without corrupting a neighbouring descriptor — the same guard
class the struct's own doc comment already describes for `mat_anim`).
`cargo clippy --release` (workspace): clean. Rebuilt the real pack: 901 →
**903 textures** (exactly 2 new entries, confirming the dedup key
collapses 26 real binds correctly), size 5264.1 → 5267.7 KiB (+3.6 KiB,
almost entirely `TextureDesc::SIZE`'s growth applied to the 901
pre-existing textures, not new texel bytes — a framebuffer entry bakes
none).

**Confirmed on the real device profile, not just compiled.** A temporary,
reverted example binary found all 13 transition files' scene graphs
already exist as ordinary pack objects and really do carry
`ROLE_FRAMEBUFFER` textures. A temporary, reverted `psp/src/main.rs`
patch (`git diff --stat` after reverting is empty) let Dream Land render
for 30 frames, set the clear colour to an unmistakable magenta for
exactly frame 30, called `gpu.request_transition_capture()` that same
frame, then forced the viewer onto file 40's transition object (1,000
triangles, the largest single object in the whole pack) from frame 35
onward. Screenshot: the transition's largest primitive renders the
**magenta test colour** — direct evidence the capture reads real
just-rendered screen content and the bind displays it correctly, not a
plausibility argument. `tools/run-ppsspp.sh` on the unmodified build
afterward: Dream Land renders pixel-normal at 60 FPS, confirming this
session's changes don't disturb the default rendering path.

**Not fully closed.** A second, smaller primitive on the same object
rendered solid black throughout — not investigated (plausibly a
deliberate black panel behind the "photo" window, not confirmed). Nothing
calls `request_transition_capture` from real game logic: there is no
match-start/match-end event to call it from, since this project has no
game-state/transition system at all — `screen wipes implemented` and
`visual verification completed` stay open. `PLAN.md` R0.13 moved
`TODO` → `IN_PROGRESS`, with `framebuffer texture paths implemented` and
`render-to-texture paths implemented where required` (RE-099/RE-100 both
confirm there is no render-to-texture pass to implement — satisfied by
there being nothing here that applies) now checked.

Immediately before this, RE-099 (an earlier session) scoped `R0.13` precisely instead of starting
implementation cold. Read `refs/ssb-decomp-re/src/lb/lbtransition.c`
directly (239 lines, the whole file) rather than continuing to reason
from RE-055's own paraphrase, and found the mechanism is considerably
simpler than "framebuffer rendering" suggested: `lbTransitionSetupTransition`
runs **once**, when a transition begins, doing a plain CPU-side copy of
the current framebuffer into a `300×220` `u16` heap — not a per-frame
render-to-texture pass. Every frame after that, `lbTransitionProcDisplay`
just binds the already-captured snapshot as RSP segment `0x1` and draws
the transition's own ordinary `DObjDesc`/`AObjEvent32` scene graph, the
same tree-walk this project already converts for every other object.

Also corrected RE-055's own scope: measured directly against the ROM
(`romtool textures --file <id>`, not the decomp's `dLBTransitionDescs`
table) that **13 files carry this exact two-bind signature, not 11** —
files 39 (`IFCommonObject`, a different module prefix, purpose not
identified this session) and 47 (`LBTransitionPaperAirplane`, not in the
11-entry transition table) also match it byte-for-byte. 13 files × 2
binds = 26, exactly matching R0.3's original segment-`0x01` failure
count — so RE-055's "26" was right, its "11 files" attribution was an
undercount by 2, now fully resolved. A likely design simplification was
identified but explicitly left unverified: the N64 tiles the image into
small strips purely because the RSP's TMEM is 4 KB; the PSP GE has no
equivalent limit, so one full-size PSP texture capture might need no
strip-by-strip capture logic at all. **RE-100 (an earlier session) checked
this directly and found it was wrong** — see "Previous Task Status" above.

Not implemented that session — a scoping pass, the same shape as
RE-076/081/096. `git diff --stat` after RE-099's own `R0.13` work was
documentation-only.

RE-098 (an earlier session) implemented and shipped multi-costume packing and
device-side selection, closing four of `R0.11`'s five acceptance items.
Confirmed by reading the real consuming code (not assumed) that a
fighter's alternate costumes share identical geometry and vary only
material (colour/palette) — a separate, gameplay-state-driven mechanism
(`modelpart_id_curr`) is the only thing that ever swaps geometry, and it
is never driven by `costume`. This settled the runtime-representation
design in favour of a sparse per-(node, costume) mesh substitution
layered on the existing shared mesh set, not per-costume geometry
duplication.

Measured the real per-node cost archive-wide first (matching RE-076/077's
discipline): real per-fighter costume counts (hand-transcribed and cited
from `dFTParamCostumeIDs`, `refs/ssb-decomp-re/src/ft/ftparam.c:56`) are
Mario 5, Fox 4, Donkey Kong 5, Samus 5, Luigi 4, Link 4, Kirby 5,
Jigglypuff 4, Captain Falcon 6, Ness 4, Yoshi 6, Pikachu 4. A temporary,
reverted `romtool costcensus` census found 10-16 of each fighter's
~25-33 nodes (a third to two-thirds, never all) actually differ from
costume 0 — some palette-dominated (Donkey Kong: 9 colour vs 96 palette
differences), some colour-dominated (Yoshi: 80 vs 30), one barely
touched (Link: 2 of 32 nodes).

Shipped `CostumeOverride` (`crates/ssb-rom/src/pack.rs`, `pack::VERSION`
12 → 13): a sparse table keyed by global node index, `Pack::costume_mesh`
binary-searching it and falling back to the node's own baked mesh for
the common (no-override) case. `tools/romtool/src/main.rs`'s build loop
converts each costume-bearing graph once per costume and registers a
substitute mesh only where the *converted mesh content* differs from
costume 0 — comparing raw per-node `MObj` fields instead would have
missed differences caused by cross-node state inheritance (RE-064), so
`Mesh`/`Primitive` gained `PartialEq` to make the real comparison
possible. Found and fixed a real, costume-unrelated bug along the way:
`pack_mesh`'s texture cache was keyed by image location alone, which
would have let a costume's different palette on a shared image silently
reuse costume 0's cached texture — fixed by keying on palette identity
too (`TexKey`), a correctness improvement to the existing non-costume
path, not only a costume-specific one.

Wired device-side: `draw_object`/`draw_object_posed` gained a `costume`
parameter (`0` for every pre-existing caller, reproducing prior behaviour
exactly); the debug viewer gained a costume-cycle key (`L`, newly mapped
from the PSP's previously-idle `SELECT` button in
`crates/ssb-engine/src/input.rs`) and an overlay readout. Verified on the
real device profile for two fighters, not just compiled: Mario
(colour-dominated) visibly recolours between costumes 0 and 2; Donkey
Kong (palette-dominated) correctly renders the game's well-known "Blue
Kong" alternate colour at costume 3, confirming the palette-substitution
path specifically (a fresh texture per differing palette), not only the
vertex-colour path Mario exercised. Both verifications used a temporary,
fully reverted forced-object-index patch (`git diff --stat` on
`psp/src/main.rs` after reverting matches the permanent wiring only),
matching RE-074's established precedent.

`cargo test --workspace`: 398 passing (was 394; new coverage: two
`pack.rs` `costume_mesh` tests including one verified incapable of
passing without `finish()`'s sort, two `object_costume_count` tests
including one verified incapable of passing without the node-range
upper bound). `cargo clippy --release` (workspace): clean. Rebuilt the
real pack: 1287 per-(node, costume) mesh substitutions, size 4492.4 →
5264.1 KiB (+772 KiB, +17% — disclosed; smaller in both absolute and
proportional terms than RE-067's already-shipped 1.5× mirror-texture
cost, so not gated on further user sign-off). `cargo psp --release` +
`tools/run-ppsspp.sh --seconds 8`: Dream Land pixel-normal at 60 FPS, no
panics, no new warnings versus a stashed pre-session baseline build.

**This moved `R0.11` from `IN_PROGRESS` to `VERIFYING`** with four of
five acceptance items on real, on-device-verified evidence. A closing
addendum in the same session finished the other two: screenshotted the
remaining 10 of 12 real fighters (Fox, Samus, Luigi, Link, Kirby,
Jigglypuff, Captain Falcon, Ness, Yoshi, Pikachu) each at a non-zero
costume — all ten rendered a real, distinct, non-crashing model at 60
FPS, several matching this project's own prior knowledge of SSB64's
actual named alternate colours (purple Samus, blue Yoshi, green Kirby,
green Pikachu, blue Falcon). Investigated one oddity rather than
shipping past it: Jigglypuff's costume 3 showed an iridescent rainbow
body, but comparing against Jigglypuff's own costume 0 showed the same
pattern already present there (a 15.6% pixel diff confirmed a real
colour change still happened underneath it) — a pre-existing baseline
shading trait of this project's Jigglypuff model, not a bug this session
introduced. All temporary patches (the forced object/costume override,
a throwaway example binary used to look up object indices by source
file) were fully reverted. **`R0.11` is now `COMPLETE`.**

Immediately before this, RE-097 (previous session) implemented and shipped the concrete lead RE-096
handed off: `colors_at` (`crates/ssb-rom/src/matanim.rs`) now also reads
`PaletteID` (joint track 9) alongside its existing `PRIM`/`ENV`/`BLEND`
colour tracks, using the same step/base/target bookkeeping, resolved via
`f32::from_bits` then the same `(s32)` cast `objdisplay.c`'s own draw path
performs. `Colors` gained a `palette_id: Option<i32>` field;
`costume_colors` needed no changes. `tools/romtool/src/main.rs`'s
`Loaded::materials` — the loop that already bakes
`prim_color`/`env_color`/`blend_color` from `costume_colors` — now also
calls the already-shipped `mobj::read_palettes` (previously only ever
called from the stage material-animation path, RE-090/092) to overwrite
`m.palette` with the costume's own resolved palette entry.

Verified with 4 new `matanim` unit tests reproducing the real
shared-clock archive shape by hand-tracing the same semantics
`mario_arm`'s existing colour test already exercises. `cargo test
--workspace`: 394 passing (was 390). `cargo clippy --release
(workspace)`: clean.

Verified against the real ROM, not just unit fixtures — and did not stop
at a null result. Rebuilding the pack at the default costume (0) is
byte-for-byte unchanged (648 textures, 4492.4 KiB, every other figure
identical). Before trusting that as "correctly does nothing" rather than
"silently broken," ran a temporary, reverted census (`eprintln!` in the
same loop, matching RE-079/081/089's pattern): the new path genuinely
fires 198 times archive-wide, and every one of those 198 resolves
`palette_id = 0` at costume 0 — the pack is unchanged because costume 0's
`PaletteID` really is 0 everywhere, not because the read silently failed.
Re-ran the same census with `DEFAULT_COSTUME` temporarily set to `1.0`:
188/198 now resolve to `id = 1` (10 stay `0`, plausibly costumes that
share a palette with costume 0), and `read_palettes` succeeded for all
198 with zero failures and zero short reads — confirming the mechanism is
real and varies correctly, not hardcoded. `cargo psp --release` +
`tools/run-ppsspp.sh --seconds 8`: clean build, 8s run, no panics,
`FPS: 60.0`, screenshot has 31k+ distinct colours (not a blank/locked
screen).

**This does not close `R0.11`.** The pack still only ever builds one
costume at a time (`DEFAULT_COSTUME = 0.0`, unchanged); multi-costume
packing/selection and all five of the task's own acceptance items ("all
fighter palettes identified", "all required costumes identified",
"runtime representation complete", "palette data verified against ROM",
"representative regression renders added", "all required fighters
verified") remain open. What this session closes is narrower: the one
concrete, decomp-confirmed gap RE-096 identified (`colors_at` silently
never reading `PaletteID`) is now implemented, wired into the real build
loop, and verified end to end against the real ROM — a real step, not the
whole task.

Immediately before this, RE-095's own open item closed first: the user watched RE-095's
`MaterialAnimator` run interactively (PPSSPP launched windowed and left
running rather than the auto-killing screenshot harness) and confirmed
the `PaletteID` cycle is visibly working on file 105's stage (stage
2/41). `docs/reverse-engineering.md` RE-095 gained a closing addendum;
`PLAN.md` R0.10's "representative animated materials verified" and
"stage material animation verified" items are both now checked on that
basis.

`R0.10` is now `COMPLETE`. RE-096 (this session) closed its last
acceptance item — "fighter material animation verified where
applicable" — by checking archive-wide whether any real fighter
`p_costume_matanim_joints` script needs `MaterialAnimator`'s per-frame
runtime rather than `colors_at`'s existing one-shot evaluation.

**Result: 441 real fighter costume scripts, 0 loop via `JUMP`/`SET_ANIM`.**
Temporary census (reverted, matching RE-079/081/089's pattern): every
script either reaches `End` (max 100 frames) or parks at a long trailing
`Wait` without ever revisiting its own start. Fighter costume scripts are
structurally one-shot key lists, never real-time animations —
`colors_at`/`costume_colors` and `MaterialAnimator` are correctly
separate mechanisms for correctly separate shapes, confirming this task's
own acceptance note's prediction that nothing needs unifying. `R0.10`'s
own remaining checklist item is now checked; **the task is `COMPLETE`**.

**A genuine, separate gap surfaced along the way, and was traced to the
decomp source rather than left as an inference.** Track-category
breakdown of those 441 scripts: `PrimColor` 44%, `Light1Color`/
`Light2Color` 21% each, `TextureIDCurrent` 7% — and **`PaletteID` 45%
(200/441)**. `colors_at` only ever decodes `PRIM`/`ENV`/`BLEND`; it never
reads `PaletteID`. Read `refs/ssb-decomp-re`'s real consuming chain to
check whether this matters: `lbCommonAddMObjForFighterPartsDObj`
(`src/lb/lbcommon.c:955`) plays a costume script through the *same*
generic `gcPlayMObjMatAnim` engine stages use, evaluated once at
`anim_frame = fp->costume`; its `PaletteID` case (`src/sys/objanim.c:
1340`) sets `mobj->palette_id`; the draw path (`src/sys/objdisplay.c:
1184`) reads it back as `mobj->sub.palettes[(s32)mobj->palette_id]` —
**the identical `MObjSub.palettes[]` array `mobj::read_palettes` already
parses for stages.** This is confirmed, not inferred: a fighter's costume
identity genuinely includes which palette variant is active, and packing
only costume 0 today silently keeps every other costume's *palette* at
costume 0's wherever a script relies on `PaletteID`, independent of
whatever `PRIM`/`ENV`/`BLEND` per-costume work lands. Not implemented
this session (`R0.10` is scoped to material *animation*) — handed off as
`R0.11`'s new concrete lead: `costume_colors` needs a sibling one-shot
`PaletteID` read feeding which packed palette variant a given costume
bakes at pack time. No code changed this session on `R0.10`/`R0.11` — a
measurement-and-decomp-read pass, the temporary census reverted before
committing (`git diff --stat` on `tools/romtool/src/main.rs` is empty).

## Current Milestone Progress

Moving to `R0.11 — Fighter Palettes / Costumes`, the first task in
dependency order whose blocking dependency (`R0.10`) just completed.
`R0.11` also depends on `R0.4`/`R0.6` (both `IN_PROGRESS`, not blocking —
matching this project's own established reading of "depends on" as
"needs meaningful progress from," not "must be 100% complete first," the
same way `R0.9` proceeded historically). Its own "Current evidence" now
opens with a real, decomp-confirmed, actionable lead rather than a cold
start.

Immediately before this, RE-095 (previous session) shipped step 8: the
device-side `MaterialAnimator`, the piece the whole RE-086–094 pipeline
had been building toward.

**Design mirrors `StageAnimator`, but the lifecycle differs on purpose.**
A `MatAnimDesc` entry is a property of a *texture*, not a fighter or a
stage layer, so there is no per-object "start" boundary — `start(pack)`
runs once when the pack loads, `tick(pack)` runs every frame for the
pack's whole lifetime, independent of which stage or fighter is shown
(cheap regardless: 33 real scripts, `MAX_MAT_ANIMS = 64` headroom).
Array position mirrors `TextureDesc::mat_anim`'s own index directly — no
separate lookup table. `resolved_palette` clamps into each entry's own
`palette_count` before adding `first_palette`, closing a real risk (an
out-of-range replay reading a *different* `MatAnimDesc`'s own variants in
the shared table) verified via a test proven capable of failing.

**A `no_std` build error caught a real portability gap before it
shipped.** `f32::round()` does not exist in `core` without `std`/`libm`
— `scene.rs` already avoids `libm` for `sin`/`cos` for the same reason.
Fixed with the same "add a half, truncate" trick `mesh.rs`'s own vertex
rounding already uses. This was only caught because `cargo psp --release`
was actually run before calling the work done — `cargo test`/`clippy
--workspace` alone never would have, since the crate's `std` feature is
enabled by default on the host.

**Wired into every real draw path, not just one.** `bind_texture` issues
a second `sceGuClutLoad` after the static one whenever `TextureDesc::
mat_anim` names a live, resolvable entry — riding `apply_material`'s
existing per-frame texture-cache reset for correct cadence, so a texture
with no animation, or an animator with no value yet, keeps its baked
palette rather than showing nothing. Threaded `Option<&MaterialAnimator>`
through six functions (`draw_mesh`/`draw_object`/`draw_object_posed`/
`draw_stage`/`draw_stage_animated`/`apply_material`), matching how
`pack: &Pack<'_>` is already threaded explicitly rather than stashed in
`DrawState`, and updated all four real call sites in `psp/src/main.rs`.

**Verified.** Two new tests in `skeleton.rs`: one ticks a script
reproducing RE-086/087's real archive shape (three `PaletteID` steps then
`SET_ANIM` looping forever) through a full pack round-trip and confirms
every real variant is visited and it keeps cycling rather than freezing;
one proves the neighbouring-table-read clamp is a real regression risk,
not defensive theatre. `cargo test --workspace`: 247 passing (was 245).
`cargo clippy --release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, no panics, Dream Land
pixel-identical at 60 FPS.

**Loaded file 105's own stage (temporary, reverted `stage_index`
override) and confirmed it runs clean on the real device profile** — 464
triangles, one layer, 60 FPS, `tex 0/648` matching the current pack. **Did
not conclusively confirm the palette visibly cycles by screenshot.** The
harness takes one screenshot per independent launch and each launch
restarts the simulation from tick 0, so two screenshots from two separate
invocations cannot isolate "more ticks, nothing else changed" — a
stage-animated floating platform's own motion (RE-050/051, unrelated)
confounded a naive crop comparison. Left honestly open, matching this
file's own already-recorded category of limitation for `R0.12`/`R0.14`'s
remaining items: the mechanism is verified by construction, watching it
happen needs video capture or interactive play, not another screenshot
diff.

Immediately before this, RE-094 (previous session) closed the lead RE-093
(previous session) left open: node 27's `texture_enabled` was `false` for
its whole span despite actively loading textures. Traced `texture_enabled`
node-by-node across the same graph (temporary instrumentation, reverted)
and found it flips to `false` exactly once, inside **node 20's own
list** — `SetCombine → Texture{on: false} → one untextured triangle`, a
self-contained decal with no `G_SETTIMG` of its own. Nothing re-enables it
from node 21 through node 27, yet four of those seven nodes each issue a
complete, independent `G_SETTIMG`/`G_SETTILE`/`G_LOADTLUT`/`G_LOADBLOCK`
chain and draw real triangles — only explicable if texturing is genuinely
active there.

Measured before fixing (temporary bypass on `current_texture()`'s
`texture_enabled` gate, reverted): ignoring the flag outright takes the
pack from 639→648 textures and 25/33→33/33 surviving `mat_anim` scripts,
confirming the exact scope — but a blanket ignore is not *correct*, since
it would also re-texture node 20's own deliberately-untextured decal
using whatever stale binding preceded it. Shipped a narrower rule
instead: `Cmd::SetTimg` now sets `texture_enabled = true` unconditionally
— a display list has no reason to reissue a whole texture chain for
geometry meant to draw untextured, so a fresh `G_SETTIMG` is as strong a
signal as an explicit `Texture{on: true}`. Re-measured with the narrower
rule: **identical result** to the blanket bypass (639→648, 25→33),
confirming it loses nothing the blanket version gained while leaving node
20 (no `SetTimg` of its own) genuinely untouched.

**Also fixed a test that was passing for the wrong reason.**
`texture_disabled_means_no_binding` omitted `Cmd::SetTile` entirely, so
its `None` result was actually caused by a missing tile format, not the
disabled flag its name claims to cover — after this fix it would have
kept passing vacuously. Rewrote it with a complete texture setup plus an
*explicit* `Texture{on: false}`. Added
`a_later_nodes_own_settimg_overrides_an_inherited_texture_off`,
reproducing nodes 20→21's exact real shape, verified capable of failing
(reverted the `SetTimg` change, confirmed the panic, restored). `cargo
test --workspace`: 245 passing (was 244). `cargo clippy --release`
(workspace): clean.

**Result, run against the real ROM: 33 of 33 known scripts now survive**
(321 palette variants, up from 297) — every script RE-089 originally
found. Texture count 639 → 648 (+9, all static/non-animated textures
recovered the same way). Meshes/triangles unchanged; `draws` rose
3447→3494 (more primitives correctly split into their own textured group
instead of merging into an untextured one). `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, no panics, Dream Land
pixel-identical at 60 FPS. **The RE-092/093 open question is now fully
closed, not partially explained** — step 7 of this file's own pipeline
list is done.

Immediately before this, RE-093 (previous session) picked up exactly the
open item the previous session's own note flagged: why did only 17 of
RE-089's 33 known `PaletteID`-cycling scripts survive RE-092's pipeline?
Treated as a real unknown, not a rounding error, per this file's own
standing instruction.

Temporary instrumentation (reverted before committing, matching
RE-079/081/089's pattern) ruled out two of the three named candidates
directly: every resolved script's chain index is genuinely called by its
node's own display list, and every node is placed. The gap was 100%
"resolved but never reached by any primitive" — not a texture-availability
miss (`no_texture_at_that_primitive` was 0 throughout).

**Read the raw ROM bytes instead of theorizing further.** Dumped file
105 node 1's actual, recursively-decoded display list and found real
stage data calls three palette-only `MObj`s back to back against *one*
already-loaded texture image — only the first and last reissue their own
`G_SETTIMG`+`G_LOADBLOCK`; the middle one's block is just
`Call → G_LOADTLUT → G_VTX → G_TRI`, deliberately reusing the image
already resident in TMEM and swapping only the palette. `mesh.rs`'s
`Cmd::LoadTlut` handler did not model this: its own comment asserted "the
real texture follows with its own SETTIMG" and nulled the image binding
outright on that assumption. This ROM data falsifies the assumption, and
when nothing reissues `G_SETTIMG`, the null had nothing to restore it —
`current_texture()` returned `None` for the rest of that group's
geometry, which lost not just its `mat_anim` tag but its *texture
entirely* (those triangles packed as flat-shaded, untextured
primitives). This is a real rendering-correctness bug, broader than
material animation: any static, non-animated multi-palette-sharing-one-
image primitive anywhere in the archive was exposed to the same loss.

**Fixed by making `State` remember the real binding, not just the
transient one.** New field `real_timg: Option<(u32, Option<u16>)>` is
updated only by an actual `Cmd::SetTimg` or an `MObj`'s own `sprite`
field — never by a palette-only `MObj`'s injected address — and
`Cmd::LoadTlut` now restores from it instead of clearing. This is more
faithful to real hardware, not a special case: the RDP's texture-image
register has no "unset" state, so restoring the last real value is
correct whether or not a fresh `SETTIMG` follows — when one does (the
ordinary case), it overwrites the restore immediately, so nothing about
the already-working paths changes. Verified the fix can fail: new test
`a_palette_only_mobj_keeps_the_image_a_prior_settimg_bound` reproduces
file 105 node 1's exact shape and fails without the fix (two expected
primitives collapse into one, the second `MObj`'s geometry silently
merging into an untextured group), confirmed, then restored. `cargo test
--workspace`: 244 passing (was 243).

**Result, run against the real ROM: 17 → 25 of 33 known scripts now
survive** (181 → 297 palette variants). Every other pack figure —
meshes, triangles, draws, objects, node placement, and **texture count
(639, unchanged)** — is identical to the pre-fix pack, the expected
signature of a correlation fix (existing textures gaining a correct
palette/`mat_anim` attribution) rather than a new-texture side effect.
`cargo clippy --release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, no panics, Dream Land
pixel-identical at 60 FPS — notable because, unlike every RE-089–092
session, this fix is **not** animation-scoped: it corrects
`Cmd::LoadTlut` archive-wide, so Dream Land was a genuine (not
guaranteed) candidate to change, and didn't.

**Still open: 8 of 33 remain missing**, with a concrete, different,
unchecked lead this session's own diagnostic surfaced: file 105 node
27's `texture_enabled` was `false` for its *entire* span despite the
node actively loading TLUTs and, for 3 of its 7 entries, issuing real
`G_SETTIMG`/`G_LOADBLOCK` pairs — behavior that only makes sense with
texturing genuinely on. That points at `mesh.rs`'s cross-node
`texture_enabled` inheritance (RE-064) itself, a different mechanism
from this session's `Cmd::LoadTlut` fix, and was not investigated
further.

Immediately before this, RE-092 (previous session) solved RE-091's
blocker and wired `romtool`'s real build loop, closing step 6 for real
archive data.

**The fix was in `mesh.rs` all along — no new bookkeeping needed to find
the texture, only to remember the script.** Re-read `State::apply_mobj`
before designing anything new: for a palette-only `MObj` (`sprite: None`,
RE-091's own finding, true for all 33 real cases), `apply_mobj` sets
`timg_addr` to the *palette's* address first — and the display list's own
subsequent `G_LOADTLUT`+`G_SETTIMG` (ordinary commands `mesh.rs` already
walks, nothing `MObj`-specific about them) load the TLUT and then
overwrite `timg_addr` with the real texture image address. By the time a
primitive is emitted, `State::current_texture()` already resolves the
correct texture through this existing mechanism. The only real gap was
remembering *that a script drove this palette* across those same
intervening commands — a bookkeeping problem, not a texture-identification
one.

Added `MeshMaterial::mat_anim: Option<MatAnimRef>` (`{ source_file,
script }`, identity only — the same "where, not the decoded bytes"
division `TextureRef` already draws) and a parallel `mat_anims` slice on
`SequenceItem`/`State`, indexed by the same segment-`0x0E` heap index
`mobjs` already is. Set inside the *same* `if let Some(palette) = m.palette`
branch that sets `timg_addr` in `apply_mobj` — not merely "when a script
is present" — so a *later*, unanimated palette-bearing `MObj` correctly
clears a stale marker instead of leaking it onto whatever texture ends up
bound next. `forget_texture` clears it too, for the same reason it
already clears `palette_offset`.

**Verified the clearing rule can fail, not just that it compiles.** A new
test builds two `MObj` calls in one node — first animated, second not,
each binding a different texture — and asserts the second primitive's
`mat_anim` is `None`. Confirmed this test can actually fail: reverting
`apply_mobj` to the naive `if mat_anim.is_some() { ... }` form (RE-091's
own original sketch) made it fail with the stale reference still
attached, before reverting back to the fix. A second test confirms the
positive case. `cargo test --workspace`: 243 passing (was 241), all 48
pre-existing `mesh` tests and all 35 pre-existing `pack` tests unaffected.

**Wired `romtool`'s real `pack` build loop, not just `mesh.rs`.** Added
`resolve_layer_mat_anims` (per stage-layer graph, same-file only per
RE-089's scope: ticks `MaterialJoint` and calls `mobj::read_palettes`
exactly as RE-089/090 already do) and `convert_mat_anim_palette` (the
same RGBA5551→ABGR8888 conversion `convert_texture` already applies to
the static palette, just per resolved variant). `pack_mesh` now checks
every primitive's `mat_anim` after resolving its texture, dedupes by
script the same way textures dedupe by texel address, and calls
`add_mat_anim`/`set_texture_mat_anim` for real.

**Result, run against the real ROM: 17 of RE-089's 33 known scripts
survived the whole pipeline — 181 palette variants, 23 textures
animated.** Not a guess: every surviving case's entry count agrees
*exactly* with RE-089's own independently-recorded numbers — file 117
contributed both of its scripts, still 16 entries each (the
decomp-matching case); file 114 contributed 6 of 13, still 18 entries
each; file 105 contributed 8 of 18, entry counts (2, 3, 4) still inside
RE-089's own recorded range. That agreement is strong evidence the
pipeline carries data through correctly end to end, not merely that it
runs without erroring. Pack size grew 4311.0 → 4470.3 KiB (+159.3 KiB of
palette blobs). `cargo run --release -p romtool -- pack`: "verified loads
back cleanly". `cargo psp --release` + `tools/run-ppsspp.sh`: builds and
runs clean, Dream Land pixel-identical at 60 FPS — expected, since Dream
Land uses none of files 105/114/117 and nothing on the device side reads
`mat_anim` yet.

`PLAN.md` R0.10's "material state updated correctly" acceptance item is
now checked, with real verified pack data behind it. **Not investigated
this session**: why 16 of the 33 known scripts did not survive
(candidates: unplaced nodes, a display list this project's own discovery
pass never authoritatively reaches, or something else — genuinely
unknown, not swept under the 17/33 success).

Immediately before this, RE-091 (previous session) shipped step 5's pack
format — `MatAnimDesc`/`MatAnimPalette` (a new table pair mirroring
`AnimDesc`/`AnimJoint`) plus `TextureDesc::mat_anim` (filling 4 bytes of
existing padding, no size change), `pack::VERSION` 11 → 12,
`PackWriter::add_mat_anim` deduplicating each driving script's whole
source file the same way `add_anim` already does for joint animation.
Round-trip verified with 3 new unit tests (238 → 241 passing) plus all 35
pre-existing `pack` tests unaffected. `cargo run --release -p romtool --
pack` against the real ROM builds cleanly at the new version and reports
"verified loads back cleanly" — same 639 textures/41 stages/other counts
as before, since nothing populates the new tables yet (a schema-only
change so far). `cargo psp --release` + `tools/run-ppsspp.sh`: builds and
runs clean, Dream Land pixel-identical at 60 FPS.

**Wiring `romtool`'s real build loop to populate the new tables found a
genuine blocker, checked before writing around it.** The natural design
keys a texture's animated palette table by `(data_file, data_offset)` —
the same dedup key `pack_mesh`'s existing texture cache already uses —
via each animated `MObjSub`'s own `sprite` field, the exact same
`MObjMaterial` RE-090 already reads `palettes[]` from. Checked this
against the real ROM before committing to it (temporary instrumentation
on `romtool stages`'s existing replay loop, reverted): **all 33 real
`PaletteID`-cycling `MObjSub`s have `sprite: None`.** A palette-cycling
material never names its own texture image — the texture it applies to
is whichever CI4/CI8 image is already bound at that point in the node's
draw sequence, tracked correctly today only by `mesh.rs`'s own cross-node
material-state threading (RE-064), not by anything recoverable from the
`MObjSub` alone. Populating `TextureDesc::mat_anim` for real needs that
threading extended with an animation marker (architecturally similar to
how RE-073/074 added and threaded `combiner_texture_blend` through
`MeshMaterial`) — a `mesh.rs`-level change, not more `romtool`/`pack.rs`
plumbing. `TextureDesc::mat_anim` deliberately stays `NO_ANIM` for every
real texture rather than guess at a correlation; `git diff --stat` on
`tools/romtool/src/main.rs` is empty — the instrumentation used to check
this was fully reverted, matching RE-079/081/089's established pattern.

Immediately before this, RE-090 (previous session) shipped the read
RE-088 retracted, now that RE-089 (session before that) supplied a sound
bound for it: `mobj::read_palettes(file, sub_at, count)` reads exactly
`count` consecutive `palettes[]` entries, where `count` comes from outside the
struct (RE-089's script-computed bound) rather than being discovered from
local bytes the way RE-088's over-reading walk tried and failed to do.
Each of the `count` entries is validated by the same relocation-backed
check `read_material`'s existing entry-0 logic already uses; any failure
within that count returns `None` for the whole read rather than a
silently-shorter list, since a mismatch between the script's bound and
the array's real extent is a real problem worth surfacing.

Wired into the same `romtool stages` replay RE-089 built, immediately
after computing each `PaletteID` script's bound: reads that script's
actual `MObjSub.palettes[]` using the bound it just computed (now
tracking the node/chain-position indices through `resolve_scripts`'
table so the right `MObjSub` offset is available), and checks the result
is not just present but non-degenerate (every entry pairwise distinct —
a repeated palette across a cycling table's own indices would suggest
the bound or the array is wrong even if the read technically succeeds).

**Result, run against the real ROM: 33/33 succeeded, 0 failures, 0
arrays with a duplicate entry.** Every `PaletteID` script RE-089 found —
file 105's 18 (2–4 entries each), file 114's 13 (18 entries each), file
117's 2 (16 entries each, independently matching the decomp's own
declared array size) — now reads back a fully valid, all-distinct
palette-pointer array using exactly its own script's computed bound.
This is genuine end-to-end validation of the whole chain RE-088 broke
and RE-089/RE-090 rebuilt: decode script → compute bound → read exactly
that many real pointers → confirm they resolve and are not degenerate —
not a plausibility check on one hand-picked example, but 33 independent
cases across three files and three different entry counts.

`cargo test --workspace`: 238 passing (was 234; 4 new `mobj` unit tests
covering exact-count reads, not-reading-past-count, honest failure on an
overshooting count, and a cross-file entry at a non-zero index). `cargo
clippy --release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: builds and runs clean, Dream Land pixel-identical
at 60 FPS (still nothing wired to rendering or the pack format — this
session is read-path verification only, matching RE-089's own shape).

`PLAN.md` R0.10's step 2 (reading `palettes[1..]`) is now done at the
`ssb-rom`/`romtool` level. What remains is packing this into the runtime
format (step 4: a new table pair, a `pack::VERSION` bump) and the
device-side `MaterialAnimator`/`sceGuClutLoad` wiring (steps 5–6) — both
genuinely larger units of work than this session's, which was entirely
about proving the read path is correct before building anything on top
of it.

Immediately before this, RE-089 (previous session) did what RE-088's own
retraction said was the actual next step: resolve `p_matanim_joints` into
per-(node, `MObj`-chain-position) script addresses *first*, since the
`palettes[]` bound RE-088 needed can only come from there.

Generalised rather than reinvented: `matanim.rs`'s existing
`costume_colors` (fighter costume selection, RE-040) already walks this
exact shape — outer array parallel to `DObjDesc`, each entry a per-chain
script list — it just also evaluates each script's colour tracks in the
same pass. Factored the walk itself into a new public
`resolve_scripts(file, table, nodes, chain_len) -> Vec<Vec<Option<u32>>>`
and rebuilt `costume_colors` on top of it, behaviour-preserving (two new
unit tests, plus the crate's full existing suite unaffected). One
function now serves both known instances of this shape — a fighter's
`p_costume_matanim_joints` and a stage layer's `p_matanim_joints` — since
RE-086 already established they are the same struct laid out in
different files.

Wired it permanently into `romtool stages` (not a temporary reverted
census): resolves each stage layer's `p_matanim_joints` against its
same-file `mobjsub_table` chain lengths, then replays every resolved
script to completion with the already-shipped `MaterialJoint` engine —
the first archive-wide exercise of that engine beyond RE-087's one
hand-picked example. Run against the real ROM: **61 scripts resolved, 0
failures.** The category breakdown on this same-file subset (`PaletteID`
54%, the rest split across `TraU`/`TraV`/`TextureIDCurrent`) is smaller
than but consistent with RE-086's full archive-wide number (172 scripts,
`PaletteID` 71%) — cross-file `p_matanim_joints` tables are still not
attempted, so this is a subset, not a disagreement.

**This closes the loop RE-088 opened.** Ticking each resolved `PaletteID`
script to completion and taking its largest value gives exactly the
`palettes[]` entry count that script will ever need. Two concrete,
cross-checked results: file 117 (`StageMetalFile2`) — the very file
RE-088 cited from the decompilation as its largest known example — 
independently resolves to **16 entries** from the *script's own runtime
values*, matching the decomp's declared `..._palettes[16]` array exactly,
via a completely different method than reading the C source. File 105
(`StageZebesFile2`, 18 scripts needing 2–4 entries) and file 114
(`StageLastFile2`, 13 scripts needing exactly 18 entries) are concrete,
non-Dream-Land stages for the "representative palette-cycling stage"
step this task's own prior note flagged as still needed.

`cargo test --workspace`: 234 passing (was 232). `cargo clippy --release`
(workspace): clean. `cargo psp --release` + `tools/run-ppsspp.sh`: builds
and runs clean, Dream Land pixel-identical at 60 FPS (nothing wired to
rendering or the pack format yet — this session is resolution and replay
only, not consumption).

Immediately before this, RE-088 (previous session) attempted the
concrete next step the session before that had pointed at — extending
`mobj.rs` to read `MObjSub.palettes[1..]`, not just `[0]` — and retracted
it after archive-wide measurement showed it does not work.

Implemented a walk from `palettes[0]`'s array base, accepting each
further entry only if it passed the same relocation-backed validity
check `indirect`'s already-shipped entry-0 logic uses (real intern
pointer, or a zero word with an extern relocation behind it), stopping
at the first slot that failed, capped at 32 entries as a sanity bound.
New unit tests against synthetic fixtures passed cleanly and made the
approach look sound — the fixtures are sparse, single-purpose byte
arrays with nothing else nearby that could accidentally look like a
pointer, which is exactly the condition that does not hold in a real
ROM file.

Before trusting it, measured archive-wide via a temporary `romtool`
census (reverted, not committed, matching RE-079/081's pattern): of 243
palette-carrying materials, **110 (45%) hit the artificial 32-entry cap
outright**. Traced one concretely (file 75 `MVOpeningRunCrash`, `MObjSub`
at `0x2C60`) by dumping every word the walk read: the "entries" were a
perfect arithmetic sequence (`1280, 1240, 1200, ... 280`, stride `-40`,
26 steps) that wraps and repeats identically at index 26 — not a real
palette-pointer table, but the walk wandering into unrelated, densely
pointer-laden neighbouring file data (Vtx arrays, DL fragments,
sub-object pointers) once it ran past the table's true end. Root cause:
`is_ptr` proves a slot was *some* relocated pointer in the original
compiled file, not that it belongs to *this* array — and real files are
dense enough with pointers that a fixed stride past a table's end
reliably keeps finding something that validates.

Reverted the change completely (`git checkout` on `crates/ssb-rom/src/
mobj.rs` and `tools/romtool/src/main.rs`) rather than ship a
measurably-wrong heuristic. Checked whether any purely-local alternative
exists: the decomp's own two real `palettes[]` examples already disagree
on shape (`328_KirbyModel.c`'s is NULL-terminated at index 5;
`117_StageMetalFile2.c`'s 16-entry table has no terminator and runs
straight into the next struct), and a NULL mid-table is not reliably a
sentinel either — some real tables legitimately have a "no palette here"
entry that is not the end. There is no length recoverable from this
array's own bytes. Recorded as RE-088 in `docs/reverse-engineering.md`,
with `PLAN.md` R0.10 updated to reflect the correction — `mobj.rs` itself
is byte-for-byte unchanged from the previous commit.

**This changes the shape of the remaining pipeline.** `STATUS.md`'s own
prior note framed "extend `mobj.rs`" and "resolve `p_matanim_joints`" as
sequential steps 2 and 3. They are not separable that way: the palette
table's true length is only knowable once the driving material animation
script is decoded and its max `PaletteID` payload is known, so reading
`palettes[]` has to happen *at* the point a node's script reference is
resolved, using the script's own bound, not beforehand in isolation.

Immediately before this, RE-087 (previous session) picked up exactly
where RE-086 left off: found which stage to target (a `PaletteID`-cycling
script, via a temporary `romtool` subcommand, reverted) and decoded it
byte-for-byte instead of guessing at the runtime engine's shape from the
track-category census alone.

The real script is a genuine, continuous loop: `SET_VAL_AFTER_BLOCK`
steps `PaletteID` through `0,1,2,3,2,1,0,...`, then `SET_ANIM` jumps
back to the script's own start — not a one-shot key list the way
`matanim.rs`'s existing `colors_at` (built for fighter costume
selection) assumes. `colors_at` declines `JUMP` outright ("a costume
list has no reason to jump"), which was correct for its own use case
and is now confirmed *wrong* for the general one — a real script
depends on that jump to loop forever.

Added `matanim::MaterialJoint`: a persistent, tick-based player reusing
`crate::figatree::Aobj`/`Kind` — the exact interpolation state
`crate::objanim::StageJoint` already plays joint tracks with, including
already-correct `JUMP`/`SET_ANIM` handling this project didn't need to
reinvent — over a unified 15-track window (ten material tracks, then
five colour tracks), so the same engine can play `PrimColor`/`EnvColor`/
`BlendColor` later without a third parallel implementation.
`colors_at`/`Colors`/`costume_colors` (fighter costume selection,
`R0.11`'s own mechanism) are completely untouched — this is a new,
adjacent engine for the general case, not a rewrite of the existing one.

One real subtlety, found and handled rather than assumed away: a
material track's raw word is a genuine `f32` (`PaletteID`'s `0x3F800000`
really is `1.0`), but a colour track's raw word is RGBA bytes
reinterpreted, never arithmetic. Storing both in the same `f32`-typed
slots is only safe because a colour track this project's data actually
uses is always `Kind::Step` (matching `colors_at`'s own pre-existing,
accepted limitation) — `MaterialJoint::track_is_stepped` makes that
condition explicit and checkable, verified by a unit test that a
ramped (non-step) colour track is correctly flagged as untrustworthy
rather than silently trusted.

Verified with 7 new unit tests reproducing the exact real shape:
immediate (`payload=0`) and delayed (`payload=3`) steps, raw
`0x3F800000`-style float words (not integer reinterpretation), and a
`SET_ANIM`-terminated loop ticked twelve times past the script's own
length to confirm it keeps cycling rather than erroring, freezing, or
reading garbage. `cargo test --workspace`: 375 passing (was 368, no
regressions). `cargo clippy --release` (workspace): clean. `cargo psp
--release` + `tools/run-ppsspp.sh`: builds and runs clean under the
real `no_std` PSP target too (new code has to compile there even before
anything calls it), no panics, Dream Land unchanged (nothing wired to
rendering yet).

`PLAN.md` R0.10 gains two newly-checked acceptance items: "animation
data decoded" and "runtime clock implemented". What remains — "material
state updated correctly" and both "verified" items — is real,
substantial work: `mobj.rs` still reads `MObjSub.palettes[0]` only;
nothing resolves `p_matanim_joints` into per-(node, `MObj`) script
references at pack time; no pack table carries any of this; nothing on
the device side ticks a `MaterialJoint` or reloads a CLUT. This session
shipped the *engine*, tested in isolation — the *pipeline* around it
(pack format, `MaterialAnimator` lifecycle, `sceGuClutLoad` wiring) is
next, and is a genuinely different, larger piece of work than decoding
was.

Immediately before this, with every code-reading/lookup item on
`R0.12`/`R0.14` exhausted, this picked up the
first genuinely eligible `TODO` task in `PLAN.md`'s own dependency order
(`R0.11` depends on `R0.10`; `R0.10` itself depends on `R0.6`, adequately
progressed, and `R0.9`, `COMPLETE`) — per `PLAN.md` §13's autonomous rule:
resume an `IN_PROGRESS` task if one is actionable, else select the first
eligible `TODO`.

Started designing the runtime engine assuming R0.10's own framing was
right — that the interesting case is `PRIM`/`ENV`/`BLEND` **colour**
animation, since that is what `matanim.rs`'s existing decoder (built for
fighter costume selection) already reads. Before committing to that
design, dumped one concrete script (Dream Land's own material-animated
layer, file 104) and found it does not touch colour at all — it drives
`nGCAnimTrackTraU` (texture U-translate) and `nGCAnimTrackSetLFrac`
(mipmap LOD blend), a texture-sway effect. That result alone was enough
to stop and measure archive-wide before designing anything further.

RE-086 (this session) walked all 12 stage layers' `p_matanim_joints`
tables down to 172 individual scripts (temporary `romtool` subcommand,
reverted, matching RE-079/RE-081's pattern) and classified each by which
track category it ever sets. A first pass produced impossible numbers
(a track "set" more times than there were scripts) — a script that loops
back via `JUMP` re-executes the same instruction every pass, and the
walker needed to detect a revisited program counter and stop rather than
keep counting until an arbitrary cap. Fixed, the real archive-wide
picture is: **`PaletteID` cycling is 71% (122/172) of stage material
animation**, `TextureIDCurrent` (frame swapping) is 22%, UV translate/
scale/scroll is a meaningful minority, and **colour is under 2% (3/172)**
— the opposite of this task's own original framing. `mobj.rs` already
reads `MObjSub.palettes[0]` only; `palettes[1..]`, needed for the
dominant case, are never read at all currently, confirmed as a real gap.

This matters beyond trivia: implementing R0.10 as originally framed would
have correctly handled 2% of the real need and left the dominant case
(and the cheapest one to implement — the PSP GE's native indexed-texture
format already separates image from CLUT via `sceGuClutLoad`, needing no
new combiner or vertex-recolouring machinery) completely unaddressed,
with nothing that would have caught the gap short of someone noticing a
palette-cycling stage looked static. `PLAN.md` R0.10 moved `TODO` →
`IN_PROGRESS` (real, substantive investigation happened and every
acceptance item's scoping changed, even though no implementation code
exists yet — the same shape `R0.5`/`R0.7` already use for
investigation-heavy progress). No code changed this session on `R0.10` —
the census subcommand was reverted before committing; `git diff --stat`
on `tools/romtool/src/main.rs` is empty.

Immediately before this, on `R0.14`, RE-085 closed the last
actionable-right-now item this file's own previous note named:
"depth mapping verified", the one `R0.14` acceptance item with no
decomp-side constant to look up (the N64's Z-buffer is inherent RDP
hardware behavior, not a game-configurable value like the FOV RE-084
just fixed).

Confirmed `psp/src/gu.rs`'s `sceGuDepthRange(65535, 0)` +
`DepthFunc::GreaterOrEqual` matches the `psp` crate's *own documented*
`sceGuDepthRange` contract exactly: the SDK binding's own doc comment
(`sys/gu.rs:1162` in `psp` 0.3.13) states "the depth buffer is inversed,
and takes values from 65535 to 0" — this project's near=65535/far=0
call is the documented convention, not a workaround invented for a bug.
`DepthFunc::GreaterOrEqual` correctly complements it (larger buffer
value = nearer in this convention, so keeping the greater value keeps
the nearer fragment, same semantic a standard depth test achieves with
`LessOrEqual` under the opposite sign). Read the binding's own
`sceGuDepthRange` implementation to confirm the arguments are consumed
as a plain range remap, not special-cased in a way that could hide a
mismatch. Corroborated on-device: screenshotted Dream Land's stage view
(already-built, already-committed binary — no source change needed for
this check) and inspected every overlapping-geometry case visible (tree
trunk vs. canopy, decorative sprites, platform edges, the fighter
marker) for z-fighting or inside-out rendering — found none.

`PLAN.md` R0.14's "depth mapping verified" item is checked, and
`DECISIONS.md` D-007's previously uncited "Verified working" now points
at RE-085. No code changed — a documentation/audit pass confirming an
already-shipped implementation matches its own SDK's stated contract,
the same shape as RE-072/RE-082/RE-084's non-fix findings. `R0.14` is
now 5 of 7 items checked; the remaining two ("camera transforms
verified", "representative scenes compared") are structurally blocked
on a real game camera system that does not exist yet, not on further
investigation of this kind.

Immediately before this, RE-084 (this session) did the "actionable-right-
now" `R0.14` FOV lookup this file's own previous "Next Eligible Task" note
named directly: read the decompilation's real camera setup instead of
continuing to carry an unsourced constant.

`psp/src/main.rs` called `sceGumPerspective(60.0, ...)` every frame with
no comment, citation, or decision record explaining where `60.0` came
from — checked, and it does not trace to anything. `refs/ssb-decomp-re/
src/gm/gmcamera.c:1191` sets the real default battle camera's
`gGMCameraStruct.fovy = 38.0F`, and `gmCameraAdjustFOV(38.0F)` (the
function that smoothly re-targets the live FOV) is called with exactly
that value from four separate camera-behavior functions in the same
file — only two other call sites (`...PlayerZoom`/`...PlayerFollow`, a
KO/photo-finish zoom and a follow-camera mode) take a different,
caller-supplied situational value. Four-to-two in one file is strong,
non-exhaustive evidence that `38.0` degrees is the real default, and
`60.0` was this project's own unsourced guess, 58% wider than the
original.

Fixed the constant, and — because this interacts with framing, not just
a number in isolation — recomputed the two `FIT` constants
(`psp/src/main.rs`'s stage-view and object-view debug-camera distance
calculations) that were themselves derived from the *old* 60-degree FOV
(`1/tan(30°) ≈ 1.733`, by their own existing comments) to their
38-degree equivalent (`1/tan(19°) ≈ 2.904`, `≈1.677×` larger) — without
this, every stage and object would visibly zoom in and crop at the
viewer's default zoom purely as a side effect of correcting the FOV,
an unrelated regression this fix should not introduce. Verified by
screenshot, not just arithmetic: Dream Land's stage view before
(60°/`FIT=1.733`) and after (38°/`FIT=2.904`) frames the whole stage the
same way at the default zoom. `cargo psp --release` clean, fresh
`tools/run-ppsspp.sh` run clean (no panics), `cargo test --workspace`
unaffected (`psp/` has no host test suite, excluded from the workspace
per `Cargo.toml`), `cargo clippy --release` (workspace) clean.

`PLAN.md` R0.14's "projection matrix verified" item is now checked — the
FOV term is sourced from the decompilation, joining RE-082's already-
verified aspect/viewport terms. Left open: "depth mapping verified"
(D-007's evidence is thinner than the others and wasn't re-audited),
"camera transforms verified" and "representative scenes compared" (both
need a real game camera system, which doesn't exist yet — sourcing the
FOV *value* is not the same as reproducing the camera's actual
positioning/movement behavior).

Immediately before this, RE-083 (this session) picked up the concrete,
ready candidate this file's own previous "Next Eligible Task" note
pointed at once `R0.14`'s aspect-ratio work (RE-082, below) closed out
what it could reach without a real game camera.

Checked `R0.12`'s two specific open worries directly. First, "the
decomp's `rot_mode` choice between matrix kinds 45/46" turned out to be
a non-issue rather than an unmodelled gap: `gcDecideDObj3TransformsKind`
(the function with the actual `rot_mode` branch) is only ever called
from `gcSetupCustomDObjs`, the runtime/dynamic transform path RE-063
already ruled out of this project's scope. Read `gcSetupCommonDObjs`
directly (the *only* ROM-driven path, per RE-063) and confirmed it maps
`0x4000`→`Kind46`/`0x2000`→`Kind48` unconditionally, with no `rot_mode`
branch in it at all — kind 45 (and 47, 49) are structurally unreachable
from any `DObjDesc` array, so there was nothing left to model.

Second, ran an archive-wide census (temporary `romtool` subcommand,
reverted, matching RE-079/RE-081's pattern) of every billboard-flagged
node's own primitives: 109 nodes (not the `81` `PLAN.md` had stated —
stale since RE-062 added `RecalcRotRpyRSca` billboards; `STATUS.md`'s
own history already had 109, just never propagated to `PLAN.md`, now
fixed), 118 primitives. `z_buffer` is on for **100%** of them — depth
behavior for billboards is unambiguous and matches RE-068's RDP-reset
default with zero exceptions. `alpha_test` (28.8%) is the same
already-shipped RE-069 mechanism, nothing further needed. `translucent`
(29.7%) is the interesting number: it is **not** a new problem, it is
RE-069/RE-071's already-known, still-unresolved gap (deferred after
producing a checkerboard on Dream Land's own canopy-highlight surface)
— but billboards hit it at roughly **double** the archive-wide rate
(14.4%), meaning that gap's priority should be weighted by billboards
specifically, not treated as a uniform archive-wide long tail.

`PLAN.md` R0.12 gains four newly-checked acceptance items: "billboard
types enumerated" (RE-063), "camera-facing transforms verified"
(RE-049, already existed, just never checked off), "depth behavior
verified" (this session), and stays `VERIFYING` overall since "scale
verified", "orientation verified", "texture orientation verified" and
"all flagged billboard nodes verified" remain open — this census
measured render *state* distribution, not per-node visual correctness
beyond RE-049's own Dream Land spot check. No code changed; the
temporary subcommand was reverted before committing — `git diff --stat`
on `tools/romtool/src/main.rs` is empty.

Immediately before this, RE-082 (this session) picked up the fresh,
unblocked candidate this file's own previous "Next Eligible Task" note
pointed at, since both `R0.6` (blocked on `R0.7`) and `R0.5` (RE-081,
just below, now looking like it needs real hardware) ran into real
blockers this session.

Re-audited RE-034's own long-standing loose end: after fixing the real
viewport/projection mismatch it found, RE-034 reported a residual
6.6% width/height error on the fighter's collision-diamond marker
(`1.000` measured against `0.938` expected) and never explained it.
Three independent re-measurement attempts this session — the same
default-zoom screenshot with threshold-sensitivity analysis, and a
temporary (reverted) `cam_distance` override in `psp/src/main.rs` for a
bigger on-screen marker at two different zoom levels — produced three
different ratios (`0.82`, `0.90`–`0.95`, `1.14`–`1.16`), straddling both
`1.0` and the expected `0.9375` depending on method. This showed the
marker (20–80 pixels depending on zoom) is too small for the
single-digit-percent precision RE-034's own number implied, before
concluding anything about a bug either way.

Resolved it by reading the code instead of continuing to fight the
screenshot: `psp/src/gu.rs`'s `Gpu::init` (which sets `sceGuViewport`/
`sceGuScissor`) and `psp/src/main.rs` (which sets the projection's
`aspect` parameter) both call the *same* `coord::pillarboxed_viewport()`
and use its output directly — the exact two values RE-034's original bug
had disagreeing cannot diverge again by construction. Checked the `psp`
crate's `sceGumPerspective` binding itself (`sys/gum.rs`, VFPU assembly):
it computes `m.x.x = cot(fovy/2)/aspect`, `m.y.y = cot(fovy/2)` — the
standard textbook symmetric-frustum formula, no quirk. Combined with
`coord.rs`'s own existing passing unit test pinning `pillarboxed_viewport`'s
arithmetic, there is no remaining code path that could produce a real
aspect-ratio defect. Concluded RE-034's residual was pixel-counting noise
on a too-small shape, not a surviving bug — a correction to that entry's
*confidence*, not to its *fix* (which stands, unchanged).

`PLAN.md` R0.14 gains three newly-checked acceptance items: "viewport
verified", "aspect ratio verified", and "N64/PSP resolution differences
explicitly handled". Left deliberately open: "projection matrix
verified" (this audited the aspect term specifically, not the `60.0`
degree FOV constant's own provenance, which is the debug viewer's own
arbitrary choice), "camera transforms verified" (no real game camera
system exists yet to check against the original's), "depth mapping
verified" (D-007's evidence is thinner than RE-034/RE-082's and was not
re-audited this session), and "representative scenes compared" (no
side-by-side N64-vs-PSP reference imagery exists). No code changed — the
temporary `cam_distance` edit and all screenshots were reverted/discarded
before finishing; `git diff` after this session's `R0.14` work is
documentation-only, the same shape as RE-072/RE-076/RE-081.

Immediately before this, RE-081 (this session) was a brief,
self-contained detour into `R0.5` (matching RE-070's own precedent),
picked from this
file's own "push further on the dither" option once R0.6's combiner-shape
work (RE-079/RE-080, below) ran into `R0.7`'s blocker. Resolved RE-053's
long-standing apparent self-contradiction (its UV-span math said the
canopy was "minified", its visual symptom said "magnified") by measuring
Dream Land's two canopy textures *separately* instead of as one case:
`romtool textures --file 104` shows the "gradient" texture genuinely
minified (`3.70×1.36` repeats, exactly RE-053's own number) and the
"highlight" texture magnified on its V axis (`1.56×0.88`, below 1.0) —
RE-053 was right on both counts, just about two different textures the
fix was applied to as if they were one.

Also tested this file's own previously-untried "larger blur radius or
multiple passes" idea directly: a temporary (reverted, not committed)
`romtool` subcommand measured mean adjacent-pixel channel difference on
both canopy textures at 0/1/2/3 `box_blur_wrapped` passes — a second
pass reduces noise a further ~35–40% beyond the already-shipped single
pass (gradient 5.64→3.69, highlight 6.14→3.73). Tested it as a real,
reversible on-device change before trusting the number: rebuilt the pack
with a temporary double-blur edit (no `psp/` source changes needed, only
data differs), took before/after screenshots of the same cropped canopy
region, and found `magick compare` reports a real but small difference
(`MAE` 0.26%, `RMSE` 1.5%) that is not visually distinguishable at the
tested camera distance — the dither pattern looks the same in both
crops. This is the same outcome RE-075 already found for a different
change to these same textures. Per RE-071's standing rule (a measured
improvement alone is not sufficient — the image has to actually look
better), **not shipped**. Reverted the experimental subcommand and the
double-blur edit completely; `git status`/`git diff` confirmed the tree
matched `HEAD` before this investigation, and a fresh repack from the
reverted state matched the previously committed pack.

`PLAN.md` R0.5 gained two newly-checked acceptance items
("magnification behavior identified", "minification behavior
identified") from the disambiguation above, but "Dream Land canopy
discrepancy resolved" stays open — if anything, more clearly blocked on
real hardware than before, since a substantially larger blur change
than RE-070's shipped one still did not surface on screen under PPSSPP.
`git diff --stat` after this session's work is empty except for
documentation (`PLAN.md`, `STATUS.md`, `docs/reverse-engineering.md`) —
this was a measurement pass, not an implementation one, the same shape
as RE-072/RE-076.

Immediately before this detour, RE-080 (this session) fixed the one real
gap RE-079 (below) identified but left open: `(ZERO-ZERO)*ZERO+PRIM`
(1,589 primitives archive-wide), a flat constant colour with no shade or
texel dependence, which neither `combiner_shade_scale` nor
`combiner_texture_blend` can express. Added `combiner_flat_color`,
structurally disjoint from the other two (each requires a different,
mutually exclusive combination of RE-079's presence flags), gated the
same way RE-079 fixed `combiner_texture_blend` — only requires whichever
of `PRIM`/`ENV` the shape actually reads, so a bare `ONE` (28
occurrences) needs neither. Also covers `(ZERO-ZERO)*PRIM+ENV` (9
occurrences, `ENV` alone via a different slot arrangement) for free,
since it is the same underlying arithmetic case, not a new shape needing
new logic.

Wired further than `texture_blend` was left at its own equivalent stage
(RE-073 detected and packed it, RE-074 wired consumption later): since
`TEXEL` provably never enters this shape's formula, `material_now`
immediately forces the primitive untextured (rather than let the GE's
default `Modulate` silently sample and multiply in whatever texture
happened to be bound) and `push_vertex` bakes the resolved colour into
affected vertices, the same content-keyed-dedup-safe mechanism
`prim_color`'s scale and `texture_blend`'s base colour already use.
Packed as `pack.rs`'s `flags::FLAT_COLOR` and `PrimDesc::flat_color`
(`PrimDesc::SIZE` 32→36 bytes, `VERSION` 10→11) — no `psp/` changes
needed, since an untextured primitive with a baked vertex colour already
renders correctly through the existing path.

Repacking the whole archive measured a real, cross-checked side effect:
bound textures **644 → 639**, mip-carrying textures **223 → 221**. Five
textures were referenced only by primitives whose combiner never
actually reads them — previously packed and uploaded for nothing, now
correctly dropped. `cargo test --workspace`: 368 passing (was 364; four
new tests: three unit tests for `combiner_flat_color`, one integration
test through `convert` confirming a textured display list's flat-colour
primitive comes out untextured with its vertex baked). `cargo clippy
--release` (workspace): clean. `cargo psp --release` +
`tools/run-ppsspp.sh`: Dream Land renders at 60 FPS, debug overlay's
texture counter reads `0/639` matching the repack, scene visually
unchanged (Dream Land's own geometry doesn't use this shape). Both
`PLAN.md` R0.6 "primitive color"/"environment color verified" items
still stay open — not because of an uncaught shape any more, but because
`(PRIM-ENV)*TEXEL0+ENV`'s remaining misses are a genuine absence of
`prim_color`/`env_color` on this converter's own state (likely `R0.7`'s
territory), which no further combiner-classification work can reach.

RE-079 (also this session) did the
"systematic accounting of every distinct `SetCombine` shape" RE-073 left
as the open reason "primitive color verified"/"environment color
verified" were unchecked. Temporarily instrumented `mesh.rs`'s
`material_now` (reverted before committing) to log every
combiner-bearing primitive's raw combiner words and whether
`combiner_shade_scale`/`combiner_texture_blend` already recognised them,
then ran it through the real `romtool pack` archive walk: 262,778
combiner-bearing primitives, 97.0% already recognised.

Found and fixed two real bugs in the two misses that mattered, not just
measured them. First: `combiner_shade_scale` evaluated a combiner into a
`k`/`s`/`t`/`st` decomposition and inferred *which* term was present by
checking which was numerically nonzero — indistinguishable from "this
term is present with value exactly black". `(PRIM-ZERO)*SHADE+ZERO`
(RE-039's own "prim_color" mechanism) declined for 1,118 primitives
whose `PRIM` was set to exactly `[0,0,0,255]`, silently rendering
unmodified (non-black) vertex shade instead of the solid black real
hardware always produces. Second: `combiner_texture_blend` required
*both* `PRIMITIVE` and `ENVIRONMENT` to be set even for shapes that only
read one of them — `(ONE-ENV)*TEXEL0+ENV` (125 occurrences) never reads
`PRIMITIVE` at all, but declined whenever `prim_color` merely hadn't
been set elsewhere.

Fixed by giving `Combined` a `_used` presence flag per term, independent
of its numeric value, threaded through `zip`/`sub`/`add`/`mul`; and a
new `combiner_reads(hi, lo, two_cycle, code)` helper so
`combiner_texture_blend` only requires a colour the shape actually
reads. The first version of the presence-flag fix regressed a separate
27-primitive shape (`(ONE-ZERO)*ZERO+SHADE`) by conflating "multiplied
by a real constant that happens to be black" with "multiplied by a
literal, structurally-absent hardware zero" — caught by the archive-wide
before/after census (hit count dropped from 27 to 0), not by a unit
test, and fixed by having `mul` collapse the whole product away only
when the constant side's own value is itself unsourced. Verified with
three new unit tests, `cargo test --workspace` (364 passing, up from
361, no regressions), `cargo clippy --release -p romtool -p ssb-rom`
clean, and a fresh `cargo psp --release` + `tools/run-ppsspp.sh`: Dream
Land renders at 60 FPS, pixel-identical (expected — its own primitives
don't use either fixed shape). Archive-wide effect: 97.0% → 97.5%
recognised. Temporary instrumentation (the `mesh::census` module and a
`romtool combine-census` subcommand) was fully reverted before
committing, matching `AGENTS.md` §17's "do not commit temporary
debugging artifacts" — `git diff --stat` on `tools/romtool/src/main.rs`
is empty; only `mesh.rs`'s real fix and tests remain.

Both `PLAN.md` R0.6 acceptance items stay unchecked: a third real,
uncaught shape (`(ZERO-ZERO)*ZERO+PRIM`, 1,589 primitives, a flat
constant colour with no shade/texture dependence) needs a new
`MeshMaterial` field rather than a classification fix, and
`(PRIM-ENV)*TEXEL0+ENV`'s remaining 3,085/4,580 misses are a genuine
absence of `prim_color`/`env_color` on this converter's own state
(likely `R0.7`'s material-table pairing gaps), not something this pass's
fix could reach.

## Last Completed Task

`R0.7`-adjacent (RE-078, previous session) extended RE-077's method
archive-wide instead of stopping after Kirby's one fix. Ran
`ssb_rom::mobj::search_tables` over all 63 graphs R0.7 still had no
table for; 13 came back with exactly one demand-matching candidate
(the other 50 stayed ambiguous). Checked each of the 13 against its own
file's decompilation with an address-anchored `@ 0x<offset>` match, not
a substring search — that distinction caught a real mistake before it
shipped: a looser first pass "confirmed" two of file 85's candidates by
matching a sub-offset baked into an unrelated symbol's name
(`..._sub_0x108`), not the actual byte address 0x108, and both were
dropped once re-checked properly.

Six survived the stricter check, each confirmed by *both* address and
entry count matching a real, named, typed decompiled symbol: files 22
(`MNPlayersSpotlight`), 69 (`MVOpeningStandoff`), 75
(`MVOpeningRunCrash`), 83/84 (`EFCommonEffects1`/`2`), and 167
(`MNTitle`). File 84's took extra care — the search's candidate address
sat 8 bytes before the decompiled symbol's own start, which turned out
to be `PAD(8)` exactly covering that graph's two genuinely zero-demand
leading nodes, not a mismatch. Fixed via `PartTables::insert()`
(matching RE-059/060/077's established pattern); verified archive-wide:
`romtool mobj` paired 64→70, mismatches held at 0 across 383 nodes (up
from 364); `romtool textures` packed 638→646, failures held at 27 (same
known classes, none new). `cargo test --workspace` (218 passing,
unaffected) and `cargo psp --release` both clean; Dream Land's stage
view re-screenshotted and pixel-identical. The other 7 unique hits
(file 85's two false positives, plus one each in a stage file and two
special-move files landing on still-untyped bytes) were checked and
correctly left unfixed.

`R0.7` — RE-077 (earlier this session) followed up directly on RE-076's
own hedge instead of leaving it
unchecked. RE-076 guessed that several fighters' low measured texture
counts were "almost certainly" undercounts from R0.7's 64 unpaired
`MObj` graphs. Checked it file by file: `romtool mobj --file <id>` for
each of the 11 remaining real playable fighters found **nine with zero
unpaired graphs** (Mario, Fox, Donkey Kong, Samus, Luigi, Jigglypuff,
Captain Falcon, Yoshi, Pikachu) — their low counts are a real low-poly
N64 model, not a gap. Only Kirby (5 unpaired) and Ness (1) had real
gaps. Got the full, untruncated list of all 64 unpaired graphs (not the
CLI's default 12-line summary) and found most are menu/character-select
emblem models, stage files and fighters' special-move files, not core
fighter bodies.

Kirby's largest gap (`JointTree_0x19F08`, 22 nodes) had exactly one
demand-length-matching `--search` candidate (0x18D60) — anchored to real
relocation pointer slots, across a non-uniform demand sequence, not
RE-061's failure mode of one repeated value. Cross-checked against the
decompilation before trusting it: `328_KirbyModel.c:7254` types exactly
that region as a real, fully-typed 24-slot `MObjSub **` array, and
0x18D60 lands precisely on slot 2 with slots `[2..24)` matching the
graph's 22 nodes exactly — not a coincidence. Fixed via
`PartTables::insert()`; verified `romtool mobj --file 328` (paired 2→3,
unnamed 5→4, 0 mismatches across 21 nodes) and a clean `cargo psp
--release` + pixel-identical Dream Land regression screenshot. Re-
measured Kirby's aggregate texture VRAM afterward: unchanged (the
newly-resolved materials reuse textures his other objects already
reference) — this is a rendering-correctness fix, not a VRAM one, so
RE-076's larger point (archive-wide vs. per-scene VRAM shouldn't be
compared directly) still stands, but its "several fighters are
undercounted" hedge was wrong and is now corrected in
`docs/memory.md`/`PLAN.md`/`TODO.md`. Ness's one candidate (5-way
ambiguous, one candidate sitting in a decomp region flagged as
previously mis-typed) and Kirby's other four (2-node, 10-way ambiguous,
possibly further sub-ranges of the same confirmed array) were checked
and left unfixed — suggestive, not conclusive.

`TODO.md` Phase G — RE-076 (earlier this session) measured whether
"texture streaming... required" was actually supported by the number it
was being justified with. `docs/memory.md` compared the packed set's
**1170.9 KiB archive-wide total** directly against the **700 KiB
per-scene budget** — but those are different things, since no single
scene needs every stage, fighter, menu and effect loaded at once.
Walked the actual pack (not the ROM) and measured what one real scene
needs: the largest stage (Dream Land, 137.0 KiB) plus the four largest
of the 12 real playable fighters, texture indices deduped rather than
summed blind, comes to **217.1 KiB** — well under half the budget.
Flagged, at the time, that this was likely an undercount from R0.7's
unpaired `MObj` graphs — RE-077 (above) checked that specific claim and
mostly refuted it. Updated `docs/memory.md`/`TODO.md` to stop treating
"streaming is required" as settled and instead point at what's actually
known: the per-scene need is probably much smaller than the archive-wide
total, and `docs/memory.md`'s already-planned per-scene `AssetArena`
(one load per scene transition, mirroring the original's own loading
pattern) may already be enough without a separate runtime residency
system. No code changed in this pass — it was a measurement correcting
a comparison, not an implementation.

`R0.5 — Texture Sampling Correctness` — RE-075 (earlier this session)
was a brief, self-contained detour picked from the previous pass's own
"push further on the dither" option. Found and fixed a real but small
ordering bug in the canopy dither blur (`tools/romtool/src/main.rs`'s
`convert_texture`): it blurred *after* mirroring, so
`box_blur_wrapped`'s toroidal wraparound sampled the mirrored copy at the
seam instead of the texture's own true periodic neighbour. Reordered to
blur first, mirror second — same cost, more correct. Before trusting it
changed anything, built two packs (one per order, isolated via `git
stash`) and byte-diffed them directly: **6724 bytes differ** between the
two canopy textures' packed data, confirming the change is real, not a
no-op. Then screenshotted before and after anyway, because a byte diff
is not a visual confirmation: the canopy crop was **pixel-identical** at
the debug viewer's default camera distance — the difference is real but
too small (seam-adjacent texels only) and too minified at this distance
to be currently visible. Shipped as a correctness cleanup that costs
nothing, explicitly **not** claimed as progress on RE-053's still-open
dithering discrepancy.

R0.6 — Material System Correctness — RE-074 (earlier this session)
closed the loop RE-073 left open. RE-073's low-confidence deferral was
whether baking a `TEXTURE_BLEND` primitive's flat base colour into its
vertices could corrupt a vertex shared with a normally-shaded primitive
loading the same RSP vertex-cache slot. Reading `crates/ssb-rom/src/
mesh.rs`'s existing `Builder::push_vertex` answered it without new
experimentation: it already folds `prim_color`'s scale into a vertex's
`rgba` *before* deduplicating (`Builder::seen: BTreeMap<MeshVertex,
u16>`, keyed on the already-coloured vertex), and its own doc comment
says the dedup turns a vertex shared by two differently-coloured
primitives into two entries by itself. `texture_blend` baking a flat base
colour the same way inherits that guarantee for free — the "risk" was a
real gap in what had been *checked*, not a real gap in the architecture.

Shipped the bake (mirroring `prim_color`'s branch) and wired
`psp/src/meshdraw.rs`'s `apply_material` to
`sceGuTexFunc(TextureEffect::Blend, ...)`/`sceGuTexEnvColor`. Doing the
wiring correctly surfaced a real, previously-latent bug: `bind_texture`
unconditionally called `sceGuTexFunc(Modulate, ...)` on every texture
change, which would have silently clobbered a `Blend` state set moments
earlier whenever a `TEXTURE_BLEND` primitive's texture changed but its
coarse `flags` word didn't (two primitives can share identical flags with
different target colours). Fixed by removing that hardcoded call and
tracking the blend state in its own `DrawState::last_texture_blend`
field, independent of the existing `last_flags`/`last_texture` fields —
the same reasoning that gives each of those its own field already.

Verified visually, not just by compiling: no existing debug-viewer
control reaches a specific fighter's specific object headlessly, so a
temporary, fully reverted patch to `psp/src/main.rs` forced the viewer
onto object 306 (file 324 LinkModel's own `TEXTURE_BLEND` piece, found by
scanning the pack for the flag). Screenshotted before (via `git stash` of
the wiring changes, rebuilding from a deleted `EBOOT.PBP` each time per
RE-070's lesson) and after: before, a flat monochrome grey shape (the raw
packed-normal byte, no combiner colour); after, the correct grey-to-orange
gradient. Also re-screenshotted Dream Land's stage view (unaffected by
this shape) before and after — pixel-identical, no regression. `PLAN.md`
R0.6's "combiner behavior verified" item is now checked.

R0.6 — Material System Correctness — RE-073 (earlier this session)
measured what `combiner_shade_scale` actually declines. A reloc-anchored
scan (not `Exhaustive`, per RE-072's lesson) over every `G_SETCOMBINE`
found 79 of 1360 (5.8%) reading `ENVIRONMENT`, 72 of those (91%) matching
one shape in some cycle: `(PRIM-ENV)*TEXEL+ENV`, a texture-driven blend
from `ENV` to `PRIM` with no shade term at all — across 28 files,
including three fighters' own base models (Link, Ness, Pikachu). Verified
one occurrence directly against ROM bytes (Link's own model, offset
`0x11670`, `hi=0x00309661 lo=0x552EFF7F`), not just the scan output.
Added `combiner_texture_blend` (`crates/ssb-rom/src/mesh.rs`) to detect
it, gated on a real texture being bound (same reasoning as RE-069's
`alpha_test`/`translucent` gate), and factored the shared two-cycle
evaluation logic out into `evaluate_combiner` (behaviour-preserving).
Shipped detection into the pack format (`pack.rs`'s `flags::TEXTURE_BLEND`
plus two new `PrimDesc` fields, `PrimDesc::SIZE` 24→32, `VERSION` 9→10) —
device-side consumption followed in RE-074, above.

R0.6 — Material System Correctness — RE-072 (earlier this session)
closed the "fog verified" item, and along the way caught its own
near-miss: an archive-wide re-scan (using `scan::Candidates::Exhaustive`)
initially found `G_SETFOGCOLOR` 7 times and `G_FOG` geometry-mode sets 4
times, seemingly contradicting `DECISIONS.md` D-025's existing "twice"
figure. Before treating that as a correction, cross-checked against
reliable, reloc-anchored discovery (`find_root_display_lists`, which only
follows real pointers) — under that, only 2 `SetFogColor` occurrences
survive and 0 `G_FOG` hits. The `Exhaustive`-mode extras were false
positives; D-025's original number was right all along, and the
near-correction would have been the actual mistake. Went further than an
occurrence count to confirm those two are functionally inert, not just
rare: grepped the entire decompilation for `gSPFogPosition` (the call
that gives the RSP a fog range to compute against) — zero results,
anywhere, in any file. Read the one real stage that sets a fog colour
(file 118, `118_StageYosterSmallFile2`, confirmed via `romtool stages` to
be one of the 41 currently-loaded stages) and checked whether its own
`G_SETRENDERMODE` calls ever reference `G_BL_CLR_FOG` — both use
`G_BL_CLR_MEM` instead. `DECISIONS.md` D-025 stands, now backed by
checking that the surrounding machinery doesn't exist either, not just
that the command is rare. `PLAN.md` R0.6's "fog verified" item is
checked. No code changed in that pass — it concluded "correctly not
implemented," not "needs implementing."

R0.6 — Material System Correctness — RE-071 (earlier this session)
followed up on a natural question RE-070 raised: now that Dream Land's
canopy-highlight texture is pre-blurred (RE-070), is RE-069's deferred
`translucent` blend (deferred because it produced a checkerboard on that
same texture) safe to enable? Re-tested directly, as a reversible
experiment (re-enabled `GuState::Blend`, rebuilt clean from a deleted
`EBOOT.PBP` per RE-070's own lesson about stale binaries): **no** — the
result is different and *worse*, blown-out oversaturated highlights
erasing the flowers and most other detail, not the earlier checkerboard.
Objective pixel statistics alone said "~4% less noise," which would have
been a misleading green-light if trusted without also looking at the
actual image — a second methodology lesson layered on RE-070's first one.
Also tested and ruled out unpremultiplied-alpha blurring as the cause (a
premultiplied variant gave an identical result). Both experiments were
reverted before commit; only the negative-result record remains, in
`docs/reverse-engineering.md` and a permanent, updated comment in
`psp/src/meshdraw.rs` so a future session doesn't re-run either
already-eliminated experiment. `PLAN.md` R0.6's "blending verified" item
stays open, narrowed but not closed.

R0.5 — Texture Filtering / LOD / Mipmapping — RE-070 (earlier this
session) directly tested RE-053's own two suggested fixes for Dream
Land's canopy dither. Filtering alone: a reversible on-device
`Nearest`-vs-`Linear` A/B measured it helps a little but not enough
(bilinear only interpolates a 2x2 neighbourhood, narrower than the
dither's repeat). Resolving it at conversion time: box-blurring the two
canopy textures and requantizing back to their 16-entry CI4 palette
changed nothing visible (the blur mostly snaps back to the same two
palette entries); packing the same blur unquantized (`Psm8888`) instead
produced a real, measurable improvement — after catching and correcting a
methodology mistake (a stale `EBOOT.PBP` made the first look appear to be
a full fix; rebuilding clean and measuring pixel statistics objectively
showed a real but partial ~19-40% noise reduction instead). Implemented
as `crates/ssb-rom/src/texture.rs::box_blur_wrapped`, applied through a
named allowlist (`tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR`) of
exactly the two Dream Land canopy textures. Costs +112 KiB VRAM
(1059.0→1170.9 KiB). `PLAN.md` R0.5's "Dream Land canopy discrepancy
resolved" item stays unchecked — real progress with numbers behind it,
not a claim of completion.

R0.6 — Material System Correctness — RE-069 (earlier this session)
decoded `G_SETOTHERMODE_L`'s render-mode field for the first time
(`mesh.rs` never read it at all) into `alpha_test`/`translucent`. A naive
"`FORCE_BL` bit means blending" signal is wrong — the RDP reset's own
opaque default sets it too — so the real signal (whether the blend
equation reads the framebuffer weighted by `1 - alpha`) was cross-checked
against `gbi.h`'s macros and `refs/BattleShip`'s interpreter. Measured
archive-wide: 36.1% of non-default render modes are cutout (`TEX_EDGE`-
family) surfaces, 14.4% genuinely translucent — both previously completely
unimplemented on the PSP side. Shipped `alpha_test` (matching
`refs/sf64-psp`'s validated real-hardware approximation) after finding and
fixing a bug where untextured lit primitives were alpha-tested against a
packed-normal byte and discarded themselves outright, visibly deleting
Dream Land's decorative flowers. Found a second, harder bug in
`translucent` — enabling real blending on the canopy-highlight surface
produced a checkerboard, the same open dithered-texture/coverage problem
RE-053 already found — and deliberately did not ship it, leaving
detection in place but `GuState::Blend` unwired on device pending that
investigation (which RE-070, above, then partially advanced). `PLAN.md`
R0.6's "alpha behavior verified" item is checked; "blending verified"
stays open.

R0.6 — Material System Correctness — RE-068 (earlier this session) found
and fixed a structural gap far bigger than the "depth state verified"
item it started from: read `refs/ssb-decomp-re/src/sys/rdp.c`'s
`sSYRdpResetDisplayList`, replayed once per frame (`taskman.c:308`) before
any object draws, and found it sets `G_ZBUFFER | G_SHADE | G_CULL_BACK |
G_SHADING_SMOOTH` **on** by default — not all-off. `crates/ssb-rom/src/
mesh.rs`'s `State::new()` seeded an all-off `MeshMaterial::default()`
instead, so any node whose own list never mentioned geometry mode (the
common case, since this state is normally set once per frame, not per
node — the same structural shape as RE-021's lighting finding, one level
up) converted as unculled, flat-shaded and non-depth-tested. Measured
archive-wide: `Z_BUFFER` went from 6/3426 packed primitives (0.17%) to
3384/3442 (98.3%) after seeding from a new `MeshMaterial::rdp_default()`
instead; `CULL_BACK` measured 86.3%, `SMOOTH` 76.5% post-fix — the shape a
real game's geometry should have. Also wired `psp/src/meshdraw.rs`'s
`apply_material` to actually toggle `GuState::DepthTest` per primitive
from the `Z_BUFFER` flag (it was already packed, just never read on the
device side). `PLAN.md` R0.6's "depth state verified" and "culling
verified" items are checked.

R0.5 — Texture Filtering / LOD / Mipmapping — RE-067 (earlier this
session) traced `G_TX_MIRROR` (RE-066's one real, quantified gap) directly
to Dream Land's still-open canopy discrepancy (RE-053): its exact display
list (file 104, offset `0x798`) sets `cm_s=3 cm_t=3 mask_s=6 mask_t=6`.
Confirmed the wrap boundary actually mattered *before* implementing
anything — a reversible on-device `Repeat`-vs-`Clamp` experiment (2-line
change, screenshotted, reverted) showed a dramatically different image.
Implemented the real fix: `crates/ssb-rom/src/texture.rs::mirror_extend`
pre-bakes a mirrored copy of each affected texture at pack time (exact,
not approximated, since `sceGuTexScale` already renormalises UVs against
the packed texture's own dimensions). Not scoped to Dream Land alone: 187
of 638 packed textures (29%) affected archive-wide, packed texture VRAM
rose from 763.2 KiB to **1059.0 KiB (1.5x the ~700 KiB budget)**. Given
the scale of that tradeoff, stopped and asked the user how to proceed
(ship fully / scope to paletted formats only / document without shipping
/ revert) rather than deciding unilaterally — the user chose to ship it
fully. `PLAN.md` R0.5's "Dream Land canopy discrepancy resolved" item
stays unchecked — this fixed one real contributing cause, but RE-053's
separate magnification/dithering diagnosis is untouched. Texture
streaming (`TODO.md` Phase G) is no longer optional headroom given the
new VRAM figure.

RE-066 (earlier still) closed R0.5's "wrap/clamp/mirror behavior
verified" and "texture tile parameters verified" items with a measurement,
not a code fix at the time: read every tile-0 `G_SETTILE` archive-wide
(754, not sampled) and found `psp/src/meshdraw.rs`'s hardcoded
`sceGuTexWrap(Repeat, Repeat)` is already correct for clamp — every axis
that requests clamp also has its own mask nonzero (0 counterexamples), and
`refs/BattleShip`'s reference RDP interpreter strips `G_TX_CLAMP` under
exactly that condition on real hardware, confirming `mesh.rs`'s existing
mask-narrowed texture sizing (RE-044) already reproduces correct periodic
addressing. Identified
`G_TX_MIRROR` (208/754 tile-0 lists, 27.6%) as the one real, quantified
gap — which RE-067 (above) then traced and fixed.

R0.6 — Material System Correctness — its lighting placeholder replaced
with a real, ROM-measured value this session (RE-065): read
`MPGroundData.light_angle` (a per-stage field added to `stage::GroundData`,
byte offset independently corroborated against the already-verified
`camera_bound_top` offset) across all 41 stages and found 33 of them (80%)
share one `(20, 45)` degree angle — now used exactly for `pack.rs`'s baked
`LIGHT_DIR`. The other 8 stages (special-lighting locations: Brinstar,
Sector Z, Metal Mario's stage, etc.) diverge up to 111 degrees and are
recorded as an explicit, measured, `AGENTS.md` §9-compliant accepted
deviation — full per-stage correctness needs runtime `sceGuLight` lighting,
out of scope for a pack-time-baked-shading architecture. R0.6's "lighting
verified" acceptance item stays unchecked (the deviation is now properly
recorded, not eliminated), and its other open items (material tables,
combiner/alpha/blend/fog/depth/culling verification) are untouched.

R0.4 — TLUT / Palette Correctness — its "palette inheritance/state
verified" acceptance item closed in an earlier pass (RE-064): added a
direct unit test (`a_texture_binding_persists_into_a_node_that_sets_no_new_state`,
`crates/ssb-rom/src/mesh.rs`) pinning that RDP texture/palette state
threads across a node sequence the way real hardware keeps it, previously
only measured archive-wide. Confirmed the test can actually fail (broke
the inheritance mechanism temporarily, watched it fail, reverted) before
trusting it green. Also confirmed by reading `tools/romtool/src/main.rs`'s
pack loop that state cannot leak *between* different objects/graphs by
construction (fresh `State::new()` per graph). One item remains open on
R0.4 — "all missing palette cases resolved" (1 `MissingPalette` failure,
file 86) — already fully attributed to `R0.7`'s scope, not touched this
pass; `R0.4` stays `IN_PROGRESS` rather than closing, since that item is
still literally unmet even though it isn't this task's fault.

R0.8 — Transform Correctness — `COMPLETE` (RE-063, earlier pass). Closed
by enumerating every transform kind (`gcSetupCommonDObjs` only ever
emits kinds 44/46/48/50 from a ROM `DObjDesc` array; everything else is
runtime-game-code-only and out of scope), implementing `Kind50` the
same way `RecalcRotRpyRSca`/`Kind46`/`Kind48` already were, and adding
regression coverage (`a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`,
`crates/ssb-rom/src/pack.rs`). `cargo test --workspace`: 340 passing (was
339). `cargo clippy --release -p romtool -p ssb-rom`: clean. `cargo psp
--release`: builds clean. `tools/run-ppsspp.sh --no-build --seconds 8`:
Dream Land renders correctly at 60 FPS (`FPS: 60.0`, `cpu 2353us / budget
16667us`), clean log, no regression (Kind50 contributes 0 nodes so the
screenshot is pixel-identical to before). `romtool pack` still reports 109
billboard nodes.

Earlier this session (previous "Continue with the plan" pass), R0.8 also
fixed the `0x8000`/`RecalcRotRpyRSca` billboard gap (RE-062) — 28 nodes,
archive-wide — and the PPSSPP black-screen blocker was root-caused and
resolved (stale `assets/generated/ssb64.pak`, not a `psp/` regression; see
git history for the resolved blocker's detail, since it now lives only in
that commit and this file's history, not in the current Blockers section).

R0.7 was left `IN_PROGRESS` but its remaining scope was formally measured
rather than chased further: RE-061 traced file 86's last graph's `itGetPData`
byte-offset-delta mechanism and found `romtool mobj --file 86 --search`
returns **27 candidate table offsets**, not one — the same kind of
near-chance fingerprint match the project already measured and rejected
once (Samus's two identical 33-node graphs). No named record exists to
resolve it, so it was deliberately left unfixed rather than guessed. That
graph, file 353's Spin Attack graph (still needs its `WPAttributes`
instance typed upstream), and the other 62 unpaired graphs archive-wide are
now an accepted long tail, not an active work item.

Earlier this session, R0.7 fixed 7 of the archive's previously-71 unpaired
graphs: file 353's `EntryWave`/`EntryBeam` (RE-058/RE-059, `EFDesc`) and all
5 of file 52's room-scene graphs (RE-060, plain call-sequence pairing, no
struct). `R0.3 — Texture Conversion Completeness` closed earlier this
session; both of its failure classes were root-caused and reattributed to
the tasks that actually own a fix:

* 26 segment-0x01 entries → RE-055 → confirmed as `sLBTransitionPhotoHeap`,
  a runtime framebuffer photocopy bound to RSP segment 1, never present in
  any ROM file (`refs/ssb-decomp-re/src/lb/lbtransition.c`) → R0.13's scope
* 4 (now 1) `MissingPalette` entries → RE-057 (via a temporary instrumented
  trace of `crates/ssb-rom/src/mesh.rs`, reverted, not committed) →
  confirmed as a `PartTables` material-pairing gap in 3 specific files, not
  a texture/TLUT decode bug → R0.7's scope (RE-056's dedup-key theory was a
  real secondary factor but not the root cause; RE-057 corrects it) → 2 of
  the 3 files now fixed (RE-059, RE-060); the third (file 86) is the
  RE-061 measured-and-accepted case above

RE-054's S2DEX-BG lead (from a prior session) is refuted:
`romtool scan --exhaustive` finds zero `G_BG_1CYC`/`G_BG_COPY` anywhere in
the ROM. See `PLAN.md` R0.3, R0.7, R0.8 and R0.13.

`R0.2 — N64 Rendering Command Inventory`, `R0.1 — Rendering State
Reconciliation` and `R0.9 — Stage Animation` are also `COMPLETE` — see
`PLAN.md` for each.

## Next Eligible Task

**`R0.10 — Material Animation` is `COMPLETE`** (RE-086 through RE-096;
full history in `PLAN.md`'s own R0.10 section and this file's Task Status
above — engine decoded and tested, pipeline wired end to end, all 33
known real scripts survive packing, `MaterialAnimator` ships and is
confirmed working on-device by interactive play, and the one remaining
checklist item resolved to "the two mechanisms are correctly separate,
nothing to unify"). `TraU`/`TraV`/`ScrU`/`ScrV` and `TextureIDCurrent`
(the next-largest stage material-animation categories after `PaletteID`)
remain real, measured, unimplemented follow-on work if a future session
wants it, but nothing blocks on them and `PaletteID` alone was 71% of the
real archive-wide need.

**`R0.11 — Fighter Palettes / Costumes` is `COMPLETE`** — RE-098 plus its
same-session closing addendum implemented and shipped multi-costume
packing and device-side selection, then individually screenshotted all
12 real fighters at a non-zero costume, closing every acceptance item.
Full detail: `PLAN.md` R0.11, `docs/reverse-engineering.md` RE-098.

**`R0.13 — Framebuffer Rendering` is the next eligible task and is now
precisely scoped (RE-099, this session), but still `TODO` — nothing has
been implemented.** It is considerably simpler than "framebuffer
rendering" suggests: `sLBTransitionPhotoHeap` is filled by a **one-time**
CPU-side copy of the framebuffer when a transition begins, not a
per-frame render pass, and every frame after that just binds it as RSP
segment `0x1` for an otherwise completely ordinary `DObjDesc`/
`AObjEvent32` scene graph — the same conversion pipeline this project
already has. Concrete next steps, in order:

1. **Add a PSP-side one-time framebuffer-to-texture snapshot.** Needs
   design (does the PSP GE's existing render target support a direct
   copy-to-texture, or does this need an explicit `sceGuCopyImage`/
   memcpy step?) and a place to trigger it — there is no real "match
   transition" game event yet, so this likely needs a debug-viewer
   trigger first, the same precedent `R0.10`/`R0.11` both used before a
   real game system existed.
2. **Recognise segment `0x1` in `mesh.rs`** the same way
   `mobj::GRAPHICS_HEAP_SEGMENT` (`0x0E`) is already special-cased,
   marking the resulting texture as "the live snapshot" instead of
   failing to resolve it. This is additive to the existing texture
   pipeline, not a rewrite.
3. **Pack and convert files 39–51** (13 files, not the 11 RE-055 first
   suggested — RE-099 corrected this by measuring `romtool textures
   --file <id>` directly against the ROM) the same way every other
   object is already converted, once step 2 makes their segment-`0x1`
   bind resolvable.
4. **Decide the strip-vs-full-image question RE-099 flagged but did not
   verify**: the N64 tiles this image into small strips purely because
   its RSP has a 4 KB TMEM limit; the PSP GE likely has no equivalent
   constraint, so one full-size capture may need no strip logic. Confirm
   before implementing strip-by-strip capture that isn't actually needed.

Full detail: `PLAN.md` R0.13 "Current evidence", `docs/
reverse-engineering.md` RE-099.

**Other tasks with real open items, in case `R0.13` turns out blocked
once investigated:**

* `R0.12 — Billboard Correctness` (`VERIFYING`): "scale/orientation/texture
  orientation verified" and "all flagged billboard nodes verified" remain,
  but its one substantive blocker (`translucent`) is the same already-
  exhausted `R0.5`/`R0.6` dither/coverage problem below.
* `R0.15 — Render-State Isolation` (`TODO`): genuinely not started, and
  does not depend on `R0.13` — a real alternative if framebuffer work
  stalls.

**Other, now-secondary threads, each already investigated as far as this
session's methods reach:**

1. **The dither/coverage problem (`R0.5`/`R0.6`'s `translucent`) is
   fully tried out for now.** RE-081 tested the last untried idea on
   this file's list ("a larger blur radius or multiple passes") and
   found a real, substantial texture-level improvement that still does
   not surface on screen at the tested camera distance — the same
   outcome RE-075 already found for a different change to these
   textures. RE-053's own remaining suggestion — deciding this on real
   hardware, or rendering the surface in isolation at a controlled scale
   — is now the most credible next step, and physical PSP access is
   `R2`'s territory, blocked behind `R1`. `translucent` itself remains
   the separate closed line RE-071 already established (do not re-run
   either of its eliminated experiments without new evidence).
2. **`R0.6`'s remaining items are blocked on `R0.7`, not on more
   combiner-shape work.** RE-079/RE-080 (this session) classified every
   combiner shape this model can resolve at all into three structurally
   disjoint cases (shade-scale, texture-blend, flat-colour) and fixed
   every misclassification found. What remains — `(PRIM-ENV)*TEXEL0+ENV`
   missing `prim_color`/`env_color` for 3,085/4,580 primitives — is a
   genuine absence on this converter's own per-graph state, most likely
   the same `R0.7` unpaired-`MObj`-graph gap already tracked there.
   `R0.7` itself was already characterized as a long tail after RE-078:
   further progress needs upstream decomp typing or a `--search` result
   narrowing to exactly one candidate, not open-ended investigation.
   Re-running `romtool mobj --search` archive-wide without new
   decompilation coverage would just re-find the same ambiguous/untyped
   set RE-078 already found and correctly left alone.

3. **`R0.14` has no more code-reading/lookup items left.** RE-082
   closed "viewport verified", "aspect ratio verified" and "N64/PSP
   resolution differences explicitly handled"; RE-084 closed "projection
   matrix verified" (the FOV); RE-085 (this session) closed "depth
   mapping verified" (matches the `psp` crate's own documented
   `sceGuDepthRange` convention exactly). 5 of 7 items are now checked.
   What is left — "camera transforms verified", "representative scenes
   compared" — cannot be verified at all until a real game camera
   exists; right now only the debug viewer's free-roaming inspection
   camera does, and per `PLAN.md` §5's gate, gameplay/camera systems are
   downstream of rendering correctness, not the other way round. This
   task is now genuinely blocked on that, not merely under-investigated.
4. **`R0.12`'s remaining items need either a decomp-derived expected
   value or a real camera to test against — the same shape as `R0.14`'s
   remainder.** RE-083 closed "billboard types enumerated", "camera-
   facing transforms verified" and "depth behavior verified", and
   narrowed "alpha behavior verified" to a single, already-tracked
   blocker (RE-069/RE-071's `translucent` checkerboard, now known to hit
   billboards at ~2x the archive-wide rate). What remains — "scale
   verified", "orientation verified", "texture orientation verified",
   "all flagged billboard nodes verified" — all need either a
   decomp-derived expected value per billboard type or a rotated/moved
   camera to test against beyond RE-049's own single Dream Land spot
   check, which the debug viewer's zoom/orbit controls can already do —
   this is device-interactive verification work, not a code-reading or
   archive-census task the way the last several sessions' progress was.

Every code-reading/archive-census-shaped lookup this file has been able
to name is now done. What is left on `R0.12`/`R0.14` needs either
device-interactive verification (moving the debug camera by hand, not
scripting a fixed screenshot) or a real game camera system that does not
exist yet — genuinely different *kinds* of work than the last several
sessions' pattern, not just a next item in the same list.
`RE-069`/`RE-071`'s `translucent` checkerboard (still unsolved, confirmed
by RE-083 to matter more for billboards specifically than previously
weighted) remains the one standing lead of the previous kind, unchanged
from before — worth a fresh pair of eyes on the alpha-channel-provenance
idea `TODO.md` Phase D already suggests, if a future session wants one
more attempt before treating it as needing real hardware too.

**Before trusting any on-device comparison**, delete
`psp/target/mipsel-sony-psp/release/EBOOT.PBP` (or otherwise confirm a
real rebuild happened) — RE-070 was fooled once by `--no-build` reusing a
stale binary from an earlier diagnostic. And don't stop at pixel
statistics either: RE-071 found a case where "less measured noise" was
paired with a visibly worse result (blown-out highlights) — look at the
actual image *and* the numbers, neither alone is sufficient.

`TODO.md` Phase G's "texture streaming" item is **no longer a clear-cut
"required" candidate** — RE-076 found that the 1170.9 KiB figure it was
justified by is an archive-wide total being compared against a per-scene
budget, and a direct measurement of one real scene's actual need
(largest stage + four largest fighters, deduped) came to 217.1 KiB, well
under budget. RE-077 then checked, rather than left standing, RE-076's
own worry that this was a large undercount: mostly it wasn't — 9 of the
11 fighters measured have zero unpaired `MObj` graphs, so their low
texture counts are real, not hidden. One real gap (Kirby's) was found
and fixed, without changing the VRAM total. The 217.1 KiB estimate
should now be trusted more, not less, than when RE-076 first produced
it. `R0.4`'s own remaining item ("all missing palette cases resolved")
is already fully attributed to `R0.7`'s file-86 long tail, so R0.4 has
no further independently-actionable work. `R0.7` remains technically
`IN_PROGRESS`, but the "worth a `--search` pass each" suggestion from
this note's own prior draft is now done: RE-078 ran `--search` over
every one of the (then) 63 remaining unpaired graphs archive-wide in one
pass, not file by file, found 13 with exactly one candidate, and fixed
the 6 that cross-checked cleanly against a typed decompiled symbol
(Kirby's, RE-077, was the first of what turned out to be 7 findable this
way; 57 remain, most of them landing on still-untyped bytes or genuinely
ambiguous candidates with no further discriminating evidence available
right now). Further progress on the remainder needs either upstream
decomp typing (file 86, file 353's Spin Attack, and most of the other
57) or a materially different lead — re-running `--search` again without
new decompilation coverage would just re-find the same ambiguous/
untyped set. `R0.8 — Transform Correctness` is `COMPLETE`.

## Blockers

None currently open. The PPSSPP black screen below is resolved.

### Resolved: PPSSPP black screen

**Root cause: a stale `assets/generated/ssb64.pak` on disk, not a source
regression.** The prior session's blocker note assumed the staged pack was
current because it opened cleanly host-side; that assumption was wrong and
is retracted here. `assets/generated/` is gitignored and was never
regenerated after some earlier (uncommitted/experimental) build — its
header's `prims` count (3412) doesn't match what *any* commit in history
actually produces (617-texture-era commits produce 3388; current `HEAD`
produces 3398), so it was left over from a WIP state, not from a specific
commit.

Bisection method: rather than bisecting `psp/` source (the last commit to
touch it before the report was `c252960`, seven docs/data-only commits
before `HEAD`), each candidate commit was built in a worktree and run
through `tools/run-ppsspp.sh` with its own **freshly regenerated** pack
(`romtool pack`), screenshotted, and pixel-checked. `c252960` built against
the *stale* staged pack reproduced the black screen exactly like `HEAD`
did; the same commit built against a pack regenerated from its own
`romtool` rendered Dream Land correctly. `HEAD` rebuilt against a freshly
regenerated pack also renders correctly. So no commit in git history is
responsible — every tested revision of `psp/`'s renderer works given data
that actually matches it. The mismatched pack most likely made the game
panic early in load/mesh-build (out of the visible render path), which
explains why not even the unconditional clear colour or debug overlay
reached the display — PPSSPP's own FPS counter overlay is emulator UI, not
game output, so it stayed visible regardless.

Fix: regenerated `assets/generated/ssb64.pak` via `romtool pack "rom/Super
Smash Bros. (USA).z64"` against `HEAD`. Verified via the full
`tools/run-ppsspp.sh` harness (build + run): Dream Land renders with
textures, fighters and the debug overlay, PPSSPP reports `FPS: 60.0`, and
the game's own overlay reports `cpu 2353us / budget 16667us` — comfortably
under the 60 Hz frame budget, confirming the game itself (not just the
emulator host) is running at a true 60 FPS.

Takeaway: `assets/generated/ssb64.pak` is derived, gitignored, build-only
data — it must be regenerated (`romtool pack`) after any `crates/ssb-rom`
change before trusting a screenshot, the same way a stale `EBOOT.PBP` would
be. `tools/run-ppsspp.sh`'s own "stale pack" staging message names the pack's
timestamp but does not compare it against source mtimes the way it already
does for the EBOOT; that gap is worth closing so this can't silently recur.

---

# 2. Milestone Status

| Milestone                             | Status          |
| ------------------------------------- | --------------- |
| M0 — Research                         | `COMPLETE`      |
| M1 — PSP Bootstrap                    | `COMPLETE`      |
| M2 — Resource Pipeline                | `COMPLETE`      |
| M3 — Core Game / Scene Infrastructure | `COMPLETE`      |
| R0 — Rendering Correctness            | `IN_PROGRESS`   |
| R1 — Rendering Completeness           | `BLOCKED_BY_R0` |
| R2 — Physical PSP Validation          | `BLOCKED_BY_R1` |
| R3 — Rendering Performance            | `BLOCKED_BY_R2` |
| G0 — Combat Unlocked                  | `BLOCKED_BY_R3` |

---

# 3. Rendering Gate

Rendering is currently the highest-priority development area.

Combat is intentionally blocked.

Do not begin attacks, hitboxes, damage, knockback, KO, stocks, CPU combat or match gameplay until the rendering milestones explicitly unlock combat.

Movement, physics, collision and animation may continue when required for rendering validation.

---

# 4. Verified Foundation

The following foundation work has been completed:

* ROM validation
* VPK0 decompression
* relocData archive handling
* asset extraction
* runtime asset-pack generation
* F3DEX2 display-list parsing
* N64 texture decoding
* mesh conversion
* scene graph conversion
* fighter animation extraction
* stage animation extraction
* collision extraction
* PSP asset loading
* PSP mesh drawing
* core engine traits
* fixed timestep
* fighter movement infrastructure
* stage collision infrastructure

Detailed subsystem status belongs in:

`docs/porting-status.md`

---

# 5. Known Rendering Work

Known renderer work includes investigation/fixes for:

* texture conversion completeness
* CI4/CI8 palette/TLUT behavior
* texture filtering
* LOD
* mipmapping
* texture tile addressing
* wrap/clamp/mirror behavior
* Dream Land canopy rendering
* material tables
* combiner behavior
* lighting
* alpha/blending
* render state
* unresolved MObj behavior
* transform kind `0x8000`
* stage animation
* material animation
* fighter palettes/costumes
* billboard behavior
* framebuffer effects
* camera/projection behavior
* render-state leakage
* physical PSP compatibility
* VRAM behavior
* rendering performance

The exact ordered task list is in `PLAN.md`.

---

# 6. Current Task State

## R0.8 — Transform Correctness

Status: `COMPLETE` (this session)

### Objective

Implement every transform kind exercised by SSB64.

### Required Work

* [x] Investigate `0x8000`/`RecalcRotRpyRSca` — read `gcPrepDObjMatrix` case 44 (`objdisplay.c:822`): never touches `dobj->rotate`, computes the same diagonal-from-`gGCMatrixPerspF` MVP as billboard kinds 45/46 with the spin term dropped (RE-062)
* [x] Confirm dropping `rotate` is safe, not just convenient — whole-archive check: 0 of 28 `RecalcRotRpyRSca` nodes have non-zero `rotate` (RE-062)
* [x] Fix — `crates/ssb-rom/src/pack.rs`'s `add_object` now flags `TransformKind::RecalcRotRpyRSca` as `NodeDesc::FLAG_BILLBOARD`, same as `Kind46`/`Kind48`; no `psp/` changes needed since the render path is already generic over the flag
* [x] Enumerate the remaining transform kinds — RE-063: `gcSetupCommonDObjs` is the only function that turns a ROM `DObjDesc` array into `XObj`s, and it only ever emits kinds 44/46/48/50; everything else (33-40's `func_800108xx` family, 41-43/45/47/49) is real matrix math reached only by direct runtime calls from fighter/item/effect/stage-decoration game code, never from a `DObjDesc` array — confirmed out of scope, not a gap
* [x] Kind 50 — real and reachable (`0x1000`), but 0 of 3117 nodes archive-wide use it (RE-063); flagged `FLAG_BILLBOARD` anyway (same layout as `Kind48`, sourced from `sGCMatrixMod2F` instead of `sGCMatrixMod1F`) for fidelity with the decomp's case structure
* [x] Verify on device — `tools/run-ppsspp.sh --no-build --seconds 8` after regenerating the pack: Dream Land renders correctly at 60 FPS, clean log, no regression (expected, since Kind50 contributes 0 real nodes)

### Completion Evidence

* which transform kinds were traced and what the decomp's actual matrix math does for each — RE-062 (`0x8000`), RE-063 (every other case, including 33-40 and 50)
* the fix implemented, with its measured effect — `pack.rs` flags `Kind50` alongside the other three; `cargo test --workspace` 340 passing (new test `a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`)
* before/after device verification — `cargo psp --release` builds clean; `tools/run-ppsspp.sh --no-build --seconds 8` runs at 60 FPS with a clean log and a correct Dream Land screenshot
* regression test added — yes, `crates/ssb-rom/src/pack.rs::a_kind_50_node_is_flagged_as_a_billboard_like_kind_48` (plus the pre-existing RecalcRotRpyRSca test)

### R0.7 — Missing Material Tables (parked, accepted long tail)

Status: `IN_PROGRESS`, not actively worked this pass — see Last Completed
Task and RE-061. The two remaining concrete cases (file 86's last graph,
file 353's Spin Attack graph) and the other 62 unpaired graphs archive-wide
are accepted as a long tail; further progress needs upstream decomp typing,
not more `romtool` investigation.

---

# 7. Last Verification

## 2026-09-03 — R0.6: fog verified as correctly unimplemented (RE-072)

* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_fog_scan.rs`, deleted before commit) using `scan::Candidates::Exhaustive`: found `G_SETFOGCOLOR` 7 times and `G_FOG` geometry-mode sets 4 times archive-wide, apparently contradicting `DECISIONS.md` D-025's existing "twice" figure
* Cross-checked with reliable, reloc-anchored discovery (`find_root_display_lists`) before treating that as a correction: only 2 `SetFogColor` occurrences survive, 0 `G_FOG` hits — the `Exhaustive`-mode extras were false positives, a known risk of that scan mode; D-025's original number was right all along
* Grepped the entire decompilation for `gSPFogPosition` (the call that gives the RSP a fog range to compute against) — zero results anywhere, confirming no game code ever configures fog range/scale
* Identified the two surviving occurrences: file 63 (`63_MVOpeningRoomTransition`, the opening movie) and file 118 (`118_StageYosterSmallFile2`); confirmed via `romtool stages` that file 118 is one of the 41 currently-loaded, real stages, not a discarded variant
* Checked file 118's own list (offset `0x3310`) for whether its two `G_SETRENDERMODE` calls ever reference `G_BL_CLR_FOG` — both use `G_BL_CLR_MEM` instead; the fog colour is set and never read by anything in the same list
* Result: `DECISIONS.md` D-025 stands, now verified against the supporting machinery (fog range, blend-equation reference) rather than just an occurrence count; RE-072 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6's "fog verified" item checked; `docs/rendering.md` cross-referenced
* Affected subsystem: documentation/investigation only, no code changed — the correct conclusion was "already correctly unimplemented," not "needs implementing"
* `cargo test --workspace` — 354 passing, unchanged
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* PPSSPP: not run this pass (no production code changed)
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.6: RE-070's dither fix does not make blend safe, two leads ruled out (RE-071)

* Re-enabled `GuState::Blend` from `flags::TRANSLUCENT` (the exact code RE-069 deferred) as a reversible experiment, rebuilding clean from a deleted `EBOOT.PBP` first (per RE-070's own stale-binary lesson)
* Re-tested against Dream Land's canopy-highlight texture now that RE-070 pre-blurred it: objective pixel statistics showed ~4% less local noise, but the actual image was worse, not better — blown-out, oversaturated highlights erasing the flowers and most other detail, a different failure mode from RE-069's original checkerboard
* Hypothesized unpremultiplied-alpha blurring as the cause (`box_blur_wrapped` averages RGB/alpha independently, a known way to leak "invisible" colours from transparent texels); implemented a premultiplied variant as a temporary experiment and reran — identical blown-out result, ruling this out too
* Both experiments (blend-enable, premultiplied blur) fully reverted before commit; `git status` clean of them
* Updated `psp/src/meshdraw.rs`'s permanent comment on the deferred `TRANSLUCENT` flag to record both ruled-out hypotheses, so a future session doesn't re-run either
* `cargo test --workspace` — 354 passing, unchanged (no production code shipped this pass, only a comment update and documentation)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `cargo psp --release` — builds clean (comment-only change)
* Result: RE-071 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6's "blending verified" item updated with the narrowed-but-still-open status; `TODO.md` Phase D updated to match
* Affected subsystem: `psp/src/meshdraw.rs` (comment only) — plus documentation; no functional/runtime change
* PPSSPP: tested extensively this pass (two reversible experiments, both reverted), no regression in the shipped (unchanged) behavior
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.5: canopy dither measurably softened, not fully fixed (RE-070)

* Ran a reversible on-device A/B (`sceGuTexFilter(Nearest, Nearest)` vs the existing `Linear`/`LinearMipmapLinear`, screenshotted, reverted): filtering alone measurably helps a little (Nearest is visibly blockier) but does not turn the dither into a smooth gradient
* Tested conversion-time dither resolution in two steps: box-blurred (3x3, wrapped) the two canopy textures (file 103, offsets `0xE20`/`0x5F0`) and requantized back to their 16-entry CI4 palette — no visible change (blur mostly snaps back to the same two entries); packed the same blurred image unquantized (`Psm8888`) instead — this is where the real effect is
* First on-device look at the unquantized-blur result appeared to fully fix the canopy (a smooth patch replacing the checkerboard); caught this as a methodology error before trusting it — `--no-build` had reused a stale `EBOOT.PBP` left over from the filtering A/B diagnostic, not the actual candidate build. Deleted the binary, rebuilt from scratch (confirmed via `git diff` showing zero change to `meshdraw.rs`), and re-measured
* Measured objectively this time (mean absolute difference between adjacent pixels, a dither-noise proxy) rather than judging a screenshot by eye: ~40% less local noise on a clean canopy patch (8.5→5.1), ~19% over the whole visible canopy region (9.4→7.6, diluted by untouched flowers/background) — real, but a partial improvement, not a full fix
* Implemented `crates/ssb-rom/src/texture.rs::box_blur_wrapped` (3 unit tests: flat image is a no-op, a properly-sized tiling checkerboard averages to the true midpoint, wrapping reaches across the border) and `tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR` — a short, explicit, named allowlist of exactly the two Dream Land canopy textures, deliberately not a general "detect dithering" heuristic
* `cargo test --workspace` — 354 passing (was 351)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4250.0 KiB (was 4138.2), packed texture VRAM 1170.9 KiB (was 1059.0, +112 KiB for the two named textures)
* `cargo psp --release` — builds clean, rebuilt from a deleted `EBOOT.PBP` this time, not trusted from cache
* `tools/run-ppsspp.sh` — Dream Land renders at 60 FPS, clean log
* Result: RE-070 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.5 (still unchecked, now with measured evidence), `TODO.md` Phase C, `docs/rendering.md`, `docs/porting-status.md`, `docs/memory.md`, `DECISIONS.md` D-003 updated with the new VRAM figure
* Affected subsystem: `crates/ssb-rom/src/texture.rs` (new function), `tools/romtool/src/main.rs` (`convert_texture`) — code change scoped to exactly two named textures
* PPSSPP: tested this pass extensively, including catching and correcting a stale-build methodology mistake
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.6: alpha test shipped, translucency detected but deferred (RE-069)

* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_rendermode_scan.rs`, deleted before commit) decoding `G_SETOTHERMODE_L`'s alpha-compare and render-mode fields archive-wide: alpha compare nearly even (278 `G_AC_NONE` vs 269 `G_AC_THRESHOLD`); 12 distinct render-mode values across 360 non-default `G_SETRENDERMODE` commands
* Initially checked `FORCE_BL` as the "needs blending" signal — wrong: `G_RM_OPA_SURF` (the RDP reset's own opaque default) sets it too, with an equation that evaluates to no real blending; found the correct signal (blend equation reads the framebuffer weighted by `1 - alpha`) by reading `gbi.h`'s `GBL_c1`/`GBL_c2` macros directly and cross-checking against `refs/BattleShip`'s interpreter, which tests the identical bit positions
* Measured with the corrected signal: 130/360 (36.1%) cutout (`TEX_EDGE`-family) surfaces, 52/360 (14.4%) genuinely translucent
* Consulted `refs/sf64-psp` (a real, shipped N64-to-PSP port doing this same RDP-to-GE translation at runtime) for the actual approximation to use: `sceGuAlphaFunc(GU_GREATER, 0, 0xFF)` for cutouts, standard `sceGuBlendFunc(GU_ADD, GU_SRC_ALPHA, GU_ONE_MINUS_SRC_ALPHA)` for translucency
* Implemented `crates/ssb-rom/src/mesh.rs`'s `alpha_test`/`translucent` decode (`render_mode_is_translucent` mirrors the GBI macros' bit layout directly), `pack.rs`'s `flags::ALPHA_TEST`/`TRANSLUCENT` (pack version 8→9), and `psp/src/meshdraw.rs`'s `sceGuAlphaFunc`/`sceGuBlendFunc` wiring
* Found a real bug via on-device testing before shipping: 46/380 `alpha_test` and 7/362 `translucent` primitives had no texture bound, so they were testing/blending against a packed-normal byte (lit vertices' alpha is not a coverage value — `push_vertex`'s own doc comment) instead of real coverage; this visibly deleted Dream Land's decorative flower triangles. Reproduced the bug, fixed it (`material_now()` now gates both flags on `texture.is_some()`), and confirmed the flowers return with a targeted before/after diff
* Found a second bug specifically in `translucent`, isolated by toggling `AlphaTest`/`Blend` independently: enabling real blending on Dream Land's canopy-highlight surface (file 104 lists at `0x708`/`0x820`/`0xA78`, texture at file 103 `+0x5F0`) produced a checkerboard, not a soft highlight — the render mode itself is genuinely translucent per the decomp (confirmed bit-for-bit against `GBL_c1(CLR_IN, A_IN, CLR_MEM, G_BL_1MA)`), so this is not a detection bug but the same open dithered-CI4/coverage problem RE-053 already found for the canopy's opaque path
* Decision: shipped `alpha_test` (clean on device, matches a validated reference); left `translucent` detected and packed but **not** wired to `GuState::Blend` in `meshdraw.rs`, with a comment explaining why, rather than shipping an unverified visual change to the project's primary test scene
* `cargo test --workspace` — 351 passing (was 347)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4138.2 KiB (was 4138.1), pack version 9
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh` — Dream Land renders at 60 FPS, clean log, pixel-identical to the pre-alpha-test baseline in the canopy region
* Result: RE-069 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6 ("alpha behavior verified" checked, "blending verified" stays open with a concrete pointer), `TODO.md` Phase D, `docs/porting-status.md` updated to match
* Affected subsystem: `crates/ssb-rom/src/mesh.rs`, `crates/ssb-rom/src/pack.rs`, `psp/src/meshdraw.rs` — code change, affects every object with a non-default render mode, not one file
* PPSSPP: tested this pass extensively (including deliberate bug reproduction), no regression in the shipped feature
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.6: the archive-wide geometry-mode default was backwards (RE-068)

* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_zbuffer_scan.rs`, deleted before commit) reading the built pack's `PrimDesc::flags` archive-wide: `Z_BUFFER` set on only 6 of 3426 packed primitives (0.17%), most other flags similarly near-zero
* Read `refs/ssb-decomp-re/src/sys/rdp.c`'s `sSYRdpResetDisplayList` — clears every geometry mode bit then sets `G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH`; `syRdpResetSettings` plays it, called from `taskman.c:308` (the per-frame graphics task scheduler) — confirmed this is a once-per-frame default, not per-object or per-list
* Recognized the same structural shape as RE-021's lighting finding, one level up: a per-list converter can't see state some other code set earlier (there, per-object; here, per-frame)
* Added `MeshMaterial::rdp_default()` (`cull_back: true, smooth: true, z_buffer: true`) and made `crates/ssb-rom/src/mesh.rs`'s `State::new()` seed from it instead of an all-off `Default`
* Wired `psp/src/meshdraw.rs`'s `apply_material` to toggle `GuState::DepthTest` per primitive from the `Z_BUFFER` flag (already packed by `pack.rs`, never read on the device side until now), mirroring the existing `CullFace` toggle
* Re-ran the archive-wide scan after the fix: `Z_BUFFER` 6→3384 of 3442 (98.3%), `CULL_BACK` 86.3%, `CULL_FRONT` 0.1%, `SMOOTH` 76.5% — the shape a real game's geometry should have
* Added `crates/ssb-rom/src/mesh.rs::a_list_with_no_geometry_mode_command_draws_under_the_rdp_reset_default`, pinning all four defaults (and that `cull_front`/`lit` stay off) from a display list with no geometry-mode command at all
* `cargo test --workspace` — 347 passing (was 346); none of the 346 pre-existing tests broke, since every existing geometry-mode test already set an explicit command
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4138.1 KiB (was 4137.6)
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders correctly at 60 FPS, clean log; before/after pixel diff shows a small, localized change (2199 of 522240 pixels, ~0.4%) around thin/double-sided decorations, not a wholesale change or missing geometry
* Result: RE-068 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6 ("depth state verified", "culling verified" checked, with leads recorded for alpha/blending/fog defaults from the same reset list), `TODO.md` Phase D, `docs/porting-status.md` updated to match
* Affected subsystem: `crates/ssb-rom/src/mesh.rs` (`State::new()`/`MeshMaterial::rdp_default()`), `psp/src/meshdraw.rs` (`apply_material`) — code change, affects every object this project converts, not one file
* PPSSPP: tested this pass, no crash/regression, small localized visual change as expected
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.5: `G_TX_MIRROR` traced to Dream Land's canopy and fixed (RE-067)

* Ran `romtool textures --file 104` — reproduced RE-053's exact canopy binding (`64x64 Ci/Bits4 <- file 103 +0xE20`, `3.70x1.36 repeats`)
* Wrote a temporary probe (`crates/ssb-rom/examples/tmp_canopy_tile.rs`, deleted before commit) decoding file 104's display lists directly and found the one binding it (offset `0x798`): `SetTile cm_s=3 cm_t=3 mask_s=6 mask_t=6` — mirror+clamp on both axes, exactly matching the texture's 64-texel period
* Confirmed the wrap boundary mattered before implementing anything: temporarily changed `psp/src/meshdraw.rs`'s `sceGuTexWrap(Repeat, Repeat)` to `Clamp, Clamp` (2-line, reversible), rebuilt, screenshotted — the canopy's repeating pattern disappeared entirely under `Clamp`, a dramatic difference proving the boundary drives real visible output; reverted immediately after
* Implemented the real fix: `crates/ssb-rom/src/texture.rs::mirror_extend` pre-bakes a mirrored copy of the decoded image per mirrored axis (4 unit tests: no-op, S-only, T-only, both-axes-all-four-quadrants); `crates/ssb-rom/src/mesh.rs` gained `TextureRef::mirror_s`/`mirror_t` gated on a nonzero mask (1 unit test reproducing Dream Land's exact `SetTile` parameters); `tools/romtool/src/main.rs`'s `convert_texture` applies it before mip generation
* Measured the real scope and cost (temporarily instrumented `romtool textures`' existing dedup loop, reverted before commit): **187 of 638 packed textures (29%)** carry `G_TX_MIRROR` on at least one axis, not just Dream Land's; packed texture VRAM rose from 763.2 KiB to **1059.0 KiB (+39%, 1.5x the ~700 KiB budget)**
* Given the scale of that tradeoff, stopped and presented the finding + four options to the user (ship fully / paletted-formats-only / document-only / revert) rather than deciding unilaterally — user chose to ship it fully
* `cargo test --workspace` — 346 passing (was 341)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack` — 4137.6 KiB (was 3841.0 KiB), 218 of 637 textures carry mip levels
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders at 60 FPS, clean log; before/after pixel diff confirms substantial real change in the canopy region (not a no-op), though the dithered pattern still looks busy at native resolution — expected, since RE-053's separate magnification/dithering diagnosis is untouched by this fix
* Result: RE-067 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.5, `DECISIONS.md` D-003, `TODO.md` Phase C/G, `docs/rendering.md`, `docs/porting-status.md`, `docs/memory.md` updated to match. `PLAN.md` R0.5's "Dream Land canopy discrepancy resolved" item stays unchecked — one real contributing cause fixed, the magnification/dithering component is not
* Affected subsystem: `crates/ssb-rom/src/texture.rs`, `crates/ssb-rom/src/mesh.rs`, `tools/romtool/src/main.rs` — code change, plus a significant VRAM-budget documentation update
* PPSSPP: tested this pass, no crash/regression, visually different as expected
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.5: wrap/clamp verified correct, Mirror recorded as a deviation (RE-066)

* Wrote a temporary example (`crates/ssb-rom/examples/tmp_wrap_mode_scan.rs`, deleted before commit) decoding every display list archive-wide and tallying `cms`/`cmt` on tile 0 (the render tile `current_texture()` reads): 754 `G_SETTILE` commands found, 0 using plain `wrap`/`wrap`
* Cross-tabulated per-axis against that axis's own `masks`/`maskt`: every single instance where an axis requests clamp or mirror also has that axis's own mask nonzero — 0/754 counterexamples on both axes independently
* Read `refs/BattleShip/libultraship/src/fast/interpreter.cpp:3245-3251` (strips `G_TX_CLAMP` when the tile is genuinely periodic) and `:3952-3956` (forces `Clamp` for an unmasked `WRAP` tile) — confirmed real RDP tile addressing only wraps/clamps meaningfully in combination with the mask, not from the two-bit field alone; a naive `WRAP`→repeat/`CLAMP`→clamp mapping is wrong on real hardware too, not just the PSP
* Concluded `crates/ssb-rom/src/mesh.rs`'s existing mask-narrowed texture sizing (RE-044) combined with `psp/src/meshdraw.rs`'s hardcoded `sceGuTexWrap(Repeat, Repeat)` already reproduces the correct periodic addressing for every tile-0 texture in this ROM — not a bug, no code change needed
* Quantified the one real gap: `G_TX_MIRROR` on 208/754 (27.6%) tile-0 lists, which `sys::GuTexWrapMode` (`Repeat`/`Clamp` only) cannot represent at all
* Corrected `meshdraw.rs`'s comment (previously justified `Repeat` only by "UVs run outside 0..1", true but incomplete) to explain the actual mechanism and cite the measurement
* `cargo test --workspace` — 341 passing, unchanged (no functional code changed)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `cargo psp --release` — builds clean (comment-only change to `meshdraw.rs`)
* Result: RE-066 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.5 ("wrap/clamp/mirror behavior verified", "texture tile parameters verified" checked), `TODO.md` Phase C, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: `psp/src/meshdraw.rs` (comment only) — plus documentation; no functional/runtime change
* PPSSPP: not run this pass (no production behavior changed)
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.6: baked key light direction measured from real stage data (RE-065)

* Read `ftdisplaymain.c:1240` → `mpcollision.c:4008-9` → `mptypes.h:187` — the fighter draw path's key light direction ultimately comes from `MPGroundData.light_angle`, a per-stage `Vec3f` field, converted via `ftDisplayLightsDrawReflect`'s spherical-to-Cartesian math
* Computed `light_angle`'s byte offset (`0x60`) from the struct's field order (`unused` at `0x5C` + 4), independently corroborated: `0x60 + sizeof(Vec3f) = 0x6C` lands exactly on `camera_bound_top`'s already-verified offset
* Added `light_angle: [f32; 2]` to `crates/ssb-rom/src/stage.rs::GroundData`, read at the computed offset; added a fixture assertion (`reads_a_stage_header_and_its_layers`) pinning the read
* Wrote a temporary example (`crates/ssb-rom/examples/tmp_light_angle_scan.rs`, deleted before commit) reading all 41 stages' real angle and comparing against the old placeholder direction: **33 of 41 (80%) share exactly `(20.0, 45.0)` degrees**, 9.9 degrees from the old `(2, 4, 3)` guess; the other 8 (Brinstar, Sector Z, Hyrule, Final Destination, Metal Mario's stage, a jungle stage, a "Zako" stage, a bonus stage) diverge up to 111 degrees
* Implemented: `crates/ssb-rom/src/pack.rs`'s `LIGHT_DIR` now holds the real `(20, 45)`-degree direction (`[0.2419, 0.7071, 0.6645]`) instead of the arbitrary placeholder; documented the remaining 8-stage gap as an explicit accepted deviation (full fix needs runtime `sceGuLight` lighting, out of scope)
* `cargo test --workspace` — 341 passing, unchanged count (no test pinned the old constant's exact value)
* `cargo clippy --release -p romtool -p ssb-rom` — clean (one `#[allow(clippy::approx_constant)]`, since `sin(45°)` legitimately coincides with `1/sqrt(2)`)
* `romtool pack` — regenerated `assets/generated/ssb64.pak`
* `cargo psp --release` — builds clean
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders correctly at 60 FPS, clean log, subtly (and now more correctly) different shading
* Result: RE-065 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.6, `DECISIONS.md` D-024, `docs/porting-status.md` "Mesh conversion", `TODO.md` Phase D updated
* Affected subsystem: `crates/ssb-rom/src/stage.rs` (new field), `crates/ssb-rom/src/pack.rs` (`LIGHT_DIR`) — code change, plus documentation
* PPSSPP: tested this pass, no regression
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.4: palette/texture state inheritance pinned by a test (RE-064)

* Read `mesh.rs`'s `convert_sequence` (`mesh.rs:743`) — `State`'s texture/tile/palette fields are declared once and mutated across the node-sequence loop, reset only by an explicit state command or `forget_texture()`; a node with no material commands keeps whatever the previous node left, by construction
* Read `tools/romtool/src/main.rs`'s pack-building loop (`main.rs:952`) — `convert_sequence` is called fresh, with a new `State::new()`, once per scene graph; confirmed no code path reuses a `State` across two different objects, so cross-object leakage cannot happen architecturally
* Added `crates/ssb-rom/src/mesh.rs::a_texture_binding_persists_into_a_node_that_sets_no_new_state` — joint A fully binds a CI4 texture+palette, joint B sets no material state at all, asserts joint B's `TextureRef` equals joint A's exactly
* Verified the test can fail: temporarily reset `timg_addr`/`palette_offset`/`texture_enabled` per sequence item, reran, watched it fail with the expected panic, reverted the injected bug, confirmed the suite is clean again
* `cargo test --workspace` — 341 passing (was 340)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* No production code changed — the inheritance mechanism was already correct, only untested; `romtool pack`/PPSSPP verification skipped as disproportionate to a test-only diff (no build/runtime artifact could differ)
* Result: RE-064 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.4's "palette inheritance/state verified" item checked; `docs/porting-status.md` "Mesh conversion" row updated
* Affected subsystem: `crates/ssb-rom/src/mesh.rs` (test-only) — plus documentation
* PPSSPP: not run this pass (no production change to verify)
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.8 closed: transform kinds enumerated, `Kind50` fixed (RE-063)

* Read `gcSetupCommonDObjs` (`refs/ssb-decomp-re/src/sys/objanim.c:2153`) — the only function that turns a ROM `DObjDesc` array into `XObj`s; it tests exactly four high-nibble bits (`0x8000`/44, `0x4000`/46, `0x2000`/48, `0x1000`/50), matching `TransformKind` exactly
* Grepped the whole decompilation for `gcAddXObjForDObjFixed`/`gcAddXObjForDObjVar` call sites — dozens across `ef/`, `it/`, `gr/`, `mv/`, passing kinds like 28/40/42/44/46/70/72 as game-code-driven runtime XObj attachments, never derived from a `DObjDesc` array; confirmed kinds 33-40 (`func_800108xx` per-object look-at family) and 41/43/45/47/49 are unreachable from this project's ROM importer and out of scope until the calling gameplay systems exist
* Read `func_80010748`/`func_80010918`/`func_80010AE8`/`func_80010C2C` (`objdisplay.c:110-319`, kinds 33-40's underlying functions) — genuine per-object look-at billboards computed from object-to-camera distance vectors, distinct in technique from kinds 44-50's shared per-frame camera-basis approach
* Read case 50 (`objdisplay.c:1050`) and the `sGCMatrixMod1F`/`sGCMatrixMod2F` per-frame setup (`objdisplay.c:3033-3066`) — case 50 is `Kind48`'s exact move-word layout, sourced from `sGCMatrixMod2F` (camera-yaw-locked) instead of `sGCMatrixMod1F` (camera-pitch-locked); a genuinely different basis, not a duplicate
* Wrote a temporary example (`crates/ssb-rom/examples/tmp_kind50_scan.rs`, deleted before commit) walking every scene graph in the ROM: **0 of 3117** nodes archive-wide carry the `0x1000` bit (cross-checked 34/`0x4000`, 47/`0x2000`, 28/`0x8000` against RE-062's prior counts)
* Implemented: `crates/ssb-rom/src/pack.rs`'s `add_object` now maps `TransformKind::Kind50` to `NodeDesc::FLAG_BILLBOARD` in the same match arm as `Kind46`/`Kind48`/`RecalcRotRpyRSca`; recorded as fidelity with the decomp's case structure, not a measured fix, since no shipped node exercises it
* Added `crates/ssb-rom/src/pack.rs::a_kind_50_node_is_flagged_as_a_billboard_like_kind_48`, asserting a `0x1001`-id node reaches the pack flagged; also fixed a pre-existing `RE-061`→`RE-062` mislabel in an adjacent comment while editing the same block
* `cargo test --workspace` — 340 passing (was 339)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `romtool pack "rom/Super Smash Bros. (USA).z64"` — regenerated `assets/generated/ssb64.pak`; still 109 billboard nodes (Kind50 contributes 0, as expected)
* `cargo psp --release` (from `psp/`) — builds clean, `EBOOT.PBP` produced
* `tools/run-ppsspp.sh --no-build --seconds 8` — Dream Land renders correctly: textures, fighter, canopy billboards all present, `FPS: 60.0`, overlay reports `cpu 2353us / budget 16667us`, log clean, no regression (expected — no real node exercises the new flag path)
* Result: RE-063 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.8 marked `COMPLETE`; `docs/porting-status.md` "Billboard nodes" row updated to 97%
* Affected subsystem: `crates/ssb-rom/src/pack.rs` (`add_object`), `crates/ssb-rom/src/scene.rs` (doc comment) — code change, plus documentation
* PPSSPP: tested this pass, no regression
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — PPSSPP black-screen investigation (root-caused and resolved in a later pass, see §1 "Resolved: PPSSPP black screen")

* Copied the current build (`a31b081`) into the user's real PPSSPP install (`~/.var/app/org.ppsspp.PPSSPP/config/ppsspp/PSP/GAME/SSB64PSP/`) at the user's request; user reported a black screen
* Reproduced the same result through `tools/run-ppsspp.sh` itself (both `--seconds 8` and `--seconds 20`), ruling out the manual copy as the cause
* Manually launched PPSSPP outside the harness to inspect its window/log directly: window title correct ("Super Smash Bros. 64", not stuck at "Initializing Vulkan..."), traced PC values in the emulator log landing inside the loaded ELF's own address range and cycling at 60 Hz — the CPU is genuinely running the program, not stuck
* Pixel-sampled captures with Python/Pillow: pure `(0,0,0)` everywhere except PPSSPP's own FPS counter overlay text — not the frame's clear colour, not the unconditional per-frame debug text overlay (`gpu.debug_text`, `main.rs:862`)
* Confirmed host-side that the exact staged `ssb64.pak` opens cleanly via `Pack::open` (2450 meshes, 363 objects, 41 stages, 567 anims, 617 textures) — not a data/version problem
* Confirmed `SoftwareRenderer = True` was genuinely active in the live `ppsspp.ini` during the manual test, so this is not RE-014's known hardware-backend/`sceGuDebugFlush` visibility gap recurring
* Not root-caused. `psp/src` was not touched this session (only `crates/ssb-rom/src/pack.rs`), so this is not a regression from the R0.8 fix — it predates this session
* Restored the user's `ppsspp.ini` (`SoftwareRenderer` back to its prior value) and killed all PPSSPP processes before finishing
* Result: recorded as a blocker in §1 above; deferred to a dedicated session per the user's request, no code changed
* Affected subsystem: `psp/` runtime and/or PPSSPP interaction — unknown which yet
* PPSSPP: tested extensively this pass, unresolved
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.8: `0x8000`/`RecalcRotRpyRSca` fixed as a spin-free billboard (RE-062)

* Read `refs/ssb-decomp-re/src/sys/objdisplay.c`'s `gcPrepDObjMatrix` (the switch on `xobj->kind` that builds every DObj's MVP) — case 44 is `nGCMatrixKindRecalcRotRpyRSca` (`0x8000`); it computes `sGCMatrixMvpF` as a scaled diagonal from `gGCMatrixPerspF` and never reads `dobj->rotate` at all, then patches it into the RSP matrix via `gSPMvpRecalc`/`gMoveWd(G_MW_MATRIX,...)` — the same shape as the already-implemented billboard cases 45/46, just without the `sin`/`cos` spin term
* Wrote a temporary example (`crates/ssb-rom/examples/tmp_recalc_rotate.rs`, deleted before commit) walking every scene graph in the ROM: **0 of 28** `TransformKind::RecalcRotRpyRSca` nodes have non-zero `rotate` — confirms the field really is unused for this kind, not coincidentally zero
* Cross-checked the node counts: `Kind46` 34 + `Kind48` 47 + `RecalcRotRpyRSca` 28 = 109, matching `docs/porting-status.md`'s prior "81 billboard nodes flagged, 28 not" split exactly
* Implemented: `crates/ssb-rom/src/pack.rs`'s `add_object` now maps `TransformKind::RecalcRotRpyRSca` to `NodeDesc::FLAG_BILLBOARD` in the same match arm as `Kind46`/`Kind48`; no changes to `psp/src/meshdraw.rs` — its billboard path is already generic over the flag
* Added `crates/ssb-rom/src/pack.rs::a_recalc_node_is_flagged_as_a_spin_free_billboard`, asserting a `0x8001`-id node reaches the pack flagged
* `cargo test --workspace` — 339 passing (was 338)
* `cargo clippy --release -p romtool -p ssb-rom` — clean
* `cargo psp --release` (from `psp/`) — builds clean, `EBOOT.PBP` produced
* `tools/run-ppsspp.sh --seconds 8` — launched, ran the full 8s at 60 FPS, log clean (no errors/crashes); captured screenshot was black (idle boot-time frame at this point in the run, consistent with other short captures — not itself evidence either way). Did not isolate a specific `0x8000` object on screen this pass
* Result: RE-062 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.8, `TODO.md` Phase E, `docs/porting-status.md` "Billboard nodes" updated to match
* Affected subsystem: `crates/ssb-rom/src/pack.rs` (`add_object`) — code change, plus documentation
* PPSSPP: smoke-tested this pass (build + 8s run, no crash), not visually isolated
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.7: file 86's last graph measured and left open, not fixed (RE-061)

* Ran `romtool mobj --file 86` — confirmed the one remaining unpaired graph is at `0x7BE8`, the "N-Bumper" item's attached-pose `DObjDesc` (`refs/ssb-decomp-re/src/relocData/86_ITCommonObject.c:1812`)
* Traced its pairing mechanism to `itNBumperAttachedInitVars` (`refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`): `itGetPData(ip, &llITCommonDataNBumperDataStart, &llITCommonDataNBumperWaitMObjSub)` — a compile-time byte-offset delta from a runtime pointer. Confirmed neither `llITCommonDataNBumperDataStart` nor `llITCommonDataNBumperWaitMObjSub` is declared anywhere else in the decompilation (both are still-unmatched linker symbols) — no named record exists to read
* Ran `romtool mobj --file 86 --search` (the project's own demand-vector search diagnostic) — returns **27 candidate table offsets** for this one graph, not one; confirmed this is the same near-chance fingerprint-match situation the project already measured and rejected once (Samus's two identical 33-node graphs, documented in `mobj.rs`'s own doc comment)
* Decision: left unfixed. No code change. Recorded as a measured negative result rather than continuing to search
* Result: RE-061 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.7, `TODO.md` Phase E updated to mark file 86's graph, file 353's Spin Attack graph, and the archive's other 62 unpaired graphs as an accepted long tail
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.7: file 52 fully resolved via a fourth pairing mechanism (RE-060)

* Ran `romtool mobj --file 52` — confirmed 5 unpaired graphs (0x7E98, 0x1C4A8, 0x1DF28, 0x1F270, 0x22440)
* Identified file 52 as `refs/ssb-decomp-re/src/relocData/52_MVCommon.c` — the opening movie's room cutscene (`RoomBackground`, `RoomDesk`, `RoomLogo`, etc.), not a UI/menu system as earlier guessed
* Traced `refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c` — each room piece is set up with two independent calls on the same `GObj`: `gcSetupCommonDObjs(gobj, dobjdesc)` then a separate `gcAddMObjAll(gobj, mobjsub)` — no struct links them, only the call order in the executable. Checked every `gcSetupCommonDObjs` call against whether a `gcAddMObjAll` follows on the same `gobj`: exactly 5 do, matching the 5 unpaired graph offsets; the rest (`RoomDesk`, `RoomBooks`, `RoomLamp`, etc.) correctly have none
* Read the 5 graphs' and 5 tables' file offsets directly from `52_MVCommon.c`'s own comments: `(0x7E98, 0x42F8)` RoomBackground, `(0x1C4A8, 0x1BC60)` RoomLogo, `(0x1DF28, 0x1DCA0)` RoomCloseUpEffectAir, `(0x1F270, 0x1F0F8)` RoomCloseUpEffectGround, `(0x22440, 0x20480)` RoomDeskGround
* Implemented: 5 more hand-inserts in `tools/romtool/src/main.rs`'s `load_all`, same gated pattern as RE-059
* `cargo build --release -p romtool` — clean
* `romtool mobj --file 52` after — 5/5 graphs paired, 0 chain/demand mismatches, "wanting one but unnamed" 5→0
* `romtool textures --file 52` after — 58/58 packed, 0 failures (file 52 fully resolved)
* `romtool textures` (archive-wide) after — 618→638 packed, 647→665 unique bound (previously-unbound primitives now resolve a real texture), `MissingPalette` 3→1
* `romtool mobj` (archive-wide) after — 58→63 paired, 69→64 unpaired
* `cargo test --workspace` — 338 passing, unaffected
* Traced file 86's remaining graph's pairing mechanism: `refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`'s `itGetPData` computes the MObjSub pointer as a byte-offset delta from a runtime base pointer — a fifth mechanism, not yet confirmed against file 86's specific graph; left unfixed
* Result: RE-060 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.4/R0.7, `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: `tools/romtool/src/main.rs` (`load_all`) — code change, plus documentation
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-03 — R0.7: EFDesc found and fixed for file 353 (RE-059)

* Ran `romtool mobj --file 353` before any change — confirmed 3 unpaired graphs (0x3F8, 0x7B8, 0x11C0), each "calls the graphics heap but no record names"
* Traced `refs/ssb-decomp-re/src/ft/ftcommon/ftcommonentry.c:214-215` → `efManagerLinkEntryWaveMakeEffect`/`efManagerLinkEntryBeamMakeEffect` → `refs/ssb-decomp-re/src/ef/efmanager.c:1162-1219`'s `dEFManagerLinkEntryWaveEffectDesc`/`dEFManagerLinkEntryBeamEffectDesc` — found `EFDesc` (`refs/ssb-decomp-re/src/ef/eftypes.h:11-24`), a third pairing shape, with confirmed non-null `DObjDesc*`/`MObjSub***` pointers into file 353 (`EntryWaveDObjDesc @ 0x3F8`/`EntryWaveMObjSub @ 0x130`, `EntryBeamDObjDesc @ 0x7B8`/`EntryBeamMObjSub @ 0x4F0`, cross-checked against `353_LinkSpecial2.c`'s own offset comments)
* Confirmed `EFDesc` instances live in the game's static executable (fixed RAM addresses in `efmanager.c`, not relocData offsets), so `PartTables::scan`'s archive-relocation-only scan can structurally never find them
* Implemented: `tools/romtool/src/main.rs`'s `load_all` now hand-inserts `(353, 0x3F8, 0x130)` and `(353, 0x7B8, 0x4F0)` via `PartTables::insert()`, gated on `read_table` parsing (same pattern as the existing stage-layer inserts)
* `cargo build --release -p romtool` — clean
* `romtool mobj --file 353` after — pairings 56→58, 2 graphs now paired, 0 chain/demand mismatches for both
* `romtool textures --file 353` after — 1 failure → 0
* `romtool textures` (archive-wide) after — 617→618 packed, `MissingPalette` 4→3
* `romtool mobj` (archive-wide, no `--file`) after — 58 paired / 69 unpaired, exactly 71-2, confirming no other archive-scannable `WPAttributes`/`EFDesc` instance was already being silently caught and that both the old 56/71 figures were accurate, just stale
* `cargo test --workspace` — 338 passing, unaffected (fix is in `romtool`, not the library crate)
* Traced SpinAttack's pairing mechanism too: `refs/ssb-decomp-re/src/wp/wplink/wplinkspinattack.c`'s `dWPLinkSpinAttackWeaponDesc` names `&llLinkMainSpinAttackWeaponAttributes` (in file 225) as a `WPAttributes`, but unlike the boomerang's, this instance is not yet typed in the decompilation — deliberately left unfixed rather than guessing a table offset
* Result: RE-059 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.4/R0.7, `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: `tools/romtool/src/main.rs` (`load_all`) — code change, plus documentation
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.7 started: WPAttributes found, 353 sibling-file guess retracted

* Read `refs/ssb-decomp-re/src/relocData/353_LinkSpecial2.c` directly — found `dLinkSpecial2_EntryWaveDObjDesc`/`dLinkSpecial2_EntryBeamDObjDesc` (graphs) and `dLinkSpecial2_EntryWaveMObjSub`/`dLinkSpecial2_EntryBeamMObjSub` (tables) both defined in the same file, refuting RE-057's "table lives in a sibling file" guess
* Read `refs/ssb-decomp-re/src/relocData/225_LinkMain.c`'s `FTCommonPartContainer` — confirmed it pairs Link's *main* model (`dLinkModel_JointTree`, both halves in file 324) and has nothing to do with 353's sub-models
* Read `refs/ssb-decomp-re/src/wp/wptypes.h:36-45` — found `WPAttributes`, a second struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` looks for, used for weapon/projectile objects, never documented in `crates/ssb-rom/src/mobj.rs`
* Read `refs/ssb-decomp-re/src/relocData/226_LinkSpecial1.c`'s `dLinkSpecial1_Boomerang_WeaponAttributes` — a real `WPAttributes` instance with `p_mobjsubs = NULL` by design (its `data`/`anim_joints` point into file 325, not 353) — shows a null table can be intentional, tempering how much weight to put on "found the missing struct shape" as a guaranteed fix for 353
* Result: RE-058 recorded in `docs/reverse-engineering.md`, retracting RE-057's specific 353 guess while keeping its zero/partial-`mobjs` mechanism finding intact; `PLAN.md` R0.7, `TODO.md` Phase E updated
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.3 closed: segment-0x01 and MissingPalette investigations

* `cargo run --release -p romtool -- scan "rom/Super Smash Bros. (USA).z64" --exhaustive` — 0 occurrences of opcode 0x09/0x0A anywhere in the ROM's display lists; refutes RE-054's S2DEX BG lead
* `cargo run --release -p romtool -- dump "rom/Super Smash Bros. (USA).z64" 39` (and 40/41/45/50/51) — dumped raw file bytes, located the failing `G_SETTIMG` (file 39 offset 0x0E10: `fd10012b 01000000`), confirmed identical bytes recur across files 40/41/45/50/51
* Cross-checked address `0x01000000` (segment 1) against `refs/ssb-decomp-re/src/lb/lbtransition.c` — `gSPSegment(..., 0x1, sLBTransitionPhotoHeap)`, a per-frame `300x220` 16-bit framebuffer photocopy for the loading-break transition system; texture dims from `romtool textures --file 39` (`300x5`, `300x6` `Rgba/Bits16`) match
* Temporarily instrumented `crates/ssb-rom/src/mesh.rs` with `eprintln!`s (in `convert_sequence`, the segment-0x0E `Call` handler, `SetTimg`, `LoadTlut`), ran `romtool textures --file 52/86/353`, then reverted the instrumentation (`git checkout -- crates/ssb-rom/src/mesh.rs`) — confirmed files 52 and 353 get zero `MObj` materials for every graph node, file 86 gets them for most but not all nodes; every unresolved segment-0x0E call clears an otherwise-valid palette via `forget_texture()`
* Identified file names via `refs/ssb-decomp-re/src/relocData/`: 52=`MVCommon`, 86=`ITCommonObject`, 353=`LinkSpecial2` (one of Link's own model files, unlike 52/86)
* Result: RE-057 recorded in `docs/reverse-engineering.md`, correcting RE-056's dedup-key theory. `PLAN.md` R0.3 marked `COMPLETE`; R0.4, R0.7 updated; `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: documentation/investigation only — `crates/ssb-rom/src/mesh.rs` was temporarily modified for tracing and fully reverted before commit; `git diff` confirms no net code change
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below
* Result: RE-055 and RE-056 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.13, `TODO.md` Phase B, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.2 BattleShip cross-reference

* Cloned `refs/BattleShip` + `libultraship` submodule (`ssb64` branch); read `src/fast/interpreter.cpp`
* Cross-checked against `refs/ssb-decomp-re/src/sys/objdisplay.c` (already present, no clone needed)
* Result: RE-054 recorded in `docs/reverse-engineering.md`; three new leads filed in `TODO.md` (R0.13 S2DEX BG, R0.5 LOD corroboration, R0.8 transform clarification)
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — Documentation audit (R0.1)

* `cargo test --workspace` — 338 passing (`ssb-rom` 195, `ssb-engine` 36, `ssb-game` 107), 0 failed
* `cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"` — 647 bound / 617 packed / 30 failed (26 segment-0x01, 4 `MissingPalette`); matches what `docs/rendering.md` now documents
* Affected subsystem: documentation only, no code changed
* PPSSPP: not re-run this pass; prior baseline stands (see `docs/porting-status.md` "M1 verification")
* Physical PSP: not tested this pass — see §8 below

Future entries must include:

* command/test;
* result;
* relevant output;
* affected subsystem;
* whether PPSSPP was tested;
* whether physical PSP was tested.

---

# 8. Physical PSP Validation

## Status

`R2 ACCEPTANCE NOT YET PERFORMED`

The project has been booted and smoke-tested on physical PSP hardware earlier
in development. That testing was not captured against `PLAN.md` R2's
acceptance criteria (hardware model, build, asset-pack version, per-feature
observed behavior, VRAM usage) and predates the rendering work tracked under
R0, so it does not stand in for R2. Treat R2 as not started until a hardware
session records the fields below.

PPSSPP testing does not count as physical PSP validation.

When R2 hardware validation begins, record:

* PSP model
* firmware/environment
* EBOOT/build
* runtime asset pack
* test scene
* observed FPS/frame time
* VRAM behavior
* rendering failures
* screenshots/video where useful

---

# 9. Important Rules

* `PLAN.md` determines task order.
* `STATUS.md` determines current execution state.
* `docs/porting-status.md` determines verified subsystem status.
* `AGENTS.md` determines agent behavior.
* Rendering is the hard gate.
* Do not begin combat because rendering appears "good enough".
* Do not mark work complete without evidence.
* Do not treat PPSSPP as physical PSP validation.
* Do not invent original N64 behavior when the decompilation/ROM can answer it.

---

# 10. Session Continuity

Before ending a session or context compaction, update this file with:

```text
Current milestone
Current task
Task status
Last completed task
Next eligible task
Blockers
Changes made
Verification performed
Evidence
Important discoveries
Documentation updated
Relevant commit
```

A fresh agent must be able to read this file and continue without relying on conversation history.

---

# 11. Continuation Command

The intended workflow is:

> Continue with the plan.

The agent should then:

1. Read `AGENTS.md`.
2. Read `PLAN.md`.
3. Read this file.
4. Inspect the repository.
5. Resume the current task.
6. Implement.
7. Verify.
8. Document.
9. Update this file.
10. Commit.
11. Continue to the next eligible task.
