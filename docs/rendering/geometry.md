# Geometry

Part of [rendering.md](../rendering.md).

## Display-list translation

| F3DEX2 | Meaning | PSP |
|---|---|---|
| `G_VTX(count, dest, addr)` | Load into the 32-entry vertex cache | Append to vertex buffer, record cache mapping |
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

`GuVertex` is `{ u, v, color, x, y, z }`, the order the GE reads for the
chosen `VertexType` flags. Reordering renders garbage without an error.
Buffers are 16-byte aligned (`psp::Align16`).

Positions are `i16`. UVs normally use `GU_TEXTURE_16BIT`, but the GE reads
that field unsigned, so primitives with negative UVs on a clamped axis are
flagged and submitted with float UVs (RE-262). Common path: 20 bytes per
vertex.

## Coordinates

- No handedness flip: both systems are right-handed, `+Y` up, looking down
  `-Z` (RE-005).
- No matrix transpose: N64 row-major/row-vector and PSP column-major/
  column-vector cancel; only s15.16 → `f32` widening remains (RE-004).
- UVs are S10.5: divide by 32 for texels, then by the uploaded dimension.
  Linear filtering adds `+0.5 / uploaded_dim` after all tile-origin and
  material transforms; point sampling adds nothing (RE-304).
- The game renders 4:3 at 320×240. The port uses a centred, pillarboxed
  362×272 viewport, applied to both `sceGuViewport` and `sceGuScissor`
  (`coord::pillarboxed_viewport`, RE-034, [D-008](../decisions/D-008.md)).

## Conversion results

`romtool mesh` (root display lists only):

| Metric | Value |
|---|---|
| Display lists | 1,768 (0 failures) |
| Triangles | 25,562 |
| Unique vertices | 36,693 (2.09× reuse) |
| Draw calls after merge | 2,483 |
| Indexed 16-bit geometry | 579.8 KiB (−67.7% vs float soup) |

The pack is larger (2,722 meshes, 47,696 triangles) because the packer walks
each scene graph through one shared vertex cache and reaches continuation
lists a standalone conversion cannot (RE-025, RE-026).

Build-time wins: indexing undoes the RSP's 32-entry cache re-uploads, 16-bit
components halve vertex size, and adjacent primitives with the same material
merge into one draw without reordering (RE-252).

## Measured usage

`romtool scan` finds display lists by converting every aligned offset and
keeping those that convert cleanly (a real list fills its cache before
drawing). Relocation targets are not usable as list starts; most point to
vertex arrays. Result: 135 files, 1,864 lists, 0 failures.

Emitted opcodes (RE-119):

| Opcode | Count | Opcode | Count |
|---|---:|---|---:|
| `G_TRI2` | 13,523 | `G_LOADTLUT` | 1,558 |
| `G_RDPPIPESYNC` | 9,015 | `G_NOOP` | 1,484 |
| `G_VTX` | 4,756 | `G_SETOTHERMODE_L` | 1,444 |
| `G_SETTILE` | 4,327 | `G_TRI1` | 1,043 |
| `G_MOVEWORD` | 3,722 | `G_RDPTILESYNC` | 1,006 |
| `G_RDPLOADSYNC` | 3,445 | `G_DL` | 826 |
| `G_SETTIMG` | 3,074 | `G_SETBLENDCOLOR` | 366 |
| `G_SETTILESIZE` | 2,218 | `G_SETPRIMCOLOR` | 246 |
| `G_SETOTHERMODE_H` | 1,954 | `G_SETENVCOLOR` | 95 |
| `G_GEOMETRYMODE` | 1,894 | `G_RDPFULLSYNC` | 16 |
| `G_LOADBLOCK` | 1,886 | `G_SETFOGCOLOR` | 3 |
| `G_ENDDL` | 1,864 | | |
| `G_SETCOMBINE` | 1,660 | | |
| `G_TEXTURE` | 1,596 | | |

Never emitted: `G_QUAD`, `G_CULLDL`, `G_BRANCH_Z`, `G_MODIFYVTX`, `G_TEXRECT`,
`G_FILLRECT`, `G_LOADTILE`, `G_SETSCISSOR`, `G_MOVEMEM`.

Geometry-mode set counts: `G_LIGHTING` 539, `G_SHADING_SMOOTH` 449,
`G_CULL_BACK` 399, `G_TEXTURE_GEN` 156, `G_SHADE` 60, `G_ZBUFFER` 19,
`G_TEXTURE_GEN_LINEAR` 13, `G_CULL_FRONT` 5.

Fog is unused: no `gSPFogPosition` exists and the one stage that sets a fog
colour never references it ([D-025](../decisions/D-025.md), RE-072).

## Status

| Area | Status | Evidence | Remaining |
|---|---|---|---|
| Geometry and transforms | Complete; billboard kinds 44/46/48/50 and signed scale | RE-062, RE-063, RE-143–145 | Physical-PSP recheck |
| Projection | Complete; battle camera matches original ROM state | RE-034, RE-082–085, RE-131, RE-150, RE-151 | Special camera modes (gameplay) |
| Culling | Complete; RDP default `CULL_BACK` | RE-068 | — |
