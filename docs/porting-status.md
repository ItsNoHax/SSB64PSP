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
| relocData archive | COMPLETE | 2,132 files; 61,343 intern + 3,092 extern relocations, 0 mismatches | Runtime extern-relocation loader | RE-001 |
| F3DEX2 parser / DL discovery | COMPLETE | Every emitted opcode; 1,864 lists in 135 files, 0 failures | — | RE-017 |
| Texture decode | 85% | RGBA16/32, IA4/8/16, I4/8, CI4/8 | 26 runtime-framebuffer references have no ROM texels | RE-055 |
| Texture → PSP | COMPLETE for measured scope | Mirror/clamp/origin lowering, palette banks, TLUT mode, sample-centre alignment, build-time 3-point filter compensation | Fixed-function bilinear cannot equal N64 3-point exactly | RE-219–239, RE-304–313 |
| Mesh / model conversion | VERIFYING | 0 conversion failures; all 127 material graphs paired; PRIM ownership, lighting provenance, independent depth state | Physical-PSP confirmation | RE-163, RE-240–261 |
| Scene graph (DObj) | 87% | 363 `DObjDesc` arrays + 11 effects packed as 374 objects | `GObj` layer | RE-172 |
| Asset pack | COMPLETE | v36, zero-copy, 16-byte aligned; Donkey attack/special/cargo slots and Mario/Fox/Donkey grab/throw clips; two-tile blend and animated colour registers; recorded render-tile layout and `unk10 == 1` inputs | — | RE-301, RE-312, RE-319, RE-321, RE-322, RE-327, RE-330 |

## Rendering

Detail per domain: [`rendering.md`](rendering.md).

| Subsystem | Status | Capability | Gap | Evidence |
|---|---|---|---|---|
| PSP GE backend | VERIFYING (89%) | Indexed textured draws, CLUTs, addressing, alpha test/blend, depth, culling, lighting, texgen, material animation incl. `SetLFrac` two-tile blend and `PrimColor`/light tracks, effect-owned material clocks, camera head-1 XLU seed, GE state cache | Physical-PSP confirmation | RE-251–264, RE-301, RE-321, RE-322, RE-327, RE-328 |
| Camera / projection | COMPLETE | Default battle camera; matches original ROM camera state within 0.1 units | Special camera modes | RE-151 |
| Coordinate conversion | 80% | Matrices, UVs incl. signed S10.5 on clamped axes, pillarboxed viewport | On-hardware confirmation | RE-004, RE-005, RE-262 |
| Stage animation | 90% | 35 stages, 206 animated nodes; packed poses match the archive | — | RE-050–052, RE-142 |
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
| Input | 80% | `psp-game` PSP→N64 layout (see README); viewer keeps its own; shared PSP polling is nonblocking | Nub deadzone unmeasured | RE-008, RE-009, RE-295, RE-329 |
| Engine traits | 70% | Renderer, audio, input, timing, clock | — | — |
| Math | 80% | Scalar math | VFPU after profiling | [D-032](decisions/D-032.md) |
| Audio | 0% | — | Mixer thread, VADPCM, sequencer | — |
| Save data | 0% | — | — | — |
| Debug / profiler | 20% | Frame timing sections, text overlay | — | — |
| CI | COMPLETE | fmt, clippy, host tests, no_std, PSP EBOOT builds; no ROM needed | — | — |

## Gameplay

| Subsystem | Status | Capability | Gap | Evidence |
|---|---|---|---|---|
| Physics | 62% | Ground/air/knockback velocities, gravity, friction, fastfall; per-fighter constants for all 27 kinds verified against the decomp | — | RE-032 |
| Collision | 65% | All 41 stages packed; swept floor queries; weapon diamond collider vs floors, ceilings, walls | Fighter wall/ceiling solver; moving groups tested at rest | RE-030, RE-031 |
| Animation | 95% | Figatree playback at 60 Hz; posed joint transforms and TransN root motion feed gameplay; grab/thrown/cargo clips for Mario, Fox and Donkey Kong; held TopN uses the catcher's joint rotation and its own child offset | Reflector effect phases | RE-036, RE-038, RE-171, RE-299, RE-330–332 |
| Status machine | 68% | Full `FTCommonStatus` table (0–219); movement, Damage/hitstun, per-character `AnyStatus` | Most statuses beyond those listed are ordinals only | RE-033, RE-035, RE-294 |
| Hit resolution | IMPLEMENTED | Per-`(fighter, status)` `MoveData` hitboxes on posed joints, `ClearAttackCollAll` hit generations, damage, knockback, hitstun; Donkey normal/special windows; throw descriptors and release knockback | Some same-valued source joint boxes still condensed; root-sphere hurtbox, multi-hit shield accumulation, hit-location Hi/Lw, `DamageFlyRoll` (RNG) | RE-294, RE-299, RE-330, RE-332 |
| Shield / guard | 40% | `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff`, decay, shield break | Clip lengths, bubble visual, break mash-out chain | — |
| Ledges | 45% | `CliffCatch` → `CliffWait` → climb/attack/escape, re-grab cooldown | Hand-reach offset, ledge-hog, clip lengths | — |
| KO / respawn | 45% | Blast zones, stock loss, rebirth sequence, 120-frame invincibility | `DeadUpFall` (RNG), halo visuals, team/1P branches | — |
| Recovery (`FallSpecial`) | 25% | Shared helpless fall and landing; driven by Mario up-B | Drop-through, ledge auto-catch | RE-299 |
| Grabs / throws | IMPLEMENTED for Mario, Fox and Donkey Kong in two-fighter Training | Catch search on posed hand joints, linked capture/throw statuses, breakout, shield-grab damage, Donkey cargo walk/jump/turn/throw, heavy-item joint matrix and held TopN pose | Bystander throw hits, held-fighter damage, non-unit held scale | RE-330–332 |
| Weapons | 18% | Fixed pool: Mario Fireball, Fox Blaster; reflection | General item system | RE-300, RE-303 |
| Stages | 65% | Headers, collision, render layers for all 41 | No match stage loader | RE-028, RE-029, RE-170 |
| CPU AI | 0% | — | — | — |
| Menus | 35% | `psp-game` Intro → Menu → Training with Mario vs dummy on Dream Land | Text, character/stage select | RE-289–296 |

### Fighters

| Fighter | Moveset | Notes |
|---|---|---|
| Mario | Normals and specials | No `Attack100` by design; `DTilt` repeat, `LandingAir*` clips, aerial auto-cancel missing |
| Fox | Normals and specials | Fire Fox wall/ceiling response waits on the collision solver |
| Donkey Kong | Normals, specials, grabs and cargo throws | Represented hitboxes use posed joints; root-sphere hurtbox and condensed source boxes remain |
| Other 9 | Not started | Movement and models work for all |

## Known caveats

1. PPSSPP is not hardware proof. RE-320 confirms v32's short-row texture
   repair on PSP-2000; the full golden matrix remains software-only.
2. The debug HUD (`sceGuDebugFlush`) shows only under PPSSPP's software
   rasterizer (RE-014) and faults on real hardware (RE-202). It is off by
   default (`debug_overlay` feature).
3. Only the leading 45 `FTAttributes` scalars are decoded; hurtboxes, sound
   IDs and joint indices are not.
4. Extern relocations are zeroed in the pack; the converter follows them
   (RE-037) but no runtime loader patches them.
