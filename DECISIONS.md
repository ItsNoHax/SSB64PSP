# Decisions

Permanent architectural decisions. Each record in `docs/decisions/D-NNN.md`
holds the decision, reasoning and any later amendments. Revisit one only when
new evidence contradicts it.

## Rendering Architecture

- [D-001](docs/decisions/D-001.md): No RDP Emulation — Build-Time Display List Conversion
- [D-002](docs/decisions/D-002.md): Preconversion Over Runtime Conversion
- [D-003](docs/decisions/D-003.md): Texture Formats — Keep Paletted Textures Paletted
- [D-004](docs/decisions/D-004.md): Coordinate Systems — No Handedness Flip, No Matrix Transpose
- [D-005](docs/decisions/D-005.md): Fixed 60 Hz Simulation Decoupled from Rendering
- [D-006](docs/decisions/D-006.md): Vertex Format — 16-bit Normalized Requires Model Scale
- [D-007](docs/decisions/D-007.md): Depth Buffer — Inverted Range
- [D-008](docs/decisions/D-008.md): Aspect Ratio — Pillarboxed 362×272

## Asset Pipeline

- [D-009](docs/decisions/D-009.md): VPK0 Decompression — Postfix Huffman, Bit-Width Leaves
- [D-010](docs/decisions/D-010.md): relocData Archive — Intern/Extern Chains Through Pointer Slots
- [D-011](docs/decisions/D-011.md): Extern Relocations — Zeroed in Pack, Patched at Runtime Load
- [D-012](docs/decisions/D-012.md): DObjDesc Arrays — Depth-Tagged Flattened Tree
- [D-013](docs/decisions/D-013.md): DObj Display List Field — Undiscriminated Union
- [D-014](docs/decisions/D-014.md): Fighter Vertex Cache — Shared Across Joints (Rest Pose Only)
- [D-015](docs/decisions/D-015.md): Fighter Palette — Named by FTCommonPart Parallel to DObjDesc
- [D-016](docs/decisions/D-016.md): Stage Material Table — One Word Further in MPGroundDesc
- [D-017](docs/decisions/D-017.md): Stage Collision — 2D Polylines, vertex2 Is Count
- [D-018](docs/decisions/D-018.md): Surface Flags — Upper Byte State, Lower Byte Material
- [D-019](docs/decisions/D-019.md): Collision Query — Swept Segment, Not Point Test
- [D-020](docs/decisions/D-020.md): Animation — Figatree (AObjEvent16) for Fighters, AObjEvent32 for Stages
- [D-021](docs/decisions/D-021.md): Physics — Float, Not Fixed Point
- [D-022](docs/decisions/D-022.md): Fighter Constants — Data-Driven from FTAttributes
- [D-023](docs/decisions/D-023.md): Movement Status Machine — Original Interrupt Chain + Tap Counter
- [D-024](docs/decisions/D-024.md): Light Colors — Preserve Source State
- [D-025](docs/decisions/D-025.md): Fog — Effectively Unused

## Platform & Toolchain

- [D-026](docs/decisions/D-026.md): PSP Crate Outside Workspace — Pinned Nightly + build-std
- [D-027](docs/decisions/D-027.md): no_std Discipline — Workspace Default-Features = false
- [D-028](docs/decisions/D-028.md): Asset Pack Mandatory — Built Separately by romtool
- [D-029](docs/decisions/D-029.md): Debug Overlay — Software Rasteriser Required in PPSSPP
- [D-030](docs/decisions/D-030.md): Toolchain Pinning — Successful Compile ≠ Working
- [D-044](docs/decisions/D-044.md): PSP Backend Split Into Shared psp-runtime + Two Applications

## Engineering Process

- [D-031](docs/decisions/D-031.md): Unsafe Only for PSP APIs / VFPU / GPU Memory
- [D-032](docs/decisions/D-032.md): VFPU After Profiling
- [D-033](docs/decisions/D-033.md): ROM and Generated Assets Gitignored
- [D-034](docs/decisions/D-034.md): Two Independent Readings Must Agree
- [D-035](docs/decisions/D-035.md): Functional Validation Required, Not Just Compile
- [D-037](docs/decisions/D-037.md): Reference Ports Are Technical References, Not Authorities
- [D-041](docs/decisions/D-041.md): PPSSPPHeadless Is the Automated Visual-Verification Runner

## Rendering Fidelity

- [D-036](docs/decisions/D-036.md): N64 Render-State Fidelity Must Precede Optimization
- [D-038](docs/decisions/D-038.md): Generated Texture Coordinates Use the GE Texture Matrix, Not Environment Mapping
- [D-039](docs/decisions/D-039.md): Texgen State Is Primitive-Level Because the Archive Says So
- [D-040](docs/decisions/D-040.md): Exact Linear Texgen Is CPU-Generated into the Authored-UV Pipeline, Not a Second GE Mode or a Lookup Table
- [D-042](docs/decisions/D-042.md): Renderer correctness claims stay provisional until the corrective gate passes
- [D-043](docs/decisions/D-043.md): Filtering and Tile-Addressing Equivalence Claims Remain Provisional Pending R2.0
