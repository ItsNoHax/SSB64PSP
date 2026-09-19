# Current State

Milestone: `P0` (closed) → gameplay source-port (`P1`-`P4` combined, user
directive 2026-09-19: fighter runtime/common state machinery → fighter-common
gameplay → combat systems → all 12 fighters → match gameplay, translated in
large coherent batches rather than per-function).

Current subsystem/batch: none open. Combat systems' self-contained
subsystems (shield, KO/death/respawn, ledges) are done; grabs/throws stays
deferred. Mario's ground+aerial moveset is complete except specials and the
`Attack13`/`Attack100` combo extension (both need infrastructure this
codebase doesn't have yet — see "Immediate next batch").

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
- **Mario's ground moveset**: jab, dash attack, all forward-tilt angles,
  up/down tilt — plus the architectural change that made every later move
  cheap to add: `crate::attack`'s hit resolution is a per-`(FighterKind,
  Status)` `MoveData` table (`ActiveHitbox` + `move_data` lookup) instead of
  a single hardcoded hitbox/window, and a hitbox's `ox` offset mirrors with
  facing.
- **Mario's aerials**: all five, including a genuinely pulsing down-aerial
  hitbox (16 real disjoint windows) and real landing-lag handling
  (`Fighter::tick_air` → `set_landing_or_landing_air`).
- **Mario's smashes**: all five forward-smash angles, up/down smash.
  Discovered while researching this one: **SSB64 smashes have no charge
  mechanic at all** (a Melee addition) — a smash is just a hard flick inside
  a short tap window versus a tilt's plain push, so smashes turned out to be
  the same shape as tilts, not a new subsystem as previously assumed.
- **Jab combo, `Attack11`→`Attack12`** (this batch): a repeated, well-timed
  tap during `Jab1` now really chains into `Jab2` instead of always ending
  in `Wait` — `Attack1State` (`followup_frames`/`is_goto_followup`) plus a
  collapsed version of the original's two per-frame callbacks
  (`ftCommonAttack11ProcUpdate`/`...ProcInterrupt`). The collapse is exact,
  not approximate: `Jab1`/`Jab2`'s motion scripts both set their
  `SetFlag1(1)` "combo window is live" flag at exactly their own total
  frame length, so `StatusState::animation_ended` already *is* that flag,
  and one check per frame covers what the original spreads across two
  callbacks. Ported `MARIO_JAB2`'s real two-hitbox data
  (`dMarioMainMotion_Jab2`, a literal 70° launch angle) alongside it.
- Documented gaps, now sharpened: `Attack12`'s own further chain into
  `Attack13` needs a per-character status (`nFTMarioStatusAttack13`) beyond
  the common 0..=219 table — this codebase has no per-character status
  extension point yet, which is the real blocker, not missing timing logic.
  `Attack100` (rapid-jab loop) doesn't apply to Mario at all
  (`ftCommonAttack100CheckFighterKind` excludes him) so it isn't a gap for
  him specifically. Also still open: specials, grabs/throws, `DTilt`'s
  repeated-tap extension, the narrow dash-startup pivot-smash window,
  `LandingAirF`/`Hi`/`B`/`Lw` collapsing to one tick, the real aerial
  auto-cancel branch, and every fighter besides Mario having zero moveset
  data.
- Verified for everything above together: 193 `ssb-game` tests, full
  workspace (`cargo test --workspace`, 691 tests) green, `cargo psp
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

Mario's individual moveset has two pieces left, and both need new
infrastructure rather than just more `MoveData` entries:

1. **A per-character status extension point.** `Attack13` (Mario's jab
   finisher), and eventually every other fighter's unique statuses, live
   above ordinal 219 (`nFTCommonStatusSpecialStart`), outside the shared
   `Status` enum's common table. Needs a design decision: a second,
   per-`FighterKind` status enum layered on top of the common one, or
   something else — worth resolving once, since every one of the 12
   fighters' specials/unique moves needs it.
2. **Specials** (`SpecialN`/`Hi`/`Lw`/`AirN`). Each is closer to its own
   mini-subsystem than a uniform group the way tilts/smashes/aerials were:
   Mario's fireball is a projectile, up-B (`SuperJumpPunch`) is a recovery
   move with its own physics arc, down-B (`MarioTornado`) is a multi-hit
   loop. Probably worth splitting into its own batch per special rather
   than one batch for all four.

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
