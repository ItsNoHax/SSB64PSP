# Technical Decisions

Permanent technical decisions recovered from the repository. Each entry records **what was decided**, **why**, and **where it's implemented**. Do not revisit unless new evidence contradicts.

---

## Rendering Architecture

### D-001: No RDP Emulation — Build-Time Display List Conversion
**Decision:** Parse F3DEX2 display lists at build time (`crates/ssb-rom/src/dl.rs`) and lower to PSP vertex buffers + `sceGu` state. Do not emulate the RDP at runtime.

**Reasoning:**
- Smash 64 geometry is **static ROM data** (unlike Star Fox 64 which generates DLs dynamically)
- Preconversion is strictly cheaper on 333 MHz MIPS CPU
- `sf64-psp` does runtime translation because it must; we don't

**Implemented:** `crates/ssb-rom/src/dl.rs`, `crates/ssb-rom/src/mesh.rs`, `crates/ssb-rom/src/pack.rs`

**Reference:** `docs/rendering.md` "The central decision"

---

### D-002: Preconversion Over Runtime Conversion
**Decision:** Convert all assets (textures, meshes, animations) at build time. Ship PSP-native formats in the runtime pack.

**Reasoning:**
- PSP CPU is the bottleneck; bandwidth matters
- N64 vertex cache (32 entries) forces re-uploads; indexing at build time gives 2.09x reuse
- 16-bit vertex components (positions already `i16`, UVs S10.5) map to `GU_VERTEX_16BIT` / `GU_TEXTURE_16BIT` directly — 12 bytes vs 24 per vertex
- Material merging at build time reduces GE state changes

**Implemented:** `crates/ssb-rom/src/texture.rs`, `psp_texture.rs`, `mesh.rs`, `pack.rs`

**Reference:** `docs/rendering.md` "Geometry conversion results"

---

### D-003: Texture Formats — Keep Paletted Textures Paletted
**Decision:** Convert CI4/CI8 to `PsmT4`/`PsmT8` + CLUT. Convert I4/I8 to palettized greyscale. Expand IA/RGBA32 to `Psm8888`.

**Reasoning:**
- 77% VRAM saved (1078 KiB packed vs 4711 KiB naive RGBA8888)
- PSP has native CLUT support; CI4/CI8 convert ~1:1
- CI4 is dominant (468/545 textures)
- VRAM budget is ~700 KiB after framebuffers + depth; full set doesn't fit at once

**Implemented:** `crates/ssb-rom/src/psp_texture.rs`

**Update (RE-053, RE-067, RE-070):** the packed total has grown since this
decision as correctness fixes were added on top of the format choice above —
mip chains (RE-053), mirrored-texture pre-baking (RE-067), and a targeted
per-texture dither blur (RE-070) all cost real VRAM. Current measured total is
**1170.9 KiB**, 1.7x the ~700 KiB budget; texture streaming (`TODO.md` Phase
G) is no longer optional headroom.

**Reference:** `docs/rendering.md` "Texture conversion results", `docs/memory.md` VRAM budget

---

### D-004: Coordinate Systems — No Handedness Flip, No Matrix Transpose
**Decision:** N64 and PSP both use right-handed, `+Y` up, view down `-Z`. N64 row-major/row-vector cancels with PSP column-major/column-vector. Only s15.16 → `f32` widening is real work.

**Reasoning:**
- Verified by algebra: two transposes cancel (`Mpsp[i][j] = M64[j][i]`)
- Unit test `row_vector_translation_lands_in_the_translation_column` caught early transpose bug
- `ftPhysicsApplyGravityClampTVel` does `vel_air.y -= gravity` → `+Y` up confirmed

**Implemented:** `crates/ssb-engine/src/coord.rs::n64_to_psp_matrix`, `n64_to_psp_position`

**Reference:** RE-004, RE-005, `docs/rendering.md` "Coordinate handling"

---

### D-005: Fixed 60 Hz Simulation Decoupled from Rendering
**Decision:** Game simulation runs at fixed 60 Hz tick. Rendering follows PSP display cadence (~59.94 Hz). Accumulator with capped catch-up.

**Reasoning:**
- Every timing constant in decomp expressed in frames
- `scheduler.c` registers `osViSetEvent(..., INTR_VRETRACE, 1)` — event every retrace = 60 Hz NTSC
- Steady state = 1 tick per vblank

**Implemented:** `crates/ssb-engine/src/timing.rs`

**Reference:** RE-006, `docs/reverse-engineering.md`

---

