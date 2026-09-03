# Rendering: N64 → PSP

## The central decision

Smash 64 stores its geometry as **ready-made F3DEX2 display lists in the ROM**.
`objdisplay.c` never builds a mesh; it walks the DObj hierarchy, pushes
matrices, sets material state, then calls `gSPDisplayList(head++, dobj->dl)`
into ROM data.

That single fact determines the whole strategy:

* Star Fox 64 generates display lists dynamically, so `sf64-psp` has to
  translate them **at runtime**.
* Smash's are **static data**. We can translate them **at build time**, once,
  and ship PSP-native vertex buffers.

On a 333 MHz MIPS CPU that difference is large. Preconversion is the plan's
§9 instruction ("prefer preconversion over runtime conversion") and here it is
not merely preferable, it is nearly free.

We do **not** emulate the RDP.

## Pipeline

```
ROM (relocData file)
   │  ssb-rom::archive          ← VPK0 + relocation      [DONE, verified]
   ▼
decompressed asset bytes
   │  ssb-rom::dl               ← F3DEX2 command decode  [DONE, unit-tested]
   ▼
command stream + Vtx arrays
   │  converter (build time)    ← state tracking          [TODO]
   ▼
intermediate representation: mesh + material
   │  packer                    ← swizzle, PSM, index     [TODO]
   ▼
PSP vertex/index buffers + material records
   │  psp::gu                   ← sceGumDrawArray         [PARTIAL]
   ▼
GE
```

## Measured usage (`romtool scan`)

Plan §8: *"Measure actual Smash usage from the decompilation. Do not assume all
N64 rendering features are needed."*

Display lists are discovered by scanning every aligned offset and keeping the
ones that **convert cleanly** — a real list fills its own vertex cache before
drawing, so garbage fails almost immediately. Using relocation targets as
candidate list starts does *not* work: most reloc targets are the vertex-array
pointers carried by `G_VTX`, not list starts.

Result: **135 files, 1,864 display lists, 22,515 triangles, 0 conversion
failures.**

**Opcodes actually emitted:**

| Opcode | Count | | Opcode | Count |
|---|---:|---|---|---:|
| `G_TRI2` | 10954 | | `G_SETCOMBINE` | 1360 |
| `G_RDPPIPESYNC` | 6987 | | `G_LOADTLUT` | 1288 |
| `G_VTX` | 3918 | | `G_TEXTURE` | 1272 |
| `G_SETTILE` | 3483 | | `G_NOOP` | 1072 |
| `G_RDPLOADSYNC` | 2854 | | `G_RDPTILESYNC` | 776 |
| `G_SETTIMG` | 2549 | | `G_DL` | 680 |
| `G_SETTILESIZE` | 1804 | | `G_TRI1` | 607 |
| `G_GEOMETRYMODE` | 1766 | | `G_SETBLENDCOLOR` | 355 |
| `G_LOADBLOCK` | 1566 | | `G_SETPRIMCOLOR` | 209 |
| `G_SETOTHERMODE_H` | 1526 | | `G_SETENVCOLOR` | 92 |
| `G_SETOTHERMODE_L` | 1415 | | `G_SETFOGCOLOR` | 2 |

**Never emitted** — not worth implementing: `G_QUAD`, `G_CULLDL`, `G_BRANCH_Z`,
`G_MODIFYVTX`, `G_TEXRECT`, `G_FILLRECT`, `G_LOADTILE`, `G_SETSCISSOR`,
`G_MOVEMEM`, `G_MOVEWORD`.

`G_TRI2` outnumbers `G_TRI1` 18:1, so geometry is overwhelmingly paired
triangles. `G_SETFOGCOLOR` appears twice in the entire game — **fog is
effectively unused** and should not cost anything at runtime. Verified
more than just rare (RE-072): no `gSPFogPosition` call exists anywhere in
the decompilation to give a fog range meaning, and the one real stage
that sets a fog colour never references it from its own render mode.

**Texture formats**, by `G_SETTILE` count:

