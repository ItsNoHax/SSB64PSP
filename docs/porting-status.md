# Porting Status

Per-subsystem implementation state. Current batch: [`STATUS.md`](../STATUS.md).
Roadmap: [`PLAN.md`](../PLAN.md). How each result was established:
[`evidence/INDEX.md`](evidence/INDEX.md).

A subsystem is `COMPLETE` only when functionally validated, not just compiled.
Percentages are of the subsystem's intended scope. "Verified" means PPSSPP
unless a physical PSP is named.

## Asset pipeline

| Subsystem | Status | Capability | Gap | Evidence |
|---|---|---|---|---|
| ROM validation | COMPLETE | SHA-1/MD5, byte order and size checks | — | — |
| VPK0 | COMPLETE | All 499 compressed files decode | — | RE-002 |
| relocData archive | COMPLETE | 2,132 files; 61,343 intern + 3,092 extern relocations, 0 mismatches | Extern linker (`ssb_rom::reloc_link`) matches the original's allocation and RDRAM layout but is unused at runtime | RE-001, RE-340, RE-341 |
| F3DEX2 parser / DL discovery | COMPLETE | Every emitted opcode; 1,864 lists in 135 files, 0 failures | — | RE-017 |
| Texture decode | 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 | 26 runtime-framebuffer references have no ROM texels | RE-055 |
| Texture → PSP | COMPLETE for measured scope | Mirror/clamp/origin lowering, palette banks, TLUT mode, sample-centre alignment, build-time 3-point filter compensation | Fixed-function bilinear cannot equal N64 3-point exactly | RE-219–239, RE-304–313 |
| Mesh / model conversion | VERIFYING | 0 conversion failures; all 127 material graphs paired; PRIM ownership, lighting provenance, independent depth state | Physical-PSP confirmation | RE-163, RE-240–261 |
| Scene graph (DObj) | 87% | 363 `DObjDesc` arrays + 11 effects packed as 374 objects | `GObj` layer | RE-172 |
| Asset pack | COMPLETE | v41, zero-copy, 16-byte aligned; attack/special and grab/throw clips through Ness; shared `FuraSleep`, reaction, all 16 cliff clips and the shared damage and attack slots for all twelve fighters (RE-355); cliff kinetics mask (RE-353); original collision vertex IDs; two-tile blend and animated colour registers; recorded render-tile layout and `unk10 == 1` inputs; per-texture compensation flag and source digest | — | RE-301, RE-312, RE-319, RE-321, RE-322, RE-327, RE-330, RE-334, RE-336, RE-338, RE-345–347, RE-351, RE-353, RE-355 |

## Rendering

Detail per domain: [`rendering.md`](rendering.md).

| Subsystem | Status | Capability | Gap | Evidence |
|---|---|---|---|---|
| PSP GE backend | VERIFYING (89%) | Indexed textured draws, CLUTs, addressing, alpha test/blend, depth, culling, lighting, texgen, material animation incl. `SetLFrac` two-tile blend and `PrimColor`/light tracks, effect-owned material clocks, camera head-1 XLU seed, GE state cache | Physical-PSP confirmation | RE-251–264, RE-301, RE-321, RE-322, RE-327, RE-328 |
| Camera / projection | COMPLETE | Default battle camera; matches original ROM camera state within 0.1 units | Special camera modes | RE-151 |
| Coordinate conversion | 80% | Matrices, UVs incl. signed S10.5 on clamped axes, pillarboxed viewport | On-hardware confirmation | RE-004, RE-005, RE-262 |
| Stage animation | 90% | 35 stages, 206 animated nodes; packed poses match the archive; source mesh/subtree visibility flags | — | RE-050–052, RE-142, RE-353 |
| Billboards | COMPLETE | 109 nodes; camera basis, spin and signed scale rules | — | RE-131–145 |
| Effects / particles | 90% | 46 display effects, 160 `LBParticle` scripts, 246 frames | Gameplay call sites | RE-172–189 |
| Framebuffer effects | COMPLETE for renderer scope | LB-transition capture, 11 wipes, 1P wallpaper | Gameplay triggers | RE-146–149, RE-190–193 |
| Fighter shadows | COMPLETE for Training | Source `ftShadowProcDisplay` floor strip, multi-fighter | Team colours, moving map groups | RE-302 |
| UI | 0% | Debug overlay only | Real GE HUD and menus | RE-014 |