### D-006: Vertex Format — 16-bit Normalized Requires Model Scale
**Decision:** `GU_VERTEX_16BIT` interprets coordinates as normalized fixed point (divide by 32768). Apply uniform model-matrix scale of 32768 to undo. UVs: S10.5 (32 units/texel) → `sceGuTexScale(1024/w, 1024/h)`.

**Reasoning:**
- Without scale, N64 coordinates in hundreds became hundredths → invisible speck at origin
- Precision unaffected: coordinates are integers well inside `i16` range

**Implemented:** `psp/src/meshdraw.rs::MODEL_SCALE` (32768), `sceGuTexScale(1024/width, 1024/height)`

**Reference:** RE-020

---

### D-007: Depth Buffer — Inverted Range
**Decision:** PSP depth buffer inverted: near=65535, far=0. Use `sceGuDepthRange(65535, 0)` + `DepthFunc::GreaterOrEqual`.

**Reasoning:** Classic source of "everything renders in wrong order" bugs. Verified working.

**Implemented:** `psp/src/gu.rs`

**Reference:** `docs/rendering.md` "Depth"

---

### D-008: Aspect Ratio — Pillarboxed 362×272
**Decision:** Game renders 320×240 (4:3). PSP is 480×272. Default is pillarboxed 362×272 viewport, centered. Applied to **both** `sceGuViewport` and `sceGuScissor`.

**Reasoning:** Stretching would distort characters. Feeding pillarbox aspect to projection while leaving GE viewport at 480 stretches by 1.33x.

**Implemented:** `crates/ssb-engine/src/coord.rs::pillarboxed_viewport()`, `psp/src/gu.rs::Gpu::init`

**Reference:** RE-034, `docs/rendering.md` "Coordinate handling"

---

## Asset Pipeline

### D-009: VPK0 Decompression — Postfix Huffman, Bit-Width Leaves
**Decision:** VPK0 uses LZ77 with two postfix-encoded Huffman trees. Leaves hold **bit widths**, not values.

