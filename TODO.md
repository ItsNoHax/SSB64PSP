# TODO — Discovered Future Work

This file tracks work that is **discovered but currently inactive**. It uses
the milestone names from the current `PLAN.md` (R0–R3 rendering gate, G0–G5
post-combat roadmap). It does not use the old M0–M9 numbering that an earlier
version of this repository's planning docs used — that scheme was superseded
by the 2026-09-02 restructure (see `PLAN.md`, `STATUS.md`).

Most items below are rendering-fidelity gaps that feed directly into the R0.x
tasks in `PLAN.md`; a smaller set is gameplay/audio/menu work blocked behind
the rendering gate until G0 unlocks (`AGENTS.md` §5). Do not start G0+ work
until R0–R3 are complete — see "Notes" at the end of this file.

---

## Deferred Work Behind the Rendering Gate (PLAN.md G1–G5)

Not scheduled. Blocked until R0, R1, R2 and R3 are all complete and G0 (first
combat vertical slice) has landed.

### G1 — Full Combat
- [ ] All 12 original characters (Mario, Fox, Donkey Kong, Samus, Luigi, Link, Yoshi, Captain Falcon, Kirby, Pikachu, Jigglypuff, Ness)
- [ ] Unlockable characters (4 + Fighting Polygon Team + Giant DK + Metal Mario + Master Hand)
- [ ] CPU AI
- [ ] Items (spawn, behavior, pickup, effects)

### G2 — Complete Match Systems
- [ ] All original stages loadable in a match (41 including bonus/1P) — the viewer already browses stage data, a match does not yet select one
- [ ] Game modes (VS, 1P, Training, etc.)
- [ ] Stocks, timers, win conditions, match transitions

### G3 — Menus and Persistence
- [ ] Title screen
- [ ] Character select
- [ ] Mode select
- [ ] Options
- [ ] Pause menu
- [ ] Results screen
- [ ] Credits
- [ ] PSP-native save system (unlocks, records, settings, progression)

### G4 — Audio
- [ ] Build-time VADPCM decode for 439 samples (117 + 322 waveforms)
- [ ] Sequence conversion for 47 music sequences (ALSeqFile compressed-MIDI)
- [ ] Software mixer on dedicated thread (PSP audio block ≈ 23 ms > 16.67 ms frame)
- [ ] SFX engine (FGM voice IDs from `gmFGMVoiceID`)
- [ ] Music playback with correct sequencing/timing
- [ ] Volume/mixing
- [ ] Media Engine acceleration (after CPU implementation stable)

### G5 — Final Optimization
- [ ] VFPU acceleration for hot math paths (matrix mul, transforms, collision, animation)
- [ ] GPU batching + state sorting beyond build-time material merge
- [ ] Memory optimization (arenas, pools)
- [ ] Audio optimization
- [ ] Profile-guided optimization

Rendering-specific performance work (frame time, GE/CPU bottlenecks, VRAM
measurement) belongs to `PLAN.md` R3, not here — R3 is part of the rendering
gate, not post-combat work.

### R2 — Physical PSP Rendering Validation (not gameplay, but not R0 either)
`PLAN.md` already defines R2's acceptance criteria. The concrete test matrix:
- [ ] PSP-1000 test
- [ ] PSP-2000/3000 test
- [ ] Long-duration stability test

PPSSPP regression testing is not part of R2 — it is the day-to-day
verification loop used throughout R0/R1 (`AGENTS.md` §14, §16).

---

## From Reverse-Engineering Log (RE-XXX OPEN)

### RE-008 — C-button mapping
**Question:** Which C-button functions matter in Smash 64, and what should they map to on PSP?
**Status:** Placeholder mapping (C-Up→Triangle, C-Down→Square). C-Left/C-Right unmapped.
**Blocker:** Need to read `ftkey.c` and menu input paths in decompilation.
**Target:** G0 (before combat slice — input mapping is not part of the rendering gate)

### RE-009 — PSP nub deadzone
**Question:** How large should deadzone be? Does N64 `-80..=80` map linearly?
**Status:** Deadzone 20 nub units, linear rescale to ±80. Guess, not measured.
**Blocker:** Needs measurement against real PSP nub and comparison to decomp thresholds.
**Target:** G0

