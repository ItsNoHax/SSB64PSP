# Animation and Effects

Part of [rendering.md](../rendering.md).

## Stage visibility

`StageJoint` retains `SetFlags` commands. Bit 0 suppresses the node's mesh;
bit 1 suppresses its whole subtree, following `gcDrawDObjTree`. Siblings
remain visible. Training draws the same animated poses that supply map-group
translation and speed; On/Off collision groups skip their own animation
callback. Collision Hidden/Show transitions follow zero/nonzero source flags
(RE-353).

Controller objects are separate from the four render layers (RE-357).
`GroundObjects` plays packed DObj clips and retains node poses and flags
across replacements. `StageObjectsPort` exposes their GObj clocks and root
translations to gameplay. Training advances them at priority 5 before
fighter interrupts and the priority-4 stage controller.

Draw order follows the original display links: layers 4, 6, 13, 17;
Whispy eyes/mouth and back flowers follow layer 4, front flowers use 16,
and barrel/gate use 6. Per-object Whispy and cloud material animation is
deferred; Whispy currently draws rest materials.

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

Stage items use their descriptor transforms, rather than only the packed
graph flags. The Piranha Plant's promoted root uses kind 48's pitch-locked
billboard until damage changes it to kind 70's screen-facing Z spin, which
ignores object scale. Rebirth clears the spin but retains kind 70. POW and
plant animation clocks run on node 1 after the empty root is removed;
absent items are hidden (RE-429).

Status: complete for renderer scope.

Manager descriptors, transforms, material/texture/colour animation, all 160
`LBParticle` scripts, `LBGenerator` spawn trees and PSP drawing are
implemented (RE-172–189). Remaining effect call sites belong to gameplay.

Matches load the common bank as runtime bank 0 and Yoshi's `particles_unk2`
(pack bank 3) as runtime bank 1. Item common (pack bank 4) is runtime
bank 2; Charmander's flame makes scripts 2 and 0 with its position and
velocity before their make-time process (RE-430). Egg Throw's explosion
uses Yoshi script 3;
shield release uses common script 0x54. The roll egg draws on link 15 at
fighter joint 5, scaled 1.5 and spun from that joint's signed local X
rotation, with the shield's health-dependent ENV colour. Skeleton poses
are looked up by model node: animation indices omit runtime joints and
cannot be treated as fighter joint indices (RE-427).

Saffron's five item trees load from archive file 159, while the gate uses
file 160. Each promoted root supplies the source animation clock. Charmander
and Venusaur select one of two packed sprite meshes directly by texture ID;
they use the OPA item seed. Electrode spins as a screen-facing billboard
and hides during its explosion. Razor Leaf uses the weapon seed (RE-430).

Normal Egg uses file 86's child-1 kind-46 billboard, with root Z spin and
the child-2 ROM scale animation. Its raw add/play clock starts at frame
zero through `StageAnimator::start_changed`; existing stage clocks retain
their previous entry point. Capsule composes its four descriptor nodes
under the loose item transform or normalized fighter hand joint. Item
visuals cover all sixteen shared-pool slots (RE-431).

Poké Ball Pokémon use file 86's trees and maker-specific matrices, with
packed appear/status joint scripts; camera-relative kind 48 scales X/Z by
X and Y by Y. Onix, Blastoise and Hitmonlee swap attack display lists.
Hydro Pump, Smog and Beedrill swarm replay their weapon material scripts
from each weapon's play count. Pokémon status materials, translucent modes,
link 18, rock texture IDs and Poké Ball rays/open animation remain. The
FlySparks head-0 state produces black squares; its head-1 CLD counterpart
is a separate path (RE-435).

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

Status: `SObj` sprites draw through the GE (RE-392 onwards); the
developer overlay still uses `sceGuDebugFlush` (RE-014, RE-202).

- `meshdraw::draw_sprite` and its variants draw a libultra `Sprite` as one
  `GU_SPRITES` rectangle at N64 screen coordinates, the 320 × 240 screen
  scaled by 272 / 240 onto the pillarbox, with `lbCommonPrepSObjAttr`'s
  combiner per format (RE-392, RE-410, RE-411).
- The battle wallpaper (`meshdraw::draw_wallpaper`, RE-419) is the one
  exception: it is mapped as the battle's 3D is, the stage camera's
  (10, 10) to (310, 230) viewport scaled by 272 / 220 about the
  pillarbox's centre, so it stays registered with the stage. It is
  drawn first, opaque and without depth, from a 5551 texture in main RAM.
- The stage select's preview model draws under its own camera over the
  select's sprites (`meshdraw::draw_stage_preview`, RE-419).
- Training's interface uses its original sprite tables and signed label
  positions (RE-439). Its closed-menu stats use the Interface link; opening
  hides all interfaces and draws the PauseMenu link. The blue panel and
  inclusive red underline precede the menu sprites. DAMAGE/COMBO retain
  their last digits for 90 process ticks after the source counters clear.