**Reasoning:** Two independent verification paths agree for all 499 compressed files:
1. Walking intern chain through decompressed payload (single wrong byte derails it)
2. ROM gap measurement (doesn't depend on decompression)

**Implemented:** `crates/ssb-rom/src/vpk0.rs`

**Reference:** RE-001, RE-002

---

### D-010: relocData Archive — Intern/Extern Chains Through Pointer Slots
**Decision:** Intern relocations: singly linked list threaded through pointer slots being patched. Extern: identical but targets in other files; target file IDs in `u16` array after file data.

**Reasoning:** Matches `lbRelocLoadAndRelocFile` exactly. 61,343 intern + 3,092 extern slots across 2132 files, 0 mismatches.

**Implemented:** `crates/ssb-rom/src/archive.rs`

**Reference:** RE-001, `docs/ssb-architecture.md` §5

---

### D-011: Extern Relocations — Zeroed in Pack, Patched at Runtime Load
**Decision:** Pack records extern slots in manifest but leaves them zeroed. Runtime loader will compute closure, assign offsets, apply intern + extern relocations.

**Reasoning:** Target addresses depend on runtime layout. Same three-step shape as original: compute closure → allocate → patch.

**Implemented:** `crates/ssb-rom/src/pack.rs` (manifest), runtime loader TODO

**Reference:** `docs/porting-status.md` "Known gaps #6", `docs/memory.md` "Extern relocations and layout"

---

### D-012: DObjDesc Arrays — Depth-Tagged Flattened Tree
**Decision:** `DObjDesc.id & 0xFFF` = node depth. Parent = most recent node at `depth - 1`. Terminator = depth 18 (out of range). High nibble selects matrix composition kind.

**Reasoning:** Recovered by scanner with 5 constraints. Validated: 363 arrays across 134 files, per-file counts identical to decomp, 180 annotated arrays exact.

**Implemented:** `crates/ssb-rom/src/scan.rs`, `scene.rs`

**Reference:** RE-023

---

### D-013: DObj Display List Field — Undiscriminated Union
**Decision:** Field is union of `Gfx*`, `Gfx**`, `DObjDLLink*`, `DObjMultiList*`, `DObjDistDL*`. Disambiguate structurally: try `DObjDLLink` first (constrained shape: `list_id < 4` + relocated pointer), fall back to `Gfx*`.

**Reasoning:** Real display list cannot pass as link array (`G_VTX` command word `0x01xxxxxx` > 4). 1661 node fields resolve, 1417 convert with triangles.

**Implemented:** `crates/ssb-rom/src/mesh.rs`

**Reference:** RE-025

---

### D-014: Fighter Vertex Cache — Shared Across Joints (Rest Pose Only)
**Decision:** Convert scene graph lists in draw order, threading one 32-entry vertex cache. Each cached vertex records loading node; triangles borrowing vertices rebase via `inv(world_here) * world_there`. Exact for rest pose only.

**Reasoning:** `gcDrawDObjTree` walks tree emitting into one command stream — RSP cache survives across lists. Joint's list draws triangles with vertices loaded by previous joint = N64's skinning without per-vertex weights. Conversion failures 244 → 0.

**Implemented:** `crates/ssb-rom/src/mesh.rs` (draw-order traversal + cache threading)

**Reference:** RE-026

---

### D-015: Fighter Palette — Named by FTCommonPart Parallel to DObjDesc
**Decision:** `FTCommonPart` struct pairs `DObjDesc*` with `MObjSub***` (parallel arrays). Both are extern relocations recorded by archive loader. Display list calls segment `0x0E` + 8×index to select MObj.

**Reasoning:** Search-by-demand-vector failed (26/50 = coin flip). Struct pairing is definitive. Chain length matches display-list demand for 310/310 nodes. All 459 resolved offsets match decomp.

**Implemented:** `crates/ssb-rom/src/mobj.rs`, `scene.rs`

**Reference:** RE-027

---

### D-016: Stage Material Table — One Word Further in MPGroundDesc
**Decision:** `MPGroundDesc` = `{ DObjDesc* dobjdesc; AObjEvent32** anim_joints; MObjSub*** p_mobjsubs; ... }`. Table at `dobjdesc + 8` (not `+4` like fighters) because `anim_joints` sits between.

**Reasoning:** Every stage layer went unmatched while fighters matched — single word offset difference. Reading struct rather than pattern-matching adjacency is robust.

**Implemented:** `crates/ssb-rom/src/stage.rs`

**Reference:** RE-028

---

### D-017: Stage Collision — 2D Polylines, vertex2 Is Count
**Decision:** `MPVertexLinks { u16 vertex1, vertex2 }` — `vertex2` is **count**, not second index. Lines are polylines: `for (v = vertex1; v < vertex1 + vertex2 - 1; v++)`.

**Reasoning:** Dream Land line 3 = `{9, 2}` → vertices 9..11 → symmetric platform at y=0. "Second vertex index" interpretation gives wrong geometry. Array lengths derived from data (max `group_id + line_count`, etc.).

**Implemented:** `crates/ssb-rom/src/collision.rs`

**Reference:** RE-029

---

### D-018: Surface Flags — Upper Byte State, Lower Byte Material
**Decision:** `MAP_VERTEX_COLL_PASS (1<<14)` = drop-through. `MAP_VERTEX_COLL_CLIFF (1<<15)` = ledge-grabbable. Lower byte = `MPMaterial` → friction via `dMPCollisionMaterialFrictions[material] * attr->traction`.

**Reasoning:** Dream Land: three floating platforms = `pass` (drop through), main platform = `cliff` (no drop, grabbable), ceiling/walls = neither. Four independent facts, four matches. Spawn drop test: 158/162 land, 2-6 units below start.

**Implemented:** `crates/ssb-rom/src/collision.rs`, `crates/ssb-game/src/collision.rs`

**Reference:** RE-030

---

### D-019: Collision Query — Swept Segment, Not Point Test
**Decision:** `mpCollisionCheckFloorLineCollisionSame` tests swept segment from old→new position. Dispatches on segment flat vs tilted. Landing snap at 0.001 tolerance.

**Reasoning:** Stops fast fallers from crossing platform in one frame. 0.001 is literal in decomp, not tuning knob — lets fighter standing exactly on surface register as touching.

**Implemented:** `crates/ssb-game/src/collision.rs`

**Reference:** RE-030

---

### D-020: Animation — Figatree (AObjEvent16) for Fighters, AObjEvent32 for Stages
**Decision:** Fighters use compact 16-bit `AObjEvent16` (figatree). Stages use 32-bit `AObjEvent32`. Both use same `AObj` interpolation (cubic/linear/step). Pack both.

**Reasoning:** Decomp uses both formats. Figatree has per-joint command stream with `ftAnimGetTargetValue` scales. Stage animation validated 3 ways: ROM replay, pack pose match (444,960 values), device frame diff.

**Implemented:** `crates/ssb-rom/src/figatree.rs`, `anim.rs`, `objanim.rs`, `anim_table.rs`

**Reference:** RE-036, RE-050, RE-051, RE-052

---

### D-021: Physics — Float, Not Fixed Point
**Decision:** Original uses `f32` throughout (`ftPhysicsApplyGravityClampTVel` does `vel_air.y -= gravity`). No fixed-point representation to recover.

**Reasoning:** Direct port preserves behavior. `+Y` up, gravity subtracts. Z is shallow depth axis clamped to ±60.

**Implemented:** `crates/ssb-game/src/physics.rs` (16 functions, original addresses cited)

**Reference:** `docs/ssb-architecture.md` §7

---

### D-022: Fighter Constants — Data-Driven from FTAttributes
**Decision:** All 27 characters' `FTAttributes` extracted from relocData, verified field-by-field against decomp. Invented defaults were 26x off and hid stick-scaling bug in air drift.

**Reasoning:** Per-character tuning is data in original. Gravity, terminal velocities, air accel/friction, traction, dash/run speed, weight, jump params, shield size, SFX IDs, move availability bitfields.

**Implemented:** `crates/ssb-rom/src/fighter.rs`, `crates/ssb-game/src/fighter.rs`

**Reference:** RE-032, `docs/porting-status.md` Physics

---

### D-023: Movement Status Machine — Original Interrupt Chain + Tap Counter
**Decision:** Port status machine verbatim: Wait, 3×Walk, Dash, Run, RunBrake, Turn, KneeBend, Jump F/B, JumpAerial F/B, Fall, FallAerial, Squat, Landing light/heavy, Pass. Original interrupt-chain ordering and tap-counter input model. 5 statuses with no duration in `FTAttributes` take it from figatree animation.

**Reasoning:** Matches decomp exactly. All 20 movement statuses have animations (532 in pack).

**Implemented:** `crates/ssb-game/src/status.rs`, `fighter.rs`

**Reference:** RE-033, RE-035, `docs/porting-status.md` Fighter state

---

### D-024: Light Colors — White (Measured)
**Decision:** `MObjSub::light1color` / `light2color` measured across all MObjSub — they are white. Light **direction** matters, not color.

**Reasoning:** RE-024 measured them. Missing light color fields don't affect visual output.

**Implemented:** Single neutral key light in renderer, baked into vertex colour at pack time rather than lit at runtime. Its direction is `MPGroundData.light_angle` converted the way `ftDisplayLightsDrawReflect` does, measured archive-wide (RE-065): 33 of 41 stages (80%) share one `(20, 45)` degree angle, now used exactly; the other 8 (mostly special-lighting locations — Brinstar, Sector Z, Metal Mario's stage, etc.) diverge up to 111 degrees and are an accepted deviation, since varying the light per stage needs runtime `sceGuLight` lighting, not pack-time baking.

**Reference:** RE-024, RE-065, `TODO.md` Phase D (majority-vote lighting heuristic — the *shading-detection* heuristic, RE-021 — still not removed; the *direction* is now measured, not guessed)

---

### D-025: Fog — Effectively Unused
**Decision:** `G_SETFOGCOLOR` appears twice in entire game. Fog not implemented.

**Reasoning:** Not worth runtime cost. Confirmed by `romtool scan` opcode counts.

**Reference:** `docs/rendering.md` "Measured usage"

---

## Platform & Toolchain

### D-026: PSP Crate Outside Workspace — Pinned Nightly + build-std
**Decision:** `psp/` is a separate Cargo project (excluded from workspace). Uses `psp/rust-toolchain.toml` pinning `nightly-2026-08-01`. Root workspace runs `cargo test` on stable.

**Reasoning:** `rust-psp` needs nightly + `-Z build-std`. Reaches into unstable `core::panic::PanicPayload`. Keeps host CI fast and stable.

**Reference:** `psp/rust-toolchain.toml`, `.github/workflows/ci.yml`

---

### D-027: no_std Discipline — Workspace Default-Features = false
**Decision:** Core crates (`ssb-rom`, `ssb-engine`, `ssb-game`) are `no_std` with `default = ["std"]`. Workspace dependencies declare `default-features = false`. Crates wanting `std` opt in via their own `std` feature. CI builds all three for `thumbv7em-none-eabi` to catch leakage.

**Reasoning:** A crate cannot turn off a workspace dependency's default features. If `std` leaks, PSP build fails with confusing "can't find crate for `std`".

**Implemented:** Root `Cargo.toml:31-33`, each crate's `Cargo.toml`

**Reference:** `.github/workflows/ci.yml` (builds all three for `thumbv7em-none-eabi`)

---

### D-028: Asset Pack Mandatory — Built Separately by romtool
**Decision:** `cargo psp` builds executable only. `romtool pack` builds `assets/generated/ssb64.pak`. `run-ppsspp.sh` stages both together.

**Reasoning:** Clean separation. Without pack, viewer falls back to built-in tetrahedron.

**Reference:** AGENTS.md §15 (Asset Pack Discipline), README Quick Start

---

### D-029: Debug Overlay — Software Rasteriser Required in PPSSPP
**Decision:** `sceGuDebugFlush` paints VRAM with CPU. PPSSPP hardware backends don't reflect CPU VRAM writes. Force software rasteriser via config append.

**Reasoning:** Emulator limitation, not port bug. Real HUD will render as GE geometry (Renderer 3), removing dependency.

**Implemented:** `tools/run-ppsspp.sh --appendconfig`

**Reference:** RE-014, `tools/run-ppsspp.sh`

---

### D-030: Toolchain Pinning — Successful Compile ≠ Working
**Decision:** Before bumping `psp/rust-toolchain.toml`: build with new nightly AND boot in PPSSPP. Known broken: `nightly-2026-08-26+`.

**Reasoning:** `rust-psp` imports unstable `core` internals. Compile success is not sufficient evidence.

**Reference:** RE-012, `psp/rust-toolchain.toml`

---

## Architecture Comparison (From `docs/ssb-architecture.md` §11)

| Concern | SSB64 (N64) | sf64-psp | n64psp | This Port |
|---------|-------------|----------|--------|-----------|
| Language | C (IDO) | C | C | **Rust** |
| Approach | native | decomp + PSP backend | reusable N64→PSP runtime | decomp-informed rewrite |
| Rendering | F3DEX2 → RDP | GU translation of N64 DLs | backend-registration only | **build-time DL → PSP mesh** |
| Runtime DL translation | n/a | yes | n/a | **no — preconverted** |
| Audio | RSP `aspMain` | PSP audio | not implemented | build-time convert + SW mixer |
| Threading | libultra, 5 threads | PSPSDK | PSPSDK sema/threads | **2 threads (game + audio)** |
| Layering | monolithic | game + compat layer | runtime / bridge / backend | **A game / B traits / C PSP** |

**Key insight:** `n64psp` provides layering shape (runtime must not know game; graphics backend registered, not hardcoded) adopted as Layers A/B/C. `sf64-psp` is mature reference for N64→PSP rendering but translates at runtime — we preconvert because Smash geometry is static.

---

## Unsafe Discipline

### D-031: Unsafe Only for PSP APIs / VFPU / GPU Memory
**Decision:** No `unsafe` without concrete reason. When required for `sceGu`, `sceCtrl`, VFPU, GPU DMA, isolate behind small safe abstractions.

**Reasoning:** Rust safety guarantees matter. PSP FFI boundaries are the only justified `unsafe`.

**Reference:** this decision record

---

## Profiling Before Optimizing

### D-032: VFPU After Profiling
**Decision:** Scalar Rust first. Benchmark. Identify hot functions. Then VFPU. Compare output. Benchmark again.

**Reasoning:** Premature VFPU optimization is a trap. Profile data drives decisions.

**Reference:** PLAN.md R3 ("Do not perform speculative optimization before measurement")

---

## Version Control

### D-033: ROM and Generated Assets Gitignored
**Decision:** `rom/` and `assets/generated/` are gitignored. CI rejects committed ROM files.

**Reasoning:** Legal requirement. User supplies own ROM. Build generates assets locally.

**Reference:** AGENTS.md §15 (Asset Pack Discipline), README Legal

---

## Validation Philosophy

### D-034: Two Independent Readings Must Agree
**Decision:** Every verification uses two independent paths that must agree:
- VPK0: chain walk (needs correct bytes) vs ROM gap (no decompression)
- Fighter constants: ROM extraction vs decomp source
- Animation lengths: two decode paths
- Collision: spawn drop test (data-driven, not unit fixture)

**Reasoning:** A wrong offset doesn't produce near miss; it produces garbage. Agreement across independent paths is the evidence.

**Reference:** RE-002, RE-030, RE-032, RE-035, RE-036, RE-052, README "Verification"

---

## Milestone Validation

### D-035: Functional Validation Required, Not Just Compile
**Decision:** Do not advance milestone because code compiles. Each milestone requires functional validation (Rule 12). Percentages = intended scope, not line count. COMPLETE = validated (Rule 11).

**Reasoning:** Compiles ≠ works. Porting status tracks validated subsystems.

**Reference:** AGENTS.md §13 (Task Completion Semantics), `docs/porting-status.md` header