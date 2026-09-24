# Animation and Effects

Part of [rendering.md](../rendering.md).

## Billboards

Status: complete (109 nodes; RE-131–145).

- Pack flags separate camera basis from spin. `FLAG_BILLBOARD_SPIN_Z` uses
  the authored Z angle only for Kind46; Kind44/48/50 ignore authored rotation
  (RE-141). All shipped rest spin angles are zero.
- Six billboards are direct stage-animation joints and six more descend from
  animated joints. `StageAnimator::compose` propagates the parent transform
  (RE-142).
- Scale follows `gcPrepDObjMatrix`: billboard X is `ancestor_x * scale.x`, Y
  is `ancestor_x * scale.y`, and the signed X carries to children
  (`StageAnimator::billboard_scales`, RE-143). Z follows the original X/Y/X
  rule (RE-144).

## Transparency

Status: complete for classified formulas. 25 of 35 translucent billboard
primitives use the `ALPHA_BLEND` path; the other 10 are the declined
`PRIM_ALPHA`/two-cycle cases (RE-135). See
[depth-alpha-blending.md](depth-alpha-blending.md).

## Effects and particles

Status: complete for renderer scope.

Manager descriptors, transforms, material/texture/colour animation, all 160
`LBParticle` scripts, `LBGenerator` spawn trees and PSP drawing are
implemented (RE-172–189). Remaining effect call sites belong to gameplay.

## Shadows

Status: complete for the Training runtime (RE-302).

`ftShadowProcDisplay` draws a floor-conforming strip, not a blob:

- Projects onto the standing floor, or the nearest floor below while airborne.
- Clips `x ± FTAttributes::shadow_size` to the floor line and follows up to
  two bends (eight-vertex capacity).
- Texture: file 84 `0x3A68`, 16×16 I4, mirror-repeat pre-baked.
- Material: black `(0,0,0,0xA0)`, `AA_XLU_SURF` depth test without write,
  alpha threshold `0x0F`, blended, unlit, no cull.
- Drawn after stage geometry, before fighters. No altitude fade.
- Hidden during entry, KO/sleep and rebirth.

Remaining: team-colour shadows and moving map groups (match runtime).

## Framebuffer effects

Status: complete for renderer scope.

LB-transition capture and all 11 wipes (RE-146–149); 1P wallpaper capture and
its `SObj` sprite draw (RE-190–193). Results-screen and match-transition
triggers belong to gameplay.

The 26 segment-0x01 texture references are `sLBTransitionPhotoHeap`, a
runtime framebuffer copy, not ROM data (RE-055).

## UI

Status: not started. Only a developer overlay exists, built on
`sceGuDebugFlush` (RE-014, RE-202). A real HUD needs GE geometry.
