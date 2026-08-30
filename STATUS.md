# Project Status

**Last updated:** 2026-08-30  
**Branch:** main (c252960)  
**Working tree:** Clean (no uncommitted changes in worktree)

---

## Milestone Status

| Milestone | Status | Notes |
|-----------|--------|-------|
| **M0 — Research** | ✅ COMPLETE | `docs/ssb-architecture.md`, `docs/reverse-engineering.md`, `docs/porting-status.md` |
| **M1 — Rust PSP Bootstrap** | ✅ COMPLETE | Boots in PPSSPP at locked 60 FPS; renders, animates, reads input. Screenshot: `docs/images/m1-ppsspp-60fps.png` |
| **M2 — Resource Pipeline** | ✅ COMPLETE | Archive + VPK0 verified; 2450 meshes (42,417 triangles, 0 conversion failures) and 617 textures packed into 3.6 MB runtime pack that round-trips |
| **M3 — Rendering** | 🟢 90% | Textured, shaded models placed by scene graph render on device at 60 FPS. Fighters included, in their own palettes. No animation yet. |
| **M4 — Gameplay Vertical Slice** | 🟡 65% | Fighter stands on stage at 60 FPS: ported physics driven tick-by-tick through collision, on real stage data. 158/158 spawns settle with zero drift under every character's real constants. Movement status machine complete (walk, dash, run, turn, squat, jump, double-jump, drop-through, landing). Fighter renders with real model, colours, animations, on stage collision. No attacks, damage, opponent, match loop. |
| **M5 — Audio** | 🔴 0% | Traits only (`AudioBackend`) |
| **M6 — Full Gameplay** | 🔴 0% | |
| **M7 — Menus / Save** | 🔴 0% | |
| **M8 — Optimization** | 🔴 0% | Blocked on M3 per "do not optimize before profiling" |
| **M9 — Hardware Validation** | 🔴 0% | Nothing run on real PSP hardware |

---

## Subsystem Status (Source: `docs/porting-status.md`)

