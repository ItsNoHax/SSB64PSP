---
name: rendering
description: Rendering, texture, material, lighting, geometry, camera/projection, alpha/blending/depth, framebuffer effects, animation rendering, or PSP GE/sceGu fidelity work. Activates for renderer bugs, visual regressions, and rendering-correctness questions.
---

# Rendering

Investigation protocol:

```text
symptom → asset/scene/display list → decompilation → ROM data
→ display-list/GBI state → reference ports → hypothesis → smallest change
→ targeted test → original comparison → regression → evidence and docs
```

- Identify N64 behavior before changing anything. Never tune parameters or
  guess materials, palettes, formats, filtering, LOD, transforms, animation
  timing, lighting, combiner, alpha or depth.
- If the PSP cannot reproduce a behavior, record the original behavior, the
  limitation, the approximation, its measured effect and its regression
  coverage.
- Adopt a reference-port technique only after confirming SSB64 needs it.

Routing:

1. Start at [docs/rendering.md](../../../docs/rendering.md) — the architecture
   summary and status-overview table only.
2. Follow the table to the specific domain doc for the symptom:
   `docs/rendering/{geometry,textures,materials,lighting,depth-alpha-blending,animation-effects,psp-lowering}.md`.
   These describe the *current* model. Read only the one(s) relevant to the
   symptom.
3. Only retrieve `docs/evidence/re/RE-XXX.md` records when a claim needs its
   derivation — the domain docs already cite the relevant IDs. Open
   rendering work is in `PLAN.md`.
4. For code, prefer Serena symbol search over reading whole renderer files
   (`crates/ssb-rom/src/{mesh,pack,psp_texture,filter_compensation}.rs`,
   `psp-runtime/src/{gu,meshdraw}.rs`).

Do not guess materials, palettes, texture formats/filtering, LOD, mipmaps,
transforms, animation timing, lighting, combiner, alpha, or depth behavior —
identify N64 behavior from the decomp/ROM first. BattleShip, `sf64-psp`, and
`oot-PSP` are technical references for GBI/RSP/RDP/TMEM/`sceGu` technique
only (D-037) — never authoritative over SSB64's own decompilation/ROM.
