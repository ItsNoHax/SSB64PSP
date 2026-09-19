# Porting Status

`PLAN.md`'s active roadmap is milestones `P0`–`P5`. The `Task` column below
still cites legacy pre-batch-mode codes (`M`/`R0.x`/`F1`/`G0`–`G5`) recorded
when each row was last updated — read them as history, archived in
`plans/rendering/*.md`/`plans/gameplay/*.md`, not as the current tracker.

Percentages are of *intended scope for that subsystem*,
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
| Input mapping | 75% | Mapping + nub scaling unit-tested; C-button jump function confirmed against decomp and wired in `psp-game` | RE-008, RE-295 | Deadzone unresolved; taunt/camera C-button functions still unconfirmed | TODO (RE-008, RE-009) |
| PSP GU backend | VERIFYING (89%) | Init/frame lifecycle, matrices, indexed textured mesh draws, CLUT upload, mip upload, filtering, addressing, alpha test/blend, depth/culling, billboards, runtime fighter lighting all implemented | RE-251, RE-254, RE-262, RE-264 | — | R2.2 |
| PSP input backend | 70% | `sceCtrl` analog read wired to the shared mapping | — | — | M3 |
| PSP audio backend | 0% | Not started | — | Mixer thread, VADPCM decode, sequencing all open | G4 |
| Physics | 60% | 16 functions ported with original addresses cited, driven every tick against real per-character constants (all 27 fighters) extracted from ROM and verified field-by-field against decomp | RE-032 | — | M3 |
| Fighter state | 63% | The full `FTCommonStatus` ordinal table (0..=219, `ft/ftcommon/ftcommonstatus.h`) is now in `Status`, matching the decomp's own numbering exactly. Movement status machine (Wait/walks/Dash/Run/Turn/Jump/Fall/Squat/Landing/Pass) unchanged. The Damage/hitstun family (`DamageHi/N/Lw1-3`, `DamageAir1-3`, `DamageFlyN`/`FlyTop`, `DamageFall`) now has real entry/exit: a landed hit picks its status from `ftCommonDamageGetDamageLevel`'s hitstun tiers and the defender's grounded/airborne situation, and hitstun running out returns to `Wait` or `DamageFall` (`ftCommonDamageCommonProcInterrupt`/`...AirCommonProcInterrupt`). Plus, under `F1`'s scoped Training-only exception (`AGENTS.md`): Mario's neutral jab (`Attack11`), input to hitstun, pixel-confirmed landing on the real dummy target | RE-033, RE-035, RE-294, RE-295 | Every status past the wired movement/Damage/base-moveset subset exists as an ordinal only, with no callback behaviour (specials, grabs, smashes, aerials besides what's listed below — `P2` scope). Within Damage: no hit-location Hi/Lw index (every hit reads as "N"/middle), no `DamageFlyRoll` (needs a shared RNG source), and the ground-hit-still-launches-airborne angle branch is dropped (`Status::is_grounded`'s docs) | M3, G0 |
| Base moveset (Mario) | 15% | `AttackDash`/`AttackS3Hi`/`AttackS3`/`AttackS3Lw`/`AttackHi3`/`AttackLw3` ported from `dMarioMainMotion_DashAttack`/`FTiltHigh`/`FTilt`/`FTiltLow`/`UTilt`/`DTilt` (`relocData/202_MarioMainMotion.c`), plus `Attack11`'s second hitbox (was a documented gap, now both real). Hit resolution generalized to a per-`(FighterKind, Status)` `MoveData` table (`crate::attack::move_data`) carrying every hitbox and its own active-frame window, instead of the old Attack11-only special case — `apply_hit_from` now works for any ported status/fighter pair, including a hitbox's `ox` mirrored by facing (closing part of the old "no joint attachment" gap: `oy`/`oz` still are not). Entry conditions (forward/up/down stick-angle gates, the tilt-vs-jab dispatch priority, dash-attack's `A`-tap from `Dash`/`Run`) are real, transcribed from `ftcommonattacks3.c`/`attackhi3.c`/`attacklw3.c`/`attackdash.c`; the angle tests are reframed as `tan()` slope comparisons since `ssb_engine::math` has no `atan2`. Fixed a real pre-existing gap alongside this: `walk_interrupt` was missing `Attack1`/the tilts entirely even though the decomp macro has them, so attacking out of a walk did nothing | — | Everything else in Mario's moveset (smashes — need a charge-mechanic subsystem this batch didn't build — aerials, specials, the `Attack12`/`Attack13`/`Attack100` jab-combo extension, grabs/throws); no other fighter has any moveset data yet; `DTilt`'s repeated-tap extension not ported (always exits to `SquatWait` after one hit) | P2 |
| Shield/guard | 40% | `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` state machine ported (`crate::status::GuardState`, `ft/ftcommon/ftcommonguard{1,2}.c`): shield health decay while held, passive-regen timer field, release-lag-gated `GuardOff`, and a hit landing on a shielding fighter is redirected into `GuardSetOff`'s pushback (`crate::attack::is_shielding`/`apply_shield_hit`) instead of the normal Damage path — a block takes no damage/hitstun, only shield-health loss. Shield break transitions to `ShieldBreakFly` and resets health to 30 | — | No animation lengths (`GuardOn`/`GuardOff` collapse to ~1 tick and a release-lag count instead of the real multi-frame startup/lower clips — `GuardState` docs); no shield-bubble visual scaling/joints (rendering, out of this batch's scope); `ShieldBreakFly`'s fly→fall→down/stand→`FuraFura` mash-out chain is ordinal-only, no callback; no multi-hit-per-frame `shield_damage_total` accumulation (one hitbox at a time, per `crate::attack`'s existing scope); dash-into-shield (`slide_tics`) and Yoshi's hurtbox-swap special case not ported | P2 |
| KO/death/respawn | 45% | Blast-zone crossing (`crate::status::check_dead`, `ftCommonDeadCheckInterruptCommon`) enters `DeadDown`/`DeadLeftRight`/`DeadUpStar` and decrements a stock immediately, matching the original's bottom→right→left→top priority. After the real `FTCOMMON_DEAD_WAIT`/`DEADUP_WAIT`-derived timer, the caller-invoked `try_rebirth` (needs the stage's respawn point, which `update` has no way to receive) enters `Sleep` if stocks are exhausted or the `RebirthDown`→`RebirthStand`→`RebirthWait` sequence otherwise — real total duration (390 frames) preserved, ending in `Fall` with a real 120-frame invincibility window (`Fighter::invincible_frames`) that both `apply_hit_from` and `apply_shield_hit` now respect. Damage resets to 0 on respawn | — | No 1-in-6 `DeadUpFall` branch (needs an RNG source, same gap as `DamageFlyRoll`); the halo drop/flight/colour-fade visuals, camera-mode switches, and 1P-mode/team-respawn branches are not ported (rendering/scene-manager scope, module docs); no multi-respawn halo-position offsetting; `GuardOn`'s Yoshi-only side cases and Boss/`is_ignore_dead` exemptions not ported | P2 |
| Shield/guard | 40% | `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` state machine ported (`crate::status::GuardState`, `ft/ftcommon/ftcommonguard{1,2}.c`): shield health decay while held, passive-regen timer field, release-lag-gated `GuardOff`, and a hit landing on a shielding fighter is redirected into `GuardSetOff`'s pushback (`crate::attack::is_shielding`/`apply_shield_hit`) instead of the normal Damage path — a block takes no damage/hitstun, only shield-health loss. Shield break transitions to `ShieldBreakFly` and resets health to 30 | — | No animation lengths (`GuardOn`/`GuardOff` collapse to ~1 tick and a release-lag count instead of the real multi-frame startup/lower clips — `GuardState` docs); no shield-bubble visual scaling/joints (rendering, out of this batch's scope); `ShieldBreakFly`'s fly→fall→down/stand→`FuraFura` mash-out chain is ordinal-only, no callback; no multi-hit-per-frame `shield_damage_total` accumulation (one hitbox at a time, per `crate::attack`'s existing scope); dash-into-shield (`slide_tics`) and Yoshi's hurtbox-swap special case not ported | P2 |
| Ledges | 45% | Full `CliffCatch`→`CliffWait`→(`Climb`/`Attack`/`Escape`)`Quick`/`Slow`→`Wait` state machine (`crate::status::CliffState`, `ft/ftcommon/ftcommoncliffcatchwait.c`, `ftcommoncliffclimb.c`, `ftcommonattack.c`, `ftcommonescape.c`). Catch detection (`cliff_catch_candidate`) reuses `crate::collision::check_floor` — the same swept-segment primitive floor-landing already uses — plus the real `800.0` corner-proximity tolerance and `MAP_VERTEX_COLL_CLIFF` flag from `mpProcessCheckTestLCliffCollision`/`RCliffCollision`. `CliffWait`'s exits are all real: attack/escape button taps, the climb-or-drop stick-angle test (reframed as a `tan(50°)` slope comparison so it needs no `atan2`), and the damage-dependent auto-release timeout into `DamageFall` with the real 30-frame `cliffcatch_wait` re-grab cooldown (`Fighter::cliffcatch_wait`) | — | No per-character `cliffcatch_coll` hand-reach offset (not in the extracted attribute range — same gap class as `Attack11`'s hitbox offset), so the fighter hangs exactly at the ledge corner rather than at arm's length; no ledge-hog exclusivity (needs match-wide fighter awareness a single-`Fighter` function can't have — the caller must still apply it, `cliff_catch_candidate`'s docs); `CliffCatch`/climb/attack/escape animation lengths not extracted, so each phase collapses to one tick instead of its real multi-frame clip; no per-character `cliff_status_ga` table, so every recovery option ends grounded | P2 |
| KO/death/respawn | 45% | Blast-zone crossing (`crate::status::check_dead`, `ftCommonDeadCheckInterruptCommon`) enters `DeadDown`/`DeadLeftRight`/`DeadUpStar` and decrements a stock immediately, matching the original's bottom→right→left→top priority. After the real `FTCOMMON_DEAD_WAIT`/`DEADUP_WAIT`-derived timer, the caller-invoked `try_rebirth` (needs the stage's respawn point, which `update` has no way to receive) enters `Sleep` if stocks are exhausted or the `RebirthDown`→`RebirthStand`→`RebirthWait` sequence otherwise — real total duration (390 frames) preserved, ending in `Fall` with a real 120-frame invincibility window (`Fighter::invincible_frames`) that both `apply_hit_from` and `apply_shield_hit` now respect. Damage resets to 0 on respawn | — | No 1-in-6 `DeadUpFall` branch (needs an RNG source, same gap as `DamageFlyRoll`); the halo drop/flight/colour-fade visuals, camera-mode switches, and 1P-mode/team-respawn branches are not ported (rendering/scene-manager scope, module docs); no multi-respawn halo-position offsetting; `GuardOn`'s Yoshi-only side cases and Boss/`is_ignore_dead` exemptions not ported | P2 |
| Collision | 60% | Geometry extracted/packed for all 41 stages; swept + projected floor solvers agree on 158/158 spawn tests | RE-030, RE-031 | No ceiling/wall queries; moving groups tested at rest only | M3 |
| Animation | 90% | Figatree scripts decode to per-joint transforms and are packed; 189 movement animations, 4709 joint entries, all poses match ROM exactly; skeleton ticks at 60 FPS on device; all 532 sparse fighter/slot entries replay correctly | RE-036, RE-038, RE-171 | No `translate_scales`; viewer camera frames on rest bounds only | M3 |
| Scene graph (DObj) | 87% | All 363 discovered `DObjDesc` arrays + 11 direct effects packed as 374 objects; `MObj` chains cover all 127 graphs requiring them, 0 mismatches | RE-172 | `GObj` layer and general animation remain absent | R0.7 |
| Effects / particles | 90% | 53 manager descriptors, 46 display-bearing effects, 160 `LBParticle` scripts, 65 texture series, 246 frames all covered; closes R1 renderer scope | RE-172–189 | Facing-dependent streams and 25+ gameplay call sites are later integration work | R1 |
| Stages | 65% | All 41 `MPGroundData` headers recovered; collision decoded and packed for all 41; all 100 render layers resolve to a packed object; automated audit captures all 41 at 60 FPS | RE-028, RE-029, RE-170 | No stage *loader* — viewer browses stages, a match does not select one | G2 |
| Items | 0% | Not started | — | — | G1 |
| CPU AI | 0% | Not started | — | — | G1 |
| Menus | 35% | `psp-game/`, a second independent EBOOT; Intro→Menu state machine navigable; Training spawns a real, physics-ticked Mario and a stationary dummy target on a real stage (Dream Land), draws both through the real battle camera; a real, decomp-sourced jump binding lets the player reach the dummy's real spawn point, and the jab connecting is pixel-confirmed via PPSSPPHeadless and now physical PSP hardware | RE-289–296 | Menu/select labels are still colour blocks, no `sceFont` text; character/stage select feed a hardcoded default, not a real selection UI | F1, G3 |
| Save data | 0% | Not started | — | — | G3 |
| Debug/profiler | 20% | Frame timing sections defined; on-screen text overlay working | — | — | — |
| CI | COMPLETE | fmt, clippy, host tests, PSP build, EBOOT artifact — no ROM required | — | — | — |

