# Porting Status

Per `PLAN.md` §12/13. Percentages are of *intended scope for that subsystem*,
not of the original's line count. A subsystem is only `COMPLETE` when it has
been functionally validated, not merely compiled.

This file tracks **per-subsystem** implementation status only. Current
milestone/task and overall execution state live in `STATUS.md`; the ordered
roadmap with acceptance criteria lives in `PLAN.md`/`plans/**`. Do not add a
milestone table here — that state belongs in exactly one place.

Detailed investigation narratives (what was measured, how a bug was found and
fixed) live in `docs/evidence/re/RE-XXX.md`, not in this table. This file
records the current model only: what works, what evidence backs it, and what
remains.

## Subsystems

| Subsystem | Status | Current capability | Evidence | Remaining gap | Task |
|---|---|---|---|---|---|
| ROM validation | COMPLETE | SHA-1/MD5 checked against the real dump; byte-order/size rejection unit-tested | — | — | M2 |
| VPK0 decompression | COMPLETE | All 499 compressed files decode, cross-verified against independent ROM geometry | RE-002 | — | M2 |
| relocData archive | COMPLETE | 2132/2132 files load; 61,343 intern + 3,092 extern relocations, 0 mismatches | RE-001 | Runtime extern-relocation loader (patches at scene load) not built | M2 |
| Asset extraction CLI | COMPLETE | `romtool extract` produces 16.29 MiB + manifest | — | — | M2 |
| F3DEX2 DL parser | COMPLETE | All opcodes Smash emits, verified against real lists | RE-017 | — | R0.2 |
| N64 texture decode | 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 decoded; 638 material textures + 246 LBParticle frames ship in pack | — | — | R0.3 |
| Texture → PSP conversion | COMPLETE for measured renderer scope | Mirror/clamp/origin/signed-coordinate lowering, palette banks, texgen T1–T10 gate all complete. Pack format v29 | RE-201, RE-219–239, RE-262–269 | N64 3-point filtering vs PSP bilinear is an accepted fixed-function deviation (RE-219) | R0.5, R2.0 |
| DL discovery | COMPLETE | 1,864 lists across 135 files; converter used as its own validator | RE-017 | — | R0.2 |
| Mesh conversion | VERIFYING (88%) | 0 archive-wide conversion failures; PRIM ownership, load-time lighting provenance, and independent depth compare/write/mode state are implemented and closed (`R2.2` C1–C7) | RE-217, RE-240–261 | Physical-PSP confirmation of the depth-mask fix; full narrative in `plans/rendering/R2.md` | R2.2 |
| Model conversion | VERIFYING (82%) | Meshes extracted, DObj hierarchy applied, all 127 discovered material graphs paired, primitive colours applied on covered combiner paths | RE-163, RE-240–243 | — | R0.7, R2.2 |
| Stage animation | 90% | `AObjEvent32` joint stream decoded, packed, played on device: 35 stages, 206 animated nodes at 60 FPS; every packed pose matches the archive across 444,960 values | RE-050–052, RE-142, RE-143, RE-205 | Dream Land has no joint animation (scenery moves through game code, not a gap) | R0.9 |
| Billboard nodes | 100% | 109 nodes flagged and captured; real camera basis, Kind46/48 spin and scale rules, animated-parent inheritance all shipped | RE-131–133, RE-140–145 | Physical PSP validation is part of the later rendering gate | R0.12 |
| Asset pack format | COMPLETE | Zero-copy, 16-byte aligned, little-endian; writer + reader unit-tested. v29 carries animation, material-animation, lighting, rest transforms, LBParticle, alpha-compare, texgen, independent depth state, signed-clamp UV marker | — | — | M2 |
| PSP asset loading | COMPLETE on PSP-2000/3000/Slim | v29 pack (26,254,608 B) loads aligned/cache-flushed; `MEMSIZE=1` physically proven | RE-256, RE-260 | PSP-1000's 32 MiB compatibility unresolved (can't use `MEMSIZE=1`) | R2 |
| PSP mesh drawing | VERIFYING (90%) | Indexed GE draws, CLUT textures, addressing, animation, alpha gates, runtime lighting and texgen implemented and exercised in PPSSPP and on physical PSP | RE-226–239, RE-251–254, RE-261, RE-270, RE-271 | Full derivation in `plans/rendering/R2.md`; remaining gate is physical R2 coverage (PSP-1000, broader scenes, longer runs) | R2.1, R2.2, R2 |
| Coordinate conversion | 80% | Matrix/UV/viewport unit-tested; signed N64 S10.5 UVs on clamped axes now expand to float UVs instead of being misread as unsigned | RE-262 | On-hardware confirmation beyond the current physical matrix | R0.8 |
| Battle camera / projection | COMPLETE | Default camera source-ported; viewport/aspect/depth, 1-4 fighter interest union, Wait zoom, quantized trigonometry tested; matches original ROM camera state within 0.1 game units | RE-151 | Special camera modes are future gameplay states; physical PSP validation is R2 | R0.14 |
| Math (scalar) | 80% | 36 unit tests | — | No VFPU path yet — correctly, profile first (D-032) | R3 |
| VFPU optimization | 0% | Deliberately not started | — | — | R3/G5 |
| Engine traits (Layer B) | 70% | Renderer / Audio / Input / Timing / Clock traits defined | — | — | M3 |
| Timing / fixed clock | COMPLETE | Catch-up cap, backwards-clock, 60-ticks-per-second all unit-tested | — | — | M3 |
| Input mapping | 75% | Mapping + nub scaling unit-tested | — | Deadzone and C-button mapping unresolved | TODO (RE-008, RE-009) |
| PSP GU backend | VERIFYING (89%) | Init/frame lifecycle, matrices, indexed textured mesh draws, CLUT upload, mip upload, filtering, addressing, alpha test/blend, depth/culling, billboards, runtime fighter lighting all implemented | RE-251, RE-254, RE-262, RE-264 | — | R2.2 |
| PSP input backend | 70% | `sceCtrl` analog read wired to the shared mapping | — | — | M3 |
| PSP audio backend | 0% | Not started | — | Mixer thread, VADPCM decode, sequencing all open | G4 |
| Physics | 60% | 16 functions ported with original addresses cited, driven every tick against real per-character constants (all 27 fighters) extracted from ROM and verified field-by-field against decomp | RE-032 | — | M3 |
| Fighter state | 60% | Movement status machine (Wait/walks/Dash/Run/Turn/Jump/Fall/Squat/Landing/Pass) with original interrupt-chain + tap-counter model; all statuses end on their own via figatree-derived duration | RE-033, RE-035 | No attacks, specials, grabs, shields or damage states (blocked behind rendering gate) | M3, G0 |
| Collision | 60% | Geometry extracted/packed for all 41 stages; swept + projected floor solvers agree on 158/158 spawn tests | RE-030, RE-031 | No ceiling/wall queries; moving groups tested at rest only | M3 |
| Animation | 90% | Figatree scripts decode to per-joint transforms and are packed; 189 movement animations, 4709 joint entries, all poses match ROM exactly; skeleton ticks at 60 FPS on device; all 532 sparse fighter/slot entries replay correctly | RE-036, RE-038, RE-171 | No `translate_scales`; viewer camera frames on rest bounds only | M3 |
| Scene graph (DObj) | 87% | All 363 discovered `DObjDesc` arrays + 11 direct effects packed as 374 objects; `MObj` chains cover all 127 graphs requiring them, 0 mismatches | RE-172 | `GObj` layer and general animation remain absent | R0.7 |
| Effects / particles | 90% | 53 manager descriptors, 46 display-bearing effects, 160 `LBParticle` scripts, 65 texture series, 246 frames all covered; closes R1 renderer scope | RE-172–189 | Facing-dependent streams and 25+ gameplay call sites are later integration work | R1 |
| Stages | 65% | All 41 `MPGroundData` headers recovered; collision decoded and packed for all 41; all 100 render layers resolve to a packed object; automated audit captures all 41 at 60 FPS | RE-028, RE-029, RE-170 | No stage *loader* — viewer browses stages, a match does not select one | G2 |
| Items | 0% | Not started | — | — | G1 |
| CPU AI | 0% | Not started | — | — | G1 |
| Menus | 0% | Not started | — | — | G3 |
| Save data | 0% | Not started | — | — | G3 |
| Debug/profiler | 20% | Frame timing sections defined; on-screen text overlay working | — | — | — |
| CI | COMPLETE | fmt, clippy, host tests, PSP build, EBOOT artifact — no ROM required | — | — | — |

