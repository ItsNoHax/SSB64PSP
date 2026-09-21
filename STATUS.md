# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: **build-time N64 3-point texture compensation
(complete in software/PPSSPP, RE-305; physical spot check unavailable)**.
The real-UV/archive pipeline emits 406 immutable variants, keeps 274 in CI4,
promotes 132 to RGBA8888 under a measured cost gate, and reduces selected
variants' average error from 1.947/255 to 0.876/255. The pack grows 2.67%; all
selected variants have non-increasing max error, and animated indexed palette
semantics remain untouched. The full 13-fighter deterministic PPSSPP matrix
was rebaselined for the intentional filter change and passes at zero differing
pixels. The prerequisite sampling work (RE-304) used a
deterministic synthetic 2x2/4x4 GE rig measuring point and
filtered sampling at centres, halves, odd S10.5 steps, diagonals, and
clamp/repeat/pre-baked-mirror boundaries. PPSSPP software and real PSP agree
on all 96 interior readbacks: point needs no bias; filtered coordinates need
`+0.5` texel, then the GE uses four-bit truncated bilinear weights and
truncating channel arithmetic. The host `sample_bilinear()` and shared PSP
runtime lowering are corrected. Authored UVs, animated MObj UVs, ordinary
texgen and CPU linear texgen now receive the convention exactly once.
N64 3-point compensation uses that verified reference entirely at build
time; runtime remains ordinary `GU_LINEAR`.

Previous gameplay subsystem/batch: **Fox full moveset (complete)**. Fox's US normal
attack motion scripts (jab 1/2, dash, five forward tilts, up/down tilt,
forward/up/down smash, and five aerials) are transcribed into portable
`MoveData`, including replacement and rearmed multi-hit windows. Fox's
five-angle forward tilt and single-angle forward smash now select the
source's available motions. The decomp's `ftMainParseMotionEvent` confirmed
that `WaitAsync(n)` targets animation frame `n` (it does not add `n` frames;
RE-303), and the Fox transcription uses that rule. Targeted `ssb-game` tests pass.
Fox's rapid jab now has its own start/loop/end statuses, counts both A press
and release toward the source's four-edge threshold, and uses ROM-verified
8-frame start/end clips and five sourced loop hit pulses. Neutral B has
ROM-verified 55/45-frame ground/air clips, frame-25/15 shot events,
repeat-on-B gate, joint-17 plus 60 attachment sampling, and a match-owned
Blaster with sourced US damage, speed, and map/hit deletion. Fire Fox now has
start/hold/travel/end phases, sourced launch delays, angle selection, travel
deceleration, and common freefall exit. Reflector has start/loop/turn/hit/end
phases, release lag, ground/air transitions, a sourced two-frame startup
hitbox, the packed source effect hierarchy, and match-owned Fireball/Blaster
reflection with ownership and US damage scaling. Fire Fox redirects shallow
floor contacts and enters its bound state on steep impacts. All 42 Fox move
clips are packed and selected by status; Training renders the match-owned
Blaster's source display list with its authored X-scale growth. Workspace
tests, both release PSP EBOOT builds, exact ROM/pack animation replay, and a
PPSSPPHeadless Fox Training smoke pass. Fighter collision still has no
wall/ceiling solver, so Fire Fox's corresponding map callbacks remain a
shared collision limitation.

Previously, **fighter shadows** were completed for the shared
Training/runtime renderer (RE-302). The source floor-strip path, texture,
render state, lifecycle conditions, fixed-capacity multi-fighter submission,
pack rebuild and deterministic PPSSPP capture are all implemented. Mario's
ground+aerial moveset is complete through Fireball,
Super Jump Punch, and Tornado. The portable joint attachment, full map
rebound sweep, direct weapon mesh extraction, and Training input routing are
implemented. Final direct-display-list rendering verification found and
fixed two real bugs, a classification bug and a `psp-runtime` GE-state-cache
bug (RE-300, both resolved, confirmed on PPSSPP and real PSP hardware,
`tests/golden/f1-training-fireball.png` committed) — see "Immediate next
batch" below for what's next.
The shared `FallSpecial` recovery
state machine (every fighter's up-special lands in this after its launch
phase) is ported; Mario's `SpecialHi` samples its ROM-verified TransN clip
through a portable runtime-to-gameplay bridge.

## What was completed

