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

Current objective: **RE-292 (this session): `psp-game`'s 3D pipeline
(`gu.rs`), mesh drawing (`meshdraw.rs`) and gameplay-slice adapter
(`play.rs`) ported verbatim from `psp/`.** Training Mode now spawns a real,
physics-ticked Mario on a real stage (Dream Land, index 0) and draws both
through the real battle camera — the second and third items of
`plans/gameplay/F1.md`'s "Scene loading — still to build" list (RE-291
landed the first, pack loading).

`gu.rs` gained an additive 3D pipeline (depth buffer, `GuVertex`,
projection/view/model matrix helpers, a pillarboxed-viewport toggle) on top
of the existing flat-rectangle path used by Intro/Menu, which is otherwise
unchanged (RE-289/290's pixel evidence for those screens stays valid —
`draw_rect` brackets depth test/culling off around its own draw, the same
pattern `psp/gu.rs`'s own `draw_wallpaper_sprite` uses). `meshdraw.rs` and
`play.rs` are new files, verbatim copies of `psp/`'s own (crate-boundary
rule: own copy, not a shared refactor — same as RE-291's `assets.rs`).
`main.rs` wires `Screen::Menu`'s A-press into Training to spawn
`play::Play::at_spawn` on the real stage, ticks it every Training frame with
real `sceCtrl` stick input (neutral under `regression_capture`, matching the
existing screen-navigation determinism story), and draws the stage
(`meshdraw::draw_stage`) and fighter (`meshdraw::draw_object_posed`, posed
and lit per-frame) through the real `Play::camera`. Falls back to the
unchanged flat `BG_TRAINING_NO_PACK` when the pack/stage isn't available.

Known, documented simplifications (not guesses): jump is not wired (no
sourced decomp binding for this front end yet — `psp/`'s own C_LEFT stand-in
is an explicit debug-viewer substitute); material/stage-scenery animation
are not ticked; costume is always 0; character/stage select still feed a
hardcoded Mario/Dream Land default rather than a real selection UI.

**Still open, unchanged from RE-289/290/291:** no physical-PSP evidence for
`psp-game` yet. `sceFont` text for real menu/select labels, a stationary
dummy target, and all of criterion 5's combat (real hitbox/hurtbox from
`FTAttributes`, damage, knockback/hitstun per `ftphysics.c`) remain.

Prior objective: `RE-291` ported `psp-game/src/assets.rs`, wired real pack
loading into `main.rs`, fixed a missing `memsize = 1` in `psp-game/Psp.toml`.

Current blocker(s): none. Next up for F1: `sceFont` (PGF glyph rasterisation)
for real on-screen menu/select text; a stationary dummy target in Training;
Training combat (input → hitbox → hurtbox → damage → knockback → hitstun,
`FTAttributes`/`ftphysics.c`, criterion 5); a real, sourced jump-button
binding; real character/stage select UI. Once scene loading and combat land,
physical-PSP confirmation of `psp-game` (still entirely outstanding) becomes
worth doing. R3 (`plans/rendering/R3.md`) remains `NOT_STARTED` and eligible
to resume any time — F1 does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-292 (this session), RE-291, RE-290, RE-289, RE-255
(`memsize` key origin), RE-256 (`MEMSIZE`/installed-dir headless
requirement), RE-164 (per-fighter directional light), RE-131 (real battle
camera) — see [docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full
R2.2/physical chain, RE-240–292. Toolchain note: the global `cargo-psp`
install is a hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: RE-292 (this session) adds `psp-game/src/meshdraw.rs` and
`psp-game/src/play.rs` (new files, verbatim copies of `psp/`'s own), extends
`psp-game/src/gu.rs` with the 3D pipeline, and wires Training Mode's real
stage+fighter draw into `psp-game/src/main.rs`; `psp/` is untouched (verified
via `git status`/`git diff --stat -- psp/`). `psp-game`'s plain-build (no
features, normal interactive) EBOOT SHA-256:
`9627eecfd90ccdc1fd502b3d8beb5089221cf6637f26eb3cf08242442cabcc95`. `psp/`'s
own tree and build are untouched this session (not rebuilt — Rust release
builds are not bit-reproducible across runs even with identical source, so
recomputing its hash without a source change would be a spurious diff; last
recorded value remains RE-288's
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`). Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
