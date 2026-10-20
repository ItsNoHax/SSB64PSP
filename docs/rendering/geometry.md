# Geometry

Part of [rendering.md](../rendering.md).

## Display-list translation

| F3DEX2 | Meaning | PSP |
|---|---|---|
| `G_VTX(count, dest, addr)` | Load into the 32-entry vertex cache | Append to vertex buffer, record cache mapping |
| `G_MODIFYVTX` ST | Rewrite cached texture coordinates | Replace the cached S10.5 pair, preserve its loading space |
| `G_TRI1` / `G_TRI2` | 1 / 2 triangles from cache indices | 3 / 6 indices |
| `G_DL` | Call or branch | Inlined at conversion |
| `G_MTX` / `G_POPMTX` | Modelview stack | `DObj` hierarchy transform |
| `G_SETTIMG` / `G_SETTILE` / `G_LOADBLOCK` | Texture load | Converted texture, referenced by id |
| `G_SETPRIMCOLOR` / `G_SETENVCOLOR` | Combiner inputs | Material fields |
| `G_GEOMETRYMODE` | Lighting, cull, texgen bits | Material flags |

Encoding traps handled in `ssb-rom/src/dl.rs`:

- `G_TRI*` indices and `G_VTX` destinations are stored doubled.
- `G_MTX`'s parameter byte is inverted relative to F3DEX.

## Vertex layout

- `GuVertex` = `{ u, v, color, x, y, z }`: order the GE reads for the chosen `VertexType` flags.
  - Reordering renders garbage without an error.
- Buffers 16-byte aligned (`psp::Align16`).
- Positions `i16`.
- UVs: `GU_TEXTURE_16BIT` normally.
  - GE reads that field unsigned → primitives with negative UVs on a clamped axis are flagged, submitted with float UVs (RE-262).
- Common path: 20 bytes per vertex.

## Coordinates

- No handedness flip: both systems are right-handed, `+Y` up, looking down
  `-Z` (RE-005).
- No matrix transpose: N64 row-major/row-vector and PSP column-major/
  column-vector cancel; only s15.16 → `f32` widening remains (RE-004).
- UVs S10.5: divide by 32 for texels, then by the uploaded dimension.
  - Linear filtering adds `+0.5 / uploaded_dim` after all tile-origin and material transforms.
  - Point sampling adds nothing (RE-304).

### Screen mapping ([D-047](../decisions/D-047.md), RE-477)

- N64 visible box (10,10)–(310,230) fills the PSP's 272 lines at one scale, 272/220, centred.
  - PSP x = 240 + (x − 160) · 272/220
  - PSP y = 136 + (y − 120) · 272/220
- Box covers PSP columns 55–424.
- 10-pixel strip around it cropped (a CRT's overscan hid it).
- `ssb_engine::coord` is the only converter.
  - Every viewport, scissor, sprite, fill, particle, mask, photo copy goes through it.
  - Every scissor clipped to the visible area (libgc clamps each camera's scissor 10 pixels inside the frame).
- `Gpu::set_viewport_n64` sends the viewport as floats.

### Battle camera

- Inset (10,10)–(310,230) viewport, aspect 15/11, live FOV.
- Frames by that aspect, as `gGMCameraStruct.viewport_width / viewport_height` does.
- Shared by models, particles, portable projection; tags use the same scissor.
- Entry and pause zooms follow their live FOV (RE-440).

## Conversion

- Build-time wins:
  - Indexing undoes the RSP's 32-entry cache re-uploads.
  - 16-bit components halve vertex size.
  - Adjacent primitives with the same material merge into one draw, no reordering (RE-252).
- Packer walks each scene graph through one shared vertex cache.
  - Reaches continuation lists a standalone `romtool mesh` conversion cannot (RE-025, RE-026).

### Borrowed RSP cache vertices (RE-358)

- Retain loading joint and original local position.
- Posed drawing:
  1. Reconstruct each borrowed position under the loading joint's animated matrix.
  2. Express it in the drawing joint's space.
  3. Normals follow the same source matrix through the relative inverse transpose.
- Static drawing keeps the baked rest rebase.
- Integer positions and signed-byte normals round at the GE submission boundary.

### `romtool scan`

- Converts every aligned offset; keeps those that convert cleanly (a real list fills its cache before drawing).
- Relocation targets are not list starts; most point to vertex arrays.
- Opcode census: RE-119.

### `G_MODIFYVTX`

- Controller flower lists and fighter continuations use ST writes (RE-357).
- Overwrites already-scaled cached coordinates; `G_TEXTURE` scale not applied again.
- Other modify-vertex fields not translated.

### Absent commands

- Absent from the RE-119 census: `G_QUAD`, `G_CULLDL`, `G_BRANCH_Z`, `G_TEXRECT`, `G_FILLRECT`, `G_SETSCISSOR`, `G_MOVEMEM`.
- `G_LOADTILE` occurs later: [textures.md](textures.md).

### Fog

- Unused: no `gSPFogPosition` exists; the one stage that sets a fog colour never references it ([D-025](../decisions/D-025.md), RE-072).

## Evidence by area

| Area | Evidence |
|---|---|
| Geometry and transforms, billboard kinds 44/46/48/50, signed scale | RE-062, RE-063, RE-143–145 |
| Projection: original ROM camera state, inset viewport, live FOV, overscan crop (D-047) | RE-034, RE-082–085, RE-131, RE-150, RE-151, RE-440, RE-477 |
| Culling: RDP default `CULL_BACK` | RE-068 |
