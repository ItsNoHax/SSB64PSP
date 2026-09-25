# Reverse-Engineering Evidence Index

One-line entry per investigation. Load `docs/evidence/re/RE-XXX.md` for the
full record (question, evidence, implementation, verification, conclusion).
Do not bulk-read this corpus; grep this index by ID or topic tag first.

Status `OPEN` = unresolved question, still tracked in `TODO.md`.
Status `COMPLETE` = investigation concluded (may still feed a task `IN_PROGRESS`).

Regenerate with `python3 tools/docs/gen_evidence_index.py` after adding or
editing a record.

| ID | Title | Status | Topics | Task |
|---|---|---|---|---|
| RE-001 | relocData table geometry | COMPLETE | pack |  |
| RE-002 | VPK0 stream format | COMPLETE | pack, relocations |  |
| RE-003 | Microcode variant | COMPLETE | geometry, memory |  |
| RE-004 | Coordinate system and matrix conversion | COMPLETE | hardware, psp-ge |  |
| RE-005 | Handedness | COMPLETE | camera, psp-ge, hardware, texture |  |
| RE-006 | Simulation rate | COMPLETE |  |  |
| RE-007 | Physics zero-crossing asymmetry | COMPLETE | texture |  |
| RE-008 | C-button mapping | OPEN | camera |  |
| RE-009 | PSP nub deadzone | OPEN |  |  |
| RE-010 | `MObjSub` unknown fields | OPEN | material, animation, lighting, texture |  |
| RE-011 | Level of detail selection | OPEN | camera |  |
| RE-012 | Nightly toolchain pin | SUPERSEDED | toolchain |  |
| RE-013 | `psp::dprintln!` is a 30x performance trap | COMPLETE | framebuffer, geometry, psp-ge |  |
| RE-014 | GU debug text is invisible under PPSSPP's hardware backends | COMPLETE | psp-ge, framebuffer, hardware, effects |  |
| RE-015 | Unexplained horizontal drift *(RESOLVED — earlier hypothesis was wrong)* | COMPLETE |  |  |
| RE-016 | Measured frame budget at M1 | COMPLETE | fighter, hardware, psp-ge |  |
| RE-017 | `G_VTX` destination index encoding | COMPLETE | geometry, pack |  |
| RE-018 | Segmented addresses survive relocation | COMPLETE | relocations, geometry |  |
| RE-019 | `G_SETTIMG` format describes the load, not the render | COMPLETE | texture, stage |  |
| RE-020 | PSP integer vertex formats are normalised, not raw | COMPLETE | geometry, texture, psp-ge, camera |  |
| RE-021 | Lighting state is inherited, not carried by the display list | COMPLETE | lighting, geometry, material, texture |  |
| RE-022 | Mesh UVs run far outside 0..1 and need REPEAT | COMPLETE | texture, geometry, psp-ge |  |
| RE-023 | `DObjDesc` arrays are a depth-tagged flattened tree | COMPLETE | depth, relocations, geometry, fighter |  |
| RE-024 | The shipped light colours really are neutral | COMPLETE | lighting, material, pack |  |
| RE-025 | A `DObj`'s display-list field is an undiscriminated union | COMPLETE | geometry, fighter, relocations, stage |  |
| RE-026 | Fighters share a vertex cache across joints | COMPLETE | geometry, fighter, material, texture, relocations |  |
| RE-027 | A fighter's palette lives in a table another file names | COMPLETE | material, texture, fighter, relocations, geometry |  |
| RE-028 | A stage is one struct, and its material table is one word further along | COMPLETE | material, stage, camera, fighter, geometry |  |
| RE-029 | Stage collision is 2D polylines, and `vertex2` is a count | COMPLETE | geometry, stage |  |
| RE-030 | Surface flags say how a stage plays, and spawns prove the query | COMPLETE | stage, fighter, geometry, lighting, material |  |
| RE-031 | A fighter stands on a stage, and two solvers agree it is the right one | COMPLETE | fighter, stage, material |  |
| RE-032 | The fighters' real numbers, and what a guessed constant hides | COMPLETE | fighter, stage, texture, camera, pack |  |
| RE-033 | The status machine, and a tap that is a counter rather than an edge | COMPLETE | fighter, animation, effects, framebuffer, geometry |  |
| RE-034 | The pillarbox that was never applied, found by measuring a fighter | COMPLETE | fighter, stage, psp-ge, camera, geometry |  |
| RE-035 | Animation lengths, and the eighteen scripts that agree on each one | COMPLETE | fighter, animation, geometry, lighting, pack |  |
| RE-036 | The figatree's tracks, and the mask that says which joints exist | COMPLETE | fighter, geometry, animation, relocations, material |  |
| RE-037 | Why the stages draw white: the textures are in another file | COMPLETE | texture, relocations, stage, fighter, geometry |  |
| RE-038 | The pose was right | COMPLETE | fighter, animation, geometry, camera, material |  |
| RE-039 | Mario is grey because his colour is not in his model | COMPLETE | fighter, material, geometry, animation, combiner |  |
| RE-040 | A fighter's colours are a script, one costume per frame | COMPLETE | fighter, material, animation, geometry, texture |  |
| RE-041 | The other thirteen movement animations | COMPLETE | animation, fighter, geometry, pack, texture |  |
| RE-042 | The status machine drives the animation | COMPLETE | animation, fighter, stage, lighting, framebuffer |  |
| RE-043 | What is still white, measured | COMPLETE | texture, combiner, fighter, geometry, material |  |
| RE-044 | A tile's size is not the texture's size | COMPLETE | texture, stage, archive, fighter |  |
| RE-045 | An `MObj`'s texture is emitted twice under different flags | COMPLETE | material, texture, animation, fighter, relocations |  |
| RE-046 | A material's sprite table can leave the file too | COMPLETE | material, relocations, texture, stage, fighter |  |
| RE-047 | A discovered display list was decoded at the wrong base | COMPLETE | texture, framebuffer, material, relocations, effects |  |
| RE-048 | The odd shapes in Dream Land's canopy are billboards | COMPLETE | animation, stage, material, camera, geometry |  |
| RE-049 | Billboards, and a screenshot that could not have shown them | COMPLETE | camera, animation, stage, hardware, projection |  |
| RE-050 | Stage joints run on the 32-bit event stream | COMPLETE | animation, geometry, stage, effects, fighter |  |
| RE-051 | Stage scenery animates on device | COMPLETE | fighter, stage, animation, geometry, material |  |
| RE-052 | Checking the packed stage animation against the archive | COMPLETE | stage, animation, geometry, fighter, archive |  |
| RE-053 | Mipmaps, and a tree that is still not right | COMPLETE | texture, stage, hardware, lighting, psp-ge |  |
| RE-054 | BattleShip cross-reference (PLAN.md R0.2) | COMPLETE | texture, framebuffer, psp-ge, stage | R0.2 |
| RE-055 | The 26 segment-0x01 texture failures are the loading-transition photo, not S2DEX BG (PLAN.md R0.3) | COMPLETE | framebuffer, texture, effects, camera, geometry | R0.3 |
| RE-056 | A lead on the 4 MissingPalette cases (PLAN.md R0.3), not a fix | COMPLETE | texture, geometry | R0.3 |
| RE-057 | The 4 MissingPalette cases are a `PartTables` pairing gap, not a dedup artifact (PLAN.md R0.3 / R0.7) | COMPLETE | texture, material, fighter, geometry, framebuffer | R0.3 |
| RE-058 | `WPAttributes` is a second, unscanned pairing shape | COMPLETE | material, fighter, geometry, animation, texture |  |
| RE-059 | Two of file 353's three graphs paired via `EFDesc`, fixed (PLAN.md R0.7) | COMPLETE | material, effects, texture, fighter, pack | R0.7 |
| RE-060 | File 52 (`MVCommon`) fully resolved: a fourth pairing mechanism, code sequence not data (PLAN.md R0.7) | COMPLETE | material, texture, archive, camera, effects | R0.7 |
| RE-061 | File 86's last graph: measured, not guessed, and left open (PLAN.md R0.7) | COMPLETE | material, fighter, archive, effects, pack | R0.7 |
| RE-062 | `0x8000`/`RecalcRotRpyRSca` is a spin-free billboard, same as kinds 46/48 (PLAN.md R0.8) | COMPLETE | animation, toolchain, pack, camera, hardware | R0.8 |
| RE-063 | Kinds 33-40 are runtime-only | COMPLETE | animation, camera, toolchain, pack, archive |  |
| RE-064 | Cross-node palette/texture inheritance, pinned by a direct test | COMPLETE | texture, geometry, material, hardware, archive |  |
| RE-065 | The baked key light is now a real, measured stage angle | COMPLETE | lighting, stage, pack, toolchain, camera |  |
| RE-066 | The hardcoded `Repeat` wrap mode is already correct | COMPLETE | texture, hardware, psp-ge, geometry, archive |  |
| RE-067 | `G_TX_MIRROR` traced to Dream Land's canopy and fixed, at a real VRAM cost | COMPLETE | texture, stage, toolchain, geometry, psp-ge |  |
| RE-068 | The archive-wide geometry-mode default was backwards | COMPLETE | depth, lighting, archive, material, texture |  |
| RE-069 | Alpha test and blending: one shipped, one found broken and deferred | COMPLETE | texture, geometry, psp-ge, stage, toolchain |  |
| RE-070 | Pre-blurring the canopy's dither measurably helps, modestly | COMPLETE | texture, stage, toolchain, effects, lighting |  |
| RE-071 | RE-070's dither fix does not make RE-069's blend safe (two ruled-out leads) | COMPLETE | lighting, texture, combiner, effects, material |  |
| RE-072 | Fog really is unused, but not for the reason first measured | COMPLETE | framebuffer, archive, effects |  |
| RE-073 | A texture-driven PRIM/ENV blend `combiner_shade_scale` declines | COMPLETE | combiner, texture, geometry, psp-ge, fighter |  |
| RE-074 | The texture blend's "shared vertex" risk was already handled | COMPLETE | texture, geometry, combiner, fighter, material |  |
| RE-075 | Blur/mirror order fixed for correctness, not confirmed visible | COMPLETE | texture, camera, psp-ge, stage |  |
| RE-076 | The 1170.9 KiB VRAM figure is an archive-wide total, not what any one scene needs | COMPLETE | fighter, texture, stage, archive, memory |  |
| RE-077 | Most fighters are already fully paired | COMPLETE | fighter, material, texture, geometry, relocations |  |
| RE-078 | Six more graphs fixed archive-wide by the same search-plus-decomp method | COMPLETE | material, fighter, lighting, archive, stage |  |
| RE-079 | Systematic combiner-shape census finds a real black-scale bug and an over-strict gate | COMPLETE | combiner, archive, texture, material, toolchain |  |
| RE-080 | A third combiner shape, flat constant colour, detected and packed | COMPLETE | texture, combiner, geometry, toolchain, archive |  |
| RE-081 | Dream Land's two canopy textures are scaled oppositely | COMPLETE | texture, stage, camera, hardware, lighting |  |
| RE-082 | Re-auditing RE-034's aspect-ratio residual: a measurement artifact, not a bug | COMPLETE | projection, camera, psp-ge, fighter, stage |  |
| RE-083 | Billboard census: depth is uniform, the `rot_mode` worry was already answered, and `translucent` hits billb... | COMPLETE | animation, depth, archive, camera, stage |  |
| RE-084 | The debug viewer's FOV was an unsourced guess | COMPLETE | camera, stage, toolchain, projection, effects |  |
| RE-085 | Depth range inversion matches the PSP SDK's own documented convention exactly | COMPLETE | depth, psp-ge, hardware, stage, fighter |  |
| RE-086 | Stage "material animation" is mostly palette cycling, not colour: R0.10 archive-wide census before implemen... | COMPLETE | texture, animation, material, archive, stage |  |
| RE-087 | A tick-based `PaletteID` decoder, verified against a real script's exact shape | COMPLETE | material, geometry, texture, animation, fighter |  |
| RE-088 | `MObjSub.palettes[1..]` cannot be recovered from local ROM structure alone | COMPLETE | texture, material, relocations, archive, geometry |  |
| RE-089 | `p_matanim_joints` resolved into per-(node, MObj) script references | COMPLETE | texture, geometry, material, fighter, archive |  |
| RE-090 | `mobj::read_palettes` reads the real array using RE-089's bound: 33/33 correct, 0 failures, archive-wide | COMPLETE | texture, material, relocations, toolchain, animation |  |
| RE-091 | Pack format shipped for animated palettes | COMPLETE | texture, material, animation, geometry, combiner |  |
| RE-092 | Animated palettes correlated to their texture through `mesh.rs`'s existing state, and packed for real: 17/3... | COMPLETE | texture, material, geometry, animation, stage |  |
| RE-093 | A shared-texture `G_LOADTLUT` was clearing the image binding instead of restoring it, dropping both animati... | COMPLETE | texture, material, toolchain, hardware, archive |  |
| RE-094 | `Cmd::Texture`'s inherited `off` was suppressing a later node's own complete texture setup | COMPLETE | texture, geometry, toolchain, archive, animation |  |
| RE-095 | `MaterialAnimator`: the device-side player, wired into every draw path | COMPLETE | texture, animation, material, geometry, stage |  |
| RE-096 | Fighter costume material scripts never loop | COMPLETE | fighter, material, texture, animation, geometry |  |
| RE-097 | `colors_at` now reads `PaletteID` | COMPLETE | fighter, texture, material, archive, geometry |  |
| RE-098 | Multi-costume packing: shared geometry, per-node substitute meshes only where content actually differs | COMPLETE | fighter, texture, geometry, material, archive |  |
| RE-099 | The LB transition photocopy is a one-time snapshot sampled as an ordinary texture, not a per-frame render pass | COMPLETE | framebuffer, texture, animation, psp-ge, effects |  |
| RE-100 | RE-099's "one full 300x220 capture" hypothesis was wrong | COMPLETE | texture, framebuffer, psp-ge, geometry, material | R0.13 |
| RE-101 | `G_TEXTURE`'s `scale_s`/`scale_t` was never applied | COMPLETE | texture, geometry, fighter, hardware, archive |  |
| RE-102 | `G_TX_CLAMP` was dropped entirely | COMPLETE | texture, fighter, psp-ge, hardware, framebuffer | R0.5 |
| RE-103 | Lit-vs-literal-colour was decided per *primitive* by majority vote | COMPLETE | geometry, fighter, lighting, material, hardware |  |
| RE-105 | `G_MW_LIGHTCOL` is the one in-list, ROM-verified signal that a segment is about to draw lit geometry | COMPLETE | lighting, geometry, material, effects, fighter |  |
| RE-106 | `MeshMaterial::prim_color` is `combiner_shade_scale`'s result, not a literal colour | COMPLETE | geometry, texture, combiner, material, pack |  |
| RE-107 | RE-101–RE-106 recovered and documented retroactively | COMPLETE | archive, framebuffer, geometry, material, texture |  |
| RE-108 | The "black rectangle" was misattributed from the start | COMPLETE | framebuffer, texture, geometry, depth, toolchain |  |
| RE-109 | Shipped RE-108's option (b): rebasing a framebuffer-role primitive's UV by its own tile origin, fixing the... | COMPLETE | texture, framebuffer, geometry, camera, toolchain |  |
| RE-110 | RE-109's fix confirmed on the real device: the previously-black region now renders the captured colour exactly | COMPLETE | framebuffer, geometry, texture, archive, material |  |
| RE-111 | RE-110's "backing quad renders black" was misattributed too: the real cause is the pillarbox scissor, and `... | COMPLETE | framebuffer, texture, psp-ge, archive, geometry |  |
| RE-112 | The "backing quad" does not exist as reachable geometry: it is a duplicate, out-of-context decode of the sa... | COMPLETE | geometry, texture, framebuffer, archive, toolchain |  |
| RE-113 | Four more transition files visually confirmed clean | COMPLETE | framebuffer, archive, camera, toolchain, stage |  |
| RE-114 | All 13 transition files now accounted for: 9 confirmed clean, 3 blocked on the known camera-framing gap, 1... | COMPLETE | camera, framebuffer, toolchain, archive, material |  |
| RE-115 | Fixed the debug-viewer camera-framing gap: it was backface culling, not the camera | COMPLETE | camera, material, toolchain, framebuffer, psp-ge |  |
| RE-116 | File 46's "diagonal banding" was never a defect: RE-113's own pixel census had the same window-border confo... | COMPLETE | framebuffer, toolchain, archive, effects, memory |  |
| RE-117 | R0.15 started: nine of ten render-state categories had no cross-node persistence test at all | COMPLETE | texture, geometry, combiner, material, toolchain |  |
| RE-118 | `psp/src/meshdraw.rs::DrawState`'s own GE cache audited: one real gap found and fixed (the collision/fighte... | COMPLETE | texture, psp-ge, fighter, material, geometry |  |
| RE-119 | R0.16 started: a real bug in `romtool`'s own diagnostic labelling, a stale opcode table, and two genuinely... | COMPLETE | geometry, texture, archive, combiner, texgen |  |
| RE-120 | `G_SHADE`-off-with-a-shade-reading-combiner cross-referenced: 29 of 31 archive-wide cases are in content th... | COMPLETE | combiner, fighter, archive, effects, geometry |  |
| RE-121 | `blend_color` is correctly dropped between `mesh.rs` and `pack.rs` (measured, not assumed) | COMPLETE | material, archive, geometry, pack, texture |  |
| RE-122 | `TexKey` ignored wrap/mirror/clamp mode entirely: 126 archive-wide cases where two different-wrap bindings... | COMPLETE | texture, archive, geometry, material, toolchain |  |
| RE-123 | A deterministic capture mode for R0.17's visual regression methodology, and a PPSSPP debug-overlay pitfall... | COMPLETE | animation, toolchain, visual-regression, fighter, psp-ge |  |
| RE-124 | Reference-port comparative audit against `sf64-psp` and `oot-PSP` (`PLAN.md` R0.18) | COMPLETE | psp-ge, texture, hardware, material, combiner | R0.18 |
| RE-125 | 20 more material-table pairings found by systematically re-checking every "several candidates" graph agains... | COMPLETE | visual-regression, material, psp-ge, texture, archive |  |
| RE-126 | Kind48's camera-pitch-locked billboard is measurably real (47 nodes, including Dream Land), and this projec... | COMPLETE | camera, animation, stage, pack, archive |  |
| RE-127 | Real N64 LOD/mipmapping is never engaged archive-wide | COMPLETE | texture, archive, hardware, geometry, toolchain | R0.5 |
| RE-128 | RE-101/RE-102 confirmed correct on two real fighters' own face textures, and a real, unexplained black patc... | COMPLETE | texture, fighter, lighting, geometry, material | R0.5 |
| RE-129 | The canopy-highlight's real alpha combiner reads `SHADE_ALPHA`, and naively wiring that up breaks Dream Lan... | COMPLETE | geometry, combiner, texture, stage, toolchain | R0.6 |
| RE-130 | Classified the real alpha-blend formula archive-wide and shipped it: `PLAN.md` R0.6's "blending verified" i... | COMPLETE | combiner, geometry, archive, texture, visual-regression |  |
| RE-131 | A real, decomp-ported battle camera (`gmCameraDefaultFuncCamera`), replacing the debug viewer's fixed face-... | COMPLETE | camera, fighter, lighting, stage, psp-ge | R0.12 |
| RE-132 | RE-131's own camera silently broke `Kind46` billboards | COMPLETE | camera, animation, toolchain, stage, visual-regression |  |
| RE-133 | `Kind48`'s real, camera-pitch-locked billboard transform, implemented and shipped | COMPLETE | camera, animation, toolchain, visual-regression, stage |  |
| RE-134 | Billboard scale: measured, not fixed — the real and approximate formulas never actually diverge | COMPLETE | animation, archive, toolchain, depth |  |
| RE-135 | Most billboard translucency is already real blending, thanks to RE-130 — measured, not assumed | COMPLETE | archive, animation, toolchain |  |
| RE-136 | Billboards spot-checked on three more stages beyond Dream Land — real, incremental progress, not a closed item | COMPLETE | stage, animation, archive, toolchain, visual-regression |  |
| RE-137 | Every stage with billboard content has now been spot-checked on-device at least once | COMPLETE | animation, stage, toolchain, visual-regression, archive |  |
| RE-138 | Billboard texture orientation: no separate mechanism exists to verify, confirmed by direct reading on both... | COMPLETE | texture, animation, geometry, material, toolchain |  |
| RE-139 | R0.6's "unsupported material behavior identified" closed by compiling what this task's own history already... | COMPLETE | archive, combiner, lighting, material, toolchain |  |
| RE-140 | Repeatable source-indexed billboard rest-pose inventory | COMPLETE | animation, toolchain, archive, camera, geometry |  |
| RE-141 | Correct billboard spin semantics by ROM matrix kind | COMPLETE | animation, toolchain, hardware, pack, stage |  |
| RE-142 | Animated billboard census finds and fixes null-script child inheritance | COMPLETE | animation, stage, geometry, toolchain, archive |  |
| RE-143 | Signed animated billboard scale now follows the original tree accumulator | COMPLETE | animation, toolchain, fighter, geometry, stage |  |
| RE-144 | Per-node billboard audit exposes and fixes omitted Z scale | COMPLETE | animation, stage, geometry, visual-regression, camera |  |
| RE-145 | Fixed-tick PPSSPP capture completes the 109-node billboard audit | COMPLETE | animation, geometry, texture, toolchain, archive |  |
| RE-146 | The production LB wipe contract is 11 finite results-screen animations | COMPLETE | framebuffer, animation, camera, geometry, fighter |  |
| RE-147 | All eleven results wipes are packed and replay from the generated pack | COMPLETE | framebuffer, animation, fighter, geometry, stage |  |
| RE-148 | Results wipes now preserve the original capture-before-entry frame boundary | COMPLETE | framebuffer, animation, stage, camera, archive |  |
| RE-149 | R0.13 closes at the renderer boundary | COMPLETE | framebuffer, camera, animation, psp-ge |  |
| RE-150 | Original-output comparison fixes two camera-port errors | COMPLETE | camera, fighter, visual-regression, animation, stage |  |
| RE-151 | A scripted original-ROM trace closes the default battle-camera comparison | COMPLETE | fighter, camera, stage, effects, hardware |  |
| RE-152 | Fox's black face was a nonzero clamp-window coordinate bug (`PLAN.md` R0.5/R0.6) | COMPLETE | texture, fighter, visual-regression, lighting, stage | R0.5 |
| RE-153 | Source-paired all ten character-select/result emblem material tables (`PLAN.md` R0.6/R0.7) | COMPLETE | material, fighter, texture, toolchain, archive | R0.6 |
| RE-154 | Four static `EFDesc` records recover file-84 common-effect materials (`PLAN.md` R0.6/R0.7) | COMPLETE | material, effects, texture, framebuffer, toolchain | R0.6 |
| RE-155 | Direct effect and Bonus2 platform pairings reduce the material tail (`PLAN.md` R0.6/R0.7) | COMPLETE | material, toolchain, effects, archive, texture | R0.6 |
| RE-157 | Static effect descriptors recover Kongo Jungle, Pikachu, and Ness materials (`PLAN.md` R0.6/R0.7) | COMPLETE | material, effects, fighter, toolchain, texture | R0.6 |
| RE-158 | Static effect descriptors recover five special-move material tables (`PLAN.md` R0.6/R0.7) | COMPLETE | fighter, material, toolchain, effects, pack | R0.6 |
| RE-159 | Explanation-screen control-stick material table follows the original call sequence (`PLAN.md` R0.6/R0.7) | COMPLETE | material, toolchain, texture, archive | R0.6 |
| RE-160 | Source descriptors resolve Sector Z, Final Destination, and Dream Land material tables (`PLAN.md` R0.6/R0.7) | COMPLETE | material, stage, framebuffer, texture, archive | R0.6 |
| RE-161 | Link-model's final unpaired graph has no typed source relationship (`PLAN.md` R0.6/R0.7) | COMPLETE | geometry, material, fighter, texture, pack | R0.6 |
| RE-162 | Typed N-Bumper item attributes resolve the final palette gap (`PLAN.md` R0.4/R0.6/R0.7) | COMPLETE | material, texture, pack, relocations, toolchain | R0.4 |
| RE-163 | Link passive-part dispatch resolves the final material table (`PLAN.md` R0.6/R0.7) | COMPLETE | material, fighter, geometry, texture, toolchain | R0.6 |
| RE-164 | Runtime-lighting inputs survive pack conversion (`PLAN.md` R0.6) | COMPLETE | lighting, geometry, psp-ge, stage, fighter | R0.6 |
| RE-165 | `G_MW_LIGHTCOL` carries the missing directional/ambient colour state (`PLAN.md` R0.6) | COMPLETE | lighting, material, fighter, stage, toolchain | R0.6 |
| RE-166 | Light colours are independent, zero-valid `G_MW_LIGHTCOL` state (`PLAN.md` R0.6) | COMPLETE | lighting, material, toolchain, fighter, framebuffer | R0.6 |
| RE-167 | Runtime lighting must retain the combiner's costume-colour scale (`PLAN.md` R0.6) | COMPLETE | fighter, lighting, material, combiner, psp-ge | R0.6 |
| RE-168 | Post-table-resolution combiner census closes the stale colour attribution (`PLAN.md` R0.6) | COMPLETE | texture, geometry, material, archive, combiner | R0.6 |
| RE-169 | Deterministic physical-PSP staging closes the stale-build hole (`PLAN.md` R0.5) | COMPLETE | hardware, stage, framebuffer, visual-regression | R0.5 |
| RE-170 | Exhaustive PPSSPP stage audit makes R1's first row reproducible (`PLAN.md` R1) | COMPLETE | stage, hardware | R1 |
| RE-171 | Exhaustive fighter-animation rendering exposes a sparse lookup bug (`PLAN.md` R1) | COMPLETE | fighter, animation, stage, framebuffer, geometry | R1 |
| RE-172 | Manager-effect inventory recovers direct objects and corrects shifted file-84 materials (`PLAN.md` R1) | COMPLETE | effects, fighter, material, particles, texture | R1 |
| RE-173 | Exhaustive manager-effect rest-pose audit identifies three authored invisible states (`PLAN.md` R1) | COMPLETE | effects, animation, fighter, geometry, material | R1 |
| RE-174 | Manager-effect transform AObjEvent32 streams pack and replay (`PLAN.md` R1) | COMPLETE | effects, animation, fighter, material, geometry | R1 |
| RE-175 | Manager-effect material `AObjEvent32` tables pack and host-replay (`PLAN.md` R1) | COMPLETE | texture, material, effects, animation, fighter | R1 |
| RE-176 | Manager-effect material AObjEvent32 texture-swap consumption on the PSP renderer (`PLAN.md` R1) | COMPLETE | effects, texture, material, animation, camera | R1 |
| RE-177 | Closes 6 of RE-175's 9 unresolved manager-effect sprite tables (`PLAN.md` R1) | COMPLETE | texture, material, effects, animation, geometry | R1 |
| RE-178 | Closes RE-177's remaining 3-table manager-effect material gap (`PLAN.md` R1) | COMPLETE | geometry, effects, texture, material, toolchain | R1 |
| RE-179 | Manager-effect live colour tracks reach PSP draw state (`PLAN.md` R1) | COMPLETE | effects, material, texture, geometry, lighting | R1 |
| RE-180 | All LBParticle banks decode from the original ROM (`PLAN.md` R1) | COMPLETE | texture, particles, effects, toolchain, animation | R1 |
| RE-181 | LBParticle banks survive PSP pack conversion (`PLAN.md` R1) | COMPLETE | texture, particles, effects, toolchain | R1 |
| RE-182 | Deterministic single-particle `LBParticle` script playback (`PLAN.md` R1) | COMPLETE | particles, archive, toolchain, animation | R1 |
| RE-183 | PSP-side `LBParticle` billboard drawing, single-script proof of concept (`PLAN.md` R1) | COMPLETE | particles, texture, animation, psp-ge, archive | R1 |
| RE-184 | Exhaustive frame-4 `LBParticle` visibility census (`PLAN.md` R1) | COMPLETE | particles, archive, toolchain, texture, effects | R1 |
| RE-185 | Exhaustive on-device `LBParticle` frame-4 screenshot sweep, and a real camera-transform bug it found (`PLAN... | COMPLETE | particles, texture, toolchain, camera, archive | R1 |
| RE-186 | Archive-wide `LBParticle` combine-mode census: `NOISE`/`DITHER`/`ALPHABLEND` are real but unreachable at fr... | COMPLETE | particles, archive, toolchain, geometry, psp-ge | R1 |
| RE-187 | Multi-particle spawn-tree execution: `MAKESCRIPT`/`MAKERAND`/`MAKEID` actually spawn, one archive-wide resc... | COMPLETE | particles, toolchain, archive, memory, texture | R1 |
| RE-188 | `LBGenerator` implemented: cone/line spawn math ported, vortex declines to the existing `VortexUnsupported`... | COMPLETE | particles, archive, toolchain, effects, memory | R1 |
| RE-189 | real manager-effect spawn event wired into runtime: `efManagerRippleMakeEffect` ticks and draws a live `LBG... | COMPLETE | particles, effects, toolchain, archive, camera | R1 |
| RE-190 | Framebuffer-path census: exactly one content-bearing mechanism remains outside R0.13 | COMPLETE | framebuffer, texture, archive, memory, pack | R1 |
| RE-191 | Wallpaper-capture copy-order fully explained by the destination `Sprite`'s own header | COMPLETE | texture, framebuffer, pack, hardware, material | R1 |
| RE-192 | Wallpaper-capture mechanism implemented and device-verified | COMPLETE | framebuffer, texture, toolchain, psp-ge, stage | R1 |
| RE-193 | Minimal `SObj` 2D-sprite port renders the wallpaper capture through a real GE draw, closing R1's framebuffe... | COMPLETE | framebuffer, geometry, texture, psp-ge, toolchain |  |
| RE-194 | `gcDrawMObjForDObj`'s runtime tile/texture-scale state, measured and reproduced | COMPLETE | material, texture, stage, toolchain, geometry | R1 |
| RE-195 | `G_SETOTHERMODE_H`/`L`'s remaining undecoded fields measured archive-wide | COMPLETE | texture, geometry, toolchain, archive, hardware | R1 |
| RE-196 | Reconciling `PLAN.md` R1's asset/material bullets against existing evidence (no code change) | COMPLETE | material, texture, archive, combiner, framebuffer | R0.6 |
| RE-197 | Full regression stack rerun closes `PLAN.md` R1's "rendering regression suite passes" bullet | COMPLETE | visual-regression, stage, toolchain, animation, material |  |
| RE-198 | Concrete file/offset evidence for three "needs identification" test-matrix rows (`PLAN.md` R1) | COMPLETE | texture, archive, fighter, geometry, material | R1 |
| RE-199 | Second deterministic `regression_capture` scene closes two of RE-198's six test-matrix rows (`PLAN.md` R1) | COMPLETE | texture, visual-regression, toolchain, geometry, combiner | R1 |
| RE-200 | Final four golden-render matrix rows identified and captured (`PLAN.md` R1) | COMPLETE | texture, visual-regression, archive, stage, toolchain | R1 |
| RE-201 | PSPLink hardware run resolves R0.5's deferred Dream Land check (`PLAN.md` R0.5/R2) | COMPLETE | hardware, stage, geometry, animation, material | R0.5 |
| RE-202 | Interactive viewer's debug HUD crashes real PSP hardware, content-independent (`PLAN.md` R2) | COMPLETE | hardware, psp-ge, toolchain, stage, fighter | R2 |
| RE-203 | All four golden regression scenes verified on physical PSP hardware (`PLAN.md` R2) | COMPLETE | hardware, visual-regression, texture, fighter, combiner | R2 |
| RE-204 | Framebuffer-effect sprite path and VRAM budget verified on physical PSP hardware (`PLAN.md` R2) | COMPLETE | hardware, framebuffer, memory, effects, texture | R2 |
| RE-205 | Stage animation verified on physical PSP hardware | COMPLETE | animation, hardware, stage, visual-regression, geometry | R2 |
| RE-206 | Swept the crate for other unguarded/speculatable divisions like RE-201/202/205's FPU traps (`PLAN.md` R2) | COMPLETE | animation, hardware, camera, fighter, depth | R2 |
| RE-207 | Sixth golden scene (Fox) verified on physical PSP hardware (`PLAN.md` R2) | COMPLETE | fighter, hardware, visual-regression, toolchain, psp-ge | R2 |
| RE-208 | Seventh golden scene (Captain Falcon) verified on physical PSP hardware (`PLAN.md` R2) | COMPLETE | fighter, hardware, visual-regression, toolchain, stage | R2 |
| RE-209 | Eighth golden scene (Kirby) verified on physical PSP hardware (`PLAN.md` R2) | COMPLETE | fighter, hardware, visual-regression, texture, toolchain | R2 |
| RE-210 | Ninth golden scene (Ness) verified on physical PSP hardware (`PLAN.md` R2) | COMPLETE | fighter, hardware, visual-regression, toolchain, geometry | R2 |
| RE-211 | Rendering-gap audit against current decomp/runtime state (`PLAN.md` R0.10/R0.11/R1/R2) | COMPLETE | fighter, material, texture, stage, animation | R0.10 |
| RE-212 | Tenth golden scene (Donkey Kong) verified on physical PSP hardware | COMPLETE | fighter, hardware, visual-regression, toolchain, effects | R2 |
| RE-213 | Texgen/mip physical capture | COMPLETE | texgen, texture, fighter, hardware, psp-ge |  |
| RE-214 | Texgen correctness recovery: raw geometry bits, vertex-load census, and the GE texture-matrix generator (`P... | COMPLETE | texture, texgen, geometry, fighter, camera | R2 |
| RE-215 | Exact `G_TEXTURE_GEN_LINEAR`, and scenes 11/12 never actually exercised it (`PLAN.md` R2) | COMPLETE | texture, texgen, visual-regression, geometry, archive | R2 |
| RE-216 | RE-151's harness rebuilt, and the "Metal Box item" route corrected (`PLAN.md` R2) | COMPLETE | fighter, stage, material, memory, texgen | R2 |
| RE-217 | Renderer-plan reconciliation reopens four correctness claims (`PLAN.md` R0/R2) | COMPLETE | material, geometry, texgen, lighting, texture | R0 |
| RE-218 | External rendering-fidelity audit reopens filtering/addressing claims (`PLAN.md` R2.0) | COMPLETE | texture, archive, geometry, texgen, hardware | R2.0 |
| RE-219 | N64 3-point filtering vs PSP bilinear: real, material, unfixable difference | COMPLETE | texture, hardware, archive, geometry, psp-ge | R2.0 |
| RE-220 | General N64 tile-addressing reference model: two real gaps, one closed invariant (`PLAN.md` R2.0/P0b) | COMPLETE | texture, archive, hardware, psp-ge, toolchain | R2.0 |
| RE-221 | Fix mirror+clamp addressing beyond the first mirrored period (`PLAN.md` R2.0/P0c) | COMPLETE | texture, archive, fighter, toolchain, psp-ge | R2.0 |
| RE-222 | Fix PSP POT-padding vs the N64 logical clamp boundary (`PLAN.md` R2.0/P0d) | COMPLETE | texture, toolchain, archive, particles, geometry | R2.0 |
| RE-223 | Archive-wide `G_SETTILE` field census: `palette` is a real, material, still-open gap | COMPLETE | texture, archive, toolchain, geometry, material | R2.0 |
| RE-224 | Fix ignored CI4 palette bank, `G_SETTILE.palette` (`PLAN.md` R2.0/P2) | COMPLETE | texture, geometry, toolchain, hardware, archive | R2.0 |
| RE-225 | `G_VTX` model-space invariance census: not invariant, systematic joint-boundary gap found, remedy deferred... | COMPLETE | geometry, texgen, archive, fighter, psp-ge | R2.1 |
| RE-226 | `GU_NORMAL_8BIT` raw texgen semantics measured, `NormalizedNormal` bug fixed (`PLAN.md` R2.1/T2) | COMPLETE | psp-ge, hardware, texture, toolchain, texgen | R2.1 |
| RE-227 | Original LookAt basis is signed-byte quantized, not continuous float | COMPLETE | camera, texgen, visual-regression, hardware, texture | R2.1 |
| RE-228 | Shared regular/linear texgen reference math | COMPLETE | texture, texgen, visual-regression, psp-ge, toolchain | R2.1 |
| RE-229 | Linear texgen's S10.5 conversion truncates, it does not round (`PLAN.md` R2.1/T5) | COMPLETE | texgen, texture, visual-regression, hardware, psp-ge | R2.1 |
| RE-230 | Tile-state and lighting audit for texgen-bound draws: `shift_s`/`shift_t` invariant confirmed on the texgen... | COMPLETE | texgen, texture, archive, lighting, geometry | R2.1 |
| RE-231 | Texgen addressing census: real N64/PSP agreement confirmed for 25 of 34 real axis instances | COMPLETE | texture, texgen, archive, material, toolchain | R2.1 |
| RE-232 | Fixed the mask-narrowed clamp-without-mirror texgen addressing divergence RE-231 found (`PLAN.md` R2.1/T7a) | COMPLETE | texture, toolchain, archive, texgen, hardware | R2.1 |
| RE-233 | Fixed real-hardware-only collision/fighter debug-overlay corruption: `LINE_BUF` reused before the GE finish... | COMPLETE | psp-ge, hardware, fighter, geometry, stage |  |
| RE-234 | Original-ROM stage-8 "VS Metal Mario" capture obtained via real 1P Mode play (`PLAN.md` R2.1/T8, in progress) | COMPLETE | stage, fighter, memory, material, texgen | R2.1 |
| RE-235 | Refreshed the metal-texgen PPSSPP goldens post-T7a, cross-checked against the RE-234 original-ROM captures... | COMPLETE | visual-regression, texgen, texture, lighting, material | R2.1 |
| RE-236 | Physical-PSP leg of T8 captured against the refreshed goldens | COMPLETE | hardware, visual-regression, texgen, stage, camera |  |
| RE-237 | Physical-PSP raw normal diagnostic and camera-rotation case close the `R2.1`/T9 matrix | COMPLETE | hardware, camera, texgen, visual-regression, psp-ge |  |
| RE-238 | T10 texgen documentation/test cleanup: `romtool texgen --verify`, zero-normal coverage, stale `porting-stat... | COMPLETE | texgen, texture, archive, framebuffer, toolchain | R2.1 |
| RE-239 | T10 closed: `textured→untextured→texgen` transition measured absent from the real archive, covered syntheti... | COMPLETE | texture, texgen, framebuffer, material, geometry | R2.1 |
| RE-240 | `push_vertex` was baking colour into lit vertices' normals, then `pack.rs` was scaling `prim_color` twice (... | COMPLETE | geometry, fighter, pack, archive, material | R2.2 |
| RE-241 | vertex normal-vs-colour meaning was decided at triangle-draw time, not `G_VTX` load time (`PLAN.md` R2.2/C2) | COMPLETE | geometry, lighting, fighter, material, archive | R2.2 |
| RE-242 | The mesh/costume-file-to-fighter mapping RE-241 called missing already existed (`PLAN.md` R2.2/C2) | COMPLETE | fighter, relocations, geometry, material, lighting | R2.2 |
| RE-243 | Wiring the external-lighting seed closes `R2.2`/C2, but changes zero real vertices (`PLAN.md` R2.2/C2, comp... | COMPLETE | geometry, lighting, fighter, archive, toolchain | R2.2 |
| RE-244 | Independent depth compare/write state: a real 90% divergence from `G_ZBUFFER`, and the same external seed C... | COMPLETE | depth, fighter, geometry, archive, lighting | R2.2 |
| RE-245 | Ground render-layer 1 has the same external depth-state wrapper fighters do | COMPLETE | depth, archive, fighter, material, geometry | R2.2 |
| RE-246 | The dominant remainder of RE-244/245's depth-state gap was not an object-category wrapper at all, but a cam... | COMPLETE | framebuffer, depth, camera, archive, geometry | R2.2 |
| RE-247 | `ssb_rom::transition::ASSETS`'s "camera" entry pointed at the wrong file: a real pack-content bug, not just... | COMPLETE | framebuffer, camera, geometry, depth, archive | R2.2 |
| RE-248 | File 39 (`IFCommonObject`) is genuinely orphaned, never drawn by any code path | COMPLETE | archive, depth, framebuffer, geometry, texture | R2.2 |
| RE-249 | No further external depth-state wrapper exists archive-wide | COMPLETE | depth, stage, archive, camera, fighter | R2.2 |
| RE-250 | `PlannedList` now keeps `list_id` | COMPLETE | depth, geometry, toolchain, archive, pack | R2.2 |
| RE-251 | `apply_material` wired to independent depth-test/write state | COMPLETE | depth, visual-regression, stage, fighter, psp-ge | R2.2 |
| RE-252 | `merge_by_material` preserved only adjacent same-material runs, not real submission order (`PLAN.md` R2.2/C4) | COMPLETE | material, archive, geometry, effects, toolchain | R2.2 |
| RE-253 | `tools/romtool`'s texture cache key ignored crop/format/palette-shape state, silently sharing baked texture... | COMPLETE | texture, archive, fighter, material, toolchain |  |
| RE-254 | Systematic PSP GE cache isolation inventory: one narrower bypass than `forget_texture` already covered, har... | COMPLETE | texture, fighter, depth, lighting, visual-regression | R2.2 |
| RE-255 | C6 integrated regression pass finds the real pack no longer fits in PSP RAM: every golden capture is silent... | COMPLETE | memory, visual-regression, texture, toolchain, hardware | R2.2 |
| RE-256 | `MEMSIZE=1` fixes RE-255's pack-load failure | COMPLETE | toolchain, memory, visual-regression, hardware, stage | R2.2 |
| RE-257 | Per-scene explanation of RE-256's 15 golden diffs: all trace to RE-240 or RE-251, none is new corruption (`... | COMPLETE | visual-regression, fighter, texgen, archive, depth | R2.2 |
| RE-258 | Link's white tunic exposed missing costume-light-track propagation | COMPLETE | fighter, texture, lighting, visual-regression, animation | R2.2 |
| RE-259 | Scene 13's changed top bar is a non-linear RE-252/RE-253 texture-cache correction, not the linear-texgen pr... | COMPLETE | texture, visual-regression, texgen, hardware, memory | R2.2 |
| RE-260 | The physical-PSP "animation crash" is the regression build's intentional four-second freeze | COMPLETE | animation, hardware, fighter, memory, stage | R2.2 |
| RE-261 | Fighter costume light tracks restore Link's canonical green tunic and close the integrated renderer regress... | COMPLETE | fighter, lighting, animation, visual-regression, material | R2.2 |
| RE-262 | Signed clamped UVs require a float-coordinate PSP draw path | COMPLETE | texture, fighter, texgen, geometry, archive |  |
| RE-263 | Kirby's false eye spikes are a PSP texture-reconstruction artifact | COMPLETE | texture, fighter, lighting, material, stage |  |
| RE-264 | Fox's white extremities reveal a disabled PSP light channel | COMPLETE | fighter, lighting, stage, texture, material |  |
| RE-265 | Every playable fighter needs the in-game pose and fighter-light scope in its golden | COMPLETE | fighter, lighting, visual-regression, stage, animation |  |
| RE-266 | Capture identifiers name fighters and keep generic scenes contiguous | COMPLETE | fighter, visual-regression, camera, lighting, toolchain |  |
| RE-267 | Donkey Kong's CI4 textures require decoded-RGBA transport in the PSP regression capture | COMPLETE | texture, fighter, camera, hardware, lighting |  |
| RE-268 | RDP source pitch is independent of the visible texture-tile width | COMPLETE | texture, fighter, geometry, toolchain, hardware |  |
| RE-269 | RDP tile origin is not a source-image offset | COMPLETE | texture, geometry, fighter, hardware, toolchain |  |
| RE-270 | Physical confirmation of RE-262/264/269's Fox, Ness, Link and Donkey Kong fixes, and Dream Land's lighting... | COMPLETE | fighter, hardware, lighting, visual-regression, stage |  |
| RE-271 | Exhaustive fighter physical matrix | COMPLETE | fighter, hardware, psp-ge, depth, visual-regression |  |
| RE-272 | Fighter face textures show a repeating/aliased artifact absent from the raw ROM texture | COMPLETE — root cause found and fixed, see RE-280/RE-281 | texture, fighter, hardware, visual-regression, geometry |  |
| RE-273 | First physical confirmation on a second hardware unit (PSP-3000) | COMPLETE | hardware, visual-regression, fighter, psp-ge, stage |  |
| RE-274 | RE-272's forehead artifact traces to one self-contained, wide-UV draw call | COMPLETE — root cause found and fixed, see RE-280/RE-281 | texture, fighter, geometry, hardware, visual-regression |  |
| RE-275 | angrylion-rdp-plus reference-render harness ported to RMG's sandbox, segfaults on plugin startup | CLOSED — superseded. Root cause found (below), but the path is | texture, fighter, hardware, visual-regression, toolchain |  |
| RE-276 | Live N64 reference render clears Mario's face: a real PSP-side addressing bug, not a ROM quirk | COMPLETE | texture, fighter, hardware, visual-regression, toolchain |  |
| RE-277 | Dense per-texel sweep clears the addressing formula itself | COMPLETE — root cause found and fixed, see RE-280/RE-281 | texture, fighter, hardware, visual-regression |  |
| RE-278 | Fixed-coordinate GE sampling matches the addressing model exactly, including the Linear clamp-boundary blend | COMPLETE — root cause found and fixed, see RE-280/RE-281 | texture, fighter, hardware, visual-regression |  |
| RE-279 | RE-274's real primitive is correctly routed through the signed-UV float path | RESOLVED | texture, fighter, hardware, visual-regression |  |
| RE-280 | Root cause found: `Ci4`/`PsmT4` packing uses the wrong nibble order for the PSP GE | COMPLETE — fix applied and confirmed, see RE-281 | texture, fighter, hardware, visual-regression, asset-pipeline |  |
| RE-281 | RE-280's `Ci4`/`PsmT4` nibble-order fix applied, pack rebuilt, 18/22 goldens refreshed and explained | COMPLETE | texture, fighter, hardware, visual-regression, asset-pipeline |  |
| RE-282 | Physical-hardware confirmation of RE-281's `Ci4`/`PsmT4` fix (Mario) | COMPLETE | texture, fighter, hardware, visual-regression |  |
| RE-283 | Post-RE-281 texture/geometry defects survive across most playable fighters | COMPLETE (software-traced) — Fox's wrist defect, Samus's chest "black square", and |  |  |
| RE-284 | 30-minute physical-hardware sustained run, extending past the prior 10-minute sample | COMPLETE | hardware, visual-regression |  |
| RE-285 | New stage golden: Peach's Castle (scene 10), physically confirmed | COMPLETE | stage, visual-regression, hardware |  |
| RE-286 | Remaining 37 stage goldens added in one batch (software-only | COMPLETE (software); physical-PSP confirmation deferred as a follow-up batch | stage, visual-regression |  |
| RE-287 | Physical-PSP confirmation of RE-286's 37 new stage goldens | COMPLETE | stage, visual-regression, hardware |  |
| RE-288 | Physical PSP-1000 test: pack-load compatibility and an aborted second-unit 30-minute sustained run | COMPLETE | hardware, visual-regression, asset-pipeline |  |
| RE-289 | F1's `psp-game/` crate scaffolded: a second, independent EBOOT boots to an intro/menu state machine | COMPLETE (this increment) — scene loading, real text and Training Mode's combat sandbox remain, see `plans/gameplay/F1.md` | front-end, toolchain, hardware, visual-regression |  |
| RE-290 | `psp-game`'s Menu -> Training confirm transition, pixel-confirmed via PPSSPPHeadless (closes RE-289's open... | COMPLETE | front-end, toolchain, visual-regression |  |
| RE-291 | `psp-game` loads and parses the real asset pack | COMPLETE | front-end, toolchain, asset-pipeline, visual-regression |  |
| RE-292 | `psp-game`'s 3D pipeline, mesh drawing and real gameplay slice ported from `psp/` | COMPLETE | front-end, toolchain, visual-regression, physics, animation |  |
| RE-293 | `psp-game`'s Training Mode spawns a real, physics-ticked stationary dummy target | COMPLETE | front-end, physics |  |
| RE-294 | Training Mode's first real attack: Mario's jab, hitbox to hitstun | COMPLETE (numeric slice) — see "Still open" below | front-end, physics, gameplay |  |
| RE-295 | Real jump binding reaches the dummy's real spawn point | COMPLETE | front-end, physics, gameplay, input |  |
| RE-296 | First physical-PSP confirmation of `psp-game` | COMPLETE | front-end, hardware, memory, toolchain, visual-regression |  |
| RE-297 | Shared PSP GE renderer extracted into `psp-runtime` | COMPLETE | front-end, toolchain, visual-regression, hardware |  |
| RE-298 | PSP runtime refactor acceptance gate: software checks and physical-hardware smoke test both pass | COMPLETE | front-end, toolchain, visual-regression, hardware |  |
| RE-299 | Mario Super Jump Punch uses TransN root motion | COMPLETE | fighter, animation, asset-pipeline, gameplay |  |
| RE-300 | Mario Fireball's `WEAPON_EXTERNAL` translucency: `alpha_blend` seed fixed, CLUT-alpha runtime rendering roo... | COMPLETE — `alpha_blend` classification bug found and fixed (tested); root cause of the opaque-black-card rendering, an uninitialised-cache bug in `DrawState`'s GE texture-function tracking (`psp-runtime/src/meshdraw.rs`), found and fixed; confirmed on PPSSPP (`tests/golden/f1-training-fireball.png`) and real PSP hardware | fighter, rendering, texture, weapon, visual-regression |  |
| RE-301 | Stage material-track replay and pack identity correction | COMPLETE | material, animation, texture, pack, stage, psp-ge |  |
| RE-302 | Fighter floor shadows | COMPLETE for the current fighter-runtime scope | fighter, rendering, texture, collision, psp-ge, visual-regression |  |
| RE-303 | Motion-script async waits target animation frames | source verified; Fox normal-attack transcription uses this rule. |  |  |
| RE-304 | PSP GE sampling alignment measured and runtime lowering corrected | COMPLETE | texture, psp-ge, hardware, texgen, material |  |
| RE-305 | Build-time N64 3-point texture compensation | COMPLETE (software and PPSSPP; physical-PSP capture unavailable this batch) | texture, asset-pipeline, pack, psp-ge, visual-regression |  |
| RE-306 | Material-aware alpha compensation (RE-305 follow-up) | COMPLETE (software and PPSSPP; physical-PSP capture unavailable this batch) | texture, asset-pipeline, pack, psp-ge, visual-regression |  |
| RE-307 | Filter-aware palette-index optimization (RE-305/RE-306 follow-up) | COMPLETE (software and PPSSPP; physical PSP not captured) | texture, asset-pipeline, pack, psp-ge, visual-regression |  |
| RE-308 | Animated indexed N64 filter compensation | COMPLETE (host and PPSSPP smoke; physical PSP not captured) | texture, material-animation, asset-pipeline, pack, psp-ge |  |
| RE-309 | GE-exact integer refinement after 3-point compensation | COMPLETE (host and PPSSPP smoke; physical PSP not captured) | texture, asset-pipeline, pack, psp-ge |  |
| RE-310 | Critical authored-UV coverage and independent filter validation | COMPLETE (host and PPSSPP smoke; physical PSP not captured) | texture, asset-pipeline, pack, psp-ge |  |
| RE-311 | Real-normal texgen coverage for 3-point compensation | COMPLETE (host and PPSSPP smoke; physical PSP not captured) | texture, texgen, asset-pipeline, pack, psp-ge |  |
| RE-312 | Final 3-point residual census | COMPLETE (census plus post-RE-313 deployment; host and PPSSPP verified, no physical PSP capture) | texture, asset-pipeline, pack, psp-ge |  |
| RE-313 | Texture-LUT semantics, short TLUTs and format-agnostic 3-point compensation | COMPLETE (host measurement, original-game frame capture replayed through `angrylion-rdp-plus`, PPSSPP smoke; physical PSP not captured) | texture, asset-pipeline, pack, psp-ge, rdp |  |
| RE-314 | Texture dimensions over 512 overflow the GE `TSIZE` encoding | COMPLETE (host test, PPSSPP software A/B captures, 11 stage goldens refreshed; physical PSP not captured) | texture, psp-ge, pack, visual-regression |  |
| RE-315 | Viewer animators tick per simulation tick, not per render frame | COMPLETE (PPSSPP software captures of all 67 goldens with two padded packs, 27 goldens refreshed; physical PSP not captured) | animation, visual-regression |  |
| RE-316 | One-build golden captures with exit-on-capture | COMPLETE (PPSSPP software: all 67 scenes byte-identical to the per-feature pipeline at -j 1 and -j 24; physical PSP not run) | visual-regression, tooling |  |
| RE-317 | Attribution and rebaseline of the eight known-failing goldens | COMPLETE (PPSSPP software: commit-by-commit captures since a77e539; 8 goldens rebaselined, all 67 pass; physical PSP not captured) | visual-regression, texture |  |
| RE-318 | Lowering tiles whose bake exceeds the GE 512-texel limit | COMPLETE (host equivalence tests, PPSSPP software A/B and hidden-primitive controls, 10 stage goldens rebaselined; physical PSP not captured) | texture, psp-ge, pack, asset-pipeline, visual-regression |  |
| RE-319 | Texture buffer rows under 16 bytes are read at the wrong pitch | COMPLETE (PPSSPP and PSP-2000 stripe diagnostics, host tests, 44 goldens refreshed) | texture, psp-ge, pack, visual-regression |  |
| RE-320 | PSP-2000 confirms the 16-byte T4 texture-row pitch | COMPLETE (physical PSP A/B stripe diagnostic and stock-pack smoke) | texture, psp-ge, hardware, pack, visual-regression |  |
| RE-321 | RDP two-tile fractional blend (`SetLFrac` + `TextureIDNext`) on the GE | COMPLETE (PPSSPP exact-formula check, N64 reference, PSP-2000 timing and capture) | material, animation, texture, combiner, psp-ge, pack, stage, visual-regression, hardware |  |
| RE-322 | Dynamic stage `PrimColor` and `Light1Color`/`Light2Color` tracks | COMPLETE (host reference exact; PPSSPP A/B; PSP-2000 timing and capture; N64 reference partial) | material, animation, lighting, combiner, psp-ge, pack, stage, visual-regression, hardware |  |
| RE-323 | Task-list-1 XLU reset for static stage primitives | COMPLETE (pack diff measured per primitive; PPSSPP goldens; N64 references for Sector Z, Board the Platforms and Race to the Finish) | material, combiner, alpha, blending, depth, stage, pack, visual-regression |  |
| RE-324 | Material animation phase | COMPLETE (decomp reference exact at phase 0; PPSSPP goldens; pre-roll control) | material, animation, stage, visual-regression |  |
| RE-325 | Material resolvers against the decomp draw path | COMPLETE (decomp reference exact on all 103 entries; host tests; PPSSPP goldens) | material, animation, texture, palette, pack, visual-regression |  |
| RE-326 | Material sampling: texture ownership, palettes and the UV affine | COMPLETE (ROM texel and sampling checks exact; host tests; PPSSPP goldens; PSP hardware captures of three stages) | material, animation, texture, palette, uv, pack, visual-regression, hardware |  |
| RE-327 | Recorded texture tiles, `unk10 == 1`, and effect material clocks | COMPLETE (decomp comparison, ROM census and texel check, host tests, PSP builds, PPSSPP smoke; no physical-PSP capture) | material, animation, texture, uv, pack, effects, visual-regression |  |