- **rust-psp fork/toolchain migration** (2026-09-20): all PSP projects and
  CI now pin `ItsNoHax/rust-psp` commit `a89142b` plus
  `nightly-2026-08-26`. The fork is current with upstream master, includes
  upstream's `PanicPayload` compatibility repair and the committed project
  `MEMSIZE` support in `cargo-psp`; its unused Rust-source submodule is
  skipped during Cargo dependency resolution. Host checks and real release
  EBOOT builds pass for both `psp-game` and `psp-asset-viewer`.

- **Player-facing PSP controller layout** (2026-09-20): `psp-game` now
  selects its own raw PSP→N64 table at the shared `sceCtrl` backend boundary,
  without changing the asset viewer's legacy controls. The analog nub stays
  analog, D-pad directions are the four independent C-buttons, and all
  requested face/shoulder/Start bindings feed the normal controller state
  consumed by the game. The temporary front-end menu now reads that mapped
  N64 stick rather than the D-pad bits repurposed for C-button jumping.

- **Real-PSP face-button input repair** (2026-09-20): corrected the shared
  `sceCtrl` masks for Triangle, Circle, Cross, and Square in `ssb-engine`;
  they had each been shifted four bits too high, making `psp-game` miss the
  physical PSP face buttons. The existing application-specific mapping table
  is unchanged, and a literal-mask regression test now covers the four ABI
  values.

- **Fighter shadows** (2026-09-20): source-traced `ftShadowProcDisplay`, not
  a generic blob. The runtime projects each fighter onto its standing or
  nearest-below floor line, clips and contours the source's `shadow_size`
  strip over slopes/edges, extracts file 84's I4 texture, preserves the
  translucent alpha/depth/cull state and draw ordering, and uses no per-frame
  heap allocation. The new Training regression freezes one Mario airborne and
  one grounded on Dream Land; `tests/golden/f1-training-shadows.png` is
  deterministic. The current-pack PSPLink smoke had no exceptions and a live
  `main_thread`; its native capture shows the grounded+airborne scene. RE-302
  records the exact source path and scope boundary.

- **Material-animation blocker repair** (2026-09-20): stage
  `TextureIDCurrent` frame tables and tile-0 UV tracks now flow from MObj
  rest state through pack v30 to the shared PSP renderer without disturbing
  palette cycling. A `(file, script, MObjSub)` identity prevents UV state
  leaking between materials; the GE mapping cache keys the live affine state.
  The current ROM census is 61 resolvable attachments, not the stale 172
  historical claim (RE-301); `ScrU`/`ScrV` are verified tile-1 inert,
  `SetLFrac` is the RDP two-tile limitation, and sparse stage colour/light
  tracks have an explicit dynamic-lowering limitation. Host tests, archive
  replay, both PSP release builds, targeted deterministic PPSSPP captures,
  and the full 13-fighter golden matrix pass; R0.10 is complete.
- `P0`, the fighter-common status table, shield/guard, KO/death/respawn,
  ledges, Mario's full ground+aerial moveset (jab through `Attack13`,
  dash attack, tilts, smashes, aerials), and the `AnyStatus` per-character
  status extension: all landed in earlier batches this session (see git
  log for detail — this file only tracks the current and next batch).
- **Fastfall, a real pre-existing bug fix** (this batch):
  `ftPhysicsCheckSetFastFall` was never called from anywhere in this
  codebase. `Fighter::tick_air` read `physics.is_fastfall` correctly but
  nothing ever *set* it from a real downward-stick input, so fastfalling
  silently did nothing for every airborne fighter, in every batch so far.
  `crate::status::check_set_fast_fall` ports the real trigger (a one-shot
  per-airtime latch: `stick.y <= -53` within a 4-frame tap window while
  already falling) and is now called every airborne tick, same as the
  original calls it from every airborne status's own `proc_physics`.
