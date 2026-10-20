---
name: rendering
description: Rendering, texture, material, lighting, geometry, camera/projection, alpha/blending/depth, framebuffer effects, animation rendering, or PSP GE/sceGu fidelity work. Activates for renderer bugs, visual regressions, and rendering-correctness questions.
---

# Rendering

Rules: AGENTS.md § Reference order, § Constraints.

Protocol:

```text
symptom → asset/scene/display list → decompilation → ROM data
→ display-list/GBI state → reference ports → hypothesis → smallest change
→ targeted test → original comparison → regression → evidence and docs
```

- N64 behaviour first
- Never tune or guess: materials, palettes, formats, filtering, LOD, mipmaps, transforms, animation timing, lighting, combiner, alpha, depth
- PSP cannot reproduce a behaviour → record:
  - original behaviour
  - limitation
  - approximation
  - measured effect
  - regression coverage
- Reference-port technique (BattleShip, `sf64-psp`, `oot-PSP`: GBI/RSP/RDP/TMEM/`sceGu`):
  - take only after confirming SSB64 needs it
  - never authoritative over decomp/ROM ([D-037](../../../docs/decisions/D-037.md))

Routing:

1. [docs/rendering.md](../../../docs/rendering.md): architecture summary and status table only
2. Domain doc for the symptom: `docs/rendering/{geometry,textures,materials,lighting,depth-alpha-blending,animation-effects,psp-lowering}.md`
3. `RE-XXX` records only when a claim needs its derivation (domain docs cite them)
4. Open rendering work: `PLAN.md`

Code: `crates/ssb-rom/src/{mesh,pack,psp_texture,filter_compensation}.rs`, `psp-runtime/src/{gu,meshdraw}.rs`.
