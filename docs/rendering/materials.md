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

Status: every stage track implemented: palette, frame, tile-0 UV, two-tile
blend, `PrimColor`, `Light1Color`, `Light2Color`.

- Each animated `MObj`'s rest `TraU`/`TraV`/`ScaU`/`ScaV` and tile parameters
  are packed; the runtime applies `gcDrawMObjForDObj`'s tile-window delta
  through the GE texture mapping (RE-301).
- `TextureIDCurrent` selects a packed texture; `PaletteID` selects its CLUT.
- Cache identity is `(file, script, MObjSub)` plus the live affine transform,
  so state never leaks between materials.
- The linear-filter `+0.5 / uploaded_dim` correction is added after the
  animated transform, so animation never scales it (RE-304).
- `SetLFrac`/`TextureIDNext`: the two-cycle `(TEXEL1 - TEXEL0) *
  PRIM_LOD_FRAC + TEXEL0` blend is drawn in two GE passes (Dream Land's two
  ponds, the only ROM users). Pass 2 draws tile 1's image, through tile 1's
  own descriptor and `ScrU`/`ScrV` window, with fixed blend weights from the
  live fraction. The packer sets `LOD_BLEND` only where the lowering is exact
  (no GE blending, alpha gate provably inert); within one step of the RDP
  per channel (RE-321).

- `MaterialAnimator` holds one joint per `MatAnimDesc` (103 in v34); a
  fixed 64-slot array had left entries 64+ unticked (RE-322).
- Colour tracks (RE-322; only Race to the Finish uses them, 2 `PrimColor`,
  1 `Light1Color` + `Light2Color` script). The packer marks where the
  register still holds the animated `MObj`'s value (`PRIM_ANIM`,
  `LIGHT1_ANIM`, `LIGHT2_ANIM`); a later `G_SETPRIMCOLOR`/`G_MW_LIGHTCOL`
  clears it. `PrimColor` feeds the `TEXTURE_BLEND` constant and vertex
  alpha; its `TEXEL0 * PRIM` alpha blends under the task-list-1 XLU reset.
  Light tracks light that one primitive with the GE under the stage light
  (see [lighting.md](lighting.md)). Exact against a decomp reference
  (`romtool matcolors`).

Limitations:

- Every material track runs two ticks ahead of the decomp's frame count
  (constant scene-start phase, RE-322).
- Static task-list-1 primitives do not yet take the XLU reset (TODO.md).
