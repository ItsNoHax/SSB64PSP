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

Last updated: 2026-09-10.

## Subsystems

| Subsystem | Status | Validation |
|---|---|---|
| ROM validation | ✅ COMPLETE | SHA-1/MD5 checked against the real dump; byte-order and size rejection unit-tested |
| VPK0 decompression | ✅ COMPLETE | All 499 compressed files cross-verified against independent ROM geometry (RE-002) |
| relocData archive | ✅ COMPLETE | 2132/2132 files load; 61,343 intern + 3,092 extern relocations, 0 mismatches |
| Asset extraction CLI | ✅ COMPLETE | `romtool extract` produces 16.29 MiB + manifest |
| F3DEX2 DL parser | ✅ COMPLETE | All opcodes Smash emits, verified against real lists; `G_VTX` encoding regression-tested (RE-017) |
| N64 texture decode | 🟢 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded and unit-tested; 638 real scene/effect material textures plus all 246 LBParticle frames decode and ship in the pack |
| Texture → PSP conversion | 🟡 97% / VERIFYING | ROM-backed conversion, mirror pre-baking, clamp/origin lowering and RE-201's Dream Land hardware capture are complete for their covered paths. Texgen's `R2.1`/T1–T10 gate (semantics, addressing, hardware, documentation and test-minimum cleanup) is complete (RE-225–239). Current pack format is version 27. |
| DL discovery | ✅ COMPLETE | 1,864 lists across 135 files; converter used as validator (RE-017) |
| Mesh conversion | 🟡 88% / VERIFYING | 0 archive-wide conversion failures; combiner classification and material threading are implemented. RE-217 reopened validation for single-source `PRIM * SHADE` (closed, RE-240/`R2.2`/C1) and `G_VTX` load-time lighting provenance (closed, RE-241–243/`R2.2`/C2, including the external-per-object `G_LIGHTING` seed for fighter skeleton graphs — measured zero real vertices affected, since RE-105's `G_MW_LIGHTCOL` signal already covers them). RE-244 (`R2.2`/C3, part 1) adds independent `depth_test`/`depth_write`/`depth_mode` fields (`Z_CMP`/`Z_UPD`/`ZMODE`, separate from `z_buffer`/`G_ZBUFFER`) and the same external-per-object seed for the fighter skeleton graphs' `G_SETRENDERMODE` — unlike C2's lighting seed, measurably not redundant (732/771 fighter-skeleton primitives change), but the wider archive still diverged 90% from `z_buffer` with the cause not yet found. RE-245 (`R2.2`/C3, part 2) found and wired the same external wrapper for stage render-layer 1 (`InitialMaterial::GROUND_LAYER1_EXTERNAL`, 577/776 primitives flip), shrinking the archive-wide gap from 4468 to 3891; items/effects carry no such wrapper, and layer 1's 21 list-1 (translucent) `DObjDLLink` entries are still seeded as opaque pending a `list_id` field on `PlannedList`. RE-246 (`R2.2`/C3, part 3) found the dominant remainder was a *camera*-level default, not an object wrapper: `sys/objdisplay.c`'s `func_8001663C` unconditionally sets `Z_CMP | Z_UPD | ZMODE_OPA` for every buffer-0 camera before it walks its own tagged `GObj` list, and the 11 loading-break transition scenes each run under a dedicated single-object camera, so the default reaches them uncorrupted; wired `InitialMaterial::LB_TRANSITION_EXTERNAL` (1951/1951 primitives flip), shrinking the gap from 3891 to 1940. This default is deliberately not generalized to the main battle camera or any other shared camera, since which object draws first there is runtime state, not archive data. Independent PSP-side depth wiring and global primitive reordering remain open; see `PLAN.md` R2.2/C3–C4. |
| Model conversion | 🟡 82% / VERIFYING | Meshes extracted, DObj hierarchy applied, all 127 discovered material graphs paired (RE-163), and primitive colours applied on the covered combiner paths. RE-240/RE-241/RE-242/RE-243 close runtime-lit/literal ownership and load-time vertex semantics (`R2.2`/C1–C2) across the named mixed-material fighters. |
| Stage animation | 🟢 90% | The 32-bit `AObjEvent32` joint stream is decoded, packed and **played on device: 35 stages, 206 animated nodes at 60 FPS**. All scripts replay from the ROM and loop after 600 frames (RE-050); **every packed pose matches the archive across 444,960 values** (`romtool stages --pack`, RE-052). RE-142 fixed `StageAnimator::compose` so null-script children inherit an animated parent's transform, pinning it with a hierarchy test and a Saffron City PPSSPP comparison (836 changed RGB pixels confined to the gate). RE-143 supplies exact signed billboard scale from the animated hierarchy. Dream Land has no joint animation; its scenery moves through game code. RE-205 verified Saffron City's animated gate on **physical PSP hardware** (`regression_capture_scene5`), matching its PPSSPP golden with zero exceptions, and found/fixed a third `1.0 / payload` FPU-trap site in `objanim.rs::StageJoint::apply` (the same class `f111892` already fixed in fighter/material animation) that this exact scene exposed. |
| Billboard nodes | 🟢 100% | 109 nodes flagged; RE-131–133 shipped the real camera and distinct Kind48 basis. RE-140 inventories every source node; RE-141 corrected Kind46 spin to Z. RE-142 fixed animated-parent inheritance; RE-143 reproduces signed animated X/Y scale. RE-144 restores the original X/Y/X scale rule and adds stable isolation. RE-145 captures and individually reviews all 109 ordinals in PPSSPP: 103 visibly nondegenerate, four intentionally subpixel from authored `0.00001` scale, and two intentionally transparent from all-zero UVs sampling an alpha-zero texel. R0.12 is complete; physical PSP validation remains part of the later rendering gate. |
| Asset pack format | ✅ COMPLETE | Zero-copy, 16-byte aligned, little-endian; writer + reader unit-tested. Current pack version 27 includes animation tables, material-animation attachments, lighting state, local rest transforms, LBParticle data, alpha-compare state and texgen scale/origin state. |
| PSP asset loading | ✅ COMPLETE | Current version-27 pack loads aligned and cache-flushed; PPSSPP smoke-tested, with representative current-format captures on physical PSP |
| PSP mesh drawing | 🟡 90% / VERIFYING | Indexed GE draws, CLUT textures, addressing, animation, alpha gates, runtime lighting and texgen are implemented and exercised in PPSSPP/physical-PSP scenes. RE-226/RE-227/RE-228/RE-229 close raw-normal semantics, LookAt quantization, shared regular/linear texgen reference math (which also found and fixed a real overcorrected matrix constant), and linear integer conversion (truncation, not rounding — another dormant bug found and fixed); RE-230 confirms texgen-bound tiles never carry a nonzero `shift_s`/`shift_t` (no N64 shifting implementation needed) and records per-mode tile/scale/`G_LIGHTING` state for T7/T8. RE-231 (T7) checks real texgen materials' generated coordinates against `R2.0`/P0b's hardware addressing model: 25 of 34 real axis instances agree; 9 diverge by one texel, only at the sweep's `dot = +1` extreme, on a mask-narrowed clamp-without-mirror axis — measured and pinned as a regression baseline, fix opened as `T7a`. RE-232 (T7a) fixes the divergence at its real cause (`n64_addressing::psp_lowering_axis`'s host model and the matching real bake, `texture::mirror_extend`/`meshdraw::bind_texture`'s wrap-mode target), re-measured at a strict 0/34; `assets/generated/ssb64.pak` rebuilt with the fix. RE-233 fixes a real-hardware-only debug-overlay corruption found on a physical PSPLink run: `Gpu::draw_line_strip` (used by `draw_collision`/`draw_fighter`) submitted GE vertex data straight from a shared, CPU-rewritten `LINE_BUF` instead of `sceGuGetMemory`-allocated display-list-arena memory, so the async GE could read a later segment's overwritten data — invisible under PPSSPP, reproducible from frame 0 on real hardware, now fixed to match `draw_object_posed`'s existing safe pattern and re-verified clean on the same hardware. RE-234–236 close T8 (original-ROM Metal comparison: real 1P-mode capture, refreshed PPSSPP goldens, physical-PSP re-capture, all agreeing at the established noise floor). RE-236–237 close T9 (physical PSP matrix: regular rotations, linear texgen, a raw normal diagnostic and a new rotated-camera scene, all captured on real hardware). `romtool texgen --verify` (T10) runs the addressing/mode/scale/shift invariants above as a CLI gate against any ROM, not just a host test; RE-239 closed T10's last item (the textured→untextured→texgen mapping transition, measured absent from the real archive and covered by a synthetic test), closing `R2.1` (T1–T10) entirely. Independent depth writes, submission order, full DrawState cache isolation and T1's cross-node reuse gap (tracked, not blocking) remain open, along with `R2.2`'s corrective regressions. |
| Coordinate conversion | 🟢 80% | Matrix/UV/viewport unit-tested; needs on-hardware confirmation (RE-004, RE-005) |
| Battle camera / projection | ✅ COMPLETE | R0.14: default camera source port, viewport/aspect/depth, one-to-four fighter interest union, Wait zoom and original quantized trigonometry are tested. RE-151 reads the original ROM's live Dream Land camera state and matches distance/look-at within 0.1 game units and eye within 0.67; independent PPSSPP audit captures are byte-identical. Special camera modes belong to future gameplay states; physical PSP validation remains R2. |
| Math (scalar) | 🟢 80% | 36 unit tests; no VFPU path yet (correctly — profile first) |
| VFPU optimization | 🔴 0% | Deliberately not started |
| Engine traits (Layer B) | 🟢 70% | Renderer / Audio / Input / Timing / Clock defined |
| Timing / fixed clock | ✅ COMPLETE | Catch-up cap, backwards-clock, 60-ticks-per-second all unit-tested |
| Input mapping | 🟢 75% | Mapping + nub scaling unit-tested; deadzone and C-buttons unresolved (RE-008, RE-009) |
| PSP GU backend | 🟡 89% / VERIFYING | Init/frame lifecycle, matrices, indexed textured mesh draws, CI4/CI8 CLUT upload, mip-level upload, filtering, repeat/clamp addressing, alpha test/blend, depth/culling, billboard transforms, and runtime fighter lighting are implemented in `psp/src/gu.rs` and `psp/src/meshdraw.rs`; depth compare/write separation and complete GU-state invalidation remain open under R2.2/C3/C5 |
| PSP input backend | 🟢 70% | `sceCtrl` analog read wired to the shared mapping |
| PSP audio backend | 🔴 0% | |
| Physics | 🟢 60% | 16 functions ported with original addresses cited, and *driven* — `Fighter::tick` runs gravity, drift and material friction against the stage each tick. Running on all 27 characters' **real** constants, extracted from the ROM and verified field-by-field against the decompilation; the invented defaults they replaced were 26x off and had hidden a stick-scaling bug in air drift (RE-032) |
| Fighter state | 🟢 60% | The movement status machine: Wait, three walks, Dash, Run, RunBrake, Turn, KneeBend, Jump F/B, JumpAerial F/B, Fall, FallAerial, Squat, Landing light/heavy and Pass, with the original's interrupt-chain ordering and its tap-counter input model (RE-033). Plus roster, facing, hitlag/hitstun, spawn placement and every character's constants. All of them now **end on their own**: the five that had no duration in `FTAttributes` take it from their figatree animation instead, read out of the ROM and verified against the decompilation for all 27 fighters (RE-035). No attacks, specials, grabs, shields or damage states |
| Collision | 🟢 60% | Geometry extracted for all 41 stages, packed, and read back. Swept floor query, vertical floor projection, per-line surface height and the `mpprocess` floor path (substepping, landing snap, ledge corner, follow-the-surface) all ported. Surface flags confirmed against how Dream Land plays; `dMPCollisionMaterialFrictions` recovered. **158/158 spawns hold a simulated fighter still for 60 ticks at zero drift**, and the swept and projected solvers agree on every one (RE-030, RE-031). No ceiling or wall queries; moving groups are tested at rest |
| Animation | 🟢 90% | **Figatree scripts decode to per-joint transforms, and are packed.** The `AObjEvent16` command stream, `ftAnimGetTargetValue`'s per-track scales and the `AObj` cubic/linear/step interpolation are ported; `romtool figatree` plays all 189 movement animations for 40 frames with zero desynchronisation, and each one's script count matches its fighter's joint count under a rule with no exceptions (RE-036). Joints are mapped through `setup_parts` and `commonparts_container`, both read as archive relocations rather than matched by shape. The current pack carries all 189 animations, 4709 joint entries and each node's local rest transform, and `romtool figatree --pack` replays 3444 joints from it against the ROM with **every pose identical**. A `Skeleton` ticks every joint on device and the object's node matrices are recomposed from the result at 60 FPS, browsable in the viewer. **Validated**: composed poses match the ROM exactly across 3444 joints, no bone changes length across **204,547 measurements over all 189 animations**, the feet stay planted through the static grounded poses, and Turn's opening frame renders as a standing Mario (RE-038). **The status machine drives it**: `Play::tick` restarts the skeleton when the fighter changes status, at the speed the status supplies, and the stage view draws the posed model where the simulation puts it (`docs/images/m4-fighter-status.png`). **All twenty movement statuses** have an animation, not just the seven with a length — the current pack's 532 sparse fighter/slot entries replay **9,692 joints** identically against the ROM and preserve **567,662 bone lengths**. RE-171 additionally renders and captures all 532 under PPSSPP: every identity header advances uniquely, every centred crop contains model content, sampled captures hold 60 FPS, and the log is clean. The audit exposed and fixed `fighter_anim`'s false dense-table assumption, so missing original motions can no longer shift later runtime lookups. The `TransN` motions map correctly. No `translate_scales`; the viewer still frames its camera on the rest bounds |
| Scene graph (DObj) | 🟢 87% | All 363 discovered `DObjDesc` arrays plus 11 source-named direct effect constructions are packed as 374 objects (RE-172; one of the 12 direct effects aliases an already-discovered graph). Three union members of `DObj`'s display-list field resolve, and node lists convert in draw order through one shared vertex cache. `MObj` material chains cover **all 127 graphs that require them**; all 468 paired nodes have zero chain/demand mismatches. `GObj` layer and general animation remain absent. |
| Effects / particles | 🟢 90% | RE-172–189 cover all 53 manager descriptors, 46 unique display-bearing effects, 160 `LBParticle` scripts, 65 texture series, 246 frames, material/texture/colour animation, spawn trees, `LBGenerator`, PSP drawing, exhaustive device audits, and one live manager-effect runtime spawn event. This closes R1 renderer scope. Facing-dependent streams and the other 25+ gameplay call sites remain later integration work. |
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

531 workspace tests passing: `ssb-rom` 373, `ssb-engine` 36, `ssb-game` 120,
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

1. **Physical PSP hardware validation is still in progress.** PPSSPP is not
   proof of hardware behaviour (`AGENTS.md` §16). RE-201–237 record
   representative PSP Slim/6.61 captures for the current renderer. Texgen's
   physical-PSP validation is now complete: RE-236/RE-237 captured all five
   `R2.1`/T9 matrix items (regular rotations, linear texgen, a raw normal
   diagnostic, a rotated-camera scene) on real hardware, and RE-234–236 closed
   the original-N64 Metal comparison (`R2.1`/T8) against real 1P-mode play.
   Exhaustive coverage beyond texgen, and the `R2.2` corrective regressions,
   remain open. Treat every "on device" claim elsewhere in this file as PPSSPP
   unless a hardware model and build are cited. Separately, a real bug is open
   and tracked (not fixed): the `debug_overlay` PSP viewer's object-view HUD
   text renders corrupted/double-exposed in every physical capture.

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
