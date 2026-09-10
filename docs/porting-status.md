# Porting Status

Per `AGENTS.md` §13. Percentages are of *intended scope for that subsystem*,
not of the original's line count. A subsystem is only `COMPLETE` when it has
been functionally validated, not merely compiled.

This file tracks **per-subsystem** implementation status only. Current
milestone/task and overall execution state live in `STATUS.md`, and the
ordered roadmap with acceptance criteria lives in `PLAN.md` — do not look for
a milestone table here, and do not add one back; duplicating that state
across files is how it goes stale (an earlier version of this table
contradicted the subsystem rows directly below it: it claimed stage rendering
had "no animation yet" while the animation and stage-animation rows in the
same file already documented animation playing on device).

Last updated: 2026-09-09.

## Subsystems

| Subsystem | Status | Validation |
|---|---|---|
| ROM validation | ✅ COMPLETE | SHA-1/MD5 checked against the real dump; byte-order and size rejection unit-tested |
| VPK0 decompression | ✅ COMPLETE | All 499 compressed files cross-verified against independent ROM geometry (RE-002) |
| relocData archive | ✅ COMPLETE | 2132/2132 files load; 61,343 intern + 3,092 extern relocations, 0 mismatches |
| Asset extraction CLI | ✅ COMPLETE | `romtool extract` produces 16.29 MiB + manifest |
| F3DEX2 DL parser | ✅ COMPLETE | All opcodes Smash emits, verified against real lists; `G_VTX` encoding regression-tested (RE-017) |
| N64 texture decode | 🟢 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; 638 real scene/effect material textures plus all 246 LBParticle frames decode and ship in the pack |
| Texture → PSP conversion | 🟢 96% | **707 bound, 681 packed** (latest `romtool textures`, RE-172). Cross-file pointers resolve through archive relocations (RE-037/046/047); 329 entries carry PSP mip levels and mirror-copy pre-baking reproduces `G_TX_MIRROR` (RE-067). The 26 remaining non-ROM textures are implemented runtime framebuffer captures (R0.13). Dream Land's canopy mirror, filter, coordinate, LOD and blur hypotheses have been measured; R0.5 is `VERIFYING` on physical PSP rather than open to further PPSSPP tuning. |
| DL discovery | ✅ COMPLETE | 1,864 lists across 135 files; converter used as validator (RE-017) |
| Mesh conversion | 🟢 86% | 42,417 tris, vertex dedup and material merging; 0 failures archive-wide. Primitive colours are folded into vertex shade for `PRIM * SHADE` (RE-039/106), both combiner cycles are evaluated without guessing (RE-043), texture blend and flat colour are explicit classified paths (RE-073/080), and material state threads across a node sequence (RE-064). Nonzero clamp windows lower to zero-origin PSP textures (RE-152). RE-164–167 preserve stage angles, signed normals, zero-valid source light colours, and the combiner scale through runtime GE lighting. RE-168's post-table-resolution census accepts 65,000/65,199 source-attributed emitted-triangle visits and traces every remaining missing constant to runtime shield display state or non-authoritative discovery—not a material-table gap. Geometry-mode defaults, depth and culling match the original reset state (RE-068). |
| Model conversion | 🟢 80% | Meshes extracted, DObj hierarchy applied, all 127 discovered material graphs paired (RE-163), and primitive colours applied where the combiner reads them (RE-039/106). Per-costume colours are recovered from `FTCommonPart::p_costume_matanim_joints`; RE-098 packs only node/costume meshes that differ from costume 0 and verifies all 12 playable fighters at a nonzero costume. |
| Stage animation | 🟢 85% | The 32-bit `AObjEvent32` joint stream is decoded, packed and **played on device: 35 stages, 206 animated nodes at 60 FPS**. All scripts replay from the ROM and loop after 600 frames (RE-050); **every packed pose matches the archive across 444,960 values** (`romtool stages --pack`, RE-052). RE-142 fixed `StageAnimator::compose` so null-script children inherit an animated parent's transform, pinning it with a hierarchy test and a Saffron City PPSSPP comparison (836 changed RGB pixels confined to the gate). RE-143 supplies exact signed billboard scale from the animated hierarchy. Dream Land has no joint animation; its scenery moves through game code. |
| Billboard nodes | 🟢 100% | 109 nodes flagged; RE-131–133 shipped the real camera and distinct Kind48 basis. RE-140 inventories every source node; RE-141 corrected Kind46 spin to Z. RE-142 fixed animated-parent inheritance; RE-143 reproduces signed animated X/Y scale. RE-144 restores the original X/Y/X scale rule and adds stable isolation. RE-145 captures and individually reviews all 109 ordinals in PPSSPP: 103 visibly nondegenerate, four intentionally subpixel from authored `0.00001` scale, and two intentionally transparent from all-zero UVs sampling an alpha-zero texel. R0.12 is complete; physical PSP validation remains part of the later rendering gate. |
| Asset pack format | ✅ COMPLETE | Zero-copy, 16-byte aligned, little-endian; writer + reader unit-tested, 3,341 meshes and 3,148 scene-graph nodes round-trip. Current pack version 25 includes animation tables, material-animation attachments, lighting state, each node's local rest transform, and LBParticle bank/script/texture-series tables. |
| PSP asset loading | ✅ COMPLETE | Current 8,218,128-byte pack (version 25, including all LBParticle source data) loads aligned and cache-flushed; PPSSPP smoke-tested, with earlier smaller packs verified on physical hardware |
| PSP mesh drawing | 🟢 90% | Indexed GE draws, CLUT textures, measured repeat/mask/mirror/clamp addressing, baked shading, per-node matrices, per-primitive depth, and manager-effect texture/colour animation. Pack version 25 preserves version 24's normals, source LIGHT_1/LIGHT_2 values, and primitive material-animation attachments while adding LBParticle bank tables; RE-179 applies live PRIM/ENV/LIGHT_1/LIGHT_2 state through source-classified GE colour paths without mutating shared pack vertices. The GE configures the original stage-angle directional light only around fighter draws and enables it only for `LIT` primitives (RE-165/166). RE-167's matched original comparison found and fixed the last observed combiner boundary: runtime lighting now applies the retained `PRIMITIVE * SHADE` scale as GE material colour, restoring Mario's source red/blue costume semantics. Exact cross-renderer pixels and physical PSP remain open. Alpha-tested cutouts render correctly (RE-069); RE-129/130 enable real source-alpha blending for the two classified single-cycle alpha formulas while declining the measured rare/two-cycle long tail. RE-144 restores the original billboard X/Y/X scale rule and adds isolated per-node inspection. |
| Coordinate conversion | 🟢 80% | Matrix/UV/viewport unit-tested; needs on-hardware confirmation (RE-004, RE-005) |
| Battle camera / projection | ✅ COMPLETE | R0.14: default camera source port, viewport/aspect/depth, one-to-four fighter interest union, Wait zoom and original quantized trigonometry are tested. RE-151 reads the original ROM's live Dream Land camera state and matches distance/look-at within 0.1 game units and eye within 0.67; independent PPSSPP audit captures are byte-identical. Special camera modes belong to future gameplay states; physical PSP validation remains R2. |
| Math (scalar) | 🟢 80% | 36 unit tests; no VFPU path yet (correctly — profile first) |
| VFPU optimization | 🔴 0% | Deliberately not started |
| Engine traits (Layer B) | 🟢 70% | Renderer / Audio / Input / Timing / Clock defined |
| Timing / fixed clock | ✅ COMPLETE | Catch-up cap, backwards-clock, 60-ticks-per-second all unit-tested |
| Input mapping | 🟢 75% | Mapping + nub scaling unit-tested; deadzone and C-buttons unresolved (RE-008, RE-009) |
| PSP GU backend | 🟢 89% | Init/frame lifecycle, matrices, indexed textured mesh draws, CI4/CI8 CLUT upload, mip-level upload, filtering, repeat/clamp addressing, alpha test/blend, depth/culling, billboard transforms, and runtime fighter lighting are implemented in `psp/src/gu.rs` and `psp/src/meshdraw.rs`; PPSSPP software audits cover stage/fighter/effect paths |
| PSP input backend | 🟢 70% | `sceCtrl` analog read wired to the shared mapping |
| PSP audio backend | 🔴 0% | |
| Physics | 🟢 60% | 16 functions ported with original addresses cited, and *driven* — `Fighter::tick` runs gravity, drift and material friction against the stage each tick. Running on all 27 characters' **real** constants, extracted from the ROM and verified field-by-field against the decompilation; the invented defaults they replaced were 26x off and had hidden a stick-scaling bug in air drift (RE-032) |
| Fighter state | 🟢 60% | The movement status machine: Wait, three walks, Dash, Run, RunBrake, Turn, KneeBend, Jump F/B, JumpAerial F/B, Fall, FallAerial, Squat, Landing light/heavy and Pass, with the original's interrupt-chain ordering and its tap-counter input model (RE-033). Plus roster, facing, hitlag/hitstun, spawn placement and every character's constants. All of them now **end on their own**: the five that had no duration in `FTAttributes` take it from their figatree animation instead, read out of the ROM and verified against the decompilation for all 27 fighters (RE-035). No attacks, specials, grabs, shields or damage states |
| Collision | 🟢 60% | Geometry extracted for all 41 stages, packed, and read back. Swept floor query, vertical floor projection, per-line surface height and the `mpprocess` floor path (substepping, landing snap, ledge corner, follow-the-surface) all ported. Surface flags confirmed against how Dream Land plays; `dMPCollisionMaterialFrictions` recovered. **158/158 spawns hold a simulated fighter still for 60 ticks at zero drift**, and the swept and projected solvers agree on every one (RE-030, RE-031). No ceiling or wall queries; moving groups are tested at rest |
| Animation | 🟢 90% | **Figatree scripts decode to per-joint transforms, and are packed.** The `AObjEvent16` command stream, `ftAnimGetTargetValue`'s per-track scales and the `AObj` cubic/linear/step interpolation are ported; `romtool figatree` plays all 189 movement animations for 40 frames with zero desynchronisation, and each one's script count matches its fighter's joint count under a rule with no exceptions (RE-036). Joints are mapped through `setup_parts` and `commonparts_container`, both read as archive relocations rather than matched by shape. The current pack carries all 189 animations, 4709 joint entries and each node's local rest transform, and `romtool figatree --pack` replays 3444 joints from it against the ROM with **every pose identical**. A `Skeleton` ticks every joint on device and the object's node matrices are recomposed from the result at 60 FPS, browsable in the viewer. **Validated**: composed poses match the ROM exactly across 3444 joints, no bone changes length across **204,547 measurements over all 189 animations**, the feet stay planted through the static grounded poses, and Turn's opening frame renders as a standing Mario (RE-038). **The status machine drives it**: `Play::tick` restarts the skeleton when the fighter changes status, at the speed the status supplies, and the stage view draws the posed model where the simulation puts it (`docs/images/m4-fighter-status.png`). **All twenty movement statuses** have an animation, not just the seven with a length — the current pack's 532 sparse fighter/slot entries replay **9,692 joints** identically against the ROM and preserve **567,662 bone lengths**. RE-171 additionally renders and captures all 532 under PPSSPP: every identity header advances uniquely, every centred crop contains model content, sampled captures hold 60 FPS, and the log is clean. The audit exposed and fixed `fighter_anim`'s false dense-table assumption, so missing original motions can no longer shift later runtime lookups. The `TransN` motions map correctly. No `translate_scales`; the viewer still frames its camera on the rest bounds |
| Scene graph (DObj) | 🟢 87% | All 363 discovered `DObjDesc` arrays plus 11 source-named direct effect constructions are packed as 374 objects (RE-172; one of the 12 direct effects aliases an already-discovered graph). Three union members of `DObj`'s display-list field resolve, and node lists convert in draw order through one shared vertex cache. `MObj` material chains cover **all 127 graphs that require them**; all 468 paired nodes have zero chain/demand mismatches. `GObj` layer and general animation remain absent. |
| Effects / particles | 🟢 82% | RE-172–RE-179 cover all 53 manager `EFDesc` records and all 46 unique display-bearing effects, including transform, material, texture and live colour playback. `romtool effects` reports 24/26 material-animation tables replayable; PikachuUnk's table address is fixed, while ImpactWave and MBallThrown are confirmed source-unreachable rather than converter failures. RE-179 consumes the manager census's 27 live colour-bearing primitive scripts and PPSSPP-verifies their frame-4 paths. RE-180 strictly decodes all nine independent `LBParticle` banks used by dust, flame, sparkle, hit and stage effects: 160 scripts, 65 texture series, 246 decoded image frames and 6,070 used bytecode bytes. RE-181 serializes that complete inventory into pack version 25 and validates every script bytecode and texture frame through pack readback. RE-182 adds a host-side single-particle bytecode interpreter reproducing `lbParticleUpdateStruct` exactly. RE-183 adds PSP-side drawing (`draw_particle`, a GE billboard quad) and a debug-viewer mode, verified on-device for one script. RE-184/185 extend that to an exhaustive frame-4 census and on-device sweep of all 160 real scripts (148/160 visible, 12 classified, 0 audit warnings), fixing a real camera bug along the way. RE-186 measures the archive-wide combine-mode split among visible scripts (`ENVCOLOR` 90/148 already shipped; `NOISE`/`DITHER`/`ALPHABLEND` each 0/148, confirmed unreachable). RE-187 adds multi-particle spawn-tree execution (`MAKESCRIPT`/`MAKERAND`/`MAKEID` now actually spawn and double-tick correctly), finding one archive-wide rescue (`efcommon` script 38). RE-188 ports `LBGenerator`'s cone/line spawn math (`MAKEGENERATOR`'s own subsystem, the archive's dominant spawn opcode); vortex declines to `VortexUnsupported`, confirmed unreachable by any real caller. RE-189 wires the first real manager-effect spawn event into the PSP runtime -- `efManagerRippleMakeEffect`'s own `LBGenerator` actually ticking and drawing a live particle at a live position every real frame, on-device verified non-blank, closing R1's "all required effects render" acceptance item. Facing-dependent manager streams and the other 25+ `efManager*MakeEffect` call sites remain unwired. |
| Stages | 🟢 65% | All 41 `MPGroundData` headers recovered (RE-028): render layers, camera/map bounds, BGM id. Collision decoded for all 41 (RE-029) and **packed**: 1531 polylines, 3331 vertices, 520 map points. Every one of the **100 render layers resolves to a packed object**. RE-170's automated PPSSPP-software audit captures all 41 stages in stable pack order: 41/41 nonblank unique frames, source-identifying HUDs, 60 FPS, and no logged renderer/load failures. This satisfies R1's software "all stages render" row; physical PSP remains R2. A fighter stands on stage collision (RE-031). No stage *loader* — the viewer browses stages, a match does not select one |
| Items | 🔴 0% | |
| CPU AI | 🔴 0% | |
| Menus | 🔴 0% | |
| Save data | 🔴 0% | |
| Debug/profiler | 🟡 20% | Frame timing sections defined; on-screen text overlay working |
| CI | ✅ COMPLETE | fmt, clippy, host tests, PSP build, EBOOT artifact — no ROM required |

