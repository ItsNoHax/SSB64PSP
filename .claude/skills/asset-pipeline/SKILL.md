---
name: asset-pipeline
description: relocData, VPK0, texture packing, meshes, pack format, romtool, or ROM extraction/conversion work. Activates for asset-pipeline and pack-format questions.
---

# Asset pipeline

Rules: AGENTS.md § Constraints (no ROM data in Git, rebuild the pack).

Read in order:

1. [docs/architecture.md](../../../docs/architecture.md) "Original Game Reference": relocData, VPK0, where each subsystem lands.
2. [docs/rendering/textures.md](../../../docs/rendering/textures.md), [geometry.md](../../../docs/rendering/geometry.md): texture and mesh conversion model.
3. Decisions (open the single `D-XXX.md`): D-009 VPK0, D-010/D-011 relocData and extern relocations, D-002/D-003 preconversion and texture formats, D-028 mandatory pack.
4. `docs/evidence/INDEX.md`, `rg` for topic tags `pack`, `relocations`, `texture`, `archive`.

Code: `crates/ssb-rom/src/{archive,vpk0,pack,texture,psp_texture,mesh}.rs`, `tools/romtool/src/main.rs`.

ROM code-segment tables ([D-048](../../../docs/decisions/D-048.md)):

- Tables: motion, colour-animation and CPU scripts, status flags, animation table
- Generated at build time by `crates/ssb-tablegen` into `OUT_DIR`
- Dump generated Rust: `cargo run -p ssb-tablegen -- DIR`
