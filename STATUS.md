# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Just-finished batch below; next batch is
combat systems (grabs/throws).

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library; both
  `psp-game` and `psp-asset-viewer` depend on it. Verified with workspace
  tests, both EBOOTs, PPSSPP regression captures, and a physical-hardware
  smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family**: `crate::status::Status`
  now carries the complete `FTCommonStatus` ordinal table (0..=219,
  `ft/ftcommon/ftcommonstatus.h`) instead of just the 21 movement/jab
  variants, so later batches attach behaviour without renumbering. A landed
  hit now moves the defender into a real Damage-family status
  (`DamageHi/N/Lw1-3`, `DamageAir1-3`, `DamageFlyN`/`FlyTop`) chosen by
  `ftCommonDamageGetDamageLevel`'s hitstun tiers and the defender's ground/air
  situation, instead of only pushing numeric fields; hitstun running out
  returns to `Wait` or `DamageFall`.
- **Shield/guard** (this batch): `crate::status::GuardState` plus the
  `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` state machine
  (`ft/ftcommon/ftcommonguard{1,2}.c`) — shield health decay while held
  (`FTCOMMON_GUARD_DECAY_INT` = 16-frame ticks), release-lag-gated recovery
  into `GuardOff`, and shield break into `ShieldBreakFly` (health resets to
  30). Entry is wired into both `ground_interrupt` and `walk_interrupt` at
  the same point the decomp macros put it (right after `Attack1`, ahead of
  `KneeBend`/`Dash`/etc). A hit landing on a shielding fighter is now
  redirected (`crate::attack::is_shielding`/`apply_shield_hit`) into
  `GuardSetOff`'s pushback instead of the normal Damage path — blocking
  takes no damage or hitstun, only shield-health loss, matching
  `ftMainUpdateShieldStatFighter`'s single-hit case. Documented
  simplifications: no extracted animation lengths for `GuardOn`/`GuardOff`
  (they resolve in ~1 tick / on a release-lag counter instead of a real
  clip), no shield-bubble visual scaling (rendering, out of scope), no
  `shield_damage_total` multi-hit-per-frame accumulation (this codebase
  still resolves one hitbox at a time), `ShieldBreakFly`'s
  fly→fall→down/stand→`FuraFura` chain is ordinal-only with no behaviour
  yet, and dash-into-shield/Yoshi's hurtbox swap are not ported. Verified:
  144 `ssb-game` tests (5 new), full workspace (`cargo test --workspace`,
  642 tests) green, `cargo psp --release` builds clean for `psp-game`, and
  a PPSSPP headless Training boot (built with the proper
  `regression_capture,headless_capture` features — a plain release build
  never reaches the deterministic screenshot hook and reads as a hang, not
  a regression) shows no panic/crash with a real rendered frame.
- Earlier cleanup batch: `Dummy::apply_hit_from` hit-resolution logic moved
  from `psp-game` into `ssb_game::attack::apply_hit_from`, matching the
  crate-ownership rule in `AGENTS.md`.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Combat systems, continuing in decomp dependency order: grabs/throws
(`Catch`/`CatchPull`/`CatchWait`/`ThrowF`/`ThrowB`, `ft/ftcommon/ftcommoncatch*.c`,
`ftcommonthrow*.c`) — standing grab, break-free, forward/back throw. After
that: ledges (`Cliff*`), then KO/death/respawn (`Dead*`/`Rebirth*`). Each is
a self-contained decomp subsystem per the user's batch-mode directive — do
not wait for a full `P1` audit before starting them.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