| Format | Count | PSP destination |
|---|---:|---|
| **CI4** | 1192 | `PsmT4` + 16-entry CLUT — the dominant case |
| RGBA16 | 92 | `Psm5551` |
| IA16 | 83 | expand to `Psm8888` |
| CI8 / I4 / I8 / IA8 / RGBA32 | few | `PsmT8` / `Psm8888` |

TLUT loads are overwhelmingly **16 entries**. So the common case is a CI4
texture with a 16-colour palette — 4 bits per texel, natively supported by the
PSP. That matters because only ~700 KiB of VRAM is left after framebuffers
(`docs/memory.md`).

Counts against `Ci 16bpp` and `Rgba 4bpp` are tile *descriptors* used to stage
TLUT loads, not real texture formats — CI is only ever 4- or 8-bit.

**Geometry modes set:** `G_LIGHTING`, `G_SHADING_SMOOTH`, `G_CULL_BACK`,
`G_CULL_FRONT`, `G_ZBUFFER`. No `G_FOG`.

Two hardware invariants are used as validity tests, and both earn their keep:
the vertex cache holds at most 32 entries, and triangle indices must fall
within it. Before those checks the scan reported an impossible 160-vertex
`G_VTX`.

## Geometry conversion results

`romtool mesh` converts every root display list into indexed meshes:

```
display lists converted  1768      (0 failures)
triangles                25562
triangle corners         76686
unique vertices          36693
vertex reuse             2.09x
draw calls after merge   2483
textured draws           1280

geometry memory
  triangle soup, float     1797.3 KiB
  triangle soup, 16-bit     898.7 KiB
  indexed, 16-bit           579.8 KiB
  saving vs float soup         67.7%
```

Three compounding wins, all paid for at build time:

1. **Indexing** — the RSP re-uploads shared vertices because its cache holds
   only 32. Undoing that gives 2.09x reuse.
2. **16-bit vertex components** — N64 positions are already `i16` and UVs are
   S10.5, so they map onto `GU_VERTEX_16BIT` / `GU_TEXTURE_16BIT` directly.
   12 bytes per vertex instead of 24. Converting to `f32` would double vertex
   bandwidth for no fidelity gain, on a machine that is bandwidth-bound.
3. **Material merging** — primitives sharing a material are merged into single
   draws, because GE state changes cost far more than draw calls.

All the game's geometry fits in **580 KiB**, comfortable against 32 MiB of main
RAM.

These are *root-list* figures and are deliberately smaller than the pack's
(2722 meshes, 47,696 triangles). This command converts each root display list
on its own; the packer walks each scene graph in draw order through one shared
vertex cache, which reaches the continuation lists a standalone conversion
cannot enter (RE-025, RE-026). Neither number is wrong — they measure different
traversals, and the gap between them is the scene-graph work.

## Texture conversion results

`romtool textures` extracts every texture the display lists actually bind and
packs it for the GE. Current output (`cargo run --release -p romtool --
textures "rom/Super Smash Bros. (USA).z64"`):

```
unique textures bound  665
packed                 638
failed                  27
  decode: MissingPalette          1
  segmented addr (seg 0x01)      26
note: CI texture, no TLUT recorded  1  (packs successfully; informational)
swizzled               432 (68%)

by PSP format:
  Psm8888     67 textures      641.1 KiB
  PsmT4      549 textures      442.5 KiB
  PsmT8       22 textures       87.4 KiB

VRAM budget
  packed (chosen formats)     1170.9 KiB
  naive, all RGBA8888         2432.8 KiB
  saving                        51.9%
  fits in ~700 KiB texture VRAM? no — needs streaming (1.7x over)
```

**51.9% saved** by keeping paletted textures paletted, down from 68.6%
before RE-067's mirror-texture fix (763.2 KiB) and 56.5% before RE-070's
targeted dither-blur (1059.0 KiB) — both spend real bytes converting
specific textures to `Psm8888` for correctness, on top of the format
choice below. `PsmT4` still carries 549 of 638 packed textures; expanding
those to RGBA8888 would cost eight times as much and blow the VRAM budget
far worse.

