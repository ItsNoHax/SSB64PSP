# Depth, Alpha and Blending

Part of [rendering.md](../rendering.md).

## Depth

- PSP depth range inverted: `sceGuDepthRange(65535, 0)` with `GreaterOrEqual` (`psp-runtime/src/gu.rs`, [D-007](../decisions/D-007.md)).
- `Z_CMP`, `Z_UPD`, `ZMODE` kept independently; `sceGuDepthMask` driven directly (RE-244–251).
  - `depth_mask_diagnostic` scene pins write ON → OFF → ON.

### Magnifier depth image

- Magnifier writes its ROM colour image into depth.
- `draw_depth_image`:
  1. Sample and pack RGBA5551.
  2. Decode compressed RDP Z to 18 bits.
  3. Map to inverted GE 16-bit depth.
  4. Write horizontal spans, colour writes masked off.
- Deviations: GE loses two depth bits; no RDP coverage or dZ tolerance; native-pixel sampling differs from the N64 texture rectangle ([RE-440](../evidence/re/RE-440.md)).

## Alpha test

N64 has two independent discard gates; GE has one alpha-test unit. `pack::alpha_gate` combines them (RE-214):

| N64 gate | Source | Evidence |
|---|---|---|
| Coverage cutout | `CVG_X_ALPHA \| ALPHA_CVG_SEL` → `alpha >= 32` | RE-069, RE-424 |
| Threshold compare | `G_AC_THRESHOLD` against `G_SETBLENDCOLOR`'s alpha | RE-195 |

| Combination | GE test |
|---|---|
| Cutout | `alpha >= 32` (`TEX_EDGE_MIN_ALPHA`) |
| Threshold with reference `r` and cutout | `alpha >= max(r, 32)` |
| Threshold alone | `alpha >= r`; `r = 0` is a no-op gate |

### Coverage and `CVG_X_ALPHA`

- `CVG_X_ALPHA` scales a pixel's coverage by its alpha: `(alpha * cvg + 4) >> 8`.
- With `AA_EN` (every `TEX_EDGE` mode in the archive) the RDP writes nothing where no coverage is left: `alpha < 32` at full coverage.
- Kept pixel over a fully covered background: written unblended with partial coverage (`CVG_DST_CLAMP`).
  - VI antialiasing (mode 0 in SSB64) mixes it with the background by that coverage.
- GE blends such a primitive by its alpha instead (RE-424).
- Deviations:
  - GE has no coverage → polygon edges count as fully covered.
  - Mix is by alpha, not eighths of coverage.
- RDP writes partial-coverage edge pixels and samples texture attributes ~(-0.35,-0.35) ±0.1 px from the GE pixel centre (Mario and Kirby ramps); sub-pixel seams that the N64 closes can open on PSP (RE-581 #10).  - Mix uses what is drawn before the pixel; VI uses finished neighbours.
- Example: Planet Zebes's yellow transparent TLUT entry, filtered into platform edges, mixes with the background by coverage, as on the N64.

### Other `SETOTHERMODE` fields (RE-124, RE-127, RE-195)

- Measured archive-wide.
- `ALPHADITHER`, `RGBDITHER`, `COMBKEY`, `TEXTCONV`, `TEXTPERSP`, `ZSRCSEL`: always RDP default.
- `PIPELINE`: no visible effect.
- `TEXTLUT`: modeled in [textures.md](textures.md).

## Blending

- Nine alpha-combiner shapes classified archive-wide.

| Alpha shape | GE blend | Evidence |
|---|---|---|
| `TEXEL0_ALPHA` | yes | RE-129, RE-130 |
| `TEXEL0_ALPHA * SHADE_ALPHA` | yes | RE-129, RE-130 |
| `TEXEL0_ALPHA * PRIM_ALPHA` | where render mode blends or `PRIM` animated | RE-322, RE-323 |
| Unlit untextured flat `PRIM_ALPHA` | where render mode blends | RE-440 |
| Unlit untextured `SHADE_ALPHA` alone (`G_CC_SHADE`) | by vertex alpha; texture gate on translucency does not apply | RE-422 |
| `PRIM_ALPHA`-multiply, two-cycle primitives | declined | RE-135 |

- Player-arrow wrapper seeds `G_CC_PRIMITIVE` and red alpha 128; its three primitives use it ([RE-440](../evidence/re/RE-440.md)).

### Head-1 render-mode seeds

- Task-list head 1 starts from the camera's `func_80016338` reset to `G_RM_AA_ZB_XLU_SURF` (depth and blender; RE-328).
- Stage render-layer-1 also resets head 1 in `grDisplayLayer1*ProcDisplay` (RE-250, RE-323).
- Stage layers 0, 2, 3: `SecProcDisplay` sets head 1 to `G_RM_AA_XLU_SURF`, `G_ZBUFFER` cleared (`Head1Seed::LayerXlu`).
- DL links 15 and 18:
  1. `efDisplayCLDProcDisplay` (priority 3): head 1 → `G_RM_CLD_SURF` with `G_AC_THRESHOLD`, clears `G_ZBUFFER`, before the priority-2 effects.
  2. `efDisplayXLUProcDisplay` (priority 0): restores the Z-buffered mode after them.
  - Shield bubble and other `DObjDLLink` effects there seeded with `Head1Seed::EffectCld` → a list setting no mode draws with no depth test (RE-421).

## Draw order

- `gmCameraDefaultProcDisplay` draws the battle in passes of display links: 1–2, 4, 6–12, 13–15, 16–18, 19–20.
  - Links ascending; within a link, by priority.
  - Each pass runs every head-0 list of its links before any head-1 list (`syTaskmanUpdateDLBuffers`).

| Object | Link |
|---|---|
| Stage layers | 4, 6, 13, 17 |
| Shadows | 7 |
| Fighters | 9 |
| Items | 11 |
| Weapons | 14 |
| Effects | 10, 15, 18 or 20 (`EFDesc`) |

- Layers 0, 2, 3 and Dream Land's front flowers (link 16): no depth test → each covers what an earlier pass drew.
- `draw_training` draws in this order.
  - Stage: `meshdraw::draw_stage_links`, `DrawState::heads`.
  - Pack marks head-1 primitives with `flags::HEAD1` (RE-422).
- Objects split by head (RE-563): `battle_draw::BATTLE` lists each pass's steps; a group's head-1 step runs only if its head-0 step skipped a head-1 primitive (`DrawState::head1_pending`).
  - Head tags come from the ROM (`ssb-rom` `task_head.rs`): whole-head-1 weapons and effects are packed `HEAD1`.
  - Non-mesh draws (sprites, quads, trails, particles, shadows) stay in the head-0 step (`mesh_only`).
- Stage controllers' objects on link 6 draw under layer 1's `G_ZBUFFER` and `G_RM_AA_ZB_OPA_SURF`, inherited by their lists (RE-423).
  - Objects: Saffron City's gate, Yoshi's Island's clouds, Kongo Jungle's barrel, Mushroom Kingdom's scales.

### After the battle

- Arrow camera (priority 35) draws before magnifier camera (30) and interface camera (20).
- Training's open menu hides Interface-link tags/arrows; magnifiers remain until Close-Up disables them (RE-440).
