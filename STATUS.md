# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Combat systems' self-contained
subsystems (shield, KO/death/respawn, ledges) are done; grabs/throws stays
deferred. Mario's ground+aerial moveset is complete except specials — the
jab combo now reaches its real `Attack13` finisher, on a new per-character
status extension (`AnyStatus`) every other fighter's unique moves will also
need.

## What was completed

- `P0`: `psp-runtime` split out as the shared PSP-specific library. Verified
  with workspace tests, both EBOOTs, PPSSPP regression captures, and a
  physical-hardware smoke test (`docs/evidence/re/RE-298.md`).
- **Fighter-common status table + Damage/hitstun family**: the complete
  `FTCommonStatus` ordinal table (0..=219).
- **Shield/guard**, **KO/death/respawn**, **Ledges**: full state machines,
  each covered in earlier batches this session (see git log for detail).
- **Mario's ground+aerial moveset**: jab, dash attack, all tilts, all
  smashes, all aerials — a per-`(FighterKind, Status)` `MoveData` table
  (`ActiveHitbox` + `move_data` lookup) drives every hit, including multiple
  simultaneous hitboxes, weakening phases, and a genuinely pulsing hitbox
  (down aerial). Real discovery along the way: SSB64 smashes have no charge
  mechanic (a Melee addition) — a smash is a hard flick vs. a tilt's plain
  push, the same shape as a tilt mechanically.
- **The jab combo now reaches `Attack13`, its real finisher** (this batch).
  Requesting this required resolving a real architectural fork first: how
  to represent per-character statuses (`Attack13`/`nFTMarioStatusAttack13`
  lives at ordinal 220, outside the shared common table, and every fighter
  numbers its own extended statuses independently from 220 in the
  original — so a flat shared enum would either collide fighters against
  each other or need fidelity-breaking renumbering). Asked the user;
  chosen approach: a per-fighter enum (`MarioStatus`, currently just
  `Attack13 = 220`) plus a sum type (`AnyStatus::Common(Status)` /
  `AnyStatus::Mario(MarioStatus)`), now what `StatusState`/`Fighter`
  actually store. Kept the refactor's real cost down with two tricks:
  (1) `impl PartialEq<Status> for AnyStatus` means the ~190 existing
  `f.status.status == Status::X`/`assert_eq!` comparisons across the crate
  needed zero changes — only the handful of places that *pattern-match* on
  the field (`update`'s big dispatch, `move_data`, `set_landing_or_landing_air`,
  a couple more) needed a `let AnyStatus::Common(s) = ... else { ... }`
  unwrap; (2) `update`'s existing 200+-arm common-table match is completely
  unchanged internally — it is now reached through exactly one added
  unwrap at the top, with a small sibling `update_extended` handling
  `AnyStatus::Mario` statuses. `attack13_status(kind)` gates which fighters
  even have a jab finisher, mirroring `ftCommonAttack13CheckFighterKind`
  rather than assuming every fighter does. Also fixed the same class of
  break this caused downstream: `psp-runtime`'s `tick_skeleton_animation`
  and `psp-asset-viewer`'s debug status-label overlay both held/matched a
  bare `Status` and needed the same `AnyStatus` treatment — caught by
  building both PSP EBOOTs, same lesson as an earlier batch's
  `psp-asset-viewer` break.
- Documented gaps: `Attack100` (rapid-jab loop, a different mechanic for a
  different fighter set that excludes Mario) not ported; no other fighter
  has a `MarioStatus`-equivalent enum or any moveset data yet; specials;
  grabs/throws; `DTilt`'s repeated-tap extension; the narrow dash-startup
  pivot-smash window; `LandingAirF`/`Hi`/`B`/`Lw` collapsing to one tick;
  the real aerial auto-cancel branch.
- Verified for everything above together: 196 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 694 tests) green, `cargo psp
  --release` builds clean for both `psp-game` and `psp-asset-viewer`, and
  PPSSPP headless boots of both (`regression_capture,headless_capture` —
  a plain release build never reaches the deterministic screenshot hook
  and reads as a hang, not a regression) show no panic/crash with real
  rendered frames.
- Pre-batch-mode work (rendering pipeline, asset pipeline, animation,
  collision, physics, movement-state machine) predates formal batch mode but
  is usable foundation, tracked per-subsystem in `docs/porting-status.md`.

## Immediate next batch

Specials (`SpecialN`/`Hi`/`Lw`/`AirN`) are the last piece of Mario's
individual moveset. Each is closer to its own mini-subsystem than a uniform
group the way tilts/smashes/aerials were: the fireball is a projectile
(needs a projectile/spawned-object concept this codebase doesn't have),
up-B (`SuperJumpPunch`) is a recovery move with its own physics arc,
down-B (`MarioTornado`) is a multi-hit loop. Worth splitting into its own
batch per special rather than one batch for all four — and worth deciding
up front whether Mario's projectile needs a new general "spawned object"
concept in `ssb-game` or a narrower one-off, since Fox/Samus/Ness/Yoshi
etc. will all want projectiles too.

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
