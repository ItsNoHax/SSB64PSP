# Animation and Effects

Part of [rendering.md](../rendering.md).

## Stage visibility

### Joint flags

- `StageJoint` retains `SetFlags` commands.
  - Bit 0: suppresses the node's mesh.
  - Bit 1: suppresses its whole subtree (follows `gcDrawDObjTree`).
  - Siblings stay visible.
- Training draws the same animated poses that supply map-group translation and speed.
  - On/Off collision groups skip their own animation callback.
  - Collision Hidden/Show transitions follow zero/nonzero source flags (RE-353).

### Controller objects (RE-357)

- Separate from the four render layers.
- `GroundObjects`: plays packed DObj clips; retains node poses and flags across replacements.
- `StageObjectsPort`: exposes their GObj clocks and root translations to gameplay.
- Training advances them at priority 5, before fighter interrupts and the priority-4 stage controller.

### Draw order

- Follows original display links: layers 4, 6, 13, 17.
  - Whispy eyes/mouth and back flowers: after layer 4.
  - Front flowers: link 16.
  - Barrel/gate: link 6.
- Per-object Whispy and cloud material animation not applied; Whispy draws rest materials.

## Billboards

RE-131–145.

- Pack flags separate camera basis from spin.
  - `FLAG_BILLBOARD_SPIN_Z` uses the authored Z angle only for Kind46.
  - Kind44/48/50 ignore authored rotation (RE-141).
  - All shipped rest spin angles are zero.
- Six billboards are direct stage-animation joints; six more descend from animated joints.
  - `StageAnimator::compose` propagates the parent transform (RE-142).
- Scale follows `gcPrepDObjMatrix`:
  - Billboard X = `ancestor_x * scale.x`.
  - Billboard Y = `ancestor_x * scale.y`.
  - Signed X carries to children (`StageAnimator::billboard_scales`, RE-143).
  - Z follows the original X/Y/X rule (RE-144).

## Transparency

- Translucent billboard primitives use the `ALPHA_BLEND` path, except declined `PRIM_ALPHA`/two-cycle cases (RE-135).
- See [depth-alpha-blending.md](depth-alpha-blending.md).

## Effects and particles

- Implemented (RE-172–189): manager descriptors, transforms, material/texture/colour animation, all `LBParticle` scripts, `LBGenerator` spawn trees, PSP drawing.
- Effect call sites belong to gameplay.

### Stage items (RE-429)

- Use descriptor transforms, not only packed graph flags.
- Piranha Plant promoted root:
  - Kind 48 pitch-locked billboard until damage.
  - Then kind 70 screen-facing Z spin, ignores object scale.
  - Rebirth clears the spin, retains kind 70.
- POW and plant animation clocks run on node 1 after the empty root is removed.
- Absent items hidden.

### Particle banks (RE-430)

| Runtime bank | Source |
|---|---|
| 0 | Common bank |
| 1 | Yoshi's `particles_unk2` (pack bank 3) |
| 2 | Item common (pack bank 4) |

- Charmander's flame makes scripts 2 and 0 with its position and velocity before their make-time process.
- Egg Throw explosion: Yoshi script 3.
- Shield release: common script 0x54.

### Roll egg (RE-427)

- Draws on link 15 at fighter joint 5, scaled 1.5.
- Spun from that joint's signed local X rotation.
- ENV colour = shield's health-dependent colour.
- Skeleton poses looked up by model node.
  - Animation indices omit runtime joints → not fighter joint indices.

### Saffron items (RE-430)

- Five item trees load from archive file 159; gate uses file 160.
- Each promoted root supplies the source animation clock.
- Charmander, Venusaur: select one of two packed sprite meshes directly by texture ID; OPA item seed.
- Electrode: screen-facing billboard spin; hides during its explosion.
- Razor Leaf: weapon seed.

### Item descriptor 1 (RE-444)

- Every item tree promotes descriptor 1 after the manager ejects descriptor 0.
- Loose: translation belongs to gameplay.
- Held: descriptor offset sits under the normalized hand joint.
- PK Fire: `StageAnimator::compose_item` preserves descendant scripts, without composing the placeholder or adding root translation twice.

### Eggs and containers (RE-431–432, RE-444)

- Normal Egg:
  - Promoted root = packed node 1.
  - Child = packed node 2: kind 46, ROM scale animation, root Z spin.
  - Raw add/play clock starts at frame zero via `StageAnimator::start_changed`.
