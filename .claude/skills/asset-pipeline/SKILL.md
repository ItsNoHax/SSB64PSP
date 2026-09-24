---
name: asset-pipeline
description: relocData, VPK0, texture packing, meshes, pack format, romtool, or ROM extraction/conversion work. Activates for asset-pipeline and pack-format questions.
---

# Asset pipeline

Routing:

1. [docs/ssb-architecture.md](../../docs/ssb-architecture.md) §4
   (relocData / VPK0) and §9 for where each subsystem lands.
2. [docs/rendering/textures.md](../../docs/rendering/textures.md) and
   [docs/rendering/geometry.md](../../docs/rendering/geometry.md) for texture
   and mesh conversion's current model.
3. `docs/decisions/` — D-009 (VPK0), D-010/D-011 (relocData/extern
   relocations), D-002/D-003 (preconversion, texture formats), D-028 (asset
   pack mandatory). Read the specific `D-XXX.md`, not the whole index.
4. `docs/evidence/INDEX.md` filtered to `pack`, `relocations`, `texture`, or
   `archive` topic tags for prior extraction/conversion bugs.
5. For code, prefer Serena symbol search over reading whole files
   (`crates/ssb-rom/src/{archive,vpk0,pack,texture,psp_texture,mesh}.rs`,
   `tools/romtool/src/main.rs`).

Never commit ROM-derived assets; rebuild `assets/generated/ssb64.pak` when
pipeline code changes (`AGENTS.md`).
