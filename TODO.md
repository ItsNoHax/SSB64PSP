# TODO — Discovered Future Work

This file tracks work that is **discovered but currently inactive**. Items here are not yet scheduled for the current milestone (M4 Combat Vertical Slice). They are organized by source.

---

## From Milestones M5–M9 (PLAN.md)

### M5 — Audio
- [ ] Build-time VADPCM decode for 439 samples (117 + 322 waveforms)
- [ ] Sequence conversion for 47 music sequences (ALSeqFile compressed-MIDI)
- [ ] Software mixer on dedicated thread (PSP audio block ≈ 23 ms > 16.67 ms frame)
- [ ] SFX engine (FGM voice IDs from `gmFGMVoiceID`)
- [ ] Music playback with correct sequencing/timing
- [ ] Volume/mixing
- [ ] Media Engine acceleration (after CPU implementation stable)

### M6 — Full Gameplay
- [ ] All 12 original characters (Mario, Fox, Donkey Kong, Samus, Luigi, Link, Yoshi, Captain Falcon, Kirby, Pikachu, Jigglypuff, Ness)
- [ ] Unlockable characters (4 + Fighting Polygon Team + Giant DK + Metal Mario + Master Hand)
- [ ] All original stages (41 including bonus/1P)
- [ ] CPU AI
- [ ] Items (spawn, behavior, pickup, effects)
- [ ] Game modes (VS, 1P, Training, etc.)

### M7 — Menus / Save
- [ ] Title screen
- [ ] Character select
- [ ] Mode select
- [ ] Options
- [ ] Pause menu
- [ ] Results screen
- [ ] Credits
- [ ] PSP-native save system (unlocks, records, settings, progression)

### M8 — Optimization
- [ ] VFPU acceleration for hot math paths (matrix mul, transforms, collision, animation)
- [ ] GPU batching + state sorting
- [ ] Texture caching + VRAM management
- [ ] Memory optimization (arenas, pools)
- [ ] Audio optimization
- [ ] Profile-guided optimization

### M9 — Hardware Validation
- [ ] PSP-1000 test
- [ ] PSP-2000/3000 test
- [ ] PPSSPP regression test
- [ ] Long-duration stability test

---

## From Reverse-Engineering Log (RE-XXX OPEN)

### RE-008 — C-button mapping
**Question:** Which C-button functions matter in Smash 64, and what should they map to on PSP?
**Status:** Placeholder mapping (C-Up→Triangle, C-Down→Square). C-Left/C-Right unmapped.
**Blocker:** Need to read `ftkey.c` and menu input paths in decompilation.
**Target:** M4 (before combat slice)

### RE-009 — PSP nub deadzone
**Question:** How large should deadzone be? Does N64 `-80..=80` map linearly?
**Status:** Deadzone 20 nub units, linear rescale to ±80. Guess, not measured.
**Blocker:** Needs measurement against real PSP nub and comparison to decomp thresholds.
**Target:** M4

### RE-010 — MObjSub unknown fields
**Question:** Do `unk08`…`unk74` fields carry anything the renderer needs?
**Status:** Not consumed. Converter reads only named fields.
**Blocker:** Revisit if materials look wrong in M3. Decomp can answer by finding readers.
**Target:** M3

### RE-011 — Level of detail selection
**Question:** How is `sGCDetailLevel` chosen? Should PSP force a tier?
**Status:** Setter not traced. Likely tied to player count/options.
**Blocker:** Worth resolving before M8 — forcing lower tier is cheap perf lever.
**Target:** M8

### RE-053 — Dream Land canopy texture issue
**Question:** Why does canopy still look wrong after mipmaps?
**Status:** Mipmaps generated (151 textures) but pattern survives and sharpens at higher resolution → points at magnification not minification.
**Blocker:** Needs investigation of CI4 dithered gradient handling, filtering, palette precision.
**Target:** M3

---

