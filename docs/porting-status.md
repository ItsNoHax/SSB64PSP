# Porting Status

Per Rule 11. Percentages are of *intended scope for that subsystem*, not of the
original's line count. A subsystem is only `COMPLETE` when it has been
functionally validated, not merely compiled (Rule 12).

Last updated: 2026-08-28.

## Milestones

| Milestone | Status | Notes |
|---|---|---|
| **M0 — Research** | ✅ COMPLETE | `docs/ssb-architecture.md`, this file, `docs/reverse-engineering.md` |
| **M1 — Rust PSP bootstrap** | ✅ COMPLETE | Boots in PPSSPP at a locked **60 FPS**; renders, animates, reads input. Screenshot: `docs/images/m1-ppsspp-60fps.png` |
| **M2 — Resource pipeline** | ✅ COMPLETE | Archive + VPK0 verified; 2722 meshes (47,696 triangles, 0 conversion failures) and 485 textures packed into a 2.9 MB runtime pack that round-trips |
| **M3 — Rendering** | 🟢 90% | **Textured, shaded models placed by the scene graph render on device at 60 FPS** (`docs/images/m4-scene-graph.png`), fighters included, in their own palettes (`docs/images/m4-fighter-materials.png`). No animation yet |
| **M4 — Gameplay vertical slice** | 🟡 55% | **A fighter stands on a stage on device at 60 FPS** (`docs/images/m4-fighter-status.png`): ported physics driven a tick at a time through the ported collision process, on real stage data. 158/158 spawns settle with zero drift, under every character's real extracted constants (RE-031, RE-032). The fighter now **walks, dashes, runs, turns, squats, jumps, double-jumps, drops through platforms and lands** (RE-033), confirmed on device reading its real constants and animation lengths out of the pack (RE-034, RE-035). No attacks, no damage, no opponent, no match loop. Animation now decodes to per-joint transforms on the host (RE-036), but nothing draws them yet |
| **M5 — Audio** | 🔴 0% | Traits only |
| **M6 — Full gameplay** | 🔴 0% | |
| **M7 — Menus / save** | 🔴 0% | |
| **M8 — Optimization** | 🔴 0% | Blocked on M3 per "do not optimize before profiling" |
| **M9 — Hardware validation** | 🔴 0% | |

## Subsystems

