# Rendering: N64 → PSP

Smash 64 stores its geometry as ready-made F3DEX2 display lists in the ROM.
`objdisplay.c` walks the `DObj` tree, sets material state and calls
`gSPDisplayList` into ROM data; it never builds a mesh at runtime. Those lists
are static, so the port converts them once at build time into PSP vertex
buffers and GE state ([D-001](decisions/D-001.md),
[D-002](decisions/D-002.md)). The RDP is not emulated.

## Pipeline

```
ROM relocData file
  │ ssb-rom::archive, vpk0     decompress, relocate
  ▼
  │ ssb-rom::dl                F3DEX2 command decode
  ▼
  │ ssb-rom::mesh              RDP/RSP state tracking → mesh + material
  ▼
  │ ssb-rom::psp_texture,      texture conversion, filter compensation,
  │ filter_compensation, pack  swizzle, pack
  ▼
assets/generated/ssb64.pak
  │ psp-runtime::meshdraw, gu  GE draws
  ▼
PSP GE
```

## Domains

Each domain doc describes the current model. Evidence records hold the
history.

| Area | Status | Doc |
|---|---|---|
| Geometry, projection, culling | Complete | [geometry.md](rendering/geometry.md) |
| Textures: decode, CLUT/TLUT, filtering, addressing, LOD, texgen | Complete with measured filter deviation | [textures.md](rendering/textures.md) |
| Combiner, `MObj` state, material animation | Complete for classified paths | [materials.md](rendering/materials.md) |
| Lighting | Complete | [lighting.md](rendering/lighting.md) |
| Alpha, blending, depth | Complete for classified formulas | [depth-alpha-blending.md](rendering/depth-alpha-blending.md) |
| Billboards, effects, shadows, framebuffer effects, UI | UI not started | [animation-effects.md](rendering/animation-effects.md) |
| Submission order, GE state cache | Complete | [psp-lowering.md](rendering/psp-lowering.md) |
| Filter residuals per texture | Generated report | [three-point-residuals.md](rendering/three-point-residuals.md) |

## Validation

- Deterministic PPSSPPHeadless goldens:
  [visual-regression/README.md](visual-regression/README.md).
- Physical PSP (Slim, 6.61) captures exist for stages, fighters, materials,
  framebuffer effects and texgen (RE-201–271), all on packs older than v31.
- PPSSPP is not hardware proof. Reference ports are techniques, not
  authorities ([D-037](decisions/D-037.md)).
