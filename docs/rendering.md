# Rendering: N64 → PSP

- Source geometry: ready-made F3DEX2 display lists in ROM.
- `objdisplay.c` walks the `DObj` tree, sets material state, calls `gSPDisplayList`.
- Lists are static → converted once at build time into PSP vertex buffers and GE state ([D-001](decisions/D-001.md), [D-002](decisions/D-002.md)).
- RDP not emulated.

## Pipeline

```
ROM relocData file
  │ ssb-rom::archive, vpk0     decompress, relocate
  │ ssb-rom::dl                F3DEX2 command decode
  │ ssb-rom::mesh              RDP/RSP state tracking → mesh + material
  │ ssb-rom::psp_texture,      texture conversion, filter compensation,
  │ filter_compensation, pack  swizzle, pack
  ▼
assets/generated/ssb64.pak
  │ psp-runtime::meshdraw, gu  GE draws
  ▼
PSP GE
```

## Domains

- Each doc states the current model; RE records hold derivations.
- Open work: [`PLAN.md`](../PLAN.md#remaining-work).

| Area | Doc |
|---|---|
| Geometry, projection, culling, screen mapping ([D-047](decisions/D-047.md)) | [geometry.md](rendering/geometry.md) |
| Textures: decode, CLUT/TLUT, filtering, addressing, LOD, texgen | [textures.md](rendering/textures.md) |
| Combiner, `MObj` state, material animation | [materials.md](rendering/materials.md) |
| Lighting | [lighting.md](rendering/lighting.md) |
| Alpha, blending, depth, draw order | [depth-alpha-blending.md](rendering/depth-alpha-blending.md) |
| Billboards, effects, shadows, framebuffer effects, UI | [animation-effects.md](rendering/animation-effects.md) |
| Submission order, GE state cache | [psp-lowering.md](rendering/psp-lowering.md) |

- Per-texture filter residual census: `romtool residuals` → `assets/generated/three-point-residuals.md`.
  - Gitignored, ROM-derived ([D-048](decisions/D-048.md)).

## Validation

- Goldens and physical-PSP captures: [visual-regression/README.md](visual-regression/README.md).
- PPSSPP is not hardware proof.
- Reference ports are techniques, not authorities ([D-037](decisions/D-037.md)).