### RE-010 — MObjSub unknown fields
**Question:** Do `unk08`…`unk74` fields carry anything the renderer needs?
**Status:** Not consumed. Converter reads only named fields.
**Blocker:** Revisit if materials look wrong. Decomp can answer by finding readers.
**Target:** R0.6 (Material System Correctness) / R0.7 (Missing Material Tables)

### RE-011 — Level of detail selection
**Question:** How is `sGCDetailLevel` chosen? Should PSP force a tier?
**Status:** Setter not traced. Likely tied to player count/options.
**Blocker:** Worth resolving before performance work — forcing lower tier is a cheap perf lever.
**Target:** R3 (Rendering Performance) — do not force a tier before that milestone measures whether it is needed

### RE-053 — Dream Land canopy texture issue
**Question:** Why does canopy still look wrong after mipmaps?
**Status:** Mipmaps generated (151 textures) but pattern survives and sharpens at higher resolution → points at magnification not minification.
**Blocker:** Needs investigation of CI4 dithered gradient handling, filtering, palette precision.
**Target:** R0.5 (Texture Filtering / LOD / Mipmapping) — this is that task's explicit acceptance criterion "Dream Land canopy discrepancy resolved," still open

---

## Rendering Fidelity Gaps (feeds PLAN.md R0.3–R0.15)

These phases were originally tracked in a `docs/rendering-fidelity.md` that no
longer exists in the repository; the content is preserved here. Re-verify
numbers before trusting them — `cargo run --release -p romtool -- textures
"rom/Super Smash Bros. (USA).z64"` is the source of truth and its output has
already moved since these were first written (see below).

### Phase B — Texture Identity (R0.3) (HIGH)
- [x] **Segment-0x01 "failures" (26) identified, accepted as out of scope** — RE-055 confirms these are not cross-file texture references at all: they are `G_SETTIMG` binds to `sLBTransitionPhotoHeap`, a runtime per-frame framebuffer photocopy the LB (loading-break) transition system binds to RSP segment 0x1 (`refs/ssb-decomp-re/src/lb/lbtransition.c:119,155,224`). Confirmed by direct byte inspection of file 39 (and corroborated in files 40/41/45/50/51, same instruction word) and by an exhaustive opcode scan of the whole ROM. No fix belongs here; a real implementation is R0.13.
- [x] **MissingPalette (4→0) — root-caused and resolved (RE-162).** RE-057's temporary instrumented trace established that files 52 (`MVCommon`) and 353 (`LinkSpecial2`) got no `MObj` materials for their scene graphs, while file 86 (`ITCommonObject`) missed only its N-Bumper graph. RE-059/RE-060 resolved the file-353/file-52 paths; RE-162 then follows file 251's typed N-Bumper `ITAttributes` extern pair to map file 86's parser graph `0x7BE8` to table `0x7488`. `romtool textures` now reports no `MissingPalette` failures.
- [x] **Null addresses** — was 54, no longer appears as a failure class in `romtool textures` output. Re-verify and close this line in `docs/rendering.md` "Remaining unconverted" once confirmed.
- Downgraded from failure to informational: **TLUT state across lists** — was 28 "CI texture, no TLUT recorded" failures, now only 1, reported as a *note* on a texture that still packs successfully, not as a conversion failure.
- [x] **RE-054's S2DEX BG lead — refuted.** `romtool scan --exhaustive` finds zero occurrences of `G_BG_1CYC`(0x09)/`G_BG_COPY`(0x0a) anywhere in this ROM's display lists. RE-055 supersedes this lead with the actual mechanism (see above).

