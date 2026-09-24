# Materials

Part of [rendering.md](../rendering.md).

## Combiner

Status: complete for classified static paths.

`mesh.rs` evaluates the general `(A-B)*C+D` combiner and classifies the
shapes the GE can express:

| Shape | GE lowering | Evidence |
|---|---|---|
| Shade scale | Folded into vertex colour | RE-039, RE-043, RE-106 |
| `(PRIM-ENV)*TEXEL+ENV` texture blend | `TextureEffect::Blend` (`TEXTURE_BLEND` flag) | RE-073, RE-074 |
| Flat constant colour | Constant vertex colour | RE-080 |

Coverage: 65,000 of 65,199 source-attributed triangle visits (99.7%). The
186 unsupported-equation visits are catalogued (RE-168). Runtime shield
colours belong to gameplay integration.

`G_MOVEWORD`'s `G_MW_LIGHTCOL` index is the signal that a segment relies on
externally enabled `G_LIGHTING`; it drives lit/literal vertex classification
(RE-105).

### `G_SHADE` cleared with a `SHADE`-reading combiner

`G_SHADE` is always cleared together with `G_LIGHTING` and
`G_SHADING_SMOOTH`, switching to flat `PRIMITIVE`-driven output that the
classified shapes already reproduce. 31 primitives also read `SHADE` in the
combiner: 29 are in content not yet rendered (items, special effects, opening
movie) and 2 are on Yoshi's Island. Real-hardware output for this case is
undocumented, so it is left undecided (RE-119, RE-120).

## `MObj` display state

Status: complete for measured static content.

All 665 real `MObjMaterial`s are measured; default substitution, texture
scale and tile-0 window state are implemented. `MOBJ_FLAG_FRAC` never occurs
in static content. Tile-1 scroll is inert: all 12 inputs equal tile 0 and no
`TEXEL1` consumer exists (RE-194).

## Material animation

Status: stage palette, frame and tile-0 UV tracks implemented.

- Each animated `MObj`'s rest `TraU`/`TraV`/`ScaU`/`ScaV` and tile parameters
  are packed; the runtime applies `gcDrawMObjForDObj`'s tile-window delta
  through the GE texture mapping (RE-301).
- `TextureIDCurrent` selects a packed texture; `PaletteID` selects its CLUT.
- Cache identity is `(file, script, MObjSub)` plus the live affine transform,
  so state never leaks between materials.
- The linear-filter `+0.5 / uploaded_dim` correction is added after the
  animated transform, so animation never scales it (RE-304).

Limitations (not approximated):

- `SetLFrac`/`TextureIDNext` need the RDP two-tile fractional blend.
- `ScrU`/`ScrV` only affect tile 1.
- Sparse stage `PrimColor`/`Light1Color`/`Light2Color` tracks feed baked
  vertex colour or an absent stage GE light; they need dynamic lowering.