## From Rendering Fidelity Gaps (docs/rendering-fidelity.md)

### Phase B — Texture Identity (HIGH)
- [ ] **Cross-file texture references** — 26 failures from segment 0x01 addresses. Trace through `DObjDLLink` / `DObjMultiList` display list arrays pointing to other files.
- [ ] **TLUT state across lists** — 28 "CI texture, no TLUT recorded". Track palette state when `G_LOADTLUT` appears without preceding `G_SETTIMG` for palette.
- [ ] **MissingPalette (4)** — Palette pointer resolved but palette data not found in target file.
- [ ] **Null addresses (54)** — `G_SETTIMG` with 0 address and no relocation. May be runtime-set.

### Phase C — Texture Semantics (HIGH)
- [ ] **Wrap/Clamp/Mirror** — Implement `sceGuTexWrap` from `tile0_mask` + mirror bits (RE-010, `objdisplay.c:1197-1198`).
- [ ] **UV Scroll** — Implement `scrollu`/`scrollv` from MObjSub (`objdisplay.c:1386-1397`).
- [ ] **Mipmap/LOD** — Determine if Smash uses `G_TX_MIPMAP` (RE-053 suggests yes for Dream Land tree).
- [ ] **Filtering** — Verify bilinear vs point per texture.

### Phase D — Materials (HIGH)
- [ ] **Remove majority-vote lighting heuristic** — Use `MObj` light colors + per-object `G_LIGHTING` (RE-021, RE-043).
- [ ] **Implement `MOBJ_FLAG_LIGHT1/2`** — Upload light colors via `sceGuLight`.
- [ ] **Implement `MOBJ_FLAG_FRAC`** — Fractional frame blend for animated textures.
- [ ] **Full combiner coverage** — Handle `ENV * TEXEL`, `PRIM + TEXEL`, etc. (currently only `SHADE * TEXEL`, `PRIM * SHADE`, `SHADE`).

### Phase E — Scene Graphs (HIGH)
- [ ] **71 graphs without material tables** — Use `MPGroundDesc` for stages, `FTCommonPart` for fighters, search for remaining.
- [ ] **0x8000 transform (28 nodes)** — Implement billboard matrix kinds 33-40 (RecalcRotRpyRSca).
- [ ] **Billboard modes** — Verify all 4 variants (kinds 45-48 = 33-40 with leading translate).
- [ ] **Draw order** — Implement layered rendering with per-layer state (currently global material sort).

### Phase F — Animation (MEDIUM)
- [ ] **Stage material animation** — Hook `matanim_joints` into `draw_stage_animated` (12 layers, `AObjEvent32`).
- [ ] **Fighter costume palettes** — Read `p_costume_matanim_joints` and apply per-costume (currently only costume 0).
- [ ] **Independent animation validation** — Derive expected frame state from decomp/ROM.

### Phase G — Assets (MEDIUM)
- [ ] **Complete costume palettes** — All fighter costumes, not just 0.
- [ ] **Texture streaming** — Scene-aware residency (700 KiB VRAM budget).
- [ ] **Scene dependency graph** — `scene → nodes → materials → textures → palettes`.

### Phase H — Validation (HIGH)
- [ ] **Reference renderer** — Compare PSP state against decomp-derived expected state.
- [ ] **Screenshot regression** — Per-stage/per-fighter image diffs.
- [ ] **Strict rendering mode** — Fail on unresolved texture/missing palette/unknown transform.
- [ ] **Real PSP validation** — Hardware test.

---

## From Current Milestone (M4 — Combat Vertical Slice)

### Immediate Next Steps
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

- Items are **not prioritized** — this is a holding area
- Current focus: **M4 Combat Vertical Slice** (one grounded attack end-to-end)
- Do not start M5+ work until M4 is functionally validated (Rule 12)
- When an item becomes active, move it to the current milestone tracking in STATUS.md
- Update this file when new work is discovered during implementation