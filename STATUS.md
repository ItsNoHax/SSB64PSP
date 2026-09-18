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

Current objective: **RE-290 (this session): `psp-game`'s Menu → Training
confirm transition pixel-confirmed, closing RE-289's one open item.**
Acceptance criteria 2-3 of `plans/gameplay/F1.md` (Intro → Menu → Training
navigation shape) are now fully pixel-confirmed end to end under PPSSPP
software rendering, not just code-reviewed.

RE-289's manual windowed-PPSSPP + X11 key-injection approach (Xlib fallback,
then `xdotool` — both reproduced the identical failure) could not reliably
deliver the Menu → Training confirm keypress, most likely due to this
desktop's Wayland/XWayland compositor racing synthetic focus/key events, and
separately, the user flagged that windowed/X11-input automation hijacks
real desktop input focus while it runs. Both problems are avoided by
switching to `PPSSPPHeadless` (no window, no input injection) — the
project's own documented golden-capture tool (`visual-regression` Skill),
now used for `psp-game` too, not just `psp/`'s render-freeze goldens.

`psp-game` gained two new Cargo features, `regression_capture` and
`headless_capture` (named to match `psp/`'s features of the same names for
tooling symmetry; the mechanism differs). `regression_capture` replaces real
`sceCtrl` polling with `scripted_buttons(tick)`, a fixed table that presses
`A` at tick 4 (Intro → Menu) and again at tick 8 (Menu → Training; cursor
starts on `TRAINING_ENTRY`, no D-pad needed), then freezes all further state
transitions past `DETERMINISTIC_CAPTURE_TICKS` — this is a genuinely new
pattern for the project: `psp/`'s own deterministic-capture features only
ever freeze *output* (physics/animation/camera), never override *input*,
because its viewer has no input-driven state machine to script.
`headless_capture` fires the same `sceIoDevctl` screenshot request `psp/`
uses, once, via a latch. `tools/run-ppsspp-headless.sh` gained a `--crate
psp|psp-game` flag (default `psp`, existing invocations unchanged), each
crate staged into its own memstick subfolder so captures can't clobber each
other.

Two headless captures closed the item: the final state (`DETERMINISTIC_
CAPTURE_TICKS = 16`, past both scripted presses) showed background
`(20,48,24)`, an **exact** match to `BG_TRAINING` and distinct from both
`BG_INTRO`/`BG_MENU`, proving the full Intro → Menu → Training sequence ran
(that screen is only reachable via that path). A second, temporary capture
at `DETERMINISTIC_CAPTURE_TICKS = 6` (between the two scripted presses)
independently confirmed the intermediate `Menu` state — background
`(16,16,24)` (`BG_MENU`) and the selected entry-0 rectangle `(255,200,40)`
(`ENTRY_SELECTED`) — matching RE-289's manual sampling exactly. No
error/panic/reject lines in the PPSSPP log on either capture. The crate was
rebuilt plain (`cargo psp --release`, no features) immediately after, so
the regression/scripted-input behaviour does not carry into the normal
build.

**Still open, unchanged from RE-289:** no physical-PSP evidence for
`psp-game` yet — tracked as required before this increment is physically
verified, per `AGENTS.md`'s "PPSSPP is not physical PSP proof." All of
criteria 4-7 (real scene loading, hitbox/damage/knockback, decomp
verification) remain future F1 work, detailed below.

Prior objective: `RE-289` scaffolded `psp-game/` as a second, independent
EBOOT (criterion 1 done) with the Intro/Menu navigation shape in place.

Current blocker(s): none. Next up for F1: real scene loading — `assets.rs`
(load `assets/generated/ssb64.pak`, mirroring `psp/src/assets.rs`), a full
3D `gu.rs`/`meshdraw.rs` port (depth test, culling, projection/view
matrices, textured mesh drawing — the current `gu.rs` only has the flat 2D
rectangle path the menu placeholder needs), fighter/stage instantiation
through `ssb-game`'s `Fighter`/`ground`/`camera` modules, and `sceFont`
(PGF glyph rasterisation) for real on-screen menu/select text — all detailed
in `plans/gameplay/F1.md`'s "Scene loading — still to build" section. Once
scene loading lands, physical-PSP confirmation of `psp-game` (still entirely
outstanding) becomes worth doing. R3 (`plans/rendering/R3.md`) remains
`NOT_STARTED` and eligible to resume any time — F1 does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-290 (this session), RE-289, RE-288, RE-287, RE-202 (why
menu text can't use `sceGuDebugPrint`) — see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–290. Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: RE-290 (this session) adds `regression_capture`/
`headless_capture` features to `psp-game/` and a `--crate` flag to
`tools/run-ppsspp-headless.sh`; `psp/` is untouched (verified via `git
status`/`git diff`). `psp-game`'s plain-build (no features, normal
interactive) EBOOT SHA-256:
`c12bb1b8e9fd5430738955c979f886b6484f3b2b6d3bef72d1f79f3310ede285`. `psp/`'s
own last-recorded EBOOT hash is unchanged from RE-288:
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`. Pack
unchanged this session (no asset-pipeline code touched, and `psp-game`
doesn't load it yet):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