| Subsystem | Status | Validation |
|---|---|---|
| ROM validation | ✅ COMPLETE | SHA-1/MD5 checked against the real dump; byte-order and size rejection unit-tested |
| VPK0 decompression | ✅ COMPLETE | All 499 compressed files cross-verified against independent ROM geometry (RE-002) |
| relocData archive | ✅ COMPLETE | 2132/2132 files load; 61,343 intern + 3,092 extern relocations, 0 mismatches |
| Asset extraction CLI | ✅ COMPLETE | `romtool extract` produces 16.29 MiB + manifest |
| F3DEX2 DL parser | ✅ COMPLETE | All opcodes Smash emits, verified against real lists; `G_VTX` encoding regression-tested (RE-017) |
| N64 texture decode | 🟢 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; 482 real ROM textures decode and ship in the pack |
| Texture → PSP conversion | 🟢 85% | 664 bound, 545 packed, 76.9% VRAM saved. **Cross-file texture pointers now resolve** through the archive's extern relocations, which is what stages reach their texels by: Dream Land went from 1 texture to 16 and renders textured on device (RE-037, `docs/images/m4-stage-textured.png`). 119 references still fail — 54 null pointers nothing names, 36 landing past the end of the file they name, 13 segmented, 16 missing a palette. Round-trip tested through swizzle+CLUT; verified on device (RE-022). At 1.1 MiB the full set no longer fits texture VRAM in one go — a match only needs one stage and a few fighters, but streaming is now an M8 question |
| DL discovery | ✅ COMPLETE | 1,864 lists across 135 files; converter used as validator (RE-017) |
| Mesh conversion | 🟢 85% | 47,696 tris, vertex dedup and material merging; 0 failures archive-wide. Cross-joint vertex sharing resolved for the rest pose (RE-026); no animated skinning yet |
| Model conversion | 🟢 60% | Meshes extracted, DObj hierarchy applied and baked, MObj materials resolved where named (RE-027). Animation not applied |
| Asset pack format | ✅ COMPLETE | Zero-copy, 16-byte aligned, little-endian; writer + reader unit-tested, 2722 meshes and 3137 scene-graph nodes round-trip. Version 3 adds the four stage tables |
| PSP asset loading | ✅ COMPLETE | 3.0 MB pack loads aligned, cache-flushed, verified on device |
| PSP mesh drawing | 🟢 85% | Indexed GE draws, CLUT textures, wrap, baked shading, per-node baked matrices. Yoshi's 28-joint hierarchy at 60 FPS, 589us CPU |
| Coordinate conversion | 🟢 80% | Matrix/UV/viewport unit-tested; needs on-hardware confirmation (RE-004, RE-005) |
| Math (scalar) | 🟢 80% | 36 unit tests; no VFPU path yet (correctly — profile first) |
| VFPU optimization | 🔴 0% | Deliberately not started |
| Engine traits (Layer B) | 🟢 70% | Renderer / Audio / Input / Timing / Clock defined |
| Timing / fixed clock | ✅ COMPLETE | Catch-up cap, backwards-clock, 60-ticks-per-second all unit-tested |
| Input mapping | 🟢 75% | Mapping + nub scaling unit-tested; deadzone and C-buttons unresolved (RE-008, RE-009) |
| PSP GU backend | 🟡 40% | Init, frame lifecycle, matrices, untextured triangles. No textures, no mesh path |
| PSP input backend | 🟢 70% | `sceCtrl` analog read wired to the shared mapping |
| PSP audio backend | 🔴 0% | |
| Physics | 🟢 60% | 16 functions ported with original addresses cited, and *driven* — `Fighter::tick` runs gravity, drift and material friction against the stage each tick. Running on all 27 characters' **real** constants, extracted from the ROM and verified field-by-field against the decompilation; the invented defaults they replaced were 26x off and had hidden a stick-scaling bug in air drift (RE-032) |
| Fighter state | 🟢 60% | The movement status machine: Wait, three walks, Dash, Run, RunBrake, Turn, KneeBend, Jump F/B, JumpAerial F/B, Fall, FallAerial, Squat, Landing light/heavy and Pass, with the original's interrupt-chain ordering and its tap-counter input model (RE-033). Plus roster, facing, hitlag/hitstun, spawn placement and every character's constants. All of them now **end on their own**: the five that had no duration in `FTAttributes` take it from their figatree animation instead, read out of the ROM and verified against the decompilation for all 27 fighters (RE-035). No attacks, specials, grabs, shields or damage states |
| Collision | 🟢 60% | Geometry extracted for all 41 stages, packed, and read back. Swept floor query, vertical floor projection, per-line surface height and the `mpprocess` floor path (substepping, landing snap, ledge corner, follow-the-surface) all ported. Surface flags confirmed against how Dream Land plays; `dMPCollisionMaterialFrictions` recovered. **158/158 spawns hold a simulated fighter still for 60 ticks at zero drift**, and the swept and projected solvers agree on every one (RE-030, RE-031). No ceiling or wall queries; moving groups are tested at rest |
| Animation | 🟡 30% | **Figatree scripts decode to per-joint transforms on the host.** The `AObjEvent16` command stream, `ftAnimGetTargetValue`'s per-track scales and the `AObj` cubic/linear/step interpolation are ported; `romtool figatree` plays all 189 movement animations for 40 frames with zero desynchronisation, and each one's script count matches its fighter's joint count under a rule with no exceptions (RE-036). Joints are mapped through `setup_parts` and `commonparts_container`, both read as archive relocations rather than matched by shape. **Nothing is packed or drawn yet** — no runtime joint transforms, no `TransN`, no `translate_scales` |
| Scene graph (DObj) | 🟢 85% | All 363 `DObjDesc` arrays recovered and validated against the decomp (RE-023); world transforms baked into the pack. Three union members of `DObj`'s display-list field resolved, and node lists converted in draw order through one shared vertex cache — zero conversion failures archive-wide (RE-025, RE-026). `MObj` material chains recovered for 56 graphs via the `FTCommonPart` and `MPGroundDesc` records that name them, giving fighters and stage layers their palettes (RE-027, RE-028). `GObj` layer and animation still absent |
| Stages | 🟢 55% | All 41 `MPGroundData` headers recovered (RE-028): render layers, camera/map bounds, BGM id. Collision decoded for all 41 (RE-029) and **packed**: 1531 polylines, 3331 vertices, 520 map points. Every one of the **100 render layers resolves to a packed object**. On-device stage view renders Dream Land's four layers with its collision polylines landing exactly on the platforms, 658 us at 60 FPS (`docs/images/m4-stage-collision.png`); Peach's Castle cross-checks it on sloped ground. A fighter now stands on them (RE-031). No stage *loader* — the viewer browses stages, a match does not select one |
| Items | 🔴 0% | |
| CPU AI | 🔴 0% | |
| Menus | 🔴 0% | |
| Save data | 🔴 0% | |
| Debug/profiler | 🟡 20% | Frame timing sections defined; on-screen text overlay working |
| CI | ✅ COMPLETE | fmt, clippy, host tests, PSP build, EBOOT artifact — no ROM required |