- Capsule and heavy containers compose from descriptor 1 under the loose item transform or normalized fighter hand joint.
- Item visuals cover all sixteen shared-pool slots.

### Link Bomb (RE-444)

- Promoted body/root = packed node 1; hand offset (30, 90, 15) only while held.
- Fuse = packed node 2; maker adds kind 46 to both body and fuse.
  - Fuse translation inherits the body's billboard frame.
  - Its own spin/scale uses camera axes and `gGCScaleX`.
- Bloat scales the same body node held and loose.
- Throw: refreshes sampled joint axes from TopN yaw every step; facing flips halfway.

### Poké Ball Pokémon (RE-443)

- File 86 trees; maker-specific matrices; packed appear/status joint scripts.
- Camera-relative kind 48 scales X/Z by X, Y by Y.
- Onix, Blastoise, Hitmonlee: swap attack display lists.
- Hydro Pump, Smog, Beedrill swarm: replay weapon material scripts from each weapon's play count.
- Goldeen, Charizard, Beedrill, Starmie: material scripts start on status clocks.
- Snorlax's fall, Starmie, Hitmonlee, Clefairy's imitations: move to link 18 in head insertion order.
  - Only callbacks selecting AA_XLU_SURF use that mode.
- Onix's rocks: one of three packed sprites.
- Thrown Poké Ball:
  - Cycles its eight textures.
  - Clears that script on opening.
  - Creates rays on link 10 that follow it until release.

### Head-1 effect lists

- `lbCommonDObjScaleXProcDisplay` writes direct FlyOrbs, CommonSpark (FlySparks, Star Rod), ShockSmall lists to head 1.
- Their links install CLD state first.
- Pack marks HEAD1, applies that seed.
- RE-443 corrects RE-435's head-0 diagnosis.

## Shadows

RE-302. `ftShadowProcDisplay` draws a floor-conforming strip, not a blob:

- Projects onto the standing floor, or the nearest floor below while airborne.
- Clips `x ± FTAttributes::shadow_size` to the floor line and follows up to
  two bends (eight-vertex capacity).
- Texture: file 84 `0x3A68`, 16×16 I4, mirror-repeat pre-baked.
- Material: default `(0,0,0,0xA0)`; team battles use the team colour
  (`shadow::COLOR_TEAMS`, red/blue/green, unless `is_not_teamshadows`).
  `AA_XLU_SURF` without depth test or write (RE-514), alpha threshold
  `0x0F`, blended, unlit, no cull.
- Drawn after stage geometry, before fighters. No altitude fade.
- Hidden during entry, KO/sleep and rebirth.

## Framebuffer effects

- LB-transition capture and all 11 wipes (RE-146–149).
- 1P wallpaper capture and its `SObj` sprite draw (RE-190–193).
- Campaign stage-clear trigger (RE-451):
  1. Samples the last completed active picture before opening its first result frame.
  2. Redraws at (10,10)–(310,230) with 0x80 dimming.
- Capture uses native PSP 8888 pixel centers and GE bilinear sampling; no quantizing to N64 5551.
- Viewer diagnostic keeps its separate rectangle.
- Match-transition triggers belong to gameplay.
- The 26 segment-0x01 texture references are `sLBTransitionPhotoHeap`, a runtime framebuffer copy, not ROM data (RE-055).

## UI

- `SObj` sprites draw through the GE (RE-392 onwards).
- Developer overlay uses `sceGuDebugFlush` (RE-014, RE-202).

### Sprites

- `meshdraw::draw_sprite` and variants draw a libultra `Sprite` as one `GU_SPRITES` rectangle.
  - N64 screen coordinates through `ssb_engine::coord`'s mapping.
  - Visible (10, 10)–(310, 230) box fills the PSP's height, scale 272 / 220 (D-047).
  - Each format uses `lbCommonPrepSObjAttr`'s combiner (RE-392, RE-410, RE-411).

### Wallpaper and previews (RE-419)

- Battle wallpaper (`meshdraw::draw_wallpaper`): same mapping under the wallpaper camera's viewport → stays registered with the stage.
  - Drawn first, opaque, no depth, from a 5551 texture in main RAM.
- Stage select preview model (`meshdraw::draw_stage_preview`): own camera, over the select's sprites.

### Training interface (RE-439)

- Original sprite tables and signed label positions.
- Closed-menu stats: Interface link.
- Opening: hides all interfaces, draws the PauseMenu link.
- Blue panel and inclusive red underline precede the menu sprites.
- DAMAGE/COMBO retain last digits for 90 process ticks after source counters clear.
