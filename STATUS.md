# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: **Mario Fireball presentation/map integration**
is complete. Mario's ground+aerial moveset is complete through Fireball,
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
presentation/map integration) is complete. Next: continue the `P1`-`P4`
gameplay source-port batch sequence — auditing Super Jump Punch's and
Tornado's integration against a real Training dummy (hit confirmation,
damage/knockback, the same live-target verification RE-294 did for Mario's
jab) is the natural next step, or move to the next fighter if the user
directs otherwise. User directive needed to pick the next fighter/system.

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
