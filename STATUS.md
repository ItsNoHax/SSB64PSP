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

Current objective: **RE-291 (this session): `psp-game/src/assets.rs` ported
from `psp/`, real asset pack now loads and parses in `psp-game`.** First item
of `plans/gameplay/F1.md`'s "Scene loading — still to build" list.

`assets.rs` (`AlignedBuf`, `LoadError`, `SEARCH_PATHS`, `load_pack()`) is a
verbatim copy of `psp/src/assets.rs`, per F1's crate-boundary rule (own copy,
not a shared refactor). `main.rs` now loads and opens the pack once at boot
and holds `pack_ok: bool`. No `sceFont` text exists yet, so pack status is
made pixel-provable the same way RE-290 proved screen state: the Training
background is `BG_TRAINING` (`20,48,24`, unchanged) when the pack loads and
parses, or a new `BG_TRAINING_NO_PACK` (`80,16,16`) otherwise.

First headless capture showed the new red colour — `pack_ok == false`. A
temporary per-error-variant diagnostic build isolated it to
`LoadError::OutOfMemory`: `psp-game/Psp.toml` was missing RE-255's
`memsize = 1` PARAM.SFO key (`psp/Psp.toml` has had it since RE-255 — the
~25.6 MiB pack exceeds the ~20-24 MiB default user partition, and
PPSSPPHeadless only reads `MEMSIZE` for an installed `PSP_GAME` directory,
exactly how the harness stages both crates). Adding the key fixed it. Diagnostic
build then showed `(0,255,0)` (`Ok(Ok(_))`); reverted to the real binary
`pack_ok` logic and recaptured: Training background `(20,48,24)`, exact match
to `BG_TRAINING` through the real code path, not just the diagnostic. No
error/panic/reject lines in the PPSSPP log on any capture.

**Still open, unchanged from RE-289/290:** no physical-PSP evidence for
`psp-game` yet. Nothing is drawn from the pack yet — the `gu.rs` 3D pipeline,
fighter/stage instantiation, and `sceFont` text remain (criteria 4-7).

Prior objective: `RE-290` pixel-confirmed the Menu → Training confirm
transition via PPSSPPHeadless, closing RE-289's one open item — criteria 2-3
fully pixel-confirmed end to end.

Current blocker(s): none. Next up for F1: a full 3D `gu.rs`/`meshdraw.rs`
port (depth test, culling, projection/view matrices, textured mesh drawing —
the current `gu.rs` only has the flat 2D rectangle path the menu placeholder
needs), fighter/stage instantiation through `ssb-game`'s
`Fighter`/`ground`/`camera` modules, and `sceFont` (PGF glyph rasterisation)
for real on-screen menu/select text — all detailed in
`plans/gameplay/F1.md`'s "Scene loading — still to build" section. Once scene
loading lands, physical-PSP confirmation of `psp-game` (still entirely
outstanding) becomes worth doing. R3 (`plans/rendering/R3.md`) remains
`NOT_STARTED` and eligible to resume any time — F1 does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-291 (this session), RE-290, RE-289, RE-255 (`memsize`
key origin), RE-256 (`MEMSIZE`/installed-dir headless requirement), RE-202
(why menu text can't use `sceGuDebugPrint`) — see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–291. Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: RE-291 (this session) adds `psp-game/src/assets.rs`, wires
pack loading into `main.rs`, adds `memsize = 1` to `psp-game/Psp.toml`, and
fixes a stale comment in `tools/run-ppsspp-headless.sh`; `psp/` is untouched
(verified via `git status`/`git diff`). `psp-game`'s plain-build (no
features, normal interactive) EBOOT SHA-256:
`6d3da5bdd010619441cd1f507fdf9952c2250d76c389051244392b01bcd325a4`. `psp/`'s
own tree and build are untouched this session (not rebuilt — Rust release
builds are not bit-reproducible across runs even with identical source, so
recomputing its hash without a source change would be a spurious diff; last
recorded value remains RE-288's
`81abb86233772aed57bc61c0d3ccefd3c2f09d24746993b41f7fd54de630b62e`). Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
