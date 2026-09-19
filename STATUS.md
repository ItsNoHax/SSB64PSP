# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Combat systems' self-contained
subsystems (shield, KO/death/respawn, ledges) are done; grabs/throws stays
deferred. Mario's ground+aerial, no-combo moveset is complete: jab, dash
attack, all tilts, all smashes, all aerials, on a real generalized
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
  that made every later move cheap to add: `crate::attack`'s hit resolution
  is no longer Attack11-specific. `ActiveHitbox`/`MoveData` and a
  per-`(FighterKind, Status)` `move_data` lookup replace the old
  one-hitbox/one-window special case, and a hitbox's `ox` offset mirrors
  with facing.
- **Mario's aerials**: `AttackAirN`/`F`/`B`/`Hi`/`Lw`, including a genuinely
  pulsing down-aerial hitbox (16 real disjoint windows) and real
  landing-lag handling (`Fighter::tick_air` → `set_landing_or_landing_air`,
  a dedicated `LandingAirX` status or a `LandingAirNull` scaled by the real
  extracted percentage).
- **Mario's smashes** (this batch): `AttackS4Hi`/`HiS`/`AttackS4`/`LwS`/`Lw`
  (all five forward-smash angles), `AttackHi4`, `AttackLw4` — ported
  field-for-field from `dMarioMainMotion_FSmash*`/`USmash`/`DSmash`.
  **Real discovery made while researching this batch: SSB64 smashes have no
  charge mechanic at all** — that is a Melee addition. A smash is just a
  hard flick (`|stick| >= 56`, inside a short tap window — `tap_x < 3`,
  `ftCommonAttackS4CheckInterruptCommon`) versus a tilt's plain magnitude
  push; the deferral note from the last two batches ("smashes need a
  charge-mechanic subsystem") was wrong, and smashes turned out to be the
  same shape as tilts. This also means a fast flick and a slow push now
  correctly diverge into a smash vs. a tilt via the tap-window check
  (`check_fsmash`/`check_usmash`/`check_dsmash`, checked before the tilt
  checks in `ground_interrupt`/`walk_interrupt`, matching the real macro
  order) — a few existing tilt tests had to be corrected because they were
  unknowingly using smash-shaped inputs (a fresh full flick), not tilt
  ones; the *implementation* was right, the *tests* needed a stick held
  for several frames before tapping `A` to simulate a genuine push.
- Documented gaps: specials, the `Attack12`/`Attack13`/`Attack100`
  jab-combo extension (its own substantial subsystem — per-character
  follow-up windows, a rapid-jab loop, a per-character finisher only some
  fighters have), grabs/throws (needs per-character throw motion data),
  `DTilt`'s repeated-tap extension, the narrow `anim_frame <= 5`
  pivot-smash-out-of-dash-startup window, `LandingAirF`/`Hi`/`B`/`Lw`
  collapsing to one tick, the real aerial auto-cancel/already-recovered
  landing branches, and every fighter besides Mario having zero moveset
  data.
- Verified for everything above together: 190 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 688 tests) green, `cargo psp
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

The jab-combo extension (`Attack12`→`Attack13`/`Attack100`) is the natural
next base-moveset piece: a real, self-contained state machine (follow-up
timing window via `attack1_followup_frames`, per-character branching —
`ftCommonAttack13CheckFighterKind` gates which fighters even have a
`Attack13` finisher — and an `Attack100` rapid-jab loop for the rest).

After that, specials (`SpecialN`/`Hi`/`Lw`/`AirN` etc.) are the last piece
of Mario's individual moveset, though each special is closer to its own
mini-subsystem (fireball projectile, cape reflect, up-B recovery with its
own physics, tornado multi-hit) than a uniform group the way tilts/smashes/
aerials were.

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
