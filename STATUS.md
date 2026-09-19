# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Just-finished batch below. Combat
systems' self-contained subsystems (shield, KO/death/respawn, ledges) are
now done; grabs/throws stays deferred (see below). Next batch starts `P2`'s
fighter-common gameplay/attack-table audit ahead of the 12-fighter bulk port.

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library; both
  `psp-game` and `psp-asset-viewer` depend on it. Verified with workspace
  tests, both EBOOTs, PPSSPP regression captures, and a physical-hardware
  smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family**: `crate::status::Status`
  carries the complete `FTCommonStatus` ordinal table (0..=219). A landed hit
  moves the defender into a real Damage-family status chosen by
  `ftCommonDamageGetDamageLevel`'s hitstun tiers and ground/air situation.
- **Shield/guard**: `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` — shield
  health decay, release-lag recovery, break into `ShieldBreakFly`, and a hit
  landing on a shield redirects into pushback instead of damage.
- **KO/death/respawn**: blast-zone crossing into `Dead*`, immediate stock
  decrement, `Rebirth*` sequence with real total timing (390 frames) and a
  genuine 120-frame post-respawn invincibility window
  (`Fighter::invincible_frames`, now respected everywhere a hit resolves).
- **Ledges** (this batch): `crate::status::CliffState` plus the full
  `CliffCatch`→`CliffWait`→(`Climb`/`Attack`/`Escape`)`Quick1/2`|`Slow1/2`→`Wait`
  state machine (`ft/ftcommon/ftcommoncliffcatchwait.c`, `ftcommoncliffclimb.c`,
  `ftcommoncliffattack.c`, `ftcommoncliffescape.c`). Catch detection
  (`cliff_catch_candidate`) reuses the existing `crate::collision::check_floor`
  swept-segment primitive — the same one floor-landing already uses — plus
  the real `800.0`-unit corner-proximity tolerance and `MAP_VERTEX_COLL_CLIFF`
  flag test from `mpProcessCheckTestLCliffCollision`/`RCliffCollision`.
  `CliffWait`'s three exits are all real: attack/escape button taps, the
  climb-or-drop stick-angle test (`ftCommonCliffClimbOrFallCheckInterruptCommon`,
  reframed as a `tan(50°)` slope comparison since `ssb_engine::math` has no
  `atan2`), and the damage-dependent auto-release timeout into `DamageFall`
  with the real 30-frame `cliffcatch_wait` re-grab cooldown. Because catch
  detection needs external floor data and stage-respawn-point-style
  information a single `Fighter` can't hold, it follows the same
  caller-invoked contract as `apply_hit_from`/`try_rebirth`: the caller
  snapshots the fighter's position before/after a tick and passes both in,
  plus resolves ledge-hog exclusivity itself (this codebase doesn't have a
  multi-fighter match loop yet to test that against). Documented gaps: no
  per-character `cliffcatch_coll` hand-reach offset (not in the extracted
  attribute range — same gap class as `Attack11`'s hitbox offset), so a
  fighter hangs exactly at the corner rather than at arm's length; no
  extracted animation lengths, so each climb/attack/escape phase collapses
  to one tick; no per-character `cliff_status_ga` table, so every recovery
  option ends grounded.
- Verified for all five batches above together: 163 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 661 tests) green, `cargo psp --release`
  builds clean for both `psp-game` and `psp-asset-viewer`, and a PPSSPP
  headless Training boot (built with `regression_capture,headless_capture` —
  a plain release build never reaches the deterministic screenshot hook and
  reads as a hang, not a regression) shows no panic/crash with a real
  rendered frame.
- Earlier: `Dummy::apply_hit_from` hit-resolution logic moved from
  `psp-game` into `ssb_game::attack::apply_hit_from`
  (crate-ownership rule); `psp-asset-viewer/src/play.rs`'s `status_name`
  fixed for the grown `Status` table (was an exhaustive match, broke once
  `Status` passed 21 variants — caught by building both EBOOTs).
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Grabs/throws (deferred, not skipped): a real throw's damage/knockback is
baked into each character's own motion script
(`attr->thrown_status[victim_kind]`, the same place `Attack11`'s jab numbers
came from for Mario), and the grabbed-fighter hold position is
joint-attachment matrix math this codebase has no gameplay-facing
equivalent for. Porting the grab *mechanism* without real throw numbers
would mean inventing damage/knockback data. Revisit when doing per-fighter
work (`P2`'s bulk port), where a fighter's throw data is extracted alongside
its other moves — that is also naturally where `P2` starts next: audit each
of the 12 fighters' status tables, attacks/specials, and motion-script
hitbox data (`Attack11`'s `MARIO_JAB1_HITBOX` is the template for how a
single hitbox gets ported; the bulk port scales that pattern out to full
movesets) before this codebase's `Status`/`Hitbox`/`GuardState`/`CliffState`
machinery gets its first full test against more than one attack.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
