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

Current objective: **RE-296 (this session): first physical-PSP confirmation
of `psp-game`, plus two real bugs found and fixed along the way.** RE-295
(prior session) pixel-confirmed the jab/jump scene via PPSSPPHeadless only;
`psp-game` had never once been run on real hardware. This session did that,
and found the front end was not actually in a clean state to test:

1. **`psp/` no longer built.** RE-294's `Status::Attack11` addition to the
   shared `crates/ssb-game` enum left `psp/src/play.rs`'s exhaustive
   status-label match non-exhaustive (`E0004`), breaking `psp/`'s build with
   zero files under `psp/` touched — exactly the kind of shared-crate break
   `AGENTS.md`'s F1 carve-out (criterion 1) exists to catch. Fixed: added
   `Attack11 => "jab1    "` to the match.
2. **`psp-game` failed `LoadError::OutOfMemory` under PSPLink's `ldstart`.**
   Root-caused (not just diagnosed): extended `psp-game/src/main.rs`'s
   existing pixel-provable pack-failure convention to distinguish *why*
   (`BG_TRAINING_PACK_EMPTY`/`_OUT_OF_MEMORY`/`_SHORT_READ`/`_PARSE_FAILED`,
   alongside the existing `BG_TRAINING_NO_PACK`) instead of one flat colour
   for every failure reason. That pinpointed `AlignedBuf::new` failing to
   allocate the ~28MiB pack buffer. A same-session controlled comparison —
   freshly built `psp/` hitting the identical fallback under the identical
   `ldstart` path — proved this was not a `psp-game`-specific regression:
   `pspsh -e meminfo` showed the live PSPLink session's user partition capped
   at ~22-23MiB free regardless of `psp-game/Psp.toml`'s `memsize = 1`
   (RE-255/RE-291), because PSPLink's `ldstart` never applies `MEMSIZE` in
   the first place — RE-256 already established this (`MEMSIZE` only takes
   effect from `PARAM.SFO` on a proper *installed-directory* boot), and
   RE-259/RE-260's own physical `MEMSIZE` proof already used the installed-
   EBOOT-from-XMB path for exactly this reason, never `ldstart`. This
   session had briefly lost sight of that established mechanism.

**Resolved and pixel-confirmed on real hardware.** Staged `psp-game`'s
EBOOT+pack at `ms0:/PSP/GAME/ssb64game/` via PSPLink's own `ms0:` file
commands (no re-mount needed). The user enabled extended memory on the
device (`meminfo`'s user partition free space went from ~22MiB to
~51MiB). Re-running the identical `ldstart` sequence: zero exceptions,
and a native `scrshot` capture showing the real Training scene — both
Marios standing together on Dream Land's side platform under the tree, jab
range, the same close battle-camera framing RE-295's PPSSPP-headless
capture recorded. Quantitative check against that PPSSPP golden
(box-downsampled to the hardware's native 480x272): 0.8% of pixels exceed a
30-level channel-sum threshold (max 626/765) — this project's established
hardware-vs-PPSSPP antialiasing noise floor (RE-282: 0.27%), not a new
defect. Closes the "no physical-PSP evidence for `psp-game`" gap `STATUS.md`
has carried since RE-289.

**Still open, unchanged from RE-289–295 except where noted:** `sceFont`
text for real menu/select labels. Criterion 5's remaining documented gaps
(`crates/ssb-game/src/attack.rs`'s module docs): the jab's second (joint-9)
hitbox, per-bone hurtboxes, the friction-based knockback decay curve, and a
real `Damage` status/animation for the target. Real character/stage select
UI. C-Left/C-Right remain unmapped (`RE-008`) — not needed for the jump
function, since the decomp's check accepts any one C-button. PSP-1000
compatibility (32 MiB, cannot use extended memory) remains a separate,
long-standing open item, not newly affected by this session.

Current blocker(s): none. Next up for F1 (no single mandated order — pick
by what's most load-bearing): `sceFont` (PGF glyph rasterisation) for real
on-screen menu/select text, still entirely outstanding since RE-289; a real
character/stage select UI (currently a hardcoded default); the jab's second
hitbox and a per-bone hurtbox system, if/when justified. `R3`
(`plans/rendering/R3.md`) remains `NOT_STARTED` and eligible to resume any
time — `F1` does not block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active),
[plans/rendering/R2.md](plans/rendering/R2.md) (closed),
[plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not started)
Relevant evidence: RE-296, RE-295, RE-008, RE-294, RE-293, RE-292, RE-291,
RE-290, RE-289, RE-259, RE-260 (installed-EBOOT `MEMSIZE` proof mechanism),
RE-256 (`MEMSIZE`/installed-dir requirement origin), RE-255 (`memsize` key
origin/`OutOfMemory` discovery), RE-164 (per-fighter directional light),
RE-131 (real battle camera) — see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–296. Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: RE-296 (this session) touches `psp-game/src/main.rs` (the
pack-failure-colour diagnostic split) and `psp/src/play.rs` (one added match
arm, fixing the build break) — no further changes to `crates/ssb-game`,
`crates/ssb-rom`, `crates/ssb-engine` this session. `psp-game`'s
`regression_capture`-feature EBOOT SHA-256 (physical-hardware build, this
session): `53047d31901bf69ced75a640df2c447e1a138d5d880b34092c95173ed8073862`;
PRX SHA-256: `f16bee1cadd877ffc102497c35faa80e09d596f522ff79eb1c88f7ad5cdf0942`.
`psp/`'s own build is fixed but not otherwise changed; its plain-build hash
was not re-recorded this session (only a `regression_capture` control build
was needed, and discarded after the comparison). Pack unchanged this session
(no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
Workspace test suite: `cargo test --workspace` — 425 passed, 0 failed (this
session touched no test-covered logic, only PSP-target-only code and a debug
color mapping not exercised by workspace tests).
