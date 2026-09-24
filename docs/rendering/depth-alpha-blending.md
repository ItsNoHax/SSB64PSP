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
| Coverage cutout | `CVG_X_ALPHA \| ALPHA_CVG_SEL` → `alpha >= 1` | RE-069 |
| Threshold compare | `G_MDSFT_ALPHACOMPARE` = `G_AC_THRESHOLD` | RE-195 (29.8% of commands) |

| Combination | GE test |
|---|---|
| Threshold with reference > 0 | `alpha >= reference` (implies cutout) |
| Threshold 0 with cutout | `alpha > 0` |
| Threshold 0 alone | no-op gate |

Other `SETOTHERMODE` fields were measured archive-wide (RE-124, RE-127,
RE-195): `ALPHADITHER`, `RGBDITHER`, `COMBKEY`, `TEXTCONV`, `TEXTPERSP` and
`ZSRCSEL` always equal the RDP default; `PIPELINE` has no visible effect.
`TEXTLUT` is modeled in [textures.md](textures.md).

## Blending

Status: complete for classified single-cycle formulas.

Nine alpha-combiner shapes were classified archive-wide. `TEXEL0_ALPHA` and
`TEXEL0_ALPHA * SHADE_ALPHA` blend on the GE (RE-129, RE-130).

Declined, measured: about 43 `PRIM_ALPHA`-multiply and 93 two-cycle
primitives.