### Phase C — Texture Semantics (R0.5) (HIGH)
- [x] **Wrap/Clamp — verified correct, not a gap (RE-066).** `psp/src/meshdraw.rs` hardcodes `sceGuTexWrap(Repeat, Repeat)`; measured archive-wide that every tile-0 `G_SETTILE` requesting clamp also has that axis's own mask nonzero (0/754 counterexamples), and cross-checked against `refs/BattleShip`'s interpreter (which strips `G_TX_CLAMP` under the same condition on real hardware) — `mesh.rs`'s existing mask-narrowed width (RE-044) makes `Repeat` exactly correct here, no code change needed.
- [x] **Mirror — fixed by pre-baking, not approximated (RE-067).** `G_TX_MIRROR` is set on 187/638 (29%) packed textures and has no PSP GE equivalent (`Repeat`/`Clamp` only). `crates/ssb-rom/src/texture.rs::mirror_extend` doubles the decoded image on each mirrored axis (bouncing, not repeating) before `romtool` packs it, so a plain hardware `Repeat` reproduces a real mirror exactly. Traced to Dream Land's canopy specifically and confirmed via a reversible on-device `Repeat`-vs-`Clamp` A/B that the wrap boundary was visibly wrong. Costs +296 KiB VRAM (763→1059 KiB, 1.5x the ~700 KiB budget) — shipped deliberately, per user decision, making texture streaming (below) non-optional.
- [x] **UV Scroll — measured, confirmed inert, not implemented (RE-194).** `scrollu`/`scrollv` drive the tile-1 `gDPSetTileSize(1, ...)` command (`objdisplay.c:1386-1397`), real in 12 real `MObjSub`s archive-wide — but every one has `scrollu == trau`, `scrollv == trav`, and matching tile dimensions, i.e. byte-identical inputs to that same `MObj`'s own tile-0 window. This project's renderer only ever samples render tile 0 (`mesh.rs`'s `RENDER_TILE` guard; no packed combiner shape reads `TEXEL1`, RE-130), so tile 1 has no consumer to feed. Documented in `mobj.rs`'s `MOBJ_FLAG_TILE1` constant rather than given a pack field nothing would read.
- [ ] **Mipmap/LOD** — Mip chains are now generated at build time (`psp_texture::pack_mipped`, 151 textures) but did **not** fix the Dream Land canopy discrepancy (RE-053) — the pattern sharpens at higher resolution, which points at magnification, not minification/LOD selection. Still open. RE-054's BattleShip cross-reference found the reference PC port has no LOD support at all (always samples level 0) — corroborates that mipmapping isn't the fix here.
- [x] **Dither smoothing — measurably helps, doesn't fully fix (RE-070, blur/mirror order corrected RE-075).** Tested RE-053's two suggested approaches: filtering alone (measured on-device, insufficient) and conversion-time dither resolution (box-blurring + packing unquantized as `Psm8888` instead of requantizing to the CI4 palette). The second works, partially: ~40% less adjacent-pixel noise on Dream Land's two canopy textures, confirmed by objective pixel measurement after an initial screenshot-based "it's fixed!" read turned out to be a stale-build methodology mistake. Implemented as `crates/ssb-rom/src/texture.rs::box_blur_wrapped`, applied through `tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR` — a short, named, per-texture allowlist, not a general "detect dithering" heuristic (that would risk blurring texture art meant to stay sharp). Costs +112 KiB VRAM for the two textures it's applied to. Still not a full fix — the canopy remains somewhat dithered-looking. RE-075 found and fixed a real but small ordering bug in the same conversion (blurred after mirroring instead of before, so the wraparound sampled the mirrored copy at the seam) — confirmed the reordering changes real packed bytes (6724 across the two textures) but confirmed via screenshot it is *not* visible at the tested camera distance; a correctness cleanup, not progress on the remaining discrepancy.
- [ ] **Filtering** — Verify bilinear vs point per texture.

### Phase D — Materials (R0.6 / R0.7) (HIGH)
- [x] **Shading-detection majority vote — replaced by a per-vertex, data-driven decision (RE-103/RE-105), stale item corrected.** This item previously described `pack::looks_like_unit_normal` plus an 80% *per-primitive* vote (RE-021) as still needed; that was true when written but is now stale — `PLAN.md` R0.6 records that RE-103 found the per-primitive majority vote wrong by construction (a fighter's mixed material routinely splits 20–80% within one primitive) and replaced it with a per-*vertex* decision, and RE-105 gave that per-vertex decision a real ROM-derived signal (`G_MW_LIGHTCOL`, confirmed in `crates/ssb-rom/src/mesh.rs`) instead of only `looks_like_unit_normal`'s heuristic shape check. RE-164–167 subsequently closed the separate runtime-light direction/state path.
- [x] **Light direction — runtime path shipped (RE-164–167).** Pack v23 retains every stage's X/Y angle and signed normals; fighter draws configure `sceGuLight` from the active stage, apply zero-valid LIGHT_1/LIGHT_2 state, preserve the combiner's costume-colour scale as GE material colour, and keep literal primitives unlit. A matched original-ROM Dream Land comparison restores Mario's red/blue costume semantics without a tuned brightness value. Physical PSP validation remains R2.
- [x] **Implement `MOBJ_FLAG_LIGHT1/2` (RE-165/166).** The converter retains
  directional LIGHT_1 and ambient LIGHT_2 writes—including authored zero—and
  the PSP uploads them independently of otherwise identical material flags.
- [x] **Runtime `MObj` display-state parity (RE-194, `PLAN.md` R1).** Measured
  every input archive-wide before implementing anything: `MOBJ_FLAG_NONE`
  defaults (3 real occurrences), `MOBJ_FLAG_TEXTURE` scale (10), tile-0 UV
  window (35) are real and now implemented (`mobj.rs`'s `tex_scale`/
  `tile0_uv`, wired in `mesh.rs`'s `apply_mobj`), unit-tested against real
  ROM values. `MOBJ_FLAG_FRAC` is confirmed dead code (0/665 real
  occurrences, never set at runtime either) — see below.
- [x] **`MOBJ_FLAG_FRAC` — confirmed dead code, not implemented (RE-194).** Archive-wide census over all 665 real `MObjMaterial`s this project resolves: zero set `MOBJ_FLAG_FRAC`. Grepped the whole decompilation for runtime `sub.flags |=` writes — only `MOBJ_FLAG_ENVCOLOR` (shield colours, `efmanager.c`) is ever OR'd in at runtime; `FRAC` is never set anywhere, statically or dynamically. Same "measured, not guessed" treatment RE-127 gave RDP LOD blending.
- [x] **Combiner coverage — dominant declined shape detected, wired up, and verified on device (RE-073/RE-074).** `crates/ssb-rom/src/mesh.rs` evaluates a general `(A-B)*C+D` combiner over both cycles and declines to guess at anything it can't resolve, rather than hardcoding a fixed set of modes as this item originally described. RE-073 measured what it actually declines: 79/1360 `SetCombine` commands archive-wide (5.8%) read `ENVIRONMENT`, 91% of those matching `(PRIM-ENV)*TEXEL+ENV` — a texture-driven blend from `ENV` to `PRIM` with no shade term — across 28 files including Link/Ness/Pikachu's own base models. Added `combiner_texture_blend` to detect it and packed it (`pack.rs`'s `flags::TEXTURE_BLEND`, `VERSION` 9→10). RE-074 then wired it up: baked the base colour into affected vertices in `push_vertex` (safe by the same content-keyed dedup `prim_color` folding already relies on — the "shared vertex" risk RE-073 deferred on turned out to already be handled, not actually unverified) and wired `psp/src/meshdraw.rs`'s `apply_material` to `sceGuTexFunc(Blend, ...)`/`sceGuTexEnvColor`, catching and fixing a real latent bug where `bind_texture` unconditionally reset the texture function to `Modulate` on every texture change. Verified visually on Link's own model (object 306, file 324) via a temporary, reverted debug-viewer patch: before, a flat grey shape; after, the correct grey-to-orange gradient. RE-079 then did the exhaustive archive-wide census this left open: 262,778 combiner-bearing primitives, 97.0% already recognised, and two real classification bugs found and fixed in what remained (a black-`PRIM`-scale ambiguity affecting 1,118 primitives, and an over-strict `PRIM`+`ENV` gate in `combiner_texture_blend` affecting 125) — see the new Phase D item below for the still-open remainder.
- [x] **A third combiner shape, flat constant colour — detected, packed, and wired to device (RE-080).** The archive-wide census also found `(ZERO-ZERO)*ZERO+PRIM` — a plain constant colour with no shade or texture dependence at all — for 1,589 primitives (plus 28 bare-`ONE` and 9 `ENV`-alone occurrences, the same underlying case), which neither `combiner_shade_scale` nor `combiner_texture_blend` models. Added `combiner_flat_color` (structurally disjoint from the other two via RE-079's presence flags) and `MeshMaterial::flat_color`. Wired all the way to device in the same pass, not deferred like `texture_blend` was: since `TEXEL` provably never enters this shape's formula, `material_now` forces the primitive untextured and `push_vertex` bakes the resolved colour, so no `psp/` changes were needed at all. Packed as `pack.rs`'s `flags::FLAT_COLOR` (`PrimDesc::SIZE` 32→36, `VERSION` 10→11). Repacking measured a real, expected side effect: bound textures 644→639 (five textures were referenced only by primitives whose combiner never actually reads them, now correctly dropped).
- [x] **Geometry-mode default was backwards, fixed (RE-068).** `refs/ssb-decomp-re/src/sys/rdp.c`'s `sSYRdpResetDisplayList`, replayed once per frame (`taskman.c:308`) before any object draws, sets `G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH` as the default — not all-off. `mesh.rs`'s `State::new()` seeded an all-off default instead, so a node whose own list never mentioned geometry mode (the common case) converted unculled, flat-shaded, non-depth-tested. Fixed via `MeshMaterial::rdp_default()`; `psp/src/meshdraw.rs` now also toggles `GuState::DepthTest` per primitive from the (already-packed, previously-unread) `Z_BUFFER` flag. Archive-wide: `Z_BUFFER` 0.17%→98.3%, `CULL_BACK` measured at 86.3%, `SMOOTH` at 76.5% post-fix.
- [x] **Alpha test (cutout surfaces) — decoded and shipped (RE-069).** `G_SETRENDERMODE`'s `CVG_X_ALPHA | ALPHA_CVG_SEL` (36.1% of non-default render modes archive-wide) now decodes into `MeshMaterial::alpha_test` and drives `sceGuAlphaFunc(Greater, 0, 0xFF)` on device, matching `refs/sf64-psp`'s validated real-hardware approximation (the RDP's actual multisampled-coverage behavior has no PSP equivalent). Found and fixed a bug first: untextured lit primitives were being alpha-tested against a packed-normal byte, not real coverage, and discarded themselves outright (46/380 archive-wide, visibly deleted Dream Land's flowers) — now gated on a texture actually being bound.
- [x] **Blending (`translucent`) — decoded and consumed for classified formulas (RE-129/130).** The checkerboard was caused by applying blending without decoding the independent alpha combiner. `TEXEL0_ALPHA` and `TEXEL0_ALPHA * SHADE_ALPHA` now drive real GE blending; rare `PRIM_ALPHA` and two-cycle formulas remain explicitly declined.
- [x] **Render mode / combiner defaults beyond alpha+blend.** RE-068 seeds the original reset geometry/render state and RE-039/043's unset RGB fallback is the same shade-only result; RE-129/130 independently classify alpha rather than inheriting an invented default.

### Phase E — Scene Graphs (R0.7 / R0.8) (HIGH)
- [x] **`EFDesc` — a third, unscanned pairing shape, two instances fixed (RE-059).** `refs/ssb-decomp-re/src/ef/eftypes.h:11-24` defines a fighter-entrance-effect struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` already looks for. Unlike `FTCommonPart`/`MPGroundDesc`, `EFDesc` instances live in the game's static executable, not any relocData archive file, so no archive scan can ever find them — confirmed and hand-paired via `PartTables::insert()` for file 353's `EntryWave`/`EntryBeam` graphs (`tools/romtool/src/main.rs`'s `load_all`). Verified: `romtool mobj --file 353` 0 chain/demand mismatches; `romtool textures` 617→618 packed.
- [x] **A fourth mechanism, no struct at all — file 52 fully fixed (RE-060).** `refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c` (the opening movie's room scene, not a UI system — "MVCommon" is misleading) pairs its five graphs via two independent calls on the same `GObj` — `gcSetupCommonDObjs(gobj, dobjdesc)` then a separate `gcAddMObjAll(gobj, mobjsub)` — with no struct linking them in memory at all, only the call order in the compiled code. Hand-entered all 5 pairings; `romtool mobj --file 52`: 5/5 paired, 0 mismatches; `romtool textures --file 52`: 58/58 packed, 0 failures — file 52 is **fully resolved**.
- [ ] **Graphs without material tables — 126 paired, 1 unpaired (RE-162).** File 251's typed N-Bumper `ITAttributes` extern pair resolves file 86's graph to `0x7BE8 → 0x7488`, leaving only file 324's Link-model `JointTree_0x9CF8`. RE-161 audited all three Link candidates against the decompilation: none is named for that graph, so demand-compatible tables remain leads only unless a typed source table independently confirms them.
- [ ] **`WPAttributes` — a second, unscanned pairing shape (RE-058), not yet fixed.** `refs/ssb-decomp-re/src/wp/wptypes.h:36-45` defines a weapon/projectile struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` already looks for, structurally, but `crates/ssb-rom/src/mobj.rs`'s docs and code only mention `FTCommonPart` (fighters), `MPGroundDesc` (stages) and `EFDesc` (effects, hand-entered per above) — `WPAttributes` itself was never verified against this scanner. Link's Spin Attack graph is named by one (`refs/ssb-decomp-re/src/wp/wplink/wplinkspinattack.c`), but its table was independently recovered from the static `EFDesc` record (RE-158); that does not establish a scanner path for `WPAttributes`. The one confirmed, fully typed instance elsewhere (Link's boomerang, `226_LinkSpecial1.c`) has `p_mobjsubs = NULL` by design.
- [x] **N-Bumper material table resolved from a cross-file `ITAttributes` record (RE-162).** File 251's `dITCommonData_NBumper_ItemAttributes` directly names file 86's N-Bumper DObj resource and `p_mobjsubs` extern target. Its source labels establish the runtime-offset relationship that RE-061 could not: `romtool` maps the parser's graph start `0x7BE8` to the exact table `0x7488`. The graph is fully paired, its CI4 texture gets its TLUT, and file 86 packs 55/55 textures.
- [x] **0x8000 transform (28 nodes) — fixed as a spin-free billboard (RE-062).** Read the actual `gcPrepDObjMatrix` switch case (`objdisplay.c:822`, case 44): it never touches `dobj->rotate` at all, computing the same diagonal-from-`gGCMatrixPerspF` MVP as billboard kinds 45/46 but with the `sin`/`cos` spin term dropped — a full camera-facing billboard, not a special transform. A one-off archive-wide check (temporary example, not committed) found 0 of the ROM's 28 `RecalcRotRpyRSca` nodes have non-zero `rotate`, confirming the field really is dead for this kind and that reusing the existing `FLAG_BILLBOARD` path (spin term evaluates to a no-op `0.0`) is exact. `crates/ssb-rom/src/pack.rs`'s `add_object` now flags `TransformKind::RecalcRotRpyRSca` the same as `Kind46`/`Kind48`; no `psp/` changes needed. `cargo test --workspace`: 339 passing (new test `a_recalc_node_is_flagged_as_a_spin_free_billboard`). `cargo psp --release` builds; `tools/run-ppsspp.sh` runs 8s at 60 FPS with a clean log (screenshot itself was black — an idle boot-time frame, not evidence either way).
- [ ] **Draw order** — Implement layered rendering with per-layer state (currently global material sort).

### Phase F — Animation (R0.9 / R0.10 / R0.11) (MEDIUM)
- [ ] **Material animation — partially implemented, not complete.** RE-086 measured 172 stage scripts: 122 `PaletteID`, 38 `TextureIDCurrent`, plus UV and colour tracks. RE-089–095 shipped resolution, pack data, ticking and device CLUT application for the 33 resolved palette-cycling scripts. Stage texture-frame/UV tracks and the 200/441 fighter costume scripts carrying `PaletteID` remain unconsumed. See RE-211.
- [ ] **Fighter costume palettes** — Read `p_costume_matanim_joints` and apply per-costume (currently only costume 0 is packed).
- [ ] **Independent animation validation** — Derive expected frame state from decomp/ROM. Stage animation already has this (RE-050/RE-051/RE-052), including null-script child inheritance checked against the original hierarchy and Saffron City on-device (RE-142); fighter costume/material animation does not yet.

### Phase G — Assets (R0.11) (MEDIUM)
- [ ] **Complete costume palettes** — All fighter costumes, not just 0.
- [ ] **Texture streaming — real per-scene need is much smaller than the archive-wide total, but unmeasured with confidence (RE-076).** The packed set now measures 1170.9 KiB archive-wide (1.7x the ~700 KiB budget), but that is *every* stage, fighter, menu and effect combined, not what one scene needs — comparing it directly to the per-scene budget was the wrong comparison. A direct measurement (walking the actual pack, deduping texture indices) found the worst realistic case — the largest stage (Dream Land, 137.0 KiB) plus the four largest of the 12 real playable fighters — comes to only **217.1 KiB**, well under budget. That number is very likely an undercount: `PLAN.md` R0.7's 64 still-unpaired `MObj` graphs mean several fighters (Yoshi, Mario, Kirby) measured implausibly low texture counts, almost certainly missing real references this project can't see yet, not genuinely near-empty models. Re-measure once R0.7's pairing gaps close. Until then: `docs/memory.md`'s already-planned per-scene `AssetArena` (one contiguous load per scene, sized by the dependency closure, mirroring the original's own loading pattern) may already be sufficient — a load-per-scene-transition, not a more complex runtime residency/eviction system — but this has not been implemented or tested either way.
- [ ] **Scene dependency graph** — `scene → nodes → materials → textures → palettes`.

### Phase H — Validation (R0.1 / R1 / R2) (HIGH)
- [x] **Reference renderer / Screenshot regression** — owned by `PLAN.md` R0.17 (Visual Regression Methodology, `COMPLETE`), not tracked independently here. See `docs/visual-regression.md` for the deterministic scene, the N64-reference/PPSSPP-software/PPSSPP-hardware/physical-PSP capture procedure, the test matrix, and the pixel-diff tool. R1's "golden/reference renders" acceptance criterion still tracks the matrix's remaining rows.
- [ ] **Strict rendering mode** — Fail on unresolved texture/missing palette/unknown transform.
- [ ] **Physical PSP validation** — R2. See `STATUS.md` §8 for current state; historical smoke-testing has occurred but the formal R2 acceptance criteria have not been demonstrated with evidence.

---

## Combat Vertical Slice (PLAN.md G0 — blocked, not the current milestone)

This is **not** currently active work. It is recorded here so the shape of
the first combat slice is not lost, but none of it may be started until
`PLAN.md` R0–R3 are complete and `STATUS.md` records G0 as eligible
(`AGENTS.md` §5).

### Immediate Next Steps (once G0 unlocks)
- [ ] **Grounded attack end-to-end** — Input → hitbox → hurtbox → damage → knockback
- [ ] **Hitbox/hurtbox system** — From `FTAttributes` hurtbox descriptors (RE-032, only 45 leading scalars decoded)
- [ ] **Damage/knockback physics** — Port from `ftphysics.c` attack logic
- [ ] **Opponent + match loop** — Second fighter, stock system, blast zones, KO
- [ ] **Stage loader** — Match selects stage, not viewer browse

### Fighter State Completion
- [ ] Attack states (Attack11, Attack12, Attack13, AttackS3, AttackS4, AttackHi3, AttackLw3, AttackAirF/B/N/Hi/Lw, Special*)
- [ ] Grab/throw states
- [ ] Shield states
- [ ] Damage/hitstun/knockback states
- [ ] Ledge grab/climb/attack
- [ ] Tech/roll/air dodge

### Systems Needed for Combat
- [ ] Hitbox/hurtbox collision detection
- [ ] Hitlag/hitstun frames
- [ ] Knockback velocity calculation (weight, damage, angle, DI)
- [ ] Blast zone detection + KO
- [ ] Stock system + respawn
- [ ] Match timer + sudden death

---

## Technical Debt / Refactoring

- [ ] **Extern relocation runtime loader** — Pack records them zeroed; need loader to patch at scene load
- [ ] **AssetArena implementation** — Contiguous per-scene block sized by dependency closure
- [ ] **GameArena / FrameArena / ObjectPool** — Explicit allocators (currently not implemented)
- [ ] **VFPU math module** — After profiling identifies hot paths
- [ ] **Coordinate conversion hardening** — On-hardware confirmation (RE-004, RE-005)
- [ ] **PSP GU backend completion** — Textured mesh path, material state, CLUT handling
- [ ] **PSP audio backend** — `sceAudio` mixer on dedicated thread

---

## Notes

- Items are **not prioritized** — this is a holding area.
- Current focus: **R0 — Rendering Correctness** (`PLAN.md`, `STATUS.md`). See
  `STATUS.md` for the single current task.
- Do not start G0 (combat) or later work until R0, R1, R2 and R3 are all
  complete (`AGENTS.md` §5, §13).
- When an R0.x item here becomes active work, it should be reflected as the
  current task in `STATUS.md`, not tracked independently here.
- Update this file when new work is discovered during implementation.
