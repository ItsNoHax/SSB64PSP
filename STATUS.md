# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Mario's ground+aerial moveset is
complete through the real jab finisher. The shared `FallSpecial` recovery
state machine (every fighter's up-special lands in this after its launch
phase) is ported; Mario's `SpecialHi` now has its sourced status slots and
ROM-verified TransN clip, but not its gameplay callbacks — see "Immediate
next batch".

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

1. **Port Mario's `SpecialHi` end to end.** Add a portable root-motion sample
   interface from the runtime's hidden TransN skeleton pose to `ssb-game`,
   then translate the sourced move callbacks and motion-event timing. The
   40-frame ROM clip is already packed and verified (RE-299); do not replace
   it with a guessed velocity arc. This is now gameplay/runtime integration,
   not an asset-extraction batch.
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

None outright. `SpecialHi` needs its remaining portable-gameplay/runtime
bridge and motion-event implementation, not further ROM or asset access.
Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
