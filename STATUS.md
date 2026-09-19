# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Combat systems' self-contained
subsystems (shield, KO/death/respawn, ledges) are done; grabs/throws stays
deferred. `P2`'s fighter bulk port has started, Mario-first: base moveset
(jab, dash attack, tilts) is in, generalized to a real per-fighter hit-data
table rather than staying Attack11-only.

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library. Verified
  with workspace tests, both EBOOTs, PPSSPP regression captures, and a
  physical-hardware smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family**: `crate::status::Status`
  carries the complete `FTCommonStatus` ordinal table (0..=219). A landed hit
  moves the defender into a real Damage-family status chosen by hitstun
  tier and ground/air situation.
- **Shield/guard**: full `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` state
  machine; a hit landing on a shield redirects into pushback instead of
  damage.
- **KO/death/respawn**: blast-zone crossing into `Dead*`, immediate stock
  decrement, `Rebirth*` sequence with real total timing and a genuine
  120-frame post-respawn invincibility window, now respected everywhere a
  hit resolves.
- **Ledges**: full `CliffCatch`→`CliffWait`→(`Climb`/`Attack`/`Escape`)→`Wait`
  state machine, catch detection reusing the existing floor-collision
  primitive plus the real corner-proximity tolerance and cliff-flag test.
- **Mario's base moveset** (this batch): `AttackDash` (dash attack),
  `AttackS3Hi`/`AttackS3`/`AttackS3Lw` (forward tilt's three angle
  variants), `AttackHi3` (up tilt), `AttackLw3` (down tilt) — all
  transcribed field-for-field from `dMarioMainMotion_DashAttack`/`FTiltHigh`/
  `FTilt`/`FTiltLow`/`UTilt`/`DTilt` (`relocData/202_MarioMainMotion.c`).
  Closed `Attack11`'s own "one hitbox, not two" gap along the way — its
  second hitbox has a zero offset like the first, so adding it was free.
  The real architectural change: `crate::attack` is no longer
  Attack11-specific. `ActiveHitbox`/`MoveData` and a per-`(FighterKind,
  Status)` `move_data` lookup replace the old one-hitbox/one-window special
  case; `apply_hit_from` now walks every hitbox a move throws out and picks
  whichever is active this frame, which is what dash attack's real
  sweetspot/sourspot (same hitbox slot, weaker numbers after frame 11) and
  the tilts' two-hitbox reach both need. A hitbox's `ox` offset is now
  mirrored by the attacker's facing (`oy`/`oz` are not — matches the
  original's joint-space convention closely enough for a world-space
  offset), closing part of the "no joint attachment" gap for any move with
  a nonzero reach offset, not just Mario's. Entry conditions are real:
  each attack's stick-angle/magnitude gate, the tilt-vs-jab dispatch
  priority (`AttackS3`/`Hi3`/`Lw3` outrank `Attack1` in the real interrupt
  chain), and dash attack's `A`-tap from `Dash` (within its real ≤20-frame
  window) or `Run`. Angle tests are reframed as `tan()` slope comparisons,
  same trick ledges' climb-or-drop check already used, since
  `ssb_engine::math` has no `atan2`. Fixed a real pre-existing gap found
  while wiring this in: `walk_interrupt` was missing `Attack1`/the tilts
  entirely even though the decomp macro has them — attacking out of a walk
  previously did nothing.
- Documented gaps for this batch: smashes (need a charge-mechanic
  subsystem — holding the attack button scales knockback over time, not
  built yet), aerials, specials, the `Attack12`/`Attack13`/`Attack100`
  jab-combo extension (its real trigger logic — per-character follow-up
  windows, an `Attack100` rapid-jab loop, a per-character `Attack13`
  finisher only some fighters have — is its own substantial subsystem,
  deliberately not folded into this batch), `DTilt`'s repeated-tap
  extension (always exits to `SquatWait` after one hit here), and every
  fighter besides Mario having zero moveset data.
- Verified for everything above together: 175 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 673 tests) green, `cargo psp
  --release` builds clean for both `psp-game` and `psp-asset-viewer`, and a
  PPSSPP headless Training boot (built with `regression_capture,
  headless_capture` — a plain release build never reaches the deterministic
  screenshot hook and reads as a hang, not a regression) shows no
  panic/crash with a real rendered frame.
- Earlier: `Dummy::apply_hit_from` hit-resolution logic moved from
  `psp-game` into `ssb_game::attack::apply_hit_from`; `psp-asset-viewer`'s
  `status_name` overlay fixed for the grown `Status` table.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Two reasonable directions, either is a valid next self-contained batch:

1. Mario's smashes (needs a real charge-mechanic subsystem: `A`/`B`-hold
   charges knockback growth over time, capped, released on button-up or
   after a max hold) — the next base-moveset piece, and the first
   opportunity to build charge as shared machinery other fighters' smashes
   reuse.
2. Mario's aerials (`AttackAirN`/`F`/`B`/`Hi`/`Lw` + their `LandingAirX`
   lag statuses) — airborne hitboxes interacting with the existing
   `tick_air`/landing-detection code, plus each aerial's own landing-lag
   status.

Grabs/throws stays deferred: a real throw's damage/knockback is baked into
each character's own motion script (`attr->thrown_status[victim_kind]`),
and the grabbed-fighter hold position is joint-attachment matrix math this
codebase has no gameplay-facing equivalent for. Revisit alongside a
fighter's other moves, not as its own isolated mechanism batch.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