## Platform

| Subsystem | Status | Capability | Gap | Evidence |
|---|---|---|---|---|
| PSP asset loading | COMPLETE on PSP-2000 and later | `MEMSIZE=1` 64 MiB mode; stock v32 pack loaded via PSPLink on PSP-2000 | PSP-1000 (32 MiB) | RE-256, RE-260, RE-288, RE-320 |
| Timing | COMPLETE | Fixed 60 Hz with catch-up cap | — | — |
| Input | 85% | `psp-game` PSP→N64 layout (see README); viewer keeps its own; shared PSP polling is nonblocking; C-buttons are one jump button, R expands to A + Z | Nub deadzone unmeasured; taunt (L) unported | RE-008, RE-009, RE-295, RE-329 |
| Engine traits | 70% | Renderer, audio, input, timing, clock | — | — |
| Math | 80% | Scalar math | VFPU after profiling | [D-032](decisions/D-032.md) |
| Audio | 0% | — | Mixer thread, VADPCM, sequencer | — |
| Save data | 0% | — | — | — |
| Debug / profiler | 20% | Frame timing sections, text overlay | — | — |
| CI | COMPLETE | fmt, clippy, host tests, no_std, PSP EBOOT builds; no ROM needed | — | — |

## Gameplay

| Subsystem | Status | Capability | Gap | Evidence |
|---|---|---|---|---|
| Physics | 65% | Ground/air velocities, gravity, friction, fastfall; independent damage velocity decay in air and along floors; per-fighter constants for all 27 kinds verified against the decomp | — | RE-032, RE-349 |
| Collision | 85% | All 41 maps packed; fighter/weapon diamonds against floors, ceilings and walls; relative moving-group sweeps, float offsets, first-substep carry and wind push, copied previous diamonds; source probes, normals, vertex identity, pass and edge callbacks; damage floors | Stage objects in the runtime | RE-030, RE-031, RE-345, RE-348, RE-351, RE-353, RE-356 |
| Animation | 95% | Figatree playback at 60 Hz; posed joint transforms and TransN root motion feed gameplay; grab/thrown/cargo clips through Ness; every fighter's damage, attack, taunt and `LandingAirX` clips through one status → slot mapping (`LandingAirNull` at its landing-lag rate); Ness double-jump root motion; Yoshi Bomb holds its pose at speed zero; held TopN uses the catcher's joint rotation and its own child offset | Reflector effect phases | RE-036, RE-038, RE-171, RE-299, RE-330–335, RE-337–338, RE-343, RE-345–347, RE-355 |
| Status machine | 75% | Full `FTCommonStatus` table (0–219); movement, Damage/hitstun, per-character `AnyStatus`, Yoshi Egg Lay victim and aerial jump, Falcon Dive capture, Kirby and Jigglypuff multi-jump, Ness special states and repeat down tilt, Inhale victim/star statuses, `FuraSleep` with mash-out, Escape, Down/Passive, ShieldBreak/FuraFura, Rebound; attack lengths from figatrees | Remaining statuses (guard, teeter, item and hammer statuses) are ordinals only and keep the previous pose | RE-033, RE-035, RE-294, RE-337–338, RE-343, RE-345–347, RE-351, RE-355 |
| Hit resolution | IMPLEMENTED | Motion-script bytecode for all 12 fighters and the common moveset (`crate::motion`, generated from the decomp) makes attack collisions with element, rebound, `sd` and `ga`, hit-status windows and hurtbox edits; per-frame search over every pair with group records, swept tests, clank and `throw_gobj`, then the hit log and `ftMainProcParams` (`crate::combat`); joint hurtboxes for all 12 fighters (Hi/N/Lw placement, grabbable boxes); knockback with the frame's damage queue, crouch, stacking; `damage_resist`; hitlag with electric/crouch multipliers and buffered taps; Smash DI; launch/slide/bounce, `DamageFlyRoll`, electric `DamageE1/E2`, sleep; knockdown, tech and roll chains (`crate::reaction`); weapon attributes checked against `WPAttributes`; stale queue and Training handicaps; weapon–attack clank, `FTSpecialColl` reflector/absorb, knockback-overrun invincibility, same-frame catcher/held hits, damage-sweep `WallDamage`/`StopCeil`/`DownBounce` (RE-351); item–fighter clank, reflector, shield/hop and hurt searches plus fighter/item/weapon attacks against item damage boxes; damage drops; a Training jab lands in PPSSPP | The Training `grab` scene whiffs on PSP; team checks | RE-294, RE-330, RE-332–335, RE-337–339, RE-342–352 |
| Shield / guard | 80% | `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` at clip length, shield damage with `sd`, regen, bubble scaled by health, guard jump/pass/roll/grab out, break fly/fall/down/`FuraFura` chain | Bubble visual, tilt pose | RE-349–351 |
| Ledges | 85% | All 16 authored clips and dispatch/action clocks; live corner + TransN hanging poses; phase-two floor snap, root motion and per-fighter ground/air kinetics; damage/drop default sweep; source hand reach, occupancy and re-grab cooldown | Visual/audio side effects and wider match integration | RE-348, RE-353 |
| KO / respawn | 50% | Blast zones, stock loss, rebirth sequence, 120-frame invincibility; `DeadUpStar`/`DeadUpFall` 1-in-6 choice on the shared RNG with the source's 226-update wait | Halo visuals, team/1P branches | RE-351 |
| Recovery (`FallSpecial`) | 60% | Shared helpless fall and landing; driven by Mario, Luigi, Donkey, Samus, Link, Captain Falcon, Pikachu and Ness up-B; source stick-gated platform pass and ledge auto-catch | Damage/knockdown recovery remains separate shared work | RE-299, RE-338, RE-345, RE-347–348, RE-353 |
| Grabs / throws | IMPLEMENTED for all 12 fighters in the two-fighter host match model | Catch search on posed hand joints, Samus Grapple Beam, Link Hookshot, Yoshi tongue, Falcon Dive, Kirby's Inhale and copied Egg Lay; Kirby's suplex and star spit/copy; linked capture/throw statuses, Egg Lay swallow and breakout, shield-grab damage, Donkey cargo walk/jump/turn/throw, heavy-item joint matrix and held TopN pose scaled by `size`; held-fighter damage and release; back-throw hits on bystanders | Same-frame catcher/held hits; Training has no third fighter; Falcon Dive victims use root-sphere hurtboxes | RE-330–335, RE-337–339, RE-343–347 |
| Weapons | 40% | Fixed pool: Mario and Luigi Fireball, Fox Blaster, Samus Charge Shot and Bomb, Link Boomerang with owner catch, Yoshi thrown egg and Bomb stars, Kirby's Final Cutter wave and copied Fireball, Blaster, Charge Shot and Boomerang; Pikachu and copied Thunder Jolt crawler; Thunder head/trails with shared hit records; Ness and copied PK Fire spark plus flames in the shared item pool, controlled/reflected PK Thunder head/trails, self-launch, bat reflection and PSI Magnet absorption; Link's Spin Attack weapon as fighter state; reflection; staled damage and owner stale queue; weapon–item contacts with four shared victim records and deferred item-hit callbacks; blast-zone removal, Boomerang off-camera removal through the battle camera, and every `can_hop` weapon's shield hop and `proc_shield` (RE-354) | Host-only weapon rendering | RE-300, RE-303, RE-333–335, RE-337, RE-342–345, RE-347, RE-351–354 |
| Items | IMPLEMENTED for Link Bomb / PK Fire | Shared 16-slot pool with creation order and free-list reuse; item lifecycle, map/bounds and rehit records; Bomb fuse/bloat/recoil/explosion including self-hits; PK Fire incoming damage and lifetime; light pickup and script-driven ground/air throws, guard/escape windows, damage drops and death destruction; moving-floor carry in hitlag before bounds; PSP frame integration | Other item kinds, heavy/swing/shoot/consume behavior, team checks and item visuals/audio | RE-352–353 |
| Stages | 75% | Headers, collision, render layers for all 41; Training shares stage poses with collision groups; host controllers for eight VS stages (Whispy, acid, barrel, Twister, clouds, scales and POW spawner, gate, Bumper carrier) with the obstacle/hazard registries, Twister and TaruCann statuses, checked against ROM clips | Controller objects not packed or drawn; no match stage loader; Sector Z Arwing, bonus stages and stage items | RE-028, RE-029, RE-170, RE-353, RE-356 |
| CPU AI | 0% | — | — | — |
| Menus | 35% | `psp-game` Intro → Menu → Training with Mario vs dummy on Dream Land; C-button costume picks; dummy takes the first free costume | Text, character/stage select | RE-289–296, RE-341 |

