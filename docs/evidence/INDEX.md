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
| RE-008 | C-button mapping | COMPLETE (decomp source, host tests) | input, camera, fighter |  |
| RE-009 | PSP nub deadzone | OPEN |  |  |
| RE-010 | `MObjSub` unknown fields | COMPLETE (decomp source; no ROM census this batch) | material, animation, lighting, texture |  |
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
| RE-283 | Post-RE-281 texture/geometry defects survive across most playable fighters | COMPLETE (software-traced) — Fox's wrist defect and Samus's chest "black square" |  |  |
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
| RE-328 | Camera task-head-1 XLU reset outside stage layers | COMPLETE (decomp source, ROM graph census, pack diff, 68 PPSSPP goldens) | rendering, material, alpha, blending, depth, pack, visual-regression |  |
| RE-329 | Blocking controller read causes the viewer's tick spiral | COMPLETE (PSP-2000 interleaved scan A/B and per-section timing; 3,600-tick replay) | hardware, input, timing, performance, viewer |  |
| RE-330 | Shared capture links and Donkey Kong cargo carry | COMPLETE (decomp source, host tests, ROM animation cross-check, PSP builds, PPSSPP capture) | fighter, gameplay, grab, capture, throw, animation, asset-pipeline, visual-regression |  |
| RE-331 | Grab golden model below Dream Land platform | COMPLETE (decomp motion descriptors, ROM figatree, 69 PPSSPP goldens twice) | fighter, animation, grab, rendering, asset-pipeline, visual-regression |  |
| RE-332 | Gameplay joint transforms for attacks, catches and held fighters | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP grab capture; physical PSP not captured) | fighter, gameplay, collision, grab, capture, animation, visual-regression |  |
| RE-333 | Samus motion scripts, specials and weapons | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP smoke; Samus not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, weapon, grab, animation |  |
| RE-334 | Luigi motion scripts, shared Mario specials and Fireball row | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP smoke; Luigi not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, weapon, grab, animation |  |
| RE-335 | Link motion scripts, specials, Boomerang and Spin Attack weapons | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP golden smoke; Link not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, weapon, grab, animation |  |
| RE-336 | Filter-compensated material textures in the texel check | COMPLETE (bisect, decomp and ROM check, texel check with a broken-pack control, host tests, PSP builds, 69-scene golden matrix; no physical-PSP capture) | material, texture, pack, asset-pipeline, effects, visual-regression |  |
| RE-337 | Yoshi motions, Egg Lay break timing and knockback resistance | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP Yoshi-fighter smoke; Yoshi not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, weapon, grab, capture, animation |  |
| RE-338 | Captain Falcon motions and Dive capture placement | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP Falcon-fighter smoke; Captain Falcon not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, grab, capture, animation |  |
| RE-339 | Held-fighter damage, stale moves, handicaps, bystander throws and held scale | COMPLETE (decomp source, host tests, both PSP builds; ROM, PPSSPP and PSP-2000 checks in RE-341) | fighter, gameplay, grab, capture, throw, combat, collision |  |
| RE-340 | Costume picks, strict rendering, scene dependencies, extern linking and allocators | COMPLETE (decomp source and host tests; validated against the ROM, pack v37, PPSSPP, N64 RDRAM and a PSP-2000 in RE-341) | asset-pipeline, rendering, texture, memory, input |  |
| RE-341 | RE-339/RE-340 validated against the ROM, PPSSPP, N64 RDRAM and a PSP-2000 | COMPLETE (ROM, pack v37, PPSSPPHeadless, Mupen64Plus RDRAM, PSP-2000 scripted scenes and timing; hand-input hardware checks not run) | asset-pipeline, rendering, texture, memory, relocations, hardware, timing, visual-regression, fighter |  |
| RE-342 | Weapon staling | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP goldens; no hardware input test) | weapon, gameplay, combat |  |
| RE-343 | Kirby motions, Stone, Final Cutter and Inhale | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP smoke; copy abilities not ported, Kirby not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, grab, capture, weapon, combat, animation |  |
| RE-344 | Kirby's copy abilities for the ported fighters | COMPLETE (decomp source, host tests, both PSP builds, PPSSPP goldens; Pikachu, Jigglypuff and Ness copies not ported, Kirby not reachable in `psp-game`, physical PSP not captured) | fighter, gameplay, weapon, grab, capture, combat, animation |  |
| RE-345 | Pikachu Thunder Jolt wall normals and authored edge topology | COMPLETE (decomp and ROM geometry, host tests, both PSP builds, one PPSSPP smoke; fighter wall/ceiling solver and physical PSP not validated) | gameplay, fighter, weapon, collision, animation, asset-pipeline |  |
| RE-346 | Jigglypuff's moveset, the sleep element and Kirby status ordinals | COMPLETE (decomp, host tests, both PSP builds, one PPSSPP smoke; ledge stops, per-part hit status and physical PSP not validated) | gameplay, fighter, status, hit-resolution, animation, asset-pipeline |  |
| RE-347 | Ness, PK Thunder ownership and signed motion angles | COMPLETE (bounded host port, decomp/ROM checks, PSP builds and static PPSSPP smoke; shared map, item hurt and physical PSP remain) | gameplay, fighter, status, hit-resolution, weapon, animation, asset-pipeline |  |
| RE-348 | Static fighter map ordering, cliff reach and special contacts | COMPLETE (bounded static map port; host/ROM checks, PSP builds and PPSSPP gameplay smoke; moving groups, damage recovery and physical PSP remain) | gameplay, collision, fighter, status |  |
| RE-349 | Combat audit: hurtboxes, hit application, damage velocity, hitlag | IMPLEMENTED (decomp source, merged host/ROM tests, PSP builds and PPSSPP smoke; authored hurtbox reach and physical PSP remain) | combat, gameplay, hit resolution, physics |  |
| RE-350 | Combat audit 2: motion scripts, per-frame hit log, reaction chains | IMPLEMENTED (decomp source, host tests; no ROM, PPSSPP or hardware run) | combat, gameplay, hit resolution, motion scripts, status machine, weapons |  |
| RE-351 | Damage map callbacks, reaction clips, weapon clank and reflectors | IMPLEMENTED (decomp source, host tests, pack v39 rebuilt from the ROM, PPSSPPHeadless goldens; no physical PSP) | combat, gameplay, collision, weapons, pack, animation, status machine |  |
| RE-352 | General item system, Link Bomb and PK Fire damage | IMPLEMENTED (decomp and US ROM constants, host tests, both PSP builds, PPSSPP Training smoke; no physical PSP) | gameplay, items, combat, weapons, fighter, collision, toolchain |  |
| RE-353 | Moving map groups and authored cliff recovery | IMPLEMENTED (decomp, US ROM, host tests, both PSP builds and PPSSPP Training smoke; no physical PSP) | gameplay, collision, animation, cliff, stages, items, weapons, assets |  |
| RE-354 | Weapon lifecycle and shield-hop callbacks | IMPLEMENTED (decomp, host tests, both PSP builds and PPSSPP Training goldens; no physical PSP) | gameplay, weapons, combat, shield, camera, visual-regression |  |
| RE-355 | Shared damage and attack clips | IMPLEMENTED (decomp tables, host tests, pack v41 rebuilt from the ROM, both PSP builds, full PPSSPPHeadless golden matrix; no physical PSP) | animation, pack, status machine, combat, visual-regression |  |
| RE-356 | Stage controllers and ground hazards | IMPLEMENTED (decomp, US ROM data checks, host tests, both PSP builds, PPSSPPHeadless Training goldens; no physical PSP) | gameplay, stages, collision, combat, animation, status machine |  |
| RE-357 | Pack and draw stage controller objects | IMPLEMENTED (decomp, US ROM, packed clock/pose/flag tests, both PSP builds, full PPSSPPHeadless goldens and live wind probe; no physical PSP) | gameplay, stages, animation, pack, asset-pipeline, rendering, texture, visual-regression |  |
| RE-358 | Animated RSP cache vertices close fighter joint seams | IMPLEMENTED (decomp, US ROM, host tests and PPSSPP software; no physical PSP) | geometry, fighter, animation, pack, rendering, visual-regression |  |
| RE-359 | Tile-relative mirror phase restores Mario and Luigi's buttons | IMPLEMENTED (US ROM, RDP reference, host regression and PPSSPP software; no physical PSP) | texture, geometry, fighter, pack, rendering, visual-regression |  |
| RE-360 | `psp-game` Training reaches 60 Hz on the PSP-2000 | COMPLETE (PSP-2000 per-section timing, 3,600-tick live replay, native captures; PPSSPPHeadless goldens) | hardware, timing, performance, rendering, psp-ge, memory, stage |  |
| RE-361 | Framebuffer capture buffers are written back and cache-line aligned | COMPLETE (code fix; PPSSPPHeadless goldens; PSP-2000 transition capture run. Stale texels were not reproduced on hardware) | hardware, memory, psp-ge, rendering |  |
| RE-362 | Match stage loader: kinds and hazard descriptors from the pack | IMPLEMENTED (decomp, US ROM and pack tests, both PSP builds, full PPSSPPHeadless goldens, PPSSPPHeadless Zebes probe; no physical PSP) | gameplay, stages, pack, asset-pipeline |  |
| RE-363 | Zebes acid object: root writes and animated surface | IMPLEMENTED (decomp, US ROM and pack tests, both PSP builds, full PPSSPPHeadless goldens, PPSSPPHeadless Zebes probe; no physical PSP) | gameplay, stages, animation, pack |  |
| RE-364 | Stage controller material animation on per-object clocks | IMPLEMENTED (decomp, US ROM and pack tests, both PSP builds, full PPSSPPHeadless goldens, PPSSPP boot smoke; no physical PSP) | gameplay, stages, animation, pack, asset-pipeline |  |
| RE-365 | Stage controller objects built from display lists | IMPLEMENTED (decomp, US ROM and pack tests, both PSP builds, full PPSSPPHeadless goldens, PPSSPPHeadless Yoshi's Island and Mushroom Kingdom probes, PPSSPP boot smoke; no physical PSP) | gameplay, stages, animation, pack, asset-pipeline, rendering |  |
| RE-366 | Remaining shared status clips | IMPLEMENTED (decomp tables, host tests, pack v48 rebuilt from the ROM, both PSP builds, full PPSSPPHeadless golden matrix, PPSSPP boot smoke; no physical PSP) | animation, pack, status machine, items, shield |  |
| RE-367 | Shield tilt pose and clip start frame | IMPLEMENTED (decomp, US ROM pack tests, host tests, pack v49 rebuilt from the ROM, both PSP builds, full PPSSPPHeadless golden matrix, PPSSPP boot smoke; no physical PSP) | animation, pack, status machine, shield, asset-pipeline |  |
| RE-368 | Luigi translation scales in clips and shield pose | IMPLEMENTED (decomp, US ROM pack readback, host tests, both PSP builds, PPSSPPHeadless Luigi golden and game smoke; no physical PSP) | animation, fighter, shield, asset-pipeline, visual-regression |  |
| RE-369 | A live tilted shield in scripted Training | COMPLETE (PPSSPPHeadless software capture, ROM-backed host tests, both PSP builds; no physical PSP) | shield, animation, gameplay, visual-regression, psp-ge |  |
| RE-370 | The Training grab route faced away from the dummy | COMPLETE (decomp source, PPSSPPHeadless software capture and goldens, workspace tests, both PSP builds; no physical PSP) | grab, animation, gameplay, visual-regression |  |
| RE-371 | Held fighter TopN takes only the catcher joint rotation | COMPLETE (decomp source, PPSSPPHeadless software capture and goldens, workspace tests, both PSP builds; no original-ROM or physical-PSP visual capture) | grab, animation, rendering, gameplay, visual-regression |  |
| RE-372 | Luigi in Training and his Fireball palette | IMPLEMENTED (decomp source, US ROM pack readback, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, texture, asset-pipeline, visual-regression |  |
| RE-373 | Samus in Training: Charge Shot and Bomb meshes and transforms | IMPLEMENTED (decomp source, US ROM bytes, pack readback, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, texture, asset-pipeline, visual-regression, rendering |  |
| RE-374 | Link in Training: Boomerang tree, spin animation and Spin Attack swirl transforms | IMPLEMENTED (decomp source, pack readback, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, effect, animation, asset-pipeline, visual-regression, rendering |  |
| RE-375 | Yoshi in Training: Egg Throw egg and Yoshi Bomb stars | IMPLEMENTED (decomp source, ROM display lists, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, asset-pipeline, visual-regression, rendering |  |
| RE-376 | Captain Falcon in Training: the Falcon Punch flame | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, effect, visual-regression, rendering |  |
| RE-377 | Captain Falcon in Training: the Falcon Kick flame | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, effect, visual-regression, rendering |  |
| RE-378 | Kirby in Training: Final Cutter wave, and TransN for every status | IMPLEMENTED (decomp source, ROM display lists, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, animation, asset-pipeline, visual-regression, rendering |  |
| RE-379 | Pikachu in Training: Thunder Jolt, weapon draw order and the cutout alpha gate | IMPLEMENTED (decomp source, ROM display lists, PPSSPP source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, asset-pipeline, rendering, visual-regression |  |
| RE-380 | Jigglypuff in Training: the Sing notes | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, effect, rendering, visual-regression |  |
| RE-381 | Ness in Training: PK Fire spark, PK Thunder and PSI Magnet | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, weapon, effect, asset-pipeline, visual-regression |  |
| RE-382 | Items in `psp-game`: the PK Fire flame, Link's Bomb findings and `run`'s branch range | PARTIAL (decomp source, ROM bytes, host test, `psp-asset-viewer` object capture, both PSP builds; the flame is not reached by any capture scene; Link's Bomb is not drawn; no physical PSP) | item, asset-pipeline, rendering, psp-platform |  |
| RE-383 | Link's Bomb in `psp-game`: the right attributes record and the held parent | IMPLEMENTED (decomp source, ROM bytes, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | item, asset-pipeline, rendering, visual-regression |  |
| RE-384 | The shield bubble in `psp-game` | IMPLEMENTED (decomp source, ROM bytes, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | shield, effect, asset-pipeline, rendering, visual-regression |  |
| RE-385 | The Training stage select | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | menu, stage, gameplay, visual-regression, psp-platform |  |
| RE-386 | The Training character select, and the spawn facing it exposed | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | menu, fighter, gameplay, visual-regression |  |
| RE-387 | Spawn facing: fighters face the stage centre | IMPLEMENTED (decomp source, host test, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, gameplay, visual-regression |  |
| RE-388 | Blast-zone KOs and the rebirth halo in Training | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, gameplay, stage, visual-regression |  |
| RE-389 | The VS battle's game status, countdown, timer, KO credit and sudden death | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | match, gameplay, fighter, visual-regression, psp-platform |  |
| RE-390 | The CPU player's Training behaviours drive the dummy | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | cpu-ai, gameplay, visual-regression |  |
| RE-391 | The VS CPU fights: `ftComputerProcDefault` and its attack tables | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | cpu-ai, gameplay, match, visual-regression |  |
| RE-392 | libultra sprites and the battle damage display | IMPLEMENTED (decomp source, ROM decode, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | interface, texture, pack, psp-ge, gameplay, visual-regression |  |
| RE-393 | Fighter emblems and stock icons, and the CPU's fifth emblem colour | IMPLEMENTED (decomp source, ROM decode, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | interface, fighter, stage, pack, visual-regression |  |
| RE-394 | The VS countdown's traffic light and "GO!" | IMPLEMENTED (decomp source, ROM decode, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | interface, match, pack, psp-ge, visual-regression |  |
| RE-395 | The battle timer and "TIME UP"/"GAME SET" | IMPLEMENTED (decomp source, ROM decode, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | interface, match, pack, visual-regression |  |
| RE-396 | Stock icons above the damage display | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | interface, match, fighter, visual-regression |  |
| RE-397 | "SUDDEN DEATH!" | IMPLEMENTED (decomp source, ROM decode, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | interface, match, pack, visual-regression |  |
| RE-398 | The VS pause menu and its camera | IMPLEMENTED (decomp source, ROM decode, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | interface, match, camera, pack, visual-regression |  |
| RE-399 | The VS mode menu's rule, time and stock | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | menu, match, visual-regression, psp-platform |  |
| RE-400 | The VS results' rankings and exit | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | menu, match, visual-regression, psp-platform |  |
| RE-401 | The battle-entry clips in the pack | IMPLEMENTED (decomp source, ROM decode, pack test, PPSSPPHeadless goldens unchanged, workspace tests, both PSP builds; no physical PSP) | animation, pack, fighter, asset-pipeline |  |
| RE-402 | The VS battle entry: Entry, Appear and the entry focus | IMPLEMENTED (decomp source, pack test, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, animation, match, camera, visual-regression |  |
| RE-403 | The fighters' entry effects | IMPLEMENTED (decomp source, pack probe, host test, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, effect, animation, match, visual-regression |  |
| RE-404 | The VS character select | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | menu, match, visual-regression, psp-platform |  |
| RE-405 | VS battles with up to four fighters | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | match, fighter, grab, hud, psp-platform, visual-regression |  |
| RE-406 | The battle camera frames every fighter | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | camera, match, fighter, visual-regression |  |
| RE-407 | VS team battles | IMPLEMENTED (decomp source, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | match, combat, items, weapons, cpu, hud, visual-regression |  |
| RE-408 | The results and select demo clips in the pack | IMPLEMENTED (decomp source, ROM decode, pack test, PPSSPPHeadless goldens unchanged, workspace tests, both PSP builds; no physical PSP) | animation, pack, fighter, asset-pipeline |  |
| RE-409 | The VS results screen's fighters | IMPLEMENTED (decomp source, US build disassembly, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | menus, match, animation, fighter, camera, visual-regression |  |
| RE-410 | The VS results screen's wallpaper, text and table | IMPLEMENTED (decomp source, `reloc_data.us.h`, ROM sprite decode, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | menus, match, sprites, visual-regression, pack |  |
| RE-411 | The VS character select's sprites and fighters | IMPLEMENTED (decomp source, `reloc_data.us.h`, ROM sprite decode, host tests, PPSSPPHeadless golden captured twice, workspace tests, both PSP builds; no physical PSP) | menus, match, sprites, visual-regression, pack |  |
| RE-412 | The KO presentation: blast, flash, halo and fades | IMPLEMENTED (decomp source, `reloc_data.us.h`, ROM display-list and attribute reads, host tests, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, effect, animation, visual-regression, pack |  |
| RE-413 | The in-game particle runtime and the first hit effects | IMPLEMENTED (decomp source, packed `efcommon` bank run in ROM tests, host tests, PPSSPPHeadless goldens captured twice, RNG-isolation A/B captures, workspace tests, both PSP builds; no physical PSP) | effect, particle, fighter, combat, visual-regression |  |
| RE-414 | The fighters' colour animations and the electric skeleton | IMPLEMENTED (decomp source, ROM relocation reads, host tests broken and restored, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | fighter, combat, animation, effect, pack, visual-regression |  |
| RE-415 | The fighters' effects, the display hit effects and their random draws | IMPLEMENTED (decomp source, ROM animation replays, packed-bank runs, host tests broken and restored, PPSSPPHeadless goldens captured twice, RNG-isolation A/B captures, workspace tests, both PSP builds; no physical PSP) | effect, particle, fighter, combat, animation, visual-regression |  |
| RE-416 | The weapons' own effects, weapon clashes and the quake's camera shake | IMPLEMENTED (decomp source, ROM attribute records, packed-bank runs, host tests broken and restored, PPSSPPHeadless goldens captured twice, RNG-isolation A/B captures, workspace tests, both PSP builds; no physical PSP) | weapon, effect, particle, combat, camera, asset-pack, visual-regression |  |
| RE-417 | Drawing Pikachu's Thunder, Kirby's copy hats and the Egg Lay egg | IMPLEMENTED (decomp source, ROM records, packed-asset ROM tests, host tests broken and restored, PPSSPPHeadless goldens captured twice and broken by disabling each draw, workspace tests, both PSP builds; no physical PSP) | weapon, effect, fighter, asset-pack, rendering, visual-regression |  |
| RE-418 | Drawing the damage shield, Yoshi's egg shield and the Fireball's spin | IMPLEMENTED (decomp source, ROM display list, packed-asset ROM tests, host tests broken and restored, PPSSPPHeadless goldens captured twice and broken by disabling each draw, workspace tests, both PSP builds; no physical PSP) | shield, weapon, effect, asset-pack, rendering, visual-regression |  |
| RE-419 | Stage wallpapers and the stage select's presentation | IMPLEMENTED (decomp source, `reloc_data.us.h`, ROM sprite decode, packed-asset ROM tests, host tests, N64 warp-boot references, PPSSPPHeadless goldens captured twice and broken by disabling the draw, workspace tests, both PSP builds; no physical PSP) | stage, sprites, menus, asset-pack, rendering, visual-regression, psp-platform |  |
| RE-420 | The KO halo's rays, quake and star-KO far plane | IMPLEMENTED (decomp source, `reloc_data.us.h`, ROM display lists, packed-asset ROM tests, host tests, N64 warp-boot references, PPSSPPHeadless goldens captured twice and broken by disabling each draw, workspace tests, both PSP builds; no physical PSP) | effects, rendering, camera, asset-pack, menus, visual-regression, psp-platform |  |
| RE-421 | The shield bubble draws with no depth test | IMPLEMENTED (decomp source, ROM display list, RDRAM display-list trace and joint dump from a warp-boot build, N64 captures under Rice and GLideN64, packed-asset ROM tests, host tests, PPSSPPHeadless goldens captured twice and broken by restoring the old seed, workspace tests, both PSP builds; no physical PSP) | effects, rendering, camera, asset-pack, shield, visual-regression |  |
| RE-422 | Battle draw order by display link and task head | IMPLEMENTED (decomp source, RDRAM display-list traces and layer toggles from warp-boot builds on eight VS stages, N64 captures through the real Training select, packed-asset ROM tests, host tests, PPSSPPHeadless goldens captured twice and broken by restoring the old order and the old pack; no physical PSP) | rendering, stage, depth, blending, draw-order, asset-pack, costume, visual-regression |  |
| RE-423 | Stage colours: tile-loaded strips, the lighting bit the stream sets, and the DL-link-6 objects' depth state | IMPLEMENTED (ROM display lists and texel data dumped from the relocated files, N64 frame-414 RDRAM display-list walks from the RE-422 warp-boot builds on eight VS stages, new N64 warp-boot captures of Sector Z's battle and Yoshi's Island's stage select, packed-asset ROM tests, host tests, PPSSPPHeadless goldens captured twice and broken by each intermediate pack; no physical PSP) | rendering, stage, texture, lighting, depth, asset-pack, visual-regression |  |
| RE-424 | The lighting bit decides lighting, and a cutout keeps its coverage | IMPLEMENTED (decomp source, ROM display lists, N64 RDRAM display-list walks of 657 frames from warp-boot builds covering all twelve fighters, their electric skeletons, neutral/up/down specials and VS entries, 20 items, eight VS stages, the opening room and the Training select; angrylion-rdp-plus source for the RDP coverage rules; romtool trace test; PPSSPPHeadless goldens captured twice and broken by each rule; no physical PSP) | rendering, lighting, fighter, alpha, blending, texture, asset-pack, visual-regression |  |
| RE-425 | Headgear accessories, model-part swaps and the entry vehicles | IMPLEMENTED (decomp source, ROM records, packed-asset ROM tests, host tests, N64 warp-boot captures with per-frame entry counters and RDRAM display-list walks, PPSSPPHeadless goldens captured twice and broken by disabling each draw; no physical PSP) | fighter, costume, asset-pack, effect, animation, rendering, depth, draw-order, visual-regression |  |
| RE-426 | Texture parts, demo scripts and fighter detail | IMPLEMENTED (decomp source, ROM-backed tables/material tests, host timing tests, N64 warp captures and traces, 145 paired PPSSPP goldens in two final runs; no physical PSP) | fighter, animation, costume, asset-pack, rendering, memory, visual-regression |  |
| RE-427 | Yoshi roll egg, explosion and shield-release shells | IMPLEMENTED (decomp source, ROM-backed particle/pose tests, N64 captures and traces, 11 paired targeted PPSSPP goldens; no physical PSP) | fighter, effects, particles, animation, rendering, visual-regression |  |
| RE-428 | Sector Z's Arwing, its lasers and path splines | IMPLEMENTED (decomp source, ROM-backed controller and path tests, N64 RDRAM trace and captures, 1 new golden, 7 rebaselined; no physical PSP) | stage, animation, weapons, collision, rendering, asset-pipeline, visual-regression |  |
| RE-429 | Castle Bumper, POW Block and Piranha Plants | IMPLEMENTED (decomp source, ROM attributes and animation replay, N64 reference captures, two new PSP goldens; no physical PSP) | stage, items, combat, animation, rendering, asset-pipeline, visual-regression |  |
| RE-430 | Saffron City ground Pokémon | IMPLEMENTED (source callbacks, ROM-backed animation replay and PSP captures; Chansey Egg dependency remains; no N64 visual or physical PSP comparison) | stage, items, combat, animation, rendering, asset-pipeline, visual-regression |  |
| RE-431 | Light containers and Chansey's Egg | IMPLEMENTED (Capsule/Egg callbacks, ROM attributes and Egg clock, PSP captures; utility makers and appearance actor remain; no N64 visual or physical PSP comparison) | items, combat, animation, rendering, asset-pipeline, visual-regression |  |
| RE-432 | Heavy containers, hidden-part figatree binding and the item root | IMPLEMENTED (decomp source, ROM attributes and hidden parts, N64 Mupen64Plus reference of a Mario crate lift/throw, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | items, grab, animation, asset-pipeline, rendering, visual-regression |  |
| RE-433 | Appearance actor, container contents, Tomato, Heart and Star | IMPLEMENTED (decomp source, ROM attributes, stage weights and item points, N64 Mupen64Plus reference of a Training scene with all three items, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | items, combat, colanim, asset-pipeline, visual-regression |  |
| RE-434 | Throwable utilities: Motion-Sensor Bomb, Bob-omb, Bumper, Shells, Poké Ball | IMPLEMENTED (decomp source, ROM attributes and attack events, N64 Mupen64Plus reference of a Training scene with all six items, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | items, combat, asset-pipeline, rendering, visual-regression |  |
| RE-435 | The Poké Ball's thirteen Pokémon and their weapons | IMPLEMENTED (decomp source, ROM attributes and animation replays, N64 Mupen64Plus references of two Training scenes, PPSSPPHeadless goldens captured twice, workspace tests, both PSP builds; no physical PSP) | items, combat, weapons, asset-pack, rendering, visual-regression |  |
| RE-436 | Held utilities, shooting and Hammer source quirks | IMPLEMENTED (decomp source, ROM attributes/angles, host regressions, PPSSPPHeadless captures twice, both production PSP builds; no N64 runtime comparison or physical PSP) | items, combat, weapons, animation, asset-pack, visual-regression |  |
| RE-437 | CPU item objectives and the CPU's item/weapon/hazard view | IMPLEMENTED (decomp source, host regressions, PPSSPPHeadless captures twice, both production PSP builds; no N64 runtime comparison or physical PSP) | cpu, items, weapons, stages, visual-regression |  |
| RE-438 | CPU traits, Rush and Training's menu logic | IMPLEMENTED (decomp source, host regressions, PPSSPPHeadless captures twice, both production PSP builds; no N64 runtime comparison or physical PSP) | cpu, training, visual-regression |  |
| RE-439 | Training menu sprites, stat display and combo counting | IMPLEMENTED (decomp callbacks, ROM sprite tables, original-game Mupen64Plus references, host regressions, paired Training goldens, both production PSP builds; no physical PSP) | training, interface, sprites, combat, asset-pipeline, visual-regression |  |
| RE-440 | Battle tags, off-screen arrows and circular magnifiers | IMPLEMENTED (decomp/ROM evidence, Mupen64Plus Training observations, nine host regressions, 115 paired PSP goldens, both production builds and PPSSPP smoke; no physical PSP) | interface, camera, rendering, training, asset-pipeline, visual-regression |  |
| RE-441 | Item colour animations, pickup arrows and item lighting | IMPLEMENTED (decomp/ROM evidence, four host regressions, one ROM/pack test, Mupen64Plus Training references for the arrows, the Hammer warning and the lit items, 116 paired PSP goldens, both production builds and PPSSPP smoke; no physical PSP) | items, interface, rendering, asset-pipeline, visual-regression |  |
| RE-442 | Item material scripts, root lists and destroy dust | IMPLEMENTED (decomp/ROM evidence, five host regressions, two script unit tests, one ROM/pack test, two Mupen64Plus Training references, 119 paired PSP goldens, both production builds and PPSSPP smoke; no physical PSP) | items, rendering, asset-pipeline, visual-regression |  |
| RE-443 | Poké Ball and Pokémon presentation | IMPLEMENTED (decomp/ROM evidence, host regressions, original-game Training observation, paired PSP goldens; no physical PSP) | items, effects, rendering, asset-pipeline, visual-regression |  |
| RE-444 | Live item process passes and promoted roots | IMPLEMENTED (decomp/ROM evidence, host regressions, paired targeted PSP goldens, both production PSP builds and PPSSPP smoke; no N64 runtime or physical-PSP comparison) | items, fighter, animation, rendering, visual-regression |  |
| RE-445 | Thrown-fighter attacks and throw ownership | IMPLEMENTED (decomp/ROM evidence, host regressions, both production PSP builds and PPSSPP smoke; no N64 runtime or physical-PSP comparison) | fighter, combat, motion-scripts |  |
| RE-446 | Race to the Finish ground, pipes and stage effects | IMPLEMENTED (decomp/ROM evidence, host regressions, both production PSP builds and PPSSPP smoke; no N64 runtime or physical-PSP comparison) | stage, items, fighter, effects |  |
| RE-447 | Portable 1P campaign core and Race asset bindings | IMPLEMENTED (portable core and pack bindings; host/ROM regressions, both production PSP builds and PPSSPP startup smoke; campaign frontend remains) | gameplay, battle, stage, items, asset-pipeline |  |
| RE-448 | 1P frontend process ordering and scene-return timing | IMPLEMENTED (portable controllers and session transitions; host regressions, production builds and startup smoke; PSP drawing/wiring remains) | gameplay, front-end, input, battle |  |
| RE-449 | Live 1P damage totals and attack-stat provenance | IMPLEMENTED (portable live callbacks and session collectors; host/ROM regressions, production builds and startup smoke; PSP campaign binding remains) | gameplay, battle, combat, items, weapons |  |
| RE-450 | PSP 1P campaign binding, team bounds and compressed select sprites | COMPLETE (source boundaries and PSP binding; Link capture and host gate; authored presentation and later scenes remain) | gameplay, front-end, battle, camera, pack, sprite |  |
| RE-451 | Authored campaign cameras, figure movement and result snapshot | COMPLETE (source/ROM bindings and PSP diagnostic captures; physical PSP and N64 comparison pending) | front-end, camera, animation, pack, sprite, framebuffer |  |
| RE-452 | Target course placement and independent item clocks | COMPLETE (source/ROM bindings and PSP diagnostic handoffs; physical PSP and N64 comparison pending) | stage, item, animation, pack, campaign, camera |  |
| RE-453 | PSP 1P campaign binding and its deviations | PARTIAL (select START, scene flow and seven stages' battles on PSP; scene draws, remaining stages and runtime checks remain) | gameplay, front-end, battle |  |
| RE-454 | Platform children, course Bumpers and boarding order | IMPLEMENTED (source/ROM bindings and PSP diagnostic handoffs; N64 runtime and physical PSP comparison pending) | stage, item, animation, pack, campaign, camera |  |
| RE-455 | Race gate, campaign stocks and bomb-barrel binding | IMPLEMENTED (source/ROM bindings and PSP diagnostic handoffs; N64 runtime and physical PSP comparison pending) | stage, item, pack, campaign, camera |  |
| RE-456 | Bonus pause map zoom and scene-entry fade | IMPLEMENTED (decomp source, pack round-trip, workspace compile, both PSP builds and production PPSSPP startup; bonus pause capture, N64 runtime and physical PSP checks pending) | campaign, pause, camera, stage, pack |  |
| RE-457 | Master Hand, his boss scene and the 1P Game's last stage | IMPLEMENTED (decomp/ROM bindings, host tests, both PSP builds and PPSSPP campaign captures through GAME CLEAR; one N64 runtime reference; physical PSP pending) | gameplay, fighter, weapon, campaign, camera, wallpaper, pack |  |
| RE-458 | Metal Mario, Giant Donkey Kong, the Fighting Polygon Team and TopN's scale | IMPLEMENTED (decomp/ROM bindings, host tests, both PSP builds, PPSSPP campaign captures of all three stages through their intros, replacements and STAGE CLEAR; N64 references of the three stages; physical PSP pending) | gameplay, fighter, campaign, pack, animation, rendering, front-end |  |
| RE-459 | The 1P Game's ending, staff roll, congratulations, challengers and unlock messages | IMPLEMENTED (decomp/ROM bindings, host tests, both PSP builds, PPSSPP captures of every scene and of one run from the ending through Ness's challenge into his battle; no N64 runtime reference; physical PSP pending) | gameplay, campaign, front-end, pack, sprite, animation, camera, memory |  |
| RE-460 | Save data: `lbBackup` on the memory stick | IMPLEMENTED (decomp/ROM bindings, host tests, both PSP builds, PPSSPP runs writing and reloading the save; physical PSP pending) | gameplay, save data, front-end, platform, memory |  |
| RE-461 | The options and data menus, and their per-scene sprite packs | IMPLEMENTED (decomp/ROM bindings, host tests, both PSP builds, PPSSPP captures of all six scenes, N64 references of five; physical PSP pending) | front-end, save data, pack, sprite, memory, platform |  |
| RE-462 | The front end: title, mode select, 1P and VS menus, Bonus Practice | IMPLEMENTED (decomp/ROM bindings, host tests, both PSP builds, PPSSPP production boot and eight capture scenes, N64 references of six scenes; physical PSP pending) | front-end, save data, pack, sprite, animation, campaign, memory |  |
| RE-463 | The golden matrix's drift since RE-444: three intended changes | COMPLETE (capture bisection across pack versions, a revert-only control at HEAD, decomp and `reloc_data.us.h` checks; 116 goldens rebaselined) | visual-regression, fighter, rendering, gameplay, pack |  |
| RE-464 | The VS results' wipe and fade, and the battle's stock snaps, scores and steals | IMPLEMENTED (decomp source, ROM display lists, packed-asset ROM tests, host tests, PPSSPPHeadless captures, N64 warp-boot captures of the battle's end and of a fall; the wipe itself not visible on Mupen64Plus; physical PSP pending) | menus, framebuffer, animation, effects, gameplay, rendering, pack, visual-regression |  |
| RE-465 | The title's attract modes: How to Play and the auto demo | IMPLEMENTED (decomp source, ROM data tests, host tests, both PSP builds, PPSSPPHeadless captures, N64 attract-loop captures and an RDRAM trace of How to Play; physical PSP pending) | menus, gameplay, camera, input, animation, pack, sprite, fighter |  |
| RE-466 | Fighter movement against How to Play's scripts | IMPLEMENTED (decomp source, N64 RDRAM traces of How to Play from the unmodified US ROM, host replay test, workspace tests, both PSP builds, PPSSPPHeadless trace and goldens captured twice, PPSSPP smoke; physical PSP pending) | fighter, gameplay, physics, animation, status, input, math, visual-regression |  |
| RE-467 | The N64 logo, the opening movie and the title's opening layout | IMPLEMENTED (decomp source, ROM data tests, host tests, both PSP builds, PPSSPPHeadless captures against an N64 capture of the unmodified ROM, production boot under PPSSPP; physical PSP pending) | menus, scene, camera, animation, fighter, pack, memory, rendering |  |
| RE-468 | Fighter fidelity against How to Play and the opening battles | IMPLEMENTED (decomp source, N64 RDRAM traces of the unmodified US ROM, ROM tests, host replay, workspace tests, both PSP builds, PPSSPPHeadless traces of How to Play and the nine opening battles, PPSSPP smoke; physical PSP pending) | fighter, gameplay, physics, animation, status, collision, grab, rng, math, rendering |  |
