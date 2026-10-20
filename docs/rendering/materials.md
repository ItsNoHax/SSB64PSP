# Materials

Part of [rendering.md](../rendering.md).

## Combiner

`mesh.rs`: evaluates the general `(A-B)*C+D` combiner, classifies shapes the GE can express.

| Shape | GE lowering | Evidence |
|---|---|---|
| Shade scale | Folded into vertex colour | RE-039, RE-043, RE-106 |
| `(PRIM-ENV)*TEXEL+ENV` texture blend | `TextureEffect::Blend` (`TEXTURE_BLEND` flag) | RE-073, RE-074 |
| Flat constant colour | Constant vertex colour | RE-080 |

- Unsupported-equation visits: RE-168.
- `G_MOVEWORD`'s `G_MW_LIGHTCOL` index: segment relies on externally enabled `G_LIGHTING`.
  - Drives lit/literal vertex classification (RE-105).
- `G_SHADE` always cleared together with `G_LIGHTING` and `G_SHADING_SMOOTH`.
  - Result: flat `PRIMITIVE`-driven output; classified shapes reproduce it.
- A few primitives also read `SHADE` in the combiner.
  - Real-hardware output undocumented; left undecided (RE-119, RE-120).

## `MObj` display state

- All real `MObjMaterial`s measured.
- Implemented: default substitution, texture scale, tile-0 window state.
- `MOBJ_FLAG_FRAC` never occurs in static content.
- Tile-1 scroll inert: every input equals tile 0, no `TEXEL1` consumer (RE-194).

## Material animation

Tracks: palette, frame, tile-0 UV, two-tile blend, `PrimColor`, `Light1Color`, `Light2Color`.

### UV and tile window

- Each animated `MObj`'s rest `TraU`/`TraV`/`ScaU`/`ScaV` and tile parameters are packed.
- Runtime applies `gcDrawMObjForDObj`'s tile-window delta through GE texture mapping (RE-301).
- `MaterialUv::ge_affine` reproduces:
  - RDP's truncated quarter-texel origin.
  - 12-bit field.
  - Uploaded dimension.
- Every packed vertex lands on the RDP's texel for tile 0 and tile 1 (RE-326).
- Packed UVs have the rest origin removed on every axis.
  - Affine restores it before applying live scale and origin, repeating axes included (RE-359).
- Linear-filter `+0.5 / uploaded_dim` added after the animated transform → animation never scales it (RE-304).

### Texture and palette ownership

- `TextureIDCurrent` selects a packed texture; `PaletteID` selects its CLUT.
- Each applies only while the primitive's `MObj` still owns that state (`IMAGE_ANIM`, `PALETTE_ANIM`, `TILE0_ANIM`, `SCALE_ANIM`).
  - A later display list loading its own image, TLUT, tile window or `G_TEXTURE` keeps it.
- Palettes resolved per primitive, not per texture (RE-326).
- Cache identity: `(file, script, MObjSub)` plus live affine transform → no state leaks between materials.

### `SetLFrac` / `TextureIDNext`

- Two-cycle `(TEXEL1 - TEXEL0) * PRIM_LOD_FRAC + TEXEL0` blend drawn in two GE passes.
  - Only ROM users: Dream Land's two ponds.
- Pass 2: tile 1's image, through tile 1's own descriptor and `ScrU`/`ScrV` window, fixed blend weights from the live fraction.
- Packer sets `LOD_BLEND` only where lowering is exact (no GE blending, alpha gate provably inert).
- Accuracy: within one step of the RDP per channel (RE-321).

### `MaterialAnimator`

- One joint per `MatAnimDesc`; no fixed slot cap (RE-322).
- Tick `n` = decomp frame `n`.
  - First parse keeps `AOBJ_ANIM_CHANGED`'s clock.
  - Keys start at `length = -anim_wait - anim_speed` (RE-324).
- All tracks and texture, palette, tile-0, two-tile-blend resolvers match a decomp reference bit for bit (`romtool matcolors`).
- Both index resolvers truncate, accept every live kind (RE-325).
- Every bound texture holds its `sprites[]` image's texels; every owned CLUT equals `palettes[0]` (RE-326).

### Colour tracks (RE-322)

- Only Race to the Finish uses them.
- Packer marks where the register still holds the animated `MObj`'s value: `PRIM_ANIM`, `LIGHT1_ANIM`, `LIGHT2_ANIM`.
  - Later `G_SETPRIMCOLOR`/`G_MW_LIGHTCOL` clears it.
- `PrimColor` feeds the `TEXTURE_BLEND` constant and vertex alpha.
  - Its `TEXEL0 * PRIM` alpha blends under the task-list-1 XLU reset (RE-323 applies that reset to every stage list-1 primitive).
- Light tracks light that one primitive with the GE under the stage light: [lighting.md](lighting.md).

### Item status materials (RE-443)

- Poké Ball throw materials and Goldeen, Charizard, Beedrill, Starmie status materials bind to the exact MObj the status callback selects.
- Runtime starts each script on its recorded item clock.
- Conditional Snorlax/Clefairy lists get the callback's AA_XLU_SURF seed before their own commands.
- Normal Clefairy gets AA_ZB_TEX_EDGE.
- Display-link move alone does not make Starmie or Hitmonlee translucent.

## Limitation

- Camera-level head-1 XLU reset applies to every packed graph's head-1 stream (RE-328).
  - Exceptions: weapons (`wpDisplayDrawNormal`'s own reset, RE-379); link-15/18 effects (`efDisplayCLDProcDisplay`'s no-depth state, RE-421).
- Runtime-effect commands outside those graphs need their own ordered state if they change render mode.