**Per-fighter combat progress: Mario has jab + dash attack + all three
tilts; the other 11 fighters are at 0%.** `PLAN.md`'s combat-systems and
fighter-common-gameplay work (`P2`) is underway: the shared machinery
(status table, Damage/hitstun, shield/guard, KO/death/respawn, ledges, and
now a generic per-fighter `MoveData` hit-resolution table) is built and
tested against Mario as the first fighter, following on from `F1`'s scoped
neutral-jab carve-out (`RE-294`) which predates this work. Fighter *models*,
*animation* and *movement physics* are implemented and tracked above. Still
untouched for every fighter including Mario: specials, grabs/throws (needs
per-character throw motion data — deferred, see `STATUS.md`), smashes (needs
a charge-mechanic subsystem), aerials, and the jab-combo extension past
`Attack11`/`Attack12`. Rendering fidelity/performance (`P5`) runs in
parallel with gameplay work, not as a precondition for it (`AGENTS.md`).

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

5. **The movement animation pipeline is far along; general match combat does
   not exist yet.** No opponent AI, stocks or match loop, and no attacks for
   any fighter besides Mario's jab — that is `P2`'s scope, and `P1` (decomp
   compatibility layer) is current. Ahead of `P2`, `F1`'s scoped
   Training-only exception (`AGENTS.md`) ported Mario's jab end to end
   (hitbox, hurtbox, damage, knockback, hitstun — `RE-294`) against a
   stationary dummy target, now pixel-confirmed connecting via a real jump
   binding (`RE-295`). See `TODO.md` "Combat Vertical Slice".

6. **Extern relocation slots are zeroed, not resolved.** `romtool` records
   them in the manifest; the runtime loader that patches them at scene load
   does not exist yet. The *converter* already follows them (RE-037).
