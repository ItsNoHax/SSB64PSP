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
- [x] **MissingPalette (4→1) — root-caused, moved to R0.7; two of three files fixed.** RE-057 (via a temporary instrumented trace, reverted) confirmed the mechanism: files 52 (`MVCommon`) and 353 (`LinkSpecial2`) got **zero** `MObj` materials from `PartTables` for their scene graphs, and file 86 (`ITCommonObject`) gets them for most but not all nodes. Every segment-0x0E `Call` mesh.rs can't resolve calls `forget_texture()`, which clears an otherwise-valid palette a nearby `G_LOADTLUT` had just set. Not a `romtool` dedup artifact (RE-056's earlier guess). RE-058/RE-059 fixed two of file 353's three unpaired graphs (Link's entrance effects, via `EFDesc`, hand-entered since it lives outside the archive); RE-060 then fixed all five of file 52's unpaired graphs (the opening movie's room scene, via a fourth mechanism — a code call sequence with no struct at all). `romtool textures` now reports 1 `MissingPalette`, not 4. Only file 86's one remaining graph (a fifth mechanism, untraced) and file 353's third graph (a `WPAttributes`-named Spin Attack model) remain open — see the bullets below.
- [x] **Null addresses** — was 54, no longer appears as a failure class in `romtool textures` output. Re-verify and close this line in `docs/rendering.md` "Remaining unconverted" once confirmed.
- Downgraded from failure to informational: **TLUT state across lists** — was 28 "CI texture, no TLUT recorded" failures, now only 1, reported as a *note* on a texture that still packs successfully, not as a conversion failure.
- [x] **RE-054's S2DEX BG lead — refuted.** `romtool scan --exhaustive` finds zero occurrences of `G_BG_1CYC`(0x09)/`G_BG_COPY`(0x0a) anywhere in this ROM's display lists. RE-055 supersedes this lead with the actual mechanism (see above).

### Phase C — Texture Semantics (R0.5) (HIGH)
- [x] **Wrap/Clamp — verified correct, not a gap (RE-066).** `psp/src/meshdraw.rs` hardcodes `sceGuTexWrap(Repeat, Repeat)`; measured archive-wide that every tile-0 `G_SETTILE` requesting clamp also has that axis's own mask nonzero (0/754 counterexamples), and cross-checked against `refs/BattleShip`'s interpreter (which strips `G_TX_CLAMP` under the same condition on real hardware) — `mesh.rs`'s existing mask-narrowed width (RE-044) makes `Repeat` exactly correct here, no code change needed.
- [x] **Mirror — fixed by pre-baking, not approximated (RE-067).** `G_TX_MIRROR` is set on 187/638 (29%) packed textures and has no PSP GE equivalent (`Repeat`/`Clamp` only). `crates/ssb-rom/src/texture.rs::mirror_extend` doubles the decoded image on each mirrored axis (bouncing, not repeating) before `romtool` packs it, so a plain hardware `Repeat` reproduces a real mirror exactly. Traced to Dream Land's canopy specifically and confirmed via a reversible on-device `Repeat`-vs-`Clamp` A/B that the wrap boundary was visibly wrong. Costs +296 KiB VRAM (763→1059 KiB, 1.5x the ~700 KiB budget) — shipped deliberately, per user decision, making texture streaming (below) non-optional.
- [ ] **UV Scroll** — Implement `scrollu`/`scrollv` from MObjSub (`objdisplay.c:1386-1397`).
- [ ] **Mipmap/LOD** — Mip chains are now generated at build time (`psp_texture::pack_mipped`, 151 textures) but did **not** fix the Dream Land canopy discrepancy (RE-053) — the pattern sharpens at higher resolution, which points at magnification, not minification/LOD selection. Still open. RE-054's BattleShip cross-reference found the reference PC port has no LOD support at all (always samples level 0) — corroborates that mipmapping isn't the fix here.
- [x] **Dither smoothing — measurably helps, doesn't fully fix (RE-070, blur/mirror order corrected RE-075).** Tested RE-053's two suggested approaches: filtering alone (measured on-device, insufficient) and conversion-time dither resolution (box-blurring + packing unquantized as `Psm8888` instead of requantizing to the CI4 palette). The second works, partially: ~40% less adjacent-pixel noise on Dream Land's two canopy textures, confirmed by objective pixel measurement after an initial screenshot-based "it's fixed!" read turned out to be a stale-build methodology mistake. Implemented as `crates/ssb-rom/src/texture.rs::box_blur_wrapped`, applied through `tools/romtool/src/main.rs`'s `NEEDS_DITHER_BLUR` — a short, named, per-texture allowlist, not a general "detect dithering" heuristic (that would risk blurring texture art meant to stay sharp). Costs +112 KiB VRAM for the two textures it's applied to. Still not a full fix — the canopy remains somewhat dithered-looking. RE-075 found and fixed a real but small ordering bug in the same conversion (blurred after mirroring instead of before, so the wraparound sampled the mirrored copy at the seam) — confirmed the reordering changes real packed bytes (6724 across the two textures) but confirmed via screenshot it is *not* visible at the tested camera distance; a correctness cleanup, not progress on the remaining discrepancy.
- [ ] **Filtering** — Verify bilinear vs point per texture.

