# Rendering: N64 → PSP

Compatibility entry point. Detailed per-domain technical reference now lives
under `docs/rendering/`; this file is the architecture summary and status
overview. Do not re-inline domain detail here — link to it instead.

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

On a 333 MHz MIPS CPU that difference is large. Preconversion is D-001/D-002
and here it is not merely preferable, it is nearly free.

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
   │  converter (build time)    ← state tracking          [IMPLEMENTED, partial coverage]
   ▼
intermediate representation: mesh + material
   │  packer                    ← swizzle, PSM, index     [IMPLEMENTED, partial coverage]
   ▼
PSP vertex/index buffers + material records
   │  psp::gu                   ← sceGumDrawArray         [IMPLEMENTED, partial coverage]
   ▼
GE
```

## Rendering status

Scoped to exactly the rendering-correctness categories `PLAN.md` R0 tracks
(§6.0's hierarchy cross-reference). Each area's current model, evidence and
remaining work now live in the linked domain doc — update the domain doc in
the same work cycle as any change to that area, not this table.

| Area | Status | Domain doc |
| --- | --- | --- |
| Geometry, Projection, Culling | COMPLETE | [docs/rendering/geometry.md](rendering/geometry.md) |
| Texture decode, CI4, CI8, TLUT, filtering, addressing, LOD/mipmaps, texgen | see doc | [docs/rendering/textures.md](rendering/textures.md) |
| Combiner, Runtime MObj display state | COMPLETE | [docs/rendering/materials.md](rendering/materials.md) |
| Lighting | COMPLETE | [docs/rendering/lighting.md](rendering/lighting.md) |
| Alpha, Blending, Depth | COMPLETE | [docs/rendering/depth-alpha-blending.md](rendering/depth-alpha-blending.md) |
| Transparency, Effects/particles, Shadows, Framebuffer effects, UI, billboards | see doc | [docs/rendering/animation-effects.md](rendering/animation-effects.md) |
| Primitive submission order, PSP GE state cache, renderer-evolution stages | COMPLETE | [docs/rendering/psp-lowering.md](rendering/psp-lowering.md) |
| Physical PSP | IN_PROGRESS | RE-201–271 provide Slim/6.61 captures for stages, fighters, materials, framebuffer effects, texgen and the full fighter/diagnostic matrix; PSP-1000 and exhaustive/long-duration coverage remain in the formal R2 matrix (see `plans/rendering/R2.md`, `STATUS.md`). PPSSPP is not physical proof. |

`PLAN.md` R0.18 tracks a systematic comparison against `sf64-psp` and
`oot-PSP` (both PSP targets, so their `sceGu` usage is directly comparable)
beyond ad hoc BattleShip cross-references. Treat all three as technical
references, not authorities, per D-037.