| Subsystem | Status | Validation |
|-----------|--------|------------|
| ROM validation | ✅ COMPLETE | SHA-1/MD5 checked; byte-order and size rejection unit-tested |
| VPK0 decompression | ✅ COMPLETE | All 499 compressed files cross-verified against independent ROM geometry (RE-002) |
| relocData archive | ✅ COMPLETE | 2132/2132 files load; 61,343 intern + 3,092 extern relocations, 0 mismatches |
| Asset extraction CLI | ✅ COMPLETE | `romtool extract` produces 16.29 MiB + manifest |
| F3DEX2 DL parser | ✅ COMPLETE | All opcodes Smash emits, verified against real lists; `G_VTX` encoding regression-tested (RE-017) |
| N64 texture decode | 🟢 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; 617 real ROM textures decode and ship in pack |
| Texture → PSP conversion | 🟢 90% | 647 bound, 617 packed, 74% VRAM saved. Cross-file pointers resolve via extern relocations (RE-037, RE-046, RE-047). 151 textures carry mip levels. VRAM 717 KiB (over 700 KiB all-at-once). 30 failures: 26 screen wipes, 4 CI textures no palette. Dream Land canopy still wrong (RE-053). |
| DL discovery | ✅ COMPLETE | 1,864 lists across 135 files; converter used as validator (RE-017) |
| Mesh conversion | 🟢 85% | 42,417 tris, vertex dedup + material merging; 0 failures. Primitive colours folded into vertex shade (`PRIM * SHADE`). Both combiner cycles evaluated. Dream Land ground draws textured. Cross-joint vertex sharing resolved for rest pose. |
| Model conversion | 🟢 65% | Meshes extracted, DObj hierarchy baked, MObj materials resolved where named. Per-costume colours from `FTCommonPart::p_costume_matanim_joints` — Mario renders in red/blue. Only costume 0 packed. |
| Stage animation | 🟢 80% | 32-bit `AObjEvent32` joint stream decoded, packed, played on device: 35 stages, 206 animated nodes at 60 FPS. Validated 3 ways: all 206 scripts replay from ROM, all loop after 600 frames; every packed pose matches archive (444,960 values); device frame diff shows motion. Material animation (12 layers) read but not played. |
| Billboard nodes | 🟢 80% | `DObjDesc.id & 0xF000` selects matrix kind; kinds 45–48 billboarded. All 81 nodes flagged and billboarded at draw time. Verified by A/B under rotated camera. 28 `RecalcRotRpyRSca` (0x8000) nodes still drawn plainly. |
| Asset pack format | ✅ COMPLETE | Zero-copy, 16-byte aligned, little-endian; writer + reader unit-tested. 2450 meshes, 3137 scene-graph nodes round-trip. Version 6 adds animation tables + node local rest transforms. |
| PSP asset loading | ✅ COMPLETE | 3.6 MB pack loads aligned, cache-flushed, verified on device |
| PSP mesh drawing | 🟢 85% | Indexed GE draws, CLUT textures, wrap, baked shading, per-node baked matrices. Yoshi 28-joint hierarchy at 60 FPS, 589 µs CPU. |
| Coordinate conversion | 🟢 80% | Matrix/UV/viewport unit-tested; needs on-hardware confirmation (RE-004, RE-005) |
| Math (scalar) | 🟢 80% | 36 unit tests; no VFPU path yet (correctly — profile first) |
| VFPU optimization | 🔴 0% | Deliberately not started |
| Engine traits (Layer B) | 🟢 70% | Renderer / Audio / Input / Timing / Clock defined |
| Timing / fixed clock | ✅ COMPLETE | Catch-up cap, backwards-clock, 60-ticks-per-second all unit-tested |
| Input mapping | 🟢 75% | Mapping + nub scaling unit-tested; deadzone and C-buttons unresolved (RE-008, RE-009) |
| PSP GU backend | 🟡 40% | Init, frame lifecycle, matrices, untextured triangles. No textures, no mesh path. |
| PSP input backend | 🟢 70% | `sceCtrl` analog read wired to shared mapping |
| PSP audio backend | 🔴 0% | |
| Physics | 🟢 60% | 16 functions ported with original addresses cited. `Fighter::tick` runs gravity, drift, material friction against stage each tick. All 27 characters' real constants extracted and verified. Invented defaults were 26x off and hid stick-scaling bug in air drift (RE-032). |
| Fighter state | 🟢 60% | Movement status machine: Wait, 3 walks, Dash, Run, RunBrake, Turn, KneeBend, Jump F/B, JumpAerial F/B, Fall, FallAerial, Squat, Landing light/heavy, Pass. Original interrupt-chain ordering and tap-counter input model. Roster, facing, hitlag/hitstun, spawn placement, all constants. 5 statuses with no duration in `FTAttributes` take it from figatree animation. No attacks, specials, grabs, shields, damage states. |
| Collision | 🟢 60% | Geometry for all 41 stages packed and queried. Swept floor query, vertical projection, per-line surface height, `mpprocess` floor path (substepping, landing snap, ledge corner, follow-surface). Surface flags confirmed against Dream Land play; `dMPCollisionMaterialFrictions` recovered. 158/158 spawns hold fighter still for 60 ticks at zero drift. No ceiling/wall queries; moving groups tested at rest. |
| Animation | 🟢 85% | Figatree scripts decode to per-joint transforms and are packed. `AObjEvent16`, `ftAnimGetTargetValue` scales, `AObj` cubic/linear/step interpolation ported. `romtool figatree` plays all 189 movement animations for 40 frames with zero desync. Joints mapped through `setup_parts` and `commonparts_container` as archive relocations. Pack v6 carries 189 animations, 4709 joint entries, node local rest transforms. `romtool figatree --pack` replays 3444 joints from pack against ROM — every pose identical. `Skeleton` ticks every joint on device, node matrices recomposed at 60 FPS. Validated: poses match ROM across 3444 joints; no bone length change across 204,547 measurements; feet planted; Turn opening frame renders as standing Mario. Status machine drives it: `Play::tick` restarts skeleton on status change at status-supplied speed. All 20 movement statuses have animations (532 in pack). `TransN` motions map correctly. No `translate_scales`; viewer camera on rest bounds. |
| Scene graph (DObj) | 🟢 85% | All 363 `DObjDesc` arrays recovered and validated against decomp (RE-023). World transforms baked into pack. Three union members of `DObj`'s DL field resolved, node lists converted in draw order through shared vertex cache — 0 conversion failures. `MObj` material chains recovered for 56 graphs via `FTCommonPart` and `MPGroundDesc` (RE-027, RE-028). `GObj` layer and animation absent. |
| Stages | 🟢 55% | All 41 `MPGroundData` headers recovered. Collision decoded for all 41 and packed: 1531 polylines, 3331 vertices, 520 map points. All 100 render layers resolve to packed object. On-device renders Dream Land 4 layers with collision polylines on platforms, 658 µs at 60 FPS. Fighter stands on them. No stage loader — viewer browses, match does not select. |
| Items | 🔴 0% | |
| CPU AI | 🔴 0% | |
| Menus | 🔴 0% | |
| Save data | 🔴 0% | |
| Debug/profiler | 🟡 20% | Frame timing sections defined; on-screen text overlay working |
| CI | ✅ COMPLETE | fmt, clippy, host tests, PSP build, EBOOT artifact — no ROM required |

