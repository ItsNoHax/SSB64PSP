# Animation and Effects

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

### Billboard rest-pose spin (RE-141)

The pack's billboard flags distinguish camera basis and spin independently.
`FLAG_BILLBOARD_SPIN_Z` selects the authored Z angle only for ROM Kind46;
Kind44/48/50 ignore authored rotation. Case45's X-angle convention belongs
to runtime-created transforms, outside the ROM descriptor path. Pack version
19 is required. `NodeDesc::billboard_rest_spin` supplies the PSP draw call;
posed matrices do not supply animated spin angles yet. All shipped selected
rest spin angles are zero; a synthetic nonzero-angle round-trip regression
pins the distinction without claiming an observed visual improvement.


### Animated billboard hierarchy and scale (RE-142–143)

Every packed animation was intersected with the 109 billboard nodes. Six are
direct stage-animation joints and none changes rotation in 240 frames; fighter
animations reference none. Another six have null scripts but descend from an
animated joint. `StageAnimator::compose` now propagates the parent transform
through those rest-local children, matching the original DObj hierarchy.

The persistent `billboard_animation_inventory` example records the affected
source graph/node, dynamic ranges, final scale, and first negative frame. All
six direct joints animate scale. Three Kind44 nodes reach small negative
uniform scales, which exposed the old unsigned matrix-column-length path.

RE-143 ports the original scale traversal directly. `gcPrepDObjMatrix`
computes billboard X as `ancestor_x * node.scale.x`, billboard Y as
`ancestor_x * node.scale.y`, then carries the new signed X value into children;
the tree walker restores it before visiting siblings.
`StageAnimator::billboard_scales` reproduces that calculation from current
local poses. The PSP animated-stage draw path uses the signed pair with the
model base scale, while static objects retain the already-verified
composed-column path. A regression with a negative animated parent proves that
both child axes inherit signed X, not the parent's Y. The scale acceptance item
is closed; per-node visual and physical PSP checks remain open.

### Transparency

Status: COMPLETE for classified formulas

RE-135 measured 25/35 translucent billboard primitives already carry RE-130's real `ALPHA_BLEND` path

**Remaining work:** The remaining 10 are the same documented `PRIM_ALPHA`/two-cycle long tail, not a billboard-specific gap


### Effects / particles

Status: COMPLETE for R1 renderer scope

RE-172–189 cover manager descriptors, transforms, material/texture/colour animation, all 160 particle scripts, pack serialization, host/device interpretation, spawn-tree execution, `LBGenerator`, PSP drawing, and one real manager-effect runtime spawn event

**Remaining work:** Remaining manager-effect gameplay call sites belong to later gameplay integration, not renderer completeness


### Shadows

Status: NOT STARTED

Only `shadow_size` — a fighter attribute constant extracted from `FTAttributes` — exists; nothing renders a shadow

**Remaining work:** No design exists; not yet a numbered R0.x task


### Framebuffer effects

Status: COMPLETE for R1 renderer scope

R0.13/RE-146–149 cover LB-transition capture and all 11 wipes. RE-190–193 census all remaining framebuffer references, implement 1P wallpaper capture plus its real `SObj` 2D-sprite draw, and device-verify bounded output

**Remaining work:** Real results-screen and match-transition triggers belong to G2 gameplay integration


### UI

Status: NOT STARTED

Debug overlay only, via `sceGuDebugFlush` (software-rasterizer-dependent, RE-014) — not real GE geometry

**Remaining work:** "Renderer 3" below is explicitly not started
