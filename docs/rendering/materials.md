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
  through the GE texture mapping (RE-301). `MaterialUv::ge_affine`
  reproduces the RDP's truncated quarter-texel origin, its 12-bit field and
  the uploaded dimension; every packed vertex lands on the RDP's texel for
  tile 0 and tile 1 (RE-326).
- `TextureIDCurrent` selects a packed texture; `PaletteID` selects its CLUT.
  Each applies only while the primitive's `MObj` still owns that state
  (`IMAGE_ANIM`, `PALETTE_ANIM`, `TILE0_ANIM`, `SCALE_ANIM`): a later
  display list that loads its own image, TLUT, tile window or `G_TEXTURE`
  keeps it. Palettes are resolved per primitive, not per texture (RE-326).
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

- `MaterialAnimator` holds one joint per `MatAnimDesc` (103 in v37); a
  fixed 64-slot array had left entries 64+ unticked (RE-322). Tick `n` is
  the decomp's frame `n`: the first parse keeps `AOBJ_ANIM_CHANGED`'s
  clock and keys start at `length = -anim_wait - anim_speed` (RE-324).
  All fifteen tracks of all 103 entries match a decomp reference bit for
  bit over 600 frames (`romtool matcolors`). So do the texture, palette,
  tile-0 and two-tile-blend resolvers against `gcDrawMObjForDObj`. Both
  index resolvers truncate and accept every live kind: a linear
  `PaletteID` hold selects its palette (RE-325). Every bound texture holds
  its `sprites[]` image's texels and every owned CLUT equals `palettes[0]`
  (RE-326).
- Colour tracks (RE-322; only Race to the Finish uses them, 2 `PrimColor`,
  1 `Light1Color` + `Light2Color` script). The packer marks where the
  register still holds the animated `MObj`'s value (`PRIM_ANIM`,
  `LIGHT1_ANIM`, `LIGHT2_ANIM`); a later `G_SETPRIMCOLOR`/`G_MW_LIGHTCOL`
  clears it. `PrimColor` feeds the `TEXTURE_BLEND` constant and vertex
  alpha; its `TEXEL0 * PRIM` alpha blends under the task-list-1 XLU reset
  (RE-323 applies that reset to every stage list-1 primitive).
  Light tracks light that one primitive with the GE under the stage light
  (see [lighting.md](lighting.md)). Exact against a decomp reference
  (`romtool matcolors`).

Limitations:

- The camera-level head-1 XLU reset is applied to every packed graph's
  head-1 stream (RE-328). Commands emitted by runtime effects outside those
  graphs still need their own ordered state if they change render mode.