### Fighters

| Fighter | Moveset | Notes |
|---|---|---|
| Mario | Normals and specials | No `Attack100` by design; attack timing, landing lag and Z-cancel from the motion scripts |
| Fox | Normals and specials | Fire Fox first-contact floor/wall/ceiling redirection, pass timer and cliff catch implemented (RE-348) |
| Donkey Kong | Normals, specials, grabs and cargo throws | Every source attack box from the motion scripts; Giant Punch charge damage (RE-350) |
| Samus | Normals, specials, grabs and throws | Host-only (not selectable in `psp-game`); weapons not drawn; charge loop rolls out; scripted intangibility |
| Luigi | Normals, specials, grabs and throws | Host-only (not selectable in `psp-game`); shares Mario's special statuses; scripted intangibility |
| Link | Normals, rapid jab, specials, grabs and throws | Host-only (not selectable in `psp-game`); Bomb pull/hold/throw/fuse/explosion and down-B item branch ported (RE-352); Bomb, Boomerang and Spin Attack weapon not drawn |
| Yoshi | Normals, Egg Lay/Throw/Bomb, grabs and throws | Host-only (not selectable in `psp-game`); egg and stars not drawn; authored egg hurtbox implemented (RE-349) |
| Captain Falcon | Normals, rapid jab, Falcon Punch/Kick/Dive, grabs and throws | Host-only (not selectable in `psp-game`); Kick wall rebound and Dive cliff catch implemented (RE-348) |
| Kirby | Normals, rapid jab, five aerial jumps, Final Cutter, Stone, Inhale, grabs and throws; copy abilities of Mario, Luigi, Fox, Samus, Donkey Kong, Link, Captain Falcon, Yoshi, Pikachu, Jigglypuff and Ness (RE-344–347) | Host-only (not selectable in `psp-game`); 1-in-12 copy loss on a tumble (RE-350); wave and copy hats not drawn |
| Pikachu | Normals, grabs/throws, Thunder Jolt, Thunder and Quick Attack (RE-345) | Host-only; weapons/effects not drawn; Quick Attack wall/ceiling cancellation and cliff catch implemented (RE-348); scripted intangibility |
| Jigglypuff | Normals, five aerial jumps, Pound, Sing (sleep), Rest, grabs and throws (RE-346) | Host-only; rapid jab unreachable in the source; Sing notes not drawn; Pound/Sing ledge stops implemented (RE-348); scripted hit status |
| Ness | Normals, jab 3, repeat down tilt, double jump, grabs/throws, bat reflector, PK Fire, PK Thunder/self-launch and PSI Magnet (RE-347) | Host-only; weapons/effects not drawn; map callbacks implemented (RE-348), shared DownBounce (RE-351); PK Fire flame item damage/lifetime ported (RE-352) |

## Known caveats

1. PPSSPP is not hardware proof. RE-320 confirms v32's short-row texture
   repair on PSP-2000; the full golden matrix remains software-only.
2. The debug HUD (`sceGuDebugFlush`) shows only under PPSSPP's software
   rasterizer (RE-014) and faults on real hardware (RE-202). It is off by
   default (`debug_overlay` feature).
3. Only the leading 45 `FTAttributes` scalars are decoded; hurtboxes, sound
   IDs and joint indices are not.
4. Extern relocations are zeroed in the pack; the converter follows them
   (RE-037). `ssb_rom::reloc_link` lays out and patches a closure as the
   source does (RE-340, checked in RE-341), but no runtime path uses it.
