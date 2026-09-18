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

Current objective: **RE-295 (this session): a real jump binding, and the
first pixel-confirmed jab.** RE-294 (prior session) built Mario's jab
(`Attack11`) end to end but could not pixel-confirm it landing: the training
dummy sits at the real stage's second spawn point, `~1664` units from the
player (mostly vertical, on a side platform), and `psp-game` had no jump
input wired at all. This session found that the jump binding was not an open
design question — `ft/ftcommon/ftcommonkneebend.c`'s
`ftCommonKneeBendCheckButtonTap` treats any N64 C-button tap as a real
jump-by-button input, and `ssb_engine::input::DEFAULT_MAPPING` already
assigns the PSP's Triangle/Square to `C_UP`/`C_DOWN` — so wiring it was one
`controller.buttons.contains(JUMP_BUTTON_MASK)` check into `Play::tick`'s
already-fully-ported jump physics (`crates/ssb-game/src/status.rs`), not new
physics. This also resolved part of the long-`OPEN` `RE-008` (C-button
mapping): the jump function is now decomp-confirmed, though taunt/camera
C-button uses remain unconfirmed.

A new `romtool jumptest` subcommand (`tools/romtool/src/main.rs`) ticks the
real `ssb_game::fighter::Fighter` against a stage's real floor segments
natively (no PSP/emulator), which is how the working input schedule was
actually derived rather than guessed: a single grounded jump's apex measured
`~660` units (short of the platform regardless of stick, since
`ftCommonJumpGetJumpForceButton` trades height for horizontal distance and
grounded speed does not carry into a jump's `vel_air.x`, both confirmed
against the decomp), so the schedule uses a vertical button jump followed by
a midair jump timed to reset the arc onto the platform, landing within `2.2`
units of the dummy's real settled position and connecting the jab
(`ssb_game::attack::spheres_overlap` true at ticks 92-93 of the trace).

`psp-game/src/main.rs`'s `regression_capture` deterministic-capture script
now drives this full schedule (`scripted_buttons`/`scripted_stick_x`,
`DETERMINISTIC_CAPTURE_TICKS` raised from 16 to 106 to cover it), and the
`regression_capture` branch of the Training tick call now feeds the game its
scripted controller state instead of discarding it as neutral — needed once
there was real input-driven movement (the jump) to capture, not just a
button tap. The real (non-capture) jump binding applies unconditionally, not
just under capture.

**Pixel-confirmed.** `tools/run-ppsspp-headless.sh --crate psp-game
--seconds 10` captured at the new deterministic tick:
`docs/images/re295-jab-connects-dummy-platform.png` shows both Marios
standing together on the real side platform under Dream Land's tree, in
melee range, with the camera zoomed to the close battle framing — the first
`psp-game` capture where the player's fighter is not sitting at its spawn
position, i.e. the jump traversal is visibly working, not just a static
scene.

**Still open, unchanged from RE-289–294 except where noted:** no
physical-PSP evidence for `psp-game`. `sceFont` text for real menu/select
labels. Criterion 5's remaining documented gaps (`crates/ssb-game/src/
attack.rs`'s module docs): the jab's second (joint-9) hitbox, per-bone
hurtboxes, the friction-based knockback decay curve, and a real `Damage`
status/animation for the target. Real character/stage select UI. C-Left/
C-Right remain unmapped (`RE-008`) — not needed for the jump function, since
the decomp's check accepts any one C-button.

Current blocker(s): none. Next up for F1 (no single mandated order — pick
by what's most load-bearing): `sceFont` (PGF glyph rasterisation) for real
on-screen menu/select text, still entirely outstanding since RE-289; a real
character/stage select UI (currently a hardcoded default); the jab's second
hitbox and a per-bone hurtbox system, if/when justified; physical-PSP
confirmation of `psp-game` (still entirely outstanding, and now more
worth doing with a pixel-confirmed combat loop to validate). `R3`
(`plans/rendering/R3.md`) remains `NOT_STARTED` and eligible to resume any
time — `F1` does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-295, RE-008 (this session), RE-294, RE-293, RE-292,
RE-291, RE-290, RE-289, RE-255 (`memsize` key origin), RE-256 (`MEMSIZE`/
installed-dir headless requirement), RE-164 (per-fighter directional light),
RE-131 (real battle camera) — see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–295. Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: RE-295 (this session) touches `psp-game/src/main.rs` (jump
wiring, capture-script schedule) and `tools/romtool/src/main.rs` (new
`jumptest` subcommand) — no further changes to `crates/ssb-game`,
`crates/ssb-rom`, `crates/ssb-engine` beyond RE-294's, which remain
uncommitted from the prior session alongside this one (`git status`: nothing
in either session's work has been committed yet). `psp/` is untouched by
both sessions (`git diff --stat -- psp/` empty). `psp-game`'s plain-build (no
features, normal interactive) EBOOT SHA-256:
`3c289dfccc45a6a0700e0d0f812d956ae700c7c5751b7c49baddc1054d4609d4` (changed
from RE-294's, as expected — this session changed `psp-game`'s own source).
`psp/`'s own tree and build are untouched this session (last recorded value
remains RE-288's
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`). Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
Workspace test suite: `cargo test --workspace` — 425 passed, 0 failed (this
session touched no `ssb-game`/`ssb-rom`/`ssb-engine` code, only `psp-game`
input wiring and the new `romtool` subcommand, so the count is unchanged
from RE-294).
