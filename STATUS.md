# Current State

Milestone: `F1 — Front End & Training Mode`
Primary task: in progress
Task state: `IN_PROGRESS`

`F1` (`plans/gameplay/F1.md`) is the current primary task: build the intro
screen, main menu and Training Mode next, as a new separate PSP application
from the existing debug asset viewer (`psp/`). It runs parallel to `R3`
(rendering performance, still `NOT_STARTED`, not currently active) — `R3` is
not blocked by this choice, just not the task in progress. `F1`'s
training-mode combat sandbox (single stationary dummy target; real
hitbox/hurtbox/damage/knockback/hitstun; no stocks/KO/match loop/CPU
AI/items) is a scoped, explicit exception to the rendering-gate-before-combat
rule — see `AGENTS.md`'s non-negotiable constraints. Full match combat
(`G0`–`G2`) remains `BLOCKED_BY_R3`.

Current objective: **RE-293 (this session): a real, physics-ticked stationary
dummy target in Training Mode (`play::Dummy`).** Spawns at the stage's second
spawn point (`pack.spawn(stage, 1)`), no player/AI control — permanently
neutral input, so it stands on the real ground under real physics/animation
alongside the player's fighter, realizing the "single stationary dummy
target only" line `plans/gameplay/F1.md`'s Known Limitations named.

`psp-game/src/play.rs` gained a `Dummy` struct (fighter, skeleton, object,
started), mirroring `Play::at_spawn`/`tick` but with no camera and no input
source; `Play::tick_animation`'s status-change animation-restart logic was
factored into a shared free function, `tick_skeleton_animation`, so `Dummy`
reuses it rather than duplicating it. `Dummy` has no `psp/` equivalent (the
debug viewer has no training combat), so unlike the rest of `play.rs` — a
verbatim port — it is `psp-game`-only, documented as such. `main.rs` spawns
`dummy_state` alongside `play_state` on first Training entry, ticks it every
frame, and draws it through `meshdraw::draw_object_posed` exactly like the
player fighter (own pose, own per-fighter light bracket, RE-164), with no
camera interest of its own.

**Still open, unchanged from RE-289–292:** no physical-PSP evidence for
`psp-game` yet. `sceFont` text for real menu/select labels, and all of
criterion 5's actual combat (real hitbox/hurtbox from `FTAttributes`,
damage, knockback/hitstun against the now-real dummy target, per
`ftphysics.c`) remain.

Prior objective: `RE-292` ported `psp-game/src/gu.rs`'s 3D pipeline,
`meshdraw.rs` and `play.rs` from `psp/`; Training started spawning a real
Mario on a real stage (Dream Land) drawn through the real battle camera.

Current blocker(s): none. Next up for F1: Training combat (input → hitbox →
hurtbox → damage → knockback → hitstun, `FTAttributes`/`ftphysics.c`,
criterion 5, now that a real target exists to hit); `sceFont` (PGF glyph
rasterisation) for real on-screen menu/select text; a real, sourced
jump-button binding; real character/stage select UI. Once combat lands,
physical-PSP confirmation of `psp-game` (still entirely outstanding) becomes
worth doing. R3 (`plans/rendering/R3.md`) remains `NOT_STARTED` and eligible
to resume any time — F1 does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-293 (this session), RE-292, RE-291, RE-290, RE-289,
RE-255 (`memsize` key origin), RE-256 (`MEMSIZE`/installed-dir headless
requirement), RE-164 (per-fighter directional light), RE-131 (real battle
camera) — see [docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full
R2.2/physical chain, RE-240–293. Toolchain note: the global `cargo-psp`
install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: RE-293 (this session) adds `Dummy` to `psp-game/src/play.rs`
and wires it into `psp-game/src/main.rs`; `psp/` is untouched (verified via
`git status`/`git diff --stat -- psp/`). `psp-game`'s plain-build (no
features, normal interactive) EBOOT SHA-256:
`34dd8aaf3110d3f25e05e9c4994b66ced28e93888225e0b2e2f24335e3a65c1a`. `psp/`'s
own tree and build are untouched this session (not rebuilt — Rust release
builds are not bit-reproducible across runs even with identical source, so
recomputing its hash without a source change would be a spurious diff; last
recorded value remains RE-288's
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`). Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
