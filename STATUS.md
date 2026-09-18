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

Current objective: **RE-289 (this session): F1's `psp-game/` crate
scaffolded — acceptance criterion 1 (independent EBOOT, `psp/` unmodified)
done, criteria 2-3's Intro/Menu navigation shape done and pixel-confirmed.**
New crate `psp-game/` (own `Cargo.toml`/`Psp.toml`/`rust-toolchain.toml`,
mirroring `psp/`'s `cargo psp` setup, added to the workspace `exclude` list)
implements an `Intro` → `Menu` → `Training`(placeholder) state machine using
the shared `ssb_engine::input` edge-detection helpers. The menu is three
colour-coded rectangle entries (`Training` selectable, two inert
placeholders per F1's explicit allowance) navigated with the D-pad and
confirmed with Cross; `psp-game/src/gu.rs` is a deliberately smaller,
separate copy of `psp/`'s GU glue (flat 2D rectangles only, no 3D pipeline
yet) — kept as its own file rather than a shared refactor, since `psp/`'s
own build/EBOOT must stay unmodified and the two are separate out-of-
workspace `cargo psp` crates. No on-screen text yet: `sceGuDebugPrint`/
`sceGuDebugFlush` reliably crashes real hardware (RE-202), so menu entries
are colour-only until a `sceFont`-based text renderer lands (tracked in
`plans/gameplay/F1.md`'s new "Scene loading — still to build" section,
alongside real pack/stage/fighter loading to replace the current
placeholder colour screens).

`tools/run-ppsspp.sh` gained a `--crate psp|psp-game` flag (default `psp`,
every existing invocation unchanged) so the same harness drives both EBOOTs
rather than duplicating an 800-line script.

**Verification this session:** automated PPSSPPHeadless-style capture via
`tools/run-ppsspp.sh --crate psp-game` confirmed a non-blank intro screen at
the exact expected background colour. A manually driven PPSSPP instance
(X11 key injection, the same fallback the project's exhaustive audits use)
pixel-confirmed Intro→Menu, D-pad cursor movement (including wraparound),
and confirm-is-a-no-op-on-a-stubbed-entry, each via exact RGB sampling, not
visual impression. **Not confirmed this session:** the final Menu→Training
confirm transition — code-reviewed as using the identical, already-proven
edge-detection pattern, but repeated manual attempts against a fresh
instance could not reliably deliver the key press, most likely XTEST
injection racing this desktop's Wayland/XWayland compositor focus
arbitration rather than a code defect. Flagged as the next verification
item, not assumed passing. No physical-PSP evidence for `psp-game` yet.

Prior objective: `RE-288` physically confirmed PSP-1000 pack-load
incompatibility (32 MiB RAM, clean `OutOfMemory` fallback), closing `R2`.

Current blocker(s): none. Next up for F1: pixel-confirm the Menu→Training
transition (retry the manual PPSSPP input round-trip, or install `xdotool`
for more reliable key injection than the Xlib fallback used this session),
then build real scene loading (`assets.rs`, a full 3D `gu.rs`/`meshdraw.rs`
port, fighter/stage instantiation through `ssb-game`) and `sceFont` text
rendering to replace the current placeholder colour-block UI — both detailed
in `plans/gameplay/F1.md`'s "Scene loading — still to build" section. R3
(`plans/rendering/R3.md`) remains `NOT_STARTED` and eligible to resume any
time — F1 does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-289 (this session), RE-288, RE-287, RE-202 (why menu
text can't use `sceGuDebugPrint`) — see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–289. Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink)

Current build: RE-289 (this session) adds the new `psp-game/` crate; `psp/`
is untouched (verified via `git status`/`git diff`). `psp-game` EBOOT
(PRX) SHA-256: `b7948c0c1895981785037c9adbe6f59b53c0b127baf07bfdb8d7ac9af331d6c9`.
`psp/`'s own last-recorded EBOOT hash is unchanged from RE-288:
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`. Pack
unchanged this session (no asset-pipeline code touched, and `psp-game`
doesn't load it yet):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