**Per-fighter combat progress: all 12 at 0%.** Correctly so — combat
(`PLAN.md` G0) is blocked behind the rendering gate (R0–R3) and has not
started. Fighter *models*, *animation* and *movement physics* are
implemented and tracked above; only combat-specific state (attacks,
hitboxes, damage) is unstarted.

## Test coverage

See `STATUS.md` "Current verification baseline" for the live workspace test
count. Reproduce with `cargo test --workspace`.

## M1 verification (PPSSPP)

Verified 2026-08-27 under PPSSPP 1.20.4 (OpenGL and software rasteriser),
X11: module loads and imports resolve, `PARAM.SFO` title correct, GE display
lists submit at a locked 60.0 FPS, geometry renders with correct
vertex-colour interpolation and depth, animation advances, and physics runs
on-device (test object lands at exactly y = -3.00). Exact-lockstep timing
diagnostics recorded in RE-016. These are baseline numbers on a four-triangle
scene under an emulator, not a performance prediction for real hardware.
Reproduce with `tools/run-ppsspp.sh`.

## Known gaps and honest caveats

1. **Physical PSP hardware validation is ongoing, not complete.** PPSSPP is
   not proof of hardware behaviour (`AGENTS.md`). See `STATUS.md` and
   `plans/rendering/R2.md` for the current physical-matrix state. A real,
   still-tracked bug (not confirmed fixed): the `debug_overlay` PSP viewer's
   object-view HUD text renders corrupted/double-exposed in physical capture
   (RE-224). Treat every "on device" claim elsewhere in this file as PPSSPP
   unless a hardware model and build are cited.