`unique textures bound` rose from 647 to 665 and `packed` from 617 to 638
this session (R0.7, RE-059/RE-060): resolving two files' `MObj` material
tables didn't just fix palettes on textures that were already bound —
several primitives had no texture binding at all before (their `SetTimg`
was wiped by an unresolved `forget_texture()` call), and now correctly
resolve one.

Two rules drive the packing:

* **Keep CI4/CI8 paletted.** The PSP has native CLUT support, so the dominant
  N64 format converts at 4 bits per texel with a 16-entry CLUT.
* **Keep I4/I8 paletted too**, against a greyscale CLUT. They are intensity
  ramps, so a palette is exact rather than lossy, and avoids an 8x expansion.

IA and RGBA32 have no PSP equivalent and expand to `Psm8888`.

### Swizzling

57% of packed textures are swizzled. The GE reads through a cache organised in
16-byte by 8-row blocks; storing texels linearly makes each cache line span one
row, so vertical locality is lost. Swizzling reorders texels so each block is
contiguous. Textures whose rows are under 16 bytes cannot be swizzled and are
left linear rather than padded, which would waste more than it saves.

### A texture is named by a file *and* an offset

A display list does not always draw from its own file. A stage's geometry is in
one archive file and its texels in another, reached by a pointer the archive
records as an extern relocation rather than applying — so the address word in
the list reads as zero.

For a long time that was indistinguishable from "this primitive has no
texture", and every stage in the game rendered as a white silhouette. It is
resolved by keying on the address word's own offset, which is what the
relocation is filed under (RE-037): `Cmd::SetTimg` carries that offset,
`mesh::Source` carries the relocations, and `TextureRef` names a file for its
texels and its palette independently — a fighter's palette is in its own file
while a stage's texels are not.

### The 763 KiB figure needs streaming

Only ~700 KiB of VRAM remains after the two framebuffers and the depth buffer
(`docs/memory.md`), so the full texture set does **not** fit at once — it is
1.1x over. This is not a problem in practice: a match needs one stage and up to
four fighters, not every texture in the game. But it does mean texture
residency must be **per-scene**, and that is a known requirement to be
addressed before rendering completeness (`PLAN.md` R0.3/R1), not a surprise
discovered late.

### Remaining unconverted (27 of 665, per current `romtool textures`)

| Reason | Count |
|---|---:|
| segmented address (segment 0x01) | 26 |
| `MissingPalette` at decode | 1 |

Separately, 1 more texture packs successfully but is flagged "CI texture, no
TLUT recorded" — informational, not a failure.

The 26 segment-0x01 entries are not missing texture data at all: RE-055
(`docs/reverse-engineering.md`) traces them to `sLBTransitionPhotoHeap`, a
runtime per-frame copy of the framebuffer that the loading-break ("LB")
transition system binds to RSP segment 1 once per frame
(`refs/ssb-decomp-re/src/lb/lbtransition.c`). That data never exists in any
ROM file, so no texture converter can produce it; a real implementation
belongs to framebuffer effects (`PLAN.md` R0.13), not this converter. The
`MissingPalette` case is not a decode bug either: RE-057 originally traced 4
such failures to three files (`MVCommon`, `ITCommonObject`, `LinkSpecial2`)
whose scene graphs get no — or only partial — `MObj` material-table
pairing, which causes a real, present palette load elsewhere in the same
file to get dropped when an unrelated, unresolvable material call
intervenes. `LinkSpecial2` (RE-059, a third record shape, `EFDesc`, living
outside the archive and hand-entered) and `MVCommon` (RE-060, a fourth
mechanism — no struct at all, just a code call sequence, also hand-entered)
are now fully fixed. Only `ITCommonObject`'s one remaining graph (a fifth
mechanism — a byte-offset delta from a runtime pointer, not yet traced) and
`LinkSpecial2`'s already-fixed file's third graph (a `WPAttributes`-named
Spin Attack model, RE-058, untyped in the decompilation) are still open.
All of it belongs to `PLAN.md` R0.7 (missing material tables), not this
converter. Earlier passes over this data also reported null-address and
out-of-file failure classes; neither appears in the current `romtool
textures` output, so treat them as resolved until a re-run shows otherwise.

