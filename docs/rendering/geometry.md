# Geometry

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

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


## Vertex layout

The GE reads vertex attributes in a fixed order and the `VertexType` flags must
describe exactly that order. `GuVertex` is therefore
`{ u, v, color, x, y, z }` — texture coords, then colour, then position.
Reordering those fields renders garbage with no error. It must also be
16-byte aligned for GE DMA, which is why vertex buffers are wrapped in
`psp::Align16`.


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
2. **16-bit vertex components** — N64 positions are already `i16`, and signed
   S10.5 UVs can normally use `GU_TEXTURE_16BIT`. The GE decodes that texture
   field as unsigned, though: a negative coordinate on a clamped axis would
   jump to the far edge. RE-262 marks only those primitives and expands their
   indexed corners transiently to float UVs. The common path stays at 20 bytes
   per vertex instead of 36 without sacrificing signed clamp semantics.
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


## Measured usage (`romtool scan`)

Plan §8: *"Measure actual Smash usage from the decompilation. Do not assume all
N64 rendering features are needed."*

Display lists are discovered by scanning every aligned offset and keeping the
ones that **convert cleanly** — a real list fills its own vertex cache before
drawing, so garbage fails almost immediately. Using relocation targets as
candidate list starts does *not* work: most reloc targets are the vertex-array
pointers carried by `G_VTX`, not list starts.

Result: **135 files, 1,864 display lists, 28,089 triangles, 0 conversion
failures.**


**Opcodes actually emitted** (re-measured for R0.16/RE-119; the previous
table predated several conversion-fidelity fixes — e.g. RE-093/RE-094's
texture-state corrections — that changed how many triangles the same
1,864 lists parse into, and predated RE-105's discovery that
`G_MOVEWORD` is genuinely load-bearing, not unused):

| Opcode | Count | | Opcode | Count |
|---|---:|---|---|---:|
| `G_TRI2` | 13523 | | `G_LOADTLUT` | 1558 |
| `G_RDPPIPESYNC` | 9015 | | `G_NOOP` | 1484 |
| `G_VTX` | 4756 | | `G_SETOTHERMODE_L` | 1444 |
| `G_SETTILE` | 4327 | | `G_TRI1` | 1043 |
| `G_MOVEWORD` | 3722 | | `G_RDPTILESYNC` | 1006 |
| `G_RDPLOADSYNC` | 3445 | | `G_DL` | 826 |
| `G_SETTIMG` | 3074 | | `G_SETBLENDCOLOR` | 366 |
| `G_SETTILESIZE` | 2218 | | `G_SETPRIMCOLOR` | 246 |
| `G_SETOTHERMODE_H` | 1954 | | `G_SETENVCOLOR` | 95 |
| `G_GEOMETRYMODE` | 1894 | | `G_RDPFULLSYNC` | 16 |
| `G_LOADBLOCK` | 1886 | | `G_SETFOGCOLOR` | 3 |
| `G_ENDDL` | 1864 | | | |
| `G_SETCOMBINE` | 1660 | | | |
| `G_TEXTURE` | 1596 | | | |


**Never emitted** — not worth implementing: `G_QUAD`, `G_CULLDL`, `G_BRANCH_Z`,
`G_MODIFYVTX`, `G_TEXRECT`, `G_FILLRECT`, `G_LOADTILE`, `G_SETSCISSOR`,
`G_MOVEMEM`.

`G_TRI2` outnumbers `G_TRI1` 18:1, so geometry is overwhelmingly paired
triangles. `G_SETFOGCOLOR` appears twice in the entire game — **fog is
effectively unused** and should not cost anything at runtime. Verified
more than just rare (RE-072): no `gSPFogPosition` call exists anywhere in
the decompilation to give a fog range meaning, and the one real stage
that sets a fog colour never references it from its own render mode.


**Geometry modes set:** `G_LIGHTING` (539), `G_SHADING_SMOOTH` (449),
`G_CULL_BACK` (399), `G_TEXTURE_GEN` (156), `G_SHADE` (60),
`G_ZBUFFER` (19), `G_TEXTURE_GEN_LINEAR` (13), `G_CULL_FRONT` (5). No
`G_FOG`. Counts are `set`-mask occurrences (`romtool scan`), not net
per-primitive state.

Two hardware invariants are used as validity tests, and both earn their keep:
the vertex cache holds at most 32 entries, and triangle indices must fall
within it. Before those checks the scan reported an impossible 160-vertex
`G_VTX`.


### Geometry

Status: COMPLETE

`PLAN.md` R0.8 and R0.12 `COMPLETE` (transform kinds 44/46/48/50, RE-062/RE-063); RE-143 reproduces signed animated billboard scale; RE-144 corrects billboard Z to the original X/Y/X scale rule; RE-145 individually reviews all 109 billboard ordinals; `romtool mesh` converts every root display list, 0 failures archive-wide

**Remaining work:** Physical PSP validation remains part of the later rendering gate


### Projection

Status: COMPLETE

`PLAN.md` R0.14: FOV, viewport/aspect and depth sourced and checked (RE-034/082/084/085); RE-131/150 port and correct the default battle camera; RE-151 traces the original ROM's live two-fighter camera state and pins distance/look-at/eye/FOV numerically, with byte-stable PPSSPP captures

**Remaining work:** Special entry/dead/pause modes remain future gameplay-system work; physical PSP validation remains part of R2


### Culling

Status: COMPLETE

`PLAN.md` R0.6: RDP per-frame default (`CULL_BACK` on) fixed, measured 86.3% of packed primitives post-fix (RE-068)