- **`FallSpecial`/`LandingFallSpecial`** (this batch): the shared "helpless
  fall after a recovery move's launch phase" status
  (`ftCommonFallSpecialSetStatus`/`...LandingFallSpecialSetStatus`,
  `crate::status::set_fall_special`/`set_landing_fall_special`,
  `FallSpecialState`). Every physics primitive it needs
  (`apply_gravity_clamp_tvel`, `check_clamp_air_vel_x_dec`,
  `clamp_air_vel_x_stick_range`, `apply_air_friction`) already existed in
  `physics.rs` from the pre-batch-mode physics work — this batch was pure
  status-machine orchestration on top of physics that was ready and
  waiting. Spends every remaining jump on entry (the real "no double-jump
  after up-B" rule) and allows only a jump-cancel interrupt, never an
  aerial attack, matching `ftCommonFallSpecialProcInterrupt` exactly.
  Landing correctly distinguishes "skip landing lag entirely" from a real
  `LandingFallSpecial` pose by `is_goto_landing`/fall speed, same shape as
  the aerial-landing dispatch from an earlier batch.
- **Mario Super Jump Punch animation extraction** (this batch): corrected
  the preceding claim. Mario has no `translate_scales`; its up-B root motion
  is the hidden TransN entry in the 40-frame figatree (archive file 637).
  The legacy decompilation flag macros are named opposite to the runtime
  bitfield, which is why the motion descriptor's apparent `XRotN` label was
  misleading. The generated table now carries ground and aerial up-B slots,
  the pack retains the hidden runtime joint, and `MarioStatus` selects the
  real clip. See RE-299. This deliberately does **not** make the status
  reachable: the game has no portable input path for that sampled joint yet.
  `cargo test -p ssb-rom`, `cargo test -p ssb-game`, ROM animation
  verification, pack rebuild, and 40-frame Mario figatree replay all pass.
- **Mario Super Jump Punch gameplay/runtime bridge** (this batch): B+up now
  enters sourced ground or aerial `SpecialHi`; `psp-runtime` records the
  hidden TransN pose before each skeleton advance and supplies its exact delta
  and pitch to the portable `RootMotion` input on the next gameplay tick.
  `ftPhysicsApplyGroundVelTransN` and `ftPhysicsApplyAirVelTransNAll` are
  ported against that input, including the source script's frame-9 physics
  handoff and 0.95 air damping. The 40-frame figure tree ends in the shared
  `FallSpecial` with the source `.6` drift and `.28` landing lag. The real
  motion script's strong opening, eight coin-hit pulses, and finisher windows
  are in generic `MoveData`; cleared pulse windows reset the existing target
  hit latch. See RE-299.
- **Mario Tornado (`SpecialLw`/`SpecialAirLw`)** (this batch): down+B now
  enters the source's aerial Tornado entry from either situation (including
  its counterintuitive grounded `-7` vertical velocity), switches between
  the 87-frame ground and 83-frame air clips on map contact, and preserves
  the real B-tap rise, frame-43 rise expenditure, and diminishing horizontal
  clamp. Its distinct ground/air scripts carry the opening, thirteen
  one-frame multihit pulses, and finishing hitboxes. Added the reusable
  sourced ground stick-clamp primitive and the two animation-table slots;
  rebuilt the ignored local `assets/generated/ssb64.pak` from the supplied
  ROM, which verifies all 189 decompilation-derived animation lengths.
- **Mario Fireball (`SpecialN`/`SpecialAirN`)** (this batch): neutral-B now
  preserves the source's direction-reversal gate, 46-frame ground/air clips,
  frame-16 one-shot spawn event, map-state transitions, and end-to-Wait/Fall.
  The portable, fixed-capacity weapon pool owns each Fireball after that event:
  its sourced 140-frame lifetime, -5° 50-unit launch, gravity/terminal fall,
  floor rebound/minimum-speed expiry, 7-damage hitbox, self-hit exclusion, and
  shield/damage resolution are all live in Training Mode. The paused follow-up
  samples source joint 16 at spawn, packs file 297's direct display list,
  draws the weapon mesh, and sweeps its authored diamond map collider against
  floors, ceilings, and directional walls with the real rebound/minimum-speed
  expiry. Training previously intercepted N64 B before fighter processing;
  START now owns that navigation action instead.
- Latest verification: `cargo test -p ssb-rom -p romtool` (450 tests) and
  `cargo test -p romtool -p ssb-game` (245 tests) pass; the local pack was
  rebuilt and contains the file-297/0x1D8 Fireball mesh. A focused
  PPSSPPHeadless script now spawns and displays it. The initial capture showed
  an opaque sprite rectangle because Fireball inherits `wpDisplayDrawNormal`'s
  translucent/no-depth wrapper; `InitialMaterial::WEAPON_EXTERNAL` now models
  that source state, but the rebuilt-pack visual recheck and full end-of-batch
  gates have not yet been run. Existing PSP linker warnings remain non-fatal.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Mario Fireball presentation/map integration (complete)

This batch's final verification step found and fixed two distinct things
(RE-300):