### Phase D — Materials (R0.6 / R0.7) (HIGH)
- [ ] **Remove the shading-detection majority vote** — `pack::looks_like_unit_normal` plus an 80% per-primitive vote (RE-021) stands in for per-object `G_LIGHTING` state a per-list conversion can't see; still needed. Separately, the light *direction* itself is no longer a guess: `pack.rs`'s baked `LIGHT_DIR` now uses the real, measured, ROM-derived `(20, 45)` degree angle that 80% of stages share (RE-065) instead of an arbitrary constant. Full correctness (varying the light per stage) needs runtime `sceGuLight` lighting, not pack-time baking — tracked as an accepted deviation, not attempted here.
- [ ] **Implement `MOBJ_FLAG_LIGHT1/2`** — Upload light colors via `sceGuLight`.
- [ ] **Implement `MOBJ_FLAG_FRAC`** — Fractional frame blend for animated textures.
- [x] **Combiner coverage — dominant declined shape detected, wired up, and verified on device (RE-073/RE-074).** `crates/ssb-rom/src/mesh.rs` evaluates a general `(A-B)*C+D` combiner over both cycles and declines to guess at anything it can't resolve, rather than hardcoding a fixed set of modes as this item originally described. RE-073 measured what it actually declines: 79/1360 `SetCombine` commands archive-wide (5.8%) read `ENVIRONMENT`, 91% of those matching `(PRIM-ENV)*TEXEL+ENV` — a texture-driven blend from `ENV` to `PRIM` with no shade term — across 28 files including Link/Ness/Pikachu's own base models. Added `combiner_texture_blend` to detect it and packed it (`pack.rs`'s `flags::TEXTURE_BLEND`, `VERSION` 9→10). RE-074 then wired it up: baked the base colour into affected vertices in `push_vertex` (safe by the same content-keyed dedup `prim_color` folding already relies on — the "shared vertex" risk RE-073 deferred on turned out to already be handled, not actually unverified) and wired `psp/src/meshdraw.rs`'s `apply_material` to `sceGuTexFunc(Blend, ...)`/`sceGuTexEnvColor`, catching and fixing a real latent bug where `bind_texture` unconditionally reset the texture function to `Modulate` on every texture change. Verified visually on Link's own model (object 306, file 324) via a temporary, reverted debug-viewer patch: before, a flat grey shape; after, the correct grey-to-orange gradient. Remaining, smaller-scoped work: the other ~8% of ENV-reading combiners and whatever `combiner_shade_scale` declines outside that are not exhaustively catalogued archive-wide.
- [x] **Geometry-mode default was backwards, fixed (RE-068).** `refs/ssb-decomp-re/src/sys/rdp.c`'s `sSYRdpResetDisplayList`, replayed once per frame (`taskman.c:308`) before any object draws, sets `G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH` as the default — not all-off. `mesh.rs`'s `State::new()` seeded an all-off default instead, so a node whose own list never mentioned geometry mode (the common case) converted unculled, flat-shaded, non-depth-tested. Fixed via `MeshMaterial::rdp_default()`; `psp/src/meshdraw.rs` now also toggles `GuState::DepthTest` per primitive from the (already-packed, previously-unread) `Z_BUFFER` flag. Archive-wide: `Z_BUFFER` 0.17%→98.3%, `CULL_BACK` measured at 86.3%, `SMOOTH` at 76.5% post-fix.
- [x] **Alpha test (cutout surfaces) — decoded and shipped (RE-069).** `G_SETRENDERMODE`'s `CVG_X_ALPHA | ALPHA_CVG_SEL` (36.1% of non-default render modes archive-wide) now decodes into `MeshMaterial::alpha_test` and drives `sceGuAlphaFunc(Greater, 0, 0xFF)` on device, matching `refs/sf64-psp`'s validated real-hardware approximation (the RDP's actual multisampled-coverage behavior has no PSP equivalent). Found and fixed a bug first: untextured lit primitives were being alpha-tested against a packed-normal byte, not real coverage, and discarded themselves outright (46/380 archive-wide, visibly deleted Dream Land's flowers) — now gated on a texture actually being bound.
- [ ] **Blending (`translucent`) — detected but deliberately not consumed (RE-069, re-checked RE-071).** The render mode's actual blend equation (not just `FORCE_BL`, which the opaque default also sets) is correctly decoded into `MeshMaterial::translucent` and packed (14.4% of non-default render modes), but `psp/src/meshdraw.rs` does not read the flag: enabling `GuState::Blend` from it turned Dream Land's canopy-highlight surface into a checkerboard (RE-069). RE-071 re-tested after RE-070 pre-blurred that exact texture for the opaque path — blending it now produces a *different*, worse failure (blown-out, oversaturated highlights), not an improvement, and ruled out unpremultiplied-alpha blurring as the cause (a premultiplied blur variant gave an identical result). The real cause is still unknown; both "it's the dither" and "it's unpremultiplied blur" are now eliminated, narrowing but not closing this. Next step needs a different lead entirely — maybe the decoded alpha channel itself vs. what the original combiner/`MObjSub` alpha path would produce, not more blur-side experiments.
- [ ] **Render mode / combiner defaults beyond alpha+blend** — remaining lead from RE-068/RE-069: the reset list's `G_CC_SHADE`/`G_CC_SHADE` (shade-only combiner by default) hasn't been explicitly cross-checked against `mesh.rs`'s "unset means declined/unmodified" fallback the way alpha/blend now have been (they happened to already agree for RGB, per RE-039/043, but that was never verified for the reasons alpha/blend's defaults were).