**Per-fighter progress: all 12 at 0%.** Correctly so — the vertical slice
(M4) comes before any character work.

## Test coverage

300 host tests passing across `ssb-rom` (157), `ssb-engine` (36) and
`ssb-game` (107).

## M1 verification (PPSSPP)

Verified 2026-08-27 under PPSSPP 1.20.4 (OpenGL and software rasteriser), X11:

* Module loads — `tag=ELF/ssb64_psp` at `0x08804000`, imports resolved for
  `sceGeListEnQueue`, `sceCtrlReadBufferPositive`, `sceCtrlSetSamplingMode`,
  `sceDisplaySetMode`, `sceDisplayWaitVblankStart`.
* `PARAM.SFO` title reads correctly ("Super Smash Bros. 64").
* GE display lists submit; `sceDisplaySetMode(0, 480, 272)` and
  `sceDisplaySetFrameBuf` run each frame.
* **Locked 60.0 FPS.**
* Geometry renders with correct vertex-colour interpolation and depth.
* Animation advances (rotation differs between captures).
* Physics runs on-device: the test object falls under gravity and lands on the
  test floor at exactly y = -3.00.

Measured from the on-screen diagnostics (RE-016):

```
frame 701  tick 701          <- exact 60 Hz lockstep, no drift over 700 frames
ticks/frame 1  dropped 0     <- no catch-up, no dropped ticks
cpu 13us / budget 16667us    <- 0.08% of the frame budget
frame 16682us  view 362x272  <- 59.94 Hz; the value the helper returns
```

These are *baseline* numbers on a four-triangle scene under an emulator, not a
performance prediction for a real match on real hardware.

Reproduce with `tools/run-ppsspp.sh`.

## Known gaps and honest caveats

1. **Nothing has run on real PSP hardware.** Per §37 of the plan, PPSSPP is
   not proof of hardware behaviour. This is now the single biggest unknown.

2. **The debug overlay only displays under PPSSPP's software rasteriser.**
   Resolved as an emulator limitation, not a port bug (RE-014):
   `sceGuDebugFlush` paints VRAM with the CPU, and PPSSPP's hardware backends
   do not reflect those writes. `tools/run-ppsspp.sh` forces the software
   renderer so diagnostics are always visible. The real HUD will render as GE
   geometry (Renderer 3), which removes the dependency entirely.

3. **Extracted assets are unparsed.** `romtool extract` produces byte-exact
   file payloads, but nothing yet interprets them as textures, meshes or
   attribute tables. The archive layer is trustworthy; the layers above it
   do not exist.

4. **Attribute coverage stops at the scalar head.** All 27 characters'
   `FTAttributes` are extracted and packed, cross-checked field by field
   against the decompilation (RE-032). Only the leading 45 scalars are decoded;
   the hurtbox descriptors, sound ids and joint indices further into the struct
   are still untouched, so nothing above physics and collision can read them.

5. **Animation decodes but does not render.** RE-036 gets a figatree to a
   per-joint `(rotate, translate, scale)` on the host, validated against the
   decompilation. None of it is packed into the asset pack, none of it reaches
   the PSP, and no joint transform is submitted to the GE. The next concrete
   task is packing the scripts and the joint mapping, then rebuilding node
   matrices per tick instead of using the baked rest-pose ones.

6. **The extern relocation slots are zeroed, not resolved.** `romtool` records
   them in the manifest rather than patching them, because the target address
   depends on runtime layout. The runtime loader that applies them does not
   exist yet. The *converter* now follows them (RE-037), which is what got the
   stages textured, but the PSP-side loader that would patch them at load time
   still does not exist.