## Display list translation

F3DEX2's model maps cleanly onto an indexed draw:

| F3DEX2 | Meaning | PSP |
|---|---|---|
| `G_VTX(count, dest, addr)` | load `count` vertices into the 32-entry cache at `dest` | append to a vertex buffer, remember the cache mapping |
| `G_TRI1(a,b,c)` | triangle from cache indices | three indices |
| `G_TRI2` | two triangles | six indices |
| `G_DL` (call/branch) | invoke another list | inline during conversion |
| `G_MTX` / `G_POPMTX` | modelview stack | becomes the DObj hierarchy transform |
| `G_SETTIMG`/`G_SETTILE`/`G_LOADBLOCK` | texture load | one converted texture, referenced by id |
| `G_SETPRIMCOLOR`/`G_SETENVCOLOR` | combiner inputs | material fields |
| `G_GEOMETRYMODE` | lighting/cull/fog bits | material flags |

Two F3DEX2 encoding traps, both handled in `ssb-rom/src/dl.rs`:

* **Indices are stored doubled.** `G_TRI1` holds `index * 2`; likewise `G_VTX`'s
  destination. Halve on decode.
* **`G_MTX`'s parameter byte is inverted** relative to F3DEX.

## Textures

N64 formats and their PSP destinations:

| N64 | Bits | PSP `TexturePixelFormat` | Notes |
|---|---|---|---|
| RGBA16 (5551) | 16 | `Psm5551` | direct, channel order differs |
| RGBA32 | 32 | `Psm8888` | direct |
| IA16 | 16 | `Psm8888` | expand; PSP has no IA format |
| IA8 / IA4 | 8 / 4 | `Psm8888` or `PsmT8` | expand or palettize |
| I8 / I4 | 8 / 4 | `PsmT8` / `PsmT4` + grey CLUT | intensity drives alpha too |
| CI8 / CI4 | 8 / 4 | `PsmT8` / `PsmT4` | **best case** — PSP has native CLUT support |

CI4/CI8 are the important row: Smash uses them heavily (confirmed by the
`gDPLoadTLUTCmd(..., siz == 8b ? 0xFF : 0xF)` path in `objdisplay.c`), and the
PSP supports paletted textures natively. Those convert almost 1:1 and stay
small in VRAM.

`ssb-rom::texture` decodes to RGBA8888 as a neutral intermediate, and
`ssb-rom::psp_texture` packs from there to the PSM chosen above — including
the swizzle. Both are unit-tested and confirmed on device (RE-022).

### Not yet handled

* Mipmap chains are generated at build time for 151+ textures
  (`psp_texture::pack_mipped`), but generating them did **not** fully resolve
  the Dream Land canopy discrepancy (RE-053) — a diagonal pattern survived
  and sharpened at higher resolution, which points at texture
  *magnification* behaviour rather than minification/LOD selection. RE-067
  found and fixed one real, contributing cause (a missing `G_TX_MIRROR`
  reproduction, see below); RE-070 tested RE-053's own two suggested fixes
  for the dither directly and found filtering alone insufficient (measured
  on-device) but pre-blurring the two canopy textures and packing them
  unquantized (`Psm8888`) measurably softens the dither (~40% less
  adjacent-pixel noise on the treated textures) without fully resolving it
  — `PLAN.md` R0.5's open acceptance criterion stays open, now with real
  progress and numbers behind it rather than an untried lead.
