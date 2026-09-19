# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Combat systems' self-contained
subsystems (shield, KO/death/respawn, ledges) are done; grabs/throws stays
deferred. `P2`'s fighter bulk port is underway, Mario-first: jab, dash
attack, all three tilts, and all five aerials are in, on a real generalized
per-fighter hit-data table.

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library. Verified
  with workspace tests, both EBOOTs, PPSSPP regression captures, and a
  physical-hardware smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family**: the complete
  `FTCommonStatus` ordinal table (0..=219). A landed hit moves the defender
  into a real Damage-family status chosen by hitstun tier and ground/air
  situation.
- **Shield/guard**: full `GuardOn`/`Guard`/`GuardOff`/`GuardSetOff` state
  machine; a hit landing on a shield redirects into pushback instead of
  damage.
- **KO/death/respawn**: blast-zone crossing into `Dead*`, immediate stock
  decrement, `Rebirth*` sequence with real timing and a genuine 120-frame
  post-respawn invincibility window, respected everywhere a hit resolves.
- **Ledges**: full `CliffCatch`→`CliffWait`→(`Climb`/`Attack`/`Escape`)→`Wait`
  state machine, catch detection reusing the existing floor-collision
  primitive plus the real corner-proximity tolerance and cliff-flag test.
- **Mario's ground moveset**: `AttackDash`, all three `AttackS3*` forward-tilt
  angle variants, `AttackHi3`, `AttackLw3` — plus the architectural change
  that made them cheap to add: `crate::attack`'s hit resolution is no longer
  Attack11-specific. `ActiveHitbox`/`MoveData` and a per-`(FighterKind,
  Status)` `move_data` lookup replace the old one-hitbox/one-window special
  case, and a hitbox's `ox` offset now mirrors with facing.
- **Mario's aerials** (this batch): `AttackAirN`/`F`/`B`/`Hi`/`Lw`, ported
  field-for-field from their motion scripts. This is where the `MoveData`
  generalization from the ground-moveset batch really pays for itself:
  neutral aerial has 3 simultaneous hitboxes, four of the five aerials have
  a weakening second phase, and down aerial has a genuinely *pulsing*
  hitbox — on 2 frames, off 1, eight times over — modelled as 16 real
  disjoint `ActiveHitbox` windows rather than one merged range, so landing
  in an actual gap between pulses still dodges it, matching the original.
  Landing mid-aerial is real too: `Fighter::tick_air` now calls
  `crate::status::set_landing_or_landing_air`, which takes a dedicated
  `LandingAirX` status where Mario has one, or scales `LandingAirNull`'s
  length by the real extracted landing-lag percentage where he doesn't
  (his neutral aerial) — using `f.anim.landing`, real per-character data
  already in the codebase, not an invented number. Entry is the same real
  stick-angle dispatch as the ground tilts (`tan(50°)` slope comparison,
  since `ssb_engine::math` has no `atan2`), and attacking now correctly
  outranks a second jump in the air (`ftCommonFallProcInterrupt`'s real
  order).
- Documented gaps: smashes (need a charge-mechanic subsystem — holding the
  attack button scales knockback over time), specials, the
  `Attack12`/`Attack13`/`Attack100` jab-combo extension (its real trigger
  logic is its own substantial subsystem — per-character follow-up windows,
  a rapid-jab loop, a per-character finisher only some fighters have),
  grabs/throws (needs per-character throw motion data), `DTilt`'s
  repeated-tap extension, `LandingAirF`/`Hi`/`B`/`Lw` collapsing to one tick
  (no extracted animation length for them), the real auto-cancel/
  already-recovered landing branches (dropped since the real `SetFlag1`
  window brackets nearly all of every aerial ported so far), and every
  fighter besides Mario having zero moveset data.
- Verified for everything above together: 186 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 684 tests) green, `cargo psp
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

Mario's smashes (`AttackS4Hi`/`AttackS4`/`AttackS4Lw`, `AttackHi4`,
`AttackLw4`) — the natural next base-moveset piece, and the first real
charge-mechanic subsystem: holding `A`/`B` charges knockback growth over
time (capped, released on button-up or after a max hold), which every other
fighter's smashes will reuse once ported. `dMarioMainMotion_FSmash*`/
`USmash`/`DSmash` already have the real hitbox data sitting in
`relocData/202_MarioMainMotion.c`, same as the moves already ported.

After that, the jab-combo extension (`Attack12`→`Attack13`/`Attack100`) is
worth doing as its own batch — it is a real, self-contained state machine
(follow-up timing window, per-character branching, a rapid-jab loop), not a
quick add-on to whichever batch happens to be running.

Grabs/throws stays deferred: a real throw's damage/knockback is baked into
each character's own motion script, and the grabbed-fighter hold position
is joint-attachment matrix math this codebase has no gameplay-facing
equivalent for. Revisit alongside a fighter's other moves.

## Real blockers

None. Rendering performance (`P5`) is not a blocker for this or any gameplay
batch.

---

Detailed per-subsystem status: `docs/porting-status.md`. Roadmap: `PLAN.md`.
Evidence index: `docs/evidence/INDEX.md`.
