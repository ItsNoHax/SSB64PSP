# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Just-finished batch below; next batch is
ledges (`Cliff*`) — grabs/throws was queued next but is deferred (see below).

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library; both
  `psp-game` and `psp-asset-viewer` depend on it. Verified with workspace
  tests, both EBOOTs, PPSSPP regression captures, and a physical-hardware
  smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family**: `crate::status::Status`
  now carries the complete `FTCommonStatus` ordinal table (0..=219,
  `ft/ftcommon/ftcommonstatus.h`). A landed hit moves the defender into a real
  Damage-family status (`DamageHi/N/Lw1-3`, `DamageAir1-3`, `DamageFlyN`/`FlyTop`)
  chosen by `ftCommonDamageGetDamageLevel`'s hitstun tiers and the defender's
  ground/air situation; hitstun running out returns to `Wait` or `DamageFall`.
- **Shield/guard**: `crate::status::GuardState` plus the
  `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` state machine — shield health
  decay while held, release-lag-gated recovery, shield break into
  `ShieldBreakFly`. A hit landing on a shielding fighter redirects into
  `GuardSetOff`'s pushback instead of the Damage path (no damage/hitstun,
  only shield-health loss).
- **KO/death/respawn** (this batch): blast-zone crossing
  (`crate::status::check_dead`, `ftCommonDeadCheckInterruptCommon`) enters
  `DeadDown`/`DeadLeftRight`/`DeadUpStar` in the original's bottom→right→
  left→top priority and decrements a stock immediately, matching
  `ftCommonDeadUpdateScore`'s timing. `BlastZone` is a plain Layer-A struct
  the caller fills from `ssb_rom::pack::StageDesc::bounds` — Layer A still
  doesn't depend on the pack format. After the real
  `FTCOMMON_DEAD_WAIT`/`DEADUP_WAIT`-derived wait, a new caller-invoked
  `try_rebirth` (it needs the stage's respawn point, which the plain
  per-frame `update` has no way to receive — same reason `apply_hit_from` is
  caller-invoked rather than automatic) enters `Sleep` if stocks are
  exhausted, or `RebirthDown`→`RebirthStand`→`RebirthWait` otherwise. The
  real total respawn duration (390 frames) is preserved even though the
  halo-drop flight animation itself is not rendered — ends in `Fall` with a
  genuine 120-frame invincibility window. Added `Fighter::invincible_frames`,
  now respected by both `apply_hit_from` and `apply_shield_hit` (an
  invincible fighter's hitbox test is skipped outright, matching
  `nGMHitStatusInvincible`). Damage resets to 0 on respawn
  (`dFTManagerDefaultFighterDesc`'s reset). Documented gaps: no 1-in-6
  `DeadUpFall` branch (needs an RNG source — same class of gap as
  `DamageFlyRoll`), no halo visuals/camera-mode switches/1P-team-respawn
  branches (rendering/scene-manager scope).
- Fixed a real regression this batch's status-table growth caused:
  `psp-asset-viewer/src/play.rs`'s `status_name` was an exhaustive match over
  `Status` and failed to build (E0004, 199 variants uncovered) once `Status`
  grew past its original 21 variants — added the new wired statuses' labels
  plus a wildcard fallback for the rest. Caught by building **both** PSP
  EBOOTs at the batch boundary, not just `psp-game`'s — `cargo test
  --workspace` cannot see this because `psp-asset-viewer` is a
  `mipsel-sony-psp`-only target crate.
- Verified for all three batches above together: 152 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 650 tests) green, `cargo psp --release`
  builds clean for both `psp-game` and `psp-asset-viewer`, and a PPSSPP
  headless Training boot (built with `regression_capture,headless_capture` —
  a plain release build never reaches the deterministic screenshot hook and
  reads as a hang, not a regression) shows no panic/crash with a real
  rendered frame.
- Earlier cleanup batch: `Dummy::apply_hit_from` hit-resolution logic moved
  from `psp-game` into `ssb_game::attack::apply_hit_from`, matching the
  crate-ownership rule in `AGENTS.md`.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Ledges (`Cliff*` — `CliffCatch`/`CliffWait`/`Quick`/`Slow`/`Climb*`/`Attack*`/
`Escape*`, already ordinal-only in `Status`): edge-grab detection, ledge
hang, climb/attack/roll-up options, ledge-hog.

Grabs/throws (`Catch`/`CatchPull`/`CatchWait`/`ThrowF`/`ThrowB`) was queued
here previously but is deferred: unlike Damage/Guard/Dead-Rebirth, a real
throw's actual damage/knockback is baked into each character's own motion
script (`ftCommonThrowSetStatus` reads `attr->thrown_status[victim_kind]`,
the same place `Attack11`'s jab numbers came from for Mario), and the
grabbed-fighter hold position is joint-attachment matrix math this codebase
has no gameplay-facing equivalent for. Porting the grab *mechanism* without
real throw numbers would mean inventing damage/knockback data, which
`AGENTS.md` and this codebase's existing precedent (Attack11's documented
gaps) both rule out. Revisit grabs when doing per-fighter work (`P2`'s bulk
port), where a fighter's throw data is extracted alongside its other moves.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