* `G_TX_MIRROR` is now reproduced exactly rather than approximated (RE-067):
  since the PSP GE has no native mirror wrap mode (`sceGuTexWrap` is
  `Repeat`/`Clamp` only), `romtool`'s texture conversion pre-bakes a
  mirrored copy of the decoded image on each mirrored axis before packing
  — a real fix, not a heuristic, since pack-time conversion has full
  control of the pixel data and `sceGuTexScale` already renormalises UVs
  against whatever dimensions a packed texture reports. Traced to Dream
  Land's canopy specifically (`file 104` offset `0x798` sets `cm_s=3 cm_t=3
  mask_s=6 mask_t=6` — mirror+clamp, 64-texel period) and confirmed via a
  reversible on-device experiment that the un-mirrored repeat boundary was
  visibly wrong. Costs real VRAM: 187 of 638 packed textures (29%) carry
  the flag on at least one axis, raising packed texture VRAM from 763.2 KiB
  to 1059.0 KiB (+39%). `G_TX_CLAMP`, by contrast, is *not* a gap:
  `psp/src/meshdraw.rs` hardcodes `sceGuTexWrap(Repeat, Repeat)` for every
  draw, and RE-066 measured that every clamp-flagged tile-0 axis
  archive-wide is also a masked (periodic) one, so the existing
  mask-narrowed-width `Repeat` (RE-044) already reproduces real hardware's
  addressing exactly.

## Coordinate handling

See `docs/reverse-engineering.md` RE-004 and RE-005. Summary:

* **No handedness flip.** Both systems are right-handed, `+Y` up, view down `-Z`.
* **No matrix transpose.** N64 row-major/row-vector and PSP
  column-major/column-vector cancel out; only the s15.16 → `f32` widening is
  real work.
* **UVs** are S10.5 fixed point: divide by 32 for texels, then by the texture
  dimension to normalize.
* **Aspect ratio.** The game renders 320x240 (4:3); the PSP is 480x272.
  Stretching would distort every character, so the default is a pillarboxed
  362x272 viewport, centred. `coord::pillarboxed_viewport()` computes it, and
  `Gpu::init` must apply it to **both** `sceGuViewport` and `sceGuScissor` --
  feeding its aspect to the projection while leaving the GE viewport at the
  full 480 stretches the image by 480/362 = 1.33x, which is exactly the
  distortion the pillarbox exists to prevent (RE-034).

## Depth

The PSP's depth buffer is **inverted** relative to the intuitive setup: near
maps to 65535 and far to 0, so `sceGuDepthRange(65535, 0)` pairs with
`DepthFunc::GreaterOrEqual`. This is already set up in `psp/src/gu.rs` and is a
classic source of "everything renders in the wrong order" bugs.

## Renderer evolution

Per plan §32:

* **Renderer 0** — triangle testbed. ✅ Done; runs at a locked 60 FPS.
* **Renderer 1** — SSB stage geometry. ✅ Done. Stages render from the pack
  with their collision polylines overlaid (`docs/images/m4-stage-collision.png`).
* **Renderer 2** — complete materials/textures. 🟢 Mostly. Textures, CLUTs,
  per-node baked matrices and recovered `MObj` palettes all draw on device,
  stages included (`docs/images/m4-stage-textured.png`). Outstanding: the
  majority-vote lighting heuristic, the `MObj` fields listed in RE-010, and the
  30 textures that still fail to convert.
* **Renderer 3** — transparency, fog, particles, shadows, UI. Not started. The
  debug overlay is still `sceGuDebugFlush` rather than GE geometry, which is
  why it needs the software rasteriser (RE-014).
* **Renderer 4** — batching, state sorting, caching. **Not before the game is
  visibly running.** Primitives are already merged by material at build time,
  which is the build-time half of the same idea.

## Vertex layout

The GE reads vertex attributes in a fixed order and the `VertexType` flags must
describe exactly that order. `GuVertex` is therefore
`{ u, v, color, x, y, z }` — texture coords, then colour, then position.
Reordering those fields renders garbage with no error. It must also be
16-byte aligned for GE DMA, which is why vertex buffers are wrapped in
`psp::Align16`.