2. **The debug overlay only displays under PPSSPP's software rasteriser.**
   Emulator limitation, not a port bug (RE-014): `sceGuDebugFlush` paints
   VRAM with the CPU, and PPSSPP's hardware backends don't reflect those
   writes. `tools/run-ppsspp.sh` forces the software renderer. A future GE-
   geometry HUD (Renderer 3) removes the dependency entirely.

3. **Extracted assets are unparsed below the archive layer** for anything
   `romtool` doesn't yet interpret as textures/meshes/attribute tables — the
   archive layer itself is trustworthy.

4. **Attribute coverage stops at the scalar head.** All 27 characters'
   `FTAttributes` are extracted, packed and verified field-by-field against
   the decompilation (RE-032), but only the leading 45 scalars are decoded;
   hurtbox descriptors, sound IDs and joint indices further into the struct
   are untouched.

5. **The movement animation pipeline is far along; combat does not exist.**
   No attacks, hitboxes, hurtboxes, damage, knockback, hitstun, opponent,
   stocks or match loop — per the rendering gate, none of that may start
   until R0–R3 are complete. See `TODO.md` "Combat Vertical Slice".

6. **Extern relocation slots are zeroed, not resolved.** `romtool` records
   them in the manifest; the runtime loader that patches them at scene load
   does not exist yet. The *converter* already follows them (RE-037).
