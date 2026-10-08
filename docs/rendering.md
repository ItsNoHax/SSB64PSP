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

Each domain doc describes the current model; evidence records hold the
derivations.

| Area | Status | Doc |
|---|---|---|
| Geometry, projection, culling | Complete | [geometry.md](rendering/geometry.md) |
| Textures: decode, CLUT/TLUT, filtering, addressing, LOD, texgen | Complete with measured filter deviation | [textures.md](rendering/textures.md) |
| Combiner, `MObj` state, material animation | Complete for classified paths | [materials.md](rendering/materials.md) |
| Lighting | Complete | [lighting.md](rendering/lighting.md) |
| Alpha, blending, depth, draw order | Complete for classified formulas; battle passes by display link and task head (RE-422); cutouts at alpha 32 with their edge coverage blended (RE-424) | [depth-alpha-blending.md](rendering/depth-alpha-blending.md) |
| Billboards, effects, shadows, framebuffer effects, UI | UI sprites and the wallpaper drawn (RE-392, RE-419) | [animation-effects.md](rendering/animation-effects.md) |
| Submission order, GE state cache | Complete | [psp-lowering.md](rendering/psp-lowering.md) |
| Screen mapping | The N64's visible (10,10)–(310,230) box fills the PSP's 272 lines, ~371 × 272 centred; one mapping in `ssb_engine::coord` ([D-047](decisions/D-047.md)) | [geometry.md](rendering/geometry.md) |
| Filter residuals per texture | Generated report | [three-point-residuals.md](rendering/three-point-residuals.md) |
| Section 11 visual review | A/B frame decisions | [three-point-visual-review.md](rendering/three-point-visual-review.md) |

## Validation

- Deterministic PPSSPPHeadless goldens:
  [visual-regression/README.md](visual-regression/README.md).
- Physical PSP (Slim, 6.61) captures exist for stages, fighters, materials,
  framebuffer effects and texgen (RE-201–RE-271, packs older than v31) and
  for six `psp-game` scenes (RE-341); current goldens are not re-captured
  on hardware.
- PPSSPP is not hardware proof. Reference ports are techniques, not
  authorities ([D-037](decisions/D-037.md)).

## Target

The model above is the target for every path it covers. What remains —
undrawn effects, N64 comparisons, GE fill cost of the overscan crop, VRAM
residency, state batching after profiling — is listed in
[`PLAN.md`](../PLAN.md#remaining-work).
