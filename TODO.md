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
- [x] **MissingPalette (4) — root-caused, moved to R0.7.** RE-057 (via a temporary instrumented trace, reverted) confirmed the mechanism: files 52 (`MVCommon`) and 353 (`LinkSpecial2`) get **zero** `MObj` materials from `PartTables` for their scene graphs, and file 86 (`ITCommonObject`) gets them for most but not all nodes. Every segment-0x0E `Call` mesh.rs can't resolve calls `forget_texture()`, which clears an otherwise-valid palette a nearby `G_LOADTLUT` had just set. Not a `romtool` dedup artifact (RE-056's earlier guess) and not a texture-conversion bug — it's a missing/incomplete `PartTables` pairing, i.e. `PLAN.md` R0.7's territory. RE-057's original guess that 353's table lives in a sibling file is **retracted** (RE-058): 353 already declares its own graph and its own `MObjSub` table in the same file. RE-058 found a more likely explanation instead — see the `WPAttributes` bullet below.
- [x] **Null addresses** — was 54, no longer appears as a failure class in `romtool textures` output. Re-verify and close this line in `docs/rendering.md` "Remaining unconverted" once confirmed.
- Downgraded from failure to informational: **TLUT state across lists** — was 28 "CI texture, no TLUT recorded" failures, now only 4, and those 4 are reported as a *note* on textures that still pack successfully, not as conversion failures. Confirm whether the remaining 4 need action or are benign.
- [x] **RE-054's S2DEX BG lead — refuted.** `romtool scan --exhaustive` finds zero occurrences of `G_BG_1CYC`(0x09)/`G_BG_COPY`(0x0a) anywhere in this ROM's display lists. RE-055 supersedes this lead with the actual mechanism (see above).

### Phase C — Texture Semantics (R0.5) (HIGH)
- [ ] **Wrap/Clamp/Mirror** — `psp/src/meshdraw.rs` currently hardcodes `sceGuTexWrap(Repeat, Repeat)` for every draw. `G_TX_CLAMP`/`G_TX_MIRROR` are decoded from `G_SETTILE` (RE-010, `objdisplay.c:1197-1198`) but not threaded through to the draw call.
- [ ] **UV Scroll** — Implement `scrollu`/`scrollv` from MObjSub (`objdisplay.c:1386-1397`).
- [ ] **Mipmap/LOD** — Mip chains are now generated at build time (`psp_texture::pack_mipped`, 151 textures) but did **not** fix the Dream Land canopy discrepancy (RE-053) — the pattern sharpens at higher resolution, which points at magnification, not minification/LOD selection. Still open. RE-054's BattleShip cross-reference found the reference PC port has no LOD support at all (always samples level 0) — corroborates that mipmapping isn't the fix here.
- [ ] **Filtering** — Verify bilinear vs point per texture.

### Phase D — Materials (R0.6 / R0.7) (HIGH)
- [ ] **Remove majority-vote lighting heuristic** — Use `MObj` light colors + per-object `G_LIGHTING` (RE-021, RE-043).
- [ ] **Implement `MOBJ_FLAG_LIGHT1/2`** — Upload light colors via `sceGuLight`.
- [ ] **Implement `MOBJ_FLAG_FRAC`** — Fractional frame blend for animated textures.
- [ ] **Combiner coverage** — `crates/ssb-rom/src/mesh.rs` now evaluates a general `(A-B)*C+D` combiner over both cycles and declines to guess at anything it can't resolve, rather than hardcoding a fixed set of modes as this item originally described. Remaining work is enumerating which real display-list combiner modes are still declined and whether that's acceptable or a gap.

### Phase E — Scene Graphs (R0.7 / R0.8) (HIGH)
- [ ] **Graphs without material tables** — was reported as 71; `docs/porting-status.md` now reports 56 graphs resolved via `FTCommonPart`/`MPGroundDesc`. Re-run the material-table search (`romtool mobj`/`romtool scene`) to get the current unresolved count before treating either number as current. RE-055/R0.3's texture investigation turned up three concrete, reproducible unresolved-graph cases: files 52 (`MVCommon`), 86 (`ITCommonObject`, partial), 353 (`LinkSpecial2`) — see RE-057. The "353's table lives in a sibling file" guess is **retracted** (RE-058): 353 already has its own graph and its own `MObjSub` table in the same file, so `PartTables::scan`'s same-file check isn't the blocker there.
- [ ] **`WPAttributes` — a second, unscanned pairing shape (RE-058, new finding).** `refs/ssb-decomp-re/src/wp/wptypes.h:36-45` defines a weapon/projectile struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` already looks for, structurally, but `crates/ssb-rom/src/mobj.rs`'s docs and code only mention `FTCommonPart` (fighters) and `MPGroundDesc` (stages) — `WPAttributes` was never verified against this scanner. Likely explains part of the wider 56-vs-71 gap for any fighter with a projectile-style special. Two separate next steps, not one: (1) check whether `PartTables::scan` already structurally catches `WPAttributes` instances elsewhere in the archive; (2) find 353's own `WPAttributes` instance (if one exists) and read its `p_mobjsubs` field — the one confirmed instance checked (Link's boomerang, `226_LinkSpecial1.c`) has `p_mobjsubs = NULL` by design, so a missing record is not automatically the right explanation and file 353's `MissingPalette` cases may need to be traced back into `mesh.rs`'s own state handling instead (RE-056's original direction).
- [ ] **0x8000 transform (28 nodes)** — `RecalcRotRpyRSca` nodes are still drawn plainly per `docs/porting-status.md`. Billboard kinds 45-48 (33-40 with a leading translate) are implemented and verified (RE-049); the plain 33-40 kinds are not. RE-054 confirmed via BattleShip + `objdisplay.c` that these draw types work by computing a full matrix in C and patching it into the RSP's MVP via `gSPMvpRecalc` + `gMoveWd(G_MW_MATRIX,...)` — this is ordinary CPU matrix math to port per D-001, not a special RDP/RSP behavior. The literal name `RecalcRotRpyRSca` did not turn up in `objdisplay.c`; finding the actual matrix-building function for these draw-type branches is the next step.
- [ ] **Draw order** — Implement layered rendering with per-layer state (currently global material sort).

### Phase F — Animation (R0.9 / R0.10 / R0.11) (MEDIUM)
- [ ] **Stage material animation** — Hook `matanim_joints` into `draw_stage_animated` (12 layers, `AObjEvent32`). Decoded but not played; frame 0 happens to match baked colours so nothing currently renders wrong (RE-048).
- [ ] **Fighter costume palettes** — Read `p_costume_matanim_joints` and apply per-costume (currently only costume 0 is packed).
- [ ] **Independent animation validation** — Derive expected frame state from decomp/ROM. Stage animation already has this (three independent checks, RE-050/RE-051/RE-052); fighter costume/material animation does not yet.

### Phase G — Assets (R0.11) (MEDIUM)
- [ ] **Complete costume palettes** — All fighter costumes, not just 0.
- [ ] **Texture streaming** — Scene-aware residency (700 KiB VRAM budget; current packed set measures ~717 KiB, already over).
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