---

## Test Coverage

- **341 host tests passing** across `ssb-rom` (198), `ssb-engine` (36), `ssb-game` (107)

---

## M1 Verification (PPSSPP)

Verified 2026-08-27 under PPSSPP 1.20.4 (OpenGL and software rasteriser), X11:

- Module loads — `tag=ELF/ssb64_psp` at `0x08804000`, imports resolved
- `PARAM.SFO` title reads correctly ("Super Smash Bros. 64")
- GE display lists submit; `sceDisplaySetMode(0, 480, 272)` and `sceDisplaySetFrameBuf` run each frame
- **Locked 60.0 FPS**
- Geometry renders with correct vertex-colour interpolation and depth
- Animation advances (rotation differs between captures)
- Physics runs on-device: test object falls under gravity and lands on test floor at exactly y = -3.00

Measured from on-screen diagnostics (RE-016):
```
frame 701  tick 701          <- exact 60 Hz lockstep, no drift over 700 frames
ticks/frame 1  dropped 0     <- no catch-up, no dropped ticks
cpu 13us / budget 16667us    <- 0.08% of the frame budget
frame 16682us  view 362x272  <- 59.94 Hz
```

Reproduce with `tools/run-ppsspp.sh`.

---

## Quantitative Baseline (from `docs/rendering-fidelity-baseline.json`)

| Metric | Value |
|--------|-------|
| Unique textures bound | 647 |
| Textures packed | 617 |
| Texture failures | 30 (4 missing palette, 26 seg0x01, 4 CI no TLUT) |
| VRAM packed | 717.3 KiB |
| VRAM naive RGBA8888 | 2338.8 KiB |
| VRAM saving | 69.3% |
| Fits in VRAM (700 KiB) | No (1.0x overage) |
| Swizzled textures | 356 (58%) |
| PsmT4 (CI4) | 538 textures, 340.9 KiB |
| PsmT8 (CI8) | 17 textures, 53.1 KiB |
| Psm8888 | 62 textures, 323.3 KiB |
| DObjDesc arrays | 363 |
| Total nodes | 3576 |
| Nodes with material tables | 56 |
| Nodes without material tables | 71 |
| Billboard nodes | 81 |
| 0x8000 transform nodes | 28 (drawn plainly) |
| Stages with animation | 35 |
| Animated stage nodes | 206 |
| Fighter animations packed | 532 |
| Joint entries | 13,274 |
| Joints bound to node | 9,828 |
| TransN animations | 50 |
| Host tests | 341 |
| ROM verification | Pass |
| Fighter verification | 27/27 agree with decomp |
| Animation verification | 189/189 agree with decomp |
| VPK0 cross-check | 499/499 files agree |
| Collision spawns | 158/162 land (4 misses on moving-platform bonus stage) |

---

## Known Visual Defects

1. **Dream Land canopy incorrect** — mipmap/filtering/palette issue (RE-053)
2. **Whispy Woods face missing/incorrect material** — 71 scene graphs use heuristic material lookup
3. **28 nodes with 0x8000 transform drawn plainly** — RecalcRotRpyRSca not implemented
4. **119 unconverted textures** — 54 null addr, 36 OOB, 28 no TLUT, 16 missing palette, 13 seg01
5. **Lighting uses majority-vote heuristic** — instead of real MObj state
6. **Wrap/mirror/clamp not implemented** — from `G_SETTILE` tile 0
7. **Material animation not played** — 12 layers read but inactive
8. **Stage animation validated only against itself** — both sides use this crate's player (RE-052)

---

## Open Risks

1. **Never run on real PSP hardware** — PPSSPP is not proof of hardware behavior (biggest unknown)
2. **Debug overlay requires software rasteriser** — PPSSPP hardware backends don't reflect CPU VRAM writes (RE-014)
3. **Extern relocations zeroed at runtime** — loader to patch them doesn't exist yet
4. **Attribute coverage stops at scalar head** — only leading 45 scalars of `FTAttributes` decoded; hurtboxes, sound IDs, joint indices untouched
5. **Animation pipeline complete, combat pipeline empty** — no attacks, hitboxes, hurtboxes, damage, knockback, hitstun, opponent, stocks, match loop

---

## Uncommitted Changes (Main Worktree Only)

```
modified:   crates/ssb-rom/src/mesh.rs
modified:   crates/ssb-rom/src/pack.rs
modified:   psp/src/gu.rs
modified:   psp/src/meshdraw.rs
modified:   tools/romtool/src/main.rs

Untracked files:
  AGENTS.md
  docs/rendering-fidelity-baseline.json
  docs/rendering-fidelity.md
```

Note: This worktree (`update-documentation-architecture`) is clean.