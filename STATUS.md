# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Mario's ground+aerial moveset is
complete through the real jab finisher. The shared `FallSpecial` recovery
state machine (every fighter's up-special lands in this after its launch
phase) is now ported and real-tested, but Mario's own `SpecialHi` is not
wired on top yet — see "Immediate next batch" for why.

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
- **Mario's `SpecialHi` (up-B) is deliberately not wired on top of
  `FallSpecial` this batch.** Investigation found why it looked
  straightforward but isn't: the actual rise/launch velocity for Mario's
  up-B comes from the *animation clip's own root-motion data*
  (`ftPhysicsApplyAirVelTransNAll`/`...GroundVelTransN`, which read
  per-frame translation baked into the animation file), not from a
  physics formula. This codebase's animation pipeline does not extract
  that data yet (`docs/porting-status.md`'s Animation row already lists
  "No `translate_scales`" as a known gap). Inventing a velocity arc instead
  of extracting the real one would be exactly the kind of unsupported
  heuristic `AGENTS.md` rules out, so `SpecialHi` itself waits on that
  animation-extraction prerequisite rather than shipping a guessed number.
- Verified for everything above together: 204 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 702 tests) green, `cargo psp
  --release` builds clean for both `psp-game` and `psp-asset-viewer`, and a
  PPSSPP headless Training boot shows no panic/crash with a real rendered
  frame — same pixel count as before, confirming the fastfall fix doesn't
  disturb the existing pixel-confirmed jab-connection capture.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Two real options, both blocked-open rather than blocked-shut:

1. **Extract `translate_scales`/root-motion animation data** (asset-pipeline
   work, not gameplay code) so Mario's `SpecialHi` — and likely other
   fighters' recovery/root-motion-driven moves — can be ported for real
   instead of guessed. Check `docs/porting-status.md`'s Animation row and
   the `asset-pipeline` skill before starting; this is a different kind of
   work than the last several gameplay batches.
2. **Mario's down-B (`SpecialLw`, Tornado)** and/or fireball (`SpecialN`)
   instead, since neither is blocked on root-motion data the way `SpecialHi`
   is — but both were flagged in the previous batch's scoping as needing
   their own new infrastructure first: `SpecialLw` needs new ground/air
   velocity-clamp mechanics (a mash-to-rise input, its own friction curve)
   not yet in `physics.rs`; `SpecialN` needs a projectile/spawned-object
   concept `ssb-game` does not have at all yet (`Items` is 0% in
   `docs/porting-status.md`). Worth scoping each on its own before picking.

Grabs/throws stays deferred: a real throw's damage/knockback is baked into
each character's own motion script, and the grabbed-fighter hold position
is joint-attachment matrix math this codebase has no gameplay-facing
equivalent for. Revisit alongside a fighter's other moves.

## Real blockers

None outright, but `SpecialHi` specifically is blocked on animation-data
extraction (see above) rather than gameplay-code work. Rendering
performance (`P5`) is not a blocker for this or any gameplay batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