**Per-fighter combat progress: all 12 at 0%.** Correctly so — combat (`PLAN.md`
G0) is blocked behind the rendering gate (R0–R3) and has not started. This is
independent of rendering: fighter *models*, *animation* and *movement physics*
are implemented and tracked in the rows above; only combat-specific state
(attacks, hitboxes, damage) is unstarted.

## Test coverage

460 workspace tests passing: `ssb-rom` 304, `ssb-engine` 36, `ssb-game` 118,
and `romtool` 2. Reproduce with `cargo test --workspace`.

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

1. **Physical PSP hardware validation is not complete.** PPSSPP is not proof
   of hardware behaviour (`AGENTS.md` §16). The project has been smoke-tested
   on physical PSP hardware earlier in development, but that testing was not
   captured against the current renderer's acceptance criteria. `PLAN.md`'s
   R2 milestone defines what "validated on hardware" requires here; see
   `STATUS.md` §8 for the current state. Treat every "on device" claim
   elsewhere in this file as PPSSPP unless a hardware model and build are
   cited — that is what "device" means throughout this document.

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

5. **The movement animation pipeline is far along; combat does not exist.** A
   fighter walks, dashes, jumps and lands on a stage with the right animation
   for each, in its own colours. There are no attacks, hitboxes, hurtboxes,
   damage, knockback, hitstun, opponent, stocks or match loop, and per the
   rendering gate (`AGENTS.md` §5, `PLAN.md` G0) none of that may be started
   until R0–R3 are complete. The vertical slice described in `TODO.md`
   ("Combat Vertical Slice") is recorded for when that gate opens, not as
   current or next work.

6. **The extern relocation slots are zeroed, not resolved.** `romtool` records
   them in the manifest rather than patching them, because the target address
   depends on runtime layout. The runtime loader that applies them does not
   exist yet. The *converter* now follows them (RE-037), which is what got the
   stages textured, but the PSP-side loader that would patch them at load time
   still does not exist.
