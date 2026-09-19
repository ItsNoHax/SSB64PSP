# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Just-finished batch below; next batch is
combat systems (shield/guard).

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library; both
  `psp-game` and `psp-asset-viewer` depend on it. Verified with workspace
  tests, both EBOOTs, PPSSPP regression captures, and a physical-hardware
  smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family** (this batch):
  `crate::status::Status` now carries the complete `FTCommonStatus` ordinal
  table (0..=219, `ft/ftcommon/ftcommonstatus.h`) instead of just the 21
  movement/jab variants — every common status (Damage, Guard/shield, Escape,
  ShieldBreak, FuraFura/Sleep, Catch/Throw/Capture/Thrown, cliff/ledge,
  item pickup/throw/weapon-swing, hammer, base moveset tilts/smashes/aerials)
  now has its real decomp ordinal, even though most have no callback
  behaviour yet — that is what lets later batches attach behaviour without
  renumbering. Behaviourally, the Damage/hitstun family is now wired: a
  landed hit picks `DamageHi/N/Lw1-3`, `DamageAir1-3`, `DamageFlyN`/`FlyTop`
  by `ftCommonDamageGetDamageLevel`'s hitstun tiers and the defender's
  ground/air situation (`crate::attack::damage_level`/`damage_status`), the
  defender's status actually changes on a hit (previously only the numeric
  damage/knockback/hitstun fields moved), and hitstun running out returns to
  `Wait` (grounded) or `DamageFall` (airborne) — `crate::status::update`'s
  new Damage-family arms. `crate::attack::apply_hit_from` (Mario's jab, the
  `F1` Training slice) now drives this instead of a bare knockback push.
  Documented simplifications: no hit-location Hi/Lw index (every hit is "N"),
  no `DamageFlyRoll` (needs an RNG source that does not exist yet), and the
  ground-hit-still-launches-airborne angle branch is dropped in favour of a
  static per-status grounded/airborne classification (`Status::is_grounded`'s
  doc comment). Verified: 139 `ssb-game` tests (12 new), full workspace
  (`cargo test --workspace`, 637 tests) green, `cargo psp --release` builds
  clean for `psp-game`, and a PPSSPP headless Training boot shows no
  panic/crash in the log with a real (non-blank) rendered frame.
- Earlier cleanup batch: `Dummy::apply_hit_from` hit-resolution logic moved
  from `psp-game` into `ssb_game::attack::apply_hit_from`, matching the
  crate-ownership rule in `AGENTS.md`.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Combat systems, continuing in decomp dependency order: shield/guard
(`GuardOn`/`Guard`/`GuardOff`/`GuardSetOff`, `ft/ftcommon/ftcommonguard1.c`,
`ftcommonguard2.c`) — shield HP, perfect-shield window, shield break
(`ShieldBreakFly`/`Fall`/`Down*`/`Stand*`, already ordinal-only in `Status`).
After that: grabs/throws (`Catch`/`CatchPull`/`CatchWait`/`ThrowF`/`ThrowB`),
ledges (`Cliff*`), then KO/death/respawn (`Dead*`/`Rebirth*`). Each is a
self-contained decomp subsystem per the user's batch-mode directive — do not
wait for a full `P1` audit before starting them.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