### Phase E — Scene Graphs (R0.7 / R0.8) (HIGH)
- [x] **`EFDesc` — a third, unscanned pairing shape, two instances fixed (RE-059).** `refs/ssb-decomp-re/src/ef/eftypes.h:11-24` defines a fighter-entrance-effect struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` already looks for. Unlike `FTCommonPart`/`MPGroundDesc`, `EFDesc` instances live in the game's static executable, not any relocData archive file, so no archive scan can ever find them — confirmed and hand-paired via `PartTables::insert()` for file 353's `EntryWave`/`EntryBeam` graphs (`tools/romtool/src/main.rs`'s `load_all`). Verified: `romtool mobj --file 353` 0 chain/demand mismatches; `romtool textures` 617→618 packed.
- [x] **A fourth mechanism, no struct at all — file 52 fully fixed (RE-060).** `refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c` (the opening movie's room scene, not a UI system — "MVCommon" is misleading) pairs its five graphs via two independent calls on the same `GObj` — `gcSetupCommonDObjs(gobj, dobjdesc)` then a separate `gcAddMObjAll(gobj, mobjsub)` — with no struct linking them in memory at all, only the call order in the compiled code. Hand-entered all 5 pairings; `romtool mobj --file 52`: 5/5 paired, 0 mismatches; `romtool textures --file 52`: 58/58 packed, 0 failures — file 52 is **fully resolved**.
- [ ] **Graphs without material tables** — freshly re-measured (`romtool mobj`, no `--file` filter) after RE-059/RE-060: **63 graphs paired (was 56), 64 unpaired (was 71)** — moved by exactly the 7 graphs fixed this session. Concrete, still-open cases: file 86 (`ITCommonObject`)'s one remaining graph (a fifth mechanism, see below) and file 353's third graph (`SpinAttackDObjDesc @ 0x11C0`, a `WPAttributes` not yet typed in the decompilation) — see RE-057/RE-058/RE-060. The other ~62 unpaired graphs archive-wide are completely untraced.
- [ ] **`WPAttributes` — a second, unscanned pairing shape (RE-058), not yet fixed.** `refs/ssb-decomp-re/src/wp/wptypes.h:36-45` defines a weapon/projectile struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` already looks for, structurally, but `crates/ssb-rom/src/mobj.rs`'s docs and code only mention `FTCommonPart` (fighters), `MPGroundDesc` (stages) and now `EFDesc` (fighter entrance effects, hand-entered per above) — `WPAttributes` itself was never verified against this scanner. File 353's third graph (Spin Attack) is named by one (`refs/ssb-decomp-re/src/wp/wplink/wplinkspinattack.c`), but that specific `WPAttributes` instance is not yet typed in the decompilation (still raw bytes in `225_LinkMain.c`), so its `p_mobjsubs` field can't be read from source yet — the one confirmed, fully-typed instance elsewhere (Link's boomerang, `226_LinkSpecial1.c`) has `p_mobjsubs = NULL` by design, so this is not guaranteed to be fixable the same way `EFDesc`/the call-sequence mechanism were.
- [x] **A fifth mechanism, byte-offset delta — file 86's last graph, measured and left open, not a code gap (RE-061).** `refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`'s `itGetPData(ip, &llITCommonDataNBumperDataStart, &llITCommonDataNBumperWaitMObjSub)` computes an MObjSub pointer as a compile-time byte-offset delta added to a runtime-resolved base pointer, and neither linker symbol is typed anywhere in the decompilation — there is no named record to read. `romtool mobj --file 86 --search` returns 27 candidate table offsets for this one graph, confirming this would be a guess (the same kind the Samus precedent already showed is close to chance), not a fix. Deliberately left unfixed; `PLAN.md` R0.7 accepts this graph, file 353's Spin Attack graph, and the archive's other ~62 unpaired graphs as a long tail rather than continuing to force individual fixes.
- [x] **0x8000 transform (28 nodes) — fixed as a spin-free billboard (RE-062).** Read the actual `gcPrepDObjMatrix` switch case (`objdisplay.c:822`, case 44): it never touches `dobj->rotate` at all, computing the same diagonal-from-`gGCMatrixPerspF` MVP as billboard kinds 45/46 but with the `sin`/`cos` spin term dropped — a full camera-facing billboard, not a special transform. A one-off archive-wide check (temporary example, not committed) found 0 of the ROM's 28 `RecalcRotRpyRSca` nodes have non-zero `rotate`, confirming the field really is dead for this kind and that reusing the existing `FLAG_BILLBOARD` path (spin term evaluates to a no-op `0.0`) is exact. `crates/ssb-rom/src/pack.rs`'s `add_object` now flags `TransformKind::RecalcRotRpyRSca` the same as `Kind46`/`Kind48`; no `psp/` changes needed. `cargo test --workspace`: 339 passing (new test `a_recalc_node_is_flagged_as_a_spin_free_billboard`). `cargo psp --release` builds; `tools/run-ppsspp.sh` runs 8s at 60 FPS with a clean log (screenshot itself was black — an idle boot-time frame, not evidence either way).
- [ ] **Draw order** — Implement layered rendering with per-layer state (currently global material sort).

