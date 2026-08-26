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

`ssb-rom::texture` currently decodes everything to RGBA8888 as a neutral
intermediate. The packer that goes RGBA8888 → PSM (including **swizzling**,
which the PSP needs for texture-cache efficiency) is not written yet.

### Not yet handled

* Swizzling (`sceGuTexMode`'s swizzle flag) — required for performance.
* Mipmaps — the N64 uses `G_TX_MIPMAP` in places; unclear whether Smash does.
* `G_TX_CLAMP`/`MIRROR`/`WRAP` → `sceGuTexWrap`.

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
  362x272 viewport, centred. `coord::pillarboxed_viewport()`.

## Depth

The PSP's depth buffer is **inverted** relative to the intuitive setup: near
maps to 65535 and far to 0, so `sceGuDepthRange(65535, 0)` pairs with
`DepthFunc::GreaterOrEqual`. This is already set up in `psp/src/gu.rs` and is a
classic source of "everything renders in the wrong order" bugs.

## Renderer evolution

Per plan §32:

* **Renderer 0** — triangle testbed. 🟡 *builds; not yet observed running*
* **Renderer 1** — SSB stage geometry. Not started.
* **Renderer 2** — complete materials/textures. Not started.
* **Renderer 3** — transparency, fog, particles, shadows, UI. Not started.
* **Renderer 4** — batching, state sorting, caching. **Not before the game is
  visibly running.**

## Vertex layout

The GE reads vertex attributes in a fixed order and the `VertexType` flags must
describe exactly that order. `GuVertex` is therefore
`{ u, v, color, x, y, z }` — texture coords, then colour, then position.
Reordering those fields renders garbage with no error. It must also be
16-byte aligned for GE DMA, which is why vertex buffers are wrapped in
`psp::Align16`.