1. **Fixed and landed:** `InitialMaterial` had no way to seed `alpha_blend`,
   so `WEAPON_EXTERNAL`'s `translucent: true` never actually enabled GE
   blending (`psp-runtime`'s gate requires both `TRANSLUCENT` and
   `ALPHA_BLEND`). Added `InitialMaterial::alpha_blend`, seeded
   `WEAPON_EXTERNAL` with `AlphaBlend::TexelOnly`, and added a regression
   test. Verified against the real ROM (not a stale dump) and against the
   rebuilt pack's own `PrimDesc.flags`. `cargo test -p ssb-rom -p ssb-game
   -p ssb-engine -p romtool` (426 tests), both release PSP EBOOT builds, and
   the full 23-scene deterministic regression matrix (0 differing pixels
   against every existing golden, including `r0-dream-land-default.png`)
   all pass — additive, no regression.
2. **Still open:** the `regression_capture_fireball` capture still shows the
   Fireball as an opaque black card, not a transparent flame, even though
   the ROM palette, the packed PSP CLUT, and the blend/alpha-test enable
   logic are all now individually confirmed correct. Two on-device
   experiments (forcing `Blend` on, forcing cutout `AlphaTest` on, for every
   primitive) both had a visible effect elsewhere in the same frame but
   neither made the Fireball's background transparent — narrowing this to
   the GE/PPSSPP CLUT-alpha path itself. See RE-300 for the full trace.
   Diagnosing further needs GE-level tracing or a physical-PSP comparison
   this session did not have; not attempted-and-guessed-around per this
   project's own rule against unsupported rendering heuristics.

**Update (2026-09-20):** the physical-PSP capture in option (a) below has now
been run, plus two follow-up hardware experiments. Real PSP hardware (PSP
Slim, 6.61, ARK/Infinity, PSPLink) reproduces the identical opaque-black-card
bug — confirmed **not** a PPSSPP-only limitation. The classic PSP
CLUT-DMA-alignment pitfall was checked and ruled out. Bypassing the CLUT
entirely (the Fireball's texture forced to direct `Psm8888`, RE-283's proven
pattern for other narrow-CI4 defects) changed nothing — same byte-identical
opaque card, with the packed RGBA texel data independently verified correct
(159/256 transparent background texels). This rules out CLUT/palette/narrow-
texture addressing as the cause entirely. Neither of this project's two
N64→PSP prior-art references (`refs/sf64-psp`, `refs/oot-PSP`) ever exercises
the GE's real CLUT path for game art either — both always pre-decode to
direct colour formats, a pattern worth adopting as policy separately from
this bug.

**Fixed (2026-09-20, follow-up session):** root cause was a `DrawState`
cache bug in `psp-runtime/src/meshdraw.rs`, unrelated to CLUT/texture format
entirely. `last_texture_blend: Option<u32>` used `None` for both "GE
texture-function state unknown" and "known `Modulate`", so the first
`Modulate`-function primitive of a frame (or after any `invalidate_all`)
skipped its `sceGuTexFunc(Modulate, Rgba)` call, leaving the GE's own
RGB-only default texture function active and silently dropping texture
alpha — exactly what a `TexelOnly` translucent sprite like the Fireball
depends on. Replaced with an explicit `enum TextureFuncState { Modulate,
Blend(u32) }` where the cache's outer `None` means only "unknown", never
"known Modulate". `regression_capture_fireball` (PPSSPPHeadless, software)
now shows the Fireball as a translucent flame, not an opaque card, and real
PSP hardware (PSP Slim, 6.61, ARK/Infinity, PSPLink) confirms the same fix.
`tests/golden/f1-training-fireball.png` is the visual-regression matrix's
first committed `psp-game` golden. RE-300 is closed. Full trace there.

## Immediate next batch

Mario's moveset (ground+aerial normals, specials, Fireball's full
presentation/map integration) is complete. The follow-up Training combat
audit is also complete: generic `MoveData` hit application reaches Super Jump
Punch and Tornado; source `ClearAttackCollAll` boundaries re-arm a fixed-size
per-target hit record even when adjacent windows never leave an idle frame,
while a sourspot replacement without a clear remains one hit. Focused host
tests cover both cases, and deterministic PPSSPP B+up input exercises the
rendered Training path. Next: port Donkey Kong under `P2` as the next complete
fighter batch.

Grabs/throws stays deferred: a real throw's damage/knockback is baked into
each character's own motion script, and the grabbed-fighter hold position
is joint-attachment matrix math this codebase has no gameplay-facing
equivalent for. Revisit alongside a fighter's other moves.

## Real blockers

No blocker on gameplay work. RE-300's Fireball opaque-card rendering bug is
fixed (`DrawState` texture-function cache in `psp-runtime/src/meshdraw.rs`)
and closed, confirmed on PPSSPP and real PSP hardware. Rendering performance
(`P5`) is separately not a blocker for any gameplay batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