### Phase F — Animation (R0.9 / R0.10 / R0.11) (MEDIUM)
- [ ] **Stage material animation** — Hook `matanim_joints` into `draw_stage_animated` (12 layers, `AObjEvent32`). Decoded but not played; frame 0 happens to match baked colours so nothing currently renders wrong (RE-048).
- [ ] **Fighter costume palettes** — Read `p_costume_matanim_joints` and apply per-costume (currently only costume 0 is packed).
- [ ] **Independent animation validation** — Derive expected frame state from decomp/ROM. Stage animation already has this (three independent checks, RE-050/RE-051/RE-052); fighter costume/material animation does not yet.

### Phase G — Assets (R0.11) (MEDIUM)
- [ ] **Complete costume palettes** — All fighter costumes, not just 0.
- [ ] **Texture streaming — real per-scene need is much smaller than the archive-wide total, but unmeasured with confidence (RE-076).** The packed set now measures 1170.9 KiB archive-wide (1.7x the ~700 KiB budget), but that is *every* stage, fighter, menu and effect combined, not what one scene needs — comparing it directly to the per-scene budget was the wrong comparison. A direct measurement (walking the actual pack, deduping texture indices) found the worst realistic case — the largest stage (Dream Land, 137.0 KiB) plus the four largest of the 12 real playable fighters — comes to only **217.1 KiB**, well under budget. That number is very likely an undercount: `PLAN.md` R0.7's 64 still-unpaired `MObj` graphs mean several fighters (Yoshi, Mario, Kirby) measured implausibly low texture counts, almost certainly missing real references this project can't see yet, not genuinely near-empty models. Re-measure once R0.7's pairing gaps close. Until then: `docs/memory.md`'s already-planned per-scene `AssetArena` (one contiguous load per scene, sized by the dependency closure, mirroring the original's own loading pattern) may already be sufficient — a load-per-scene-transition, not a more complex runtime residency/eviction system — but this has not been implemented or tested either way.
- [ ] **Scene dependency graph** — `scene → nodes → materials → textures → palettes`.

### Phase H — Validation (R0.1 / R1 / R2) (HIGH)
- [ ] **Reference renderer** — Compare PSP state against decomp-derived expected state.
- [ ] **Screenshot regression** — Per-stage/per-fighter image diffs. Part of R1's "golden/reference renders" acceptance criterion.
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