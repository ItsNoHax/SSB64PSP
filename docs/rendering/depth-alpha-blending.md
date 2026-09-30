# Depth, Alpha and Blending

Part of [rendering.md](../rendering.md).

## Depth

Status: complete.

- The PSP depth range is inverted: `sceGuDepthRange(65535, 0)` with
  `GreaterOrEqual` (`psp-runtime/src/gu.rs`, [D-007](../decisions/D-007.md)).
- `Z_CMP`, `Z_UPD` and `ZMODE` are kept independently; `sceGuDepthMask` is
  driven directly (RE-244–251). The `depth_mask_diagnostic` scene pins
  write ON → OFF → ON.

Remaining: physical-PSP capture of the diagnostic.

## Alpha test

Status: complete for both gates and their overlap.

The N64 has two independent discard gates; the GE has one alpha-test unit.
`pack::alpha_gate` combines them (RE-214):

| N64 gate | Source | Evidence |
|---|---|---|
| Coverage cutout | `CVG_X_ALPHA \| ALPHA_CVG_SEL` → `alpha >= 32` | RE-069, RE-424 |
| Threshold compare | `G_AC_THRESHOLD` against `G_SETBLENDCOLOR`'s alpha | RE-195 (29.8% of commands) |

| Combination | GE test |
|---|---|
| Cutout | `alpha >= 32` (`TEX_EDGE_MIN_ALPHA`) |
| Threshold with reference `r` and cutout | `alpha >= max(r, 32)` |
| Threshold alone | `alpha >= r`; `r = 0` is a no-op gate |

`CVG_X_ALPHA` scales a pixel's coverage by its alpha, `(alpha * cvg + 4)
>> 8`, and with `AA_EN` (every `TEX_EDGE` mode in the archive) the RDP
writes nothing where none is left: `alpha < 32` at full coverage. A kept
pixel over a fully covered background is written unblended with its
partial coverage (`CVG_DST_CLAMP`), and the VI's antialiasing (mode 0 in
SSB64) mixes it with the background by that coverage. The GE blends such
a primitive by its alpha instead (RE-424). Deviations: the GE has no
coverage, so polygon edges count as fully covered; the mix is by alpha,
not by eighths of coverage; it is taken from what is drawn before the
pixel, where the VI uses the finished neighbours. Planet Zebes's yellow
transparent TLUT entry, filtered into the platform edges, is therefore
mixed with the background by coverage, as on the N64.

Other `SETOTHERMODE` fields were measured archive-wide (RE-124, RE-127,
RE-195): `ALPHADITHER`, `RGBDITHER`, `COMBKEY`, `TEXTCONV`, `TEXTPERSP` and
`ZSRCSEL` always equal the RDP default; `PIPELINE` has no visible effect.
`TEXTLUT` is modeled in [textures.md](textures.md).

## Blending

Status: complete for classified single-cycle formulas.

Nine alpha-combiner shapes were classified archive-wide. `TEXEL0_ALPHA` and
`TEXEL0_ALPHA * SHADE_ALPHA` blend on the GE (RE-129, RE-130).
`TEXEL0_ALPHA * PRIM_ALPHA` blends where the render mode blends or `PRIM`
is animated (RE-322, RE-323).

Task-list head 1 starts from the camera's `func_80016338` reset to
`G_RM_AA_ZB_XLU_SURF` (depth and blender; RE-328). Stage render-layer-1
also resets head 1 in `grDisplayLayer1*ProcDisplay` (RE-250, RE-323).
On DL links 15 and 18, `efDisplayCLDProcDisplay` (priority 3) sets head 1
to `G_RM_CLD_SURF` with `G_AC_THRESHOLD` and clears `G_ZBUFFER` before the
priority-2 effects, and `efDisplayXLUProcDisplay` (priority 0) restores the
Z-buffered mode after them. The shield bubble and the other `DObjDLLink`
effects there are seeded with `Head1Seed::EffectCld`, so a list that sets
no mode of its own draws with no depth test (RE-421).
Stage layers 0, 2 and 3 set head 1 to `G_RM_AA_XLU_SURF` with `G_ZBUFFER`
cleared in their `SecProcDisplay` (`Head1Seed::LayerXlu`). An unlit,
untextured primitive whose alpha is `SHADE_ALPHA` alone (`G_CC_SHADE`)
blends by its vertex alpha; the texture gate on translucency does not
apply to it (RE-422).

## Draw order

Status: complete for the stage, fighters, items, weapons and effects.

`gmCameraDefaultProcDisplay` draws the battle in passes of display links:
1–2, 4, 6–12, 13–15, 16–18 and 19–20, each link in ascending order and,
within a link, by priority. Each pass runs every head-0 list of its links
before any head-1 list (`syTaskmanUpdateDLBuffers`). The stage layers are
on links 4, 6, 13 and 17; the shadows on 7, the fighters on 9, the items on
11, the weapons on 14, and the effects on 10, 15, 18 or 20 (`EFDesc`).
Layers 0, 2 and 3 and Dream Land's front flowers (link 16) draw with no
depth test, so each covers what an earlier pass drew. `draw_training`
draws in this order, the stage by `meshdraw::draw_stage_links` and
`DrawState::heads`, and the pack marks head-1 primitives with
`flags::HEAD1` (RE-422). The effects draw whole at their link's place; the
head-1 lists of an effect on link 10 or 18 do not wait for the stage's.
The stage controllers' objects on link 6 (Saffron City's gate, Yoshi's
Island's clouds, Kongo Jungle's barrel, Mushroom Kingdom's scales) draw
under layer 1's `G_ZBUFFER` and `G_RM_AA_ZB_OPA_SURF`, which their lists
inherit (RE-423).

Declined, measured: about 43 `PRIM_ALPHA`-multiply and 93 two-cycle
primitives.
