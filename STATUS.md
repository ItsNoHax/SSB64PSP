# Current State

Milestone: `F1 — Front End & Training Mode`
Primary task: in progress
Task state: `IN_PROGRESS`

`F1` (`plans/gameplay/F1.md`) remains the milestone, but the immediate work
this session is the **PSP runtime architecture refactor**: `psp-asset-viewer/`
(the debug/rendering-validation application, renamed from `psp/`) and
`psp-game/` (the player-facing front end/Training application) duplicated
their entire PSP backend. That backend is being consolidated into a new
shared library, `psp-runtime/`, so both applications orchestrate behavior
without duplicating the PSP backend between them. Per this project's own
rule ("do not begin additional F1 feature work while the runtime extraction
is half-complete" — see `AGENTS.md`), F1 feature work is paused until the
refactor's Step 10 acceptance gate passes.

Target dependency direction (`docs/ssb-architecture.md`, `README.md`):

```
psp-asset-viewer ─┐
                  ├──> psp-runtime ──┬──> crates/ssb-engine
psp-game ─────────┘                  ├──> crates/ssb-rom
                                      └──> crates/ssb-game
```

`crates/ssb-engine`, `crates/ssb-rom` and `crates/ssb-game` must never
depend on `psp-runtime`.

Refactor progress, each step committed and verified separately (`cargo test
--workspace`, `cargo psp --release` for both EBOOTs, PPSSPPHeadless
regression captures against `tests/golden/`):

1. `psp/` renamed to `psp-asset-viewer/` (package `ssb64-psp-asset-viewer`);
   `--crate psp` kept as a backwards-compatible alias in the PPSSPP tooling
   scripts — `DONE`
2. `psp-runtime/` (package `ssb-psp-runtime`) created as a `no_std` library
   both applications depend on as a path dependency; no functionality moved
   yet — `DONE`
3. `assets.rs`/`input.rs`/`timing.rs` extracted (`input.rs` was already
   byte-identical between the two apps) — `DONE`
4. Shared GE/GPU renderer (`gu.rs`/`meshdraw.rs`) merged into one
   implementation. One real bug found and fixed during the merge:
   `sceGuViewport`/`sceGuScissor` calls outside an open
   `sceGuStart`/`sceGuSync` display-list block are silently dropped, never
   reaching the GE — `DONE`, `docs/evidence/re/RE-297.md`
5. Full-matrix pixel parity proven before touching `play.rs`: 28 scenes
   (ordinary/animated stages, all 12 fighters including every RE-283 case,
   all four texgen scenes, depth-mask, CI8/translucency, 5 stage-index spot
   checks), all byte-identical (`AE=0`) against existing goldens — `DONE`,
   `docs/evidence/re/RE-297.md`
6. `play.rs` split into `psp-runtime::scene` (the shared pack-to-game
   bridge: `FloorSegments`, `FighterScene`, packed-data conversions,
   skeleton ticking) and app-specific state (the viewer's
   `status_name`/`material` overlay labels; `psp-game`'s `Dummy`, including
   `apply_hit_from`) — `DONE`
7. `Play::at_spawn(pack, stage)` (hardcoded Mario/spawn 0) generalized to
   `FighterScene::at_spawn(pack, stage, kind, spawn_index)`; `psp-game`'s
   `Dummy` now wraps a `FighterScene` (`Deref`/`DerefMut`) instead of
   duplicating its fields and re-deriving the packed-data conversions by
   hand — `DONE`
8. Duplication audit: two remaining identical helpers
   (`emit_headless_screenshot`, `facing_turn`) moved into `psp-runtime` —
   `DONE`
9. Naming/tooling/architecture docs updated (`README.md`, this file,
   `docs/ssb-architecture.md`, tooling scripts, `.claude/skills/`) — `DONE`
   (this entry)
10. Full acceptance gate — `cargo fmt --check`/`cargo clippy
    --workspace --all-targets`/`cargo test --workspace`, both EBOOTs, the
    deterministic regression matrix, `psp-game`'s deterministic
    Intro→Menu→Training→jump→jab flow: all pass, `docs/evidence/re/RE-298.md`.
    **Physical-hardware smoke test of both EBOOTs: `NOT_STARTED`** — this
    session ran in a sandboxed environment with no physical PSP attached.
    This is the one remaining manual step before F1 feature development
    resumes.

**Not yet physically hardware-tested:** the now-shared, unconditional
`sceGuDebugFlush()` call in `psp-runtime::gu::Gpu::end_frame`. RE-202's own
root cause (the hardware crash lives in `sceGuDebugPrint`/`sceGuDebugFlush`'s
state once glyphs are queued, not in a bare `Flush` against an empty buffer)
means this should be safe for `psp-game` too, matching years of
`psp-asset-viewer`'s own shipped behavior, but `psp-game` itself has never
been the subject of that specific hardware proof. Step 10's physical smoke
test covers this.

**Unchanged by the refactor, still open from RE-289–296:** `sceFont` text
for real menu/select labels; the jab's second (joint-9) hitbox; per-bone
hurtboxes; the friction-based knockback decay curve; a real `Damage`
status/animation for the target; real character/stage select UI.
C-Left/C-Right remain unmapped (`RE-008`) — not needed for the jump
function, since the decomp's check accepts any one C-button. PSP-1000
compatibility (32 MiB, cannot use extended memory) remains a separate,
long-standing open item.

Current blocker(s): the runtime refactor's Step 10 physical-hardware smoke
test (needs a real PSP; this session had none) is the one thing standing
between here and resuming F1. Once it passes (or a real bug is found and
fixed), close `docs/evidence/re/RE-298.md` and resume F1 — `sceFont` (PGF
glyph rasterisation), a real character/stage select UI, and the jab's
second hitbox/per-bone hurtbox system are all eligible next, no single
mandated order. `R3` (`plans/rendering/R3.md`) remains `NOT_STARTED` and
eligible to resume any time — neither F1 nor this refactor block it.

Relevant PLAN task: [plans/gameplay/F1.md](plans/gameplay/F1.md) (active,
paused for this refactor), [plans/rendering/R2.md](plans/rendering/R2.md)
(closed), [plans/rendering/R3.md](plans/rendering/R3.md) (parallel, not
started)
Relevant evidence: RE-297 (this refactor's renderer-parity record), RE-296,
RE-295, RE-008, RE-294, RE-293, RE-292, RE-291, RE-290, RE-289, RE-259,
RE-260 (installed-EBOOT `MEMSIZE` proof mechanism), RE-256 (`MEMSIZE`/
installed-dir requirement origin), RE-255 (`memsize` key origin/
`OutOfMemory` discovery), RE-164 (per-fighter directional light), RE-131
(real battle camera) — see
[docs/evidence/INDEX.md](docs/evidence/INDEX.md) for the full R2.2/physical
chain, RE-240–298. Toolchain note: the global `cargo-psp` install is a
hybrid build, see RE-256.
Relevant subsystem docs: [docs/porting-status.md](docs/porting-status.md),
[docs/rendering.md](docs/rendering.md), `psp-hardware` Skill (PSPLink),
`visual-regression` Skill (PPSSPPHeadless)

Current build: this session's refactor commits touch `psp-asset-viewer/`,
`psp-game/` and the new `psp-runtime/` extensively (ten commits on `main`
from the `psp/` → `psp-asset-viewer/` rename through Step 10's acceptance
gate); no changes to `crates/ssb-game`, `crates/ssb-rom`, `crates/ssb-engine`
this session. Final plain-build hashes (`docs/evidence/re/RE-298.md`):
`psp-asset-viewer` EBOOT
`066e4b4939f62f7fecc800577edabda33c1f21e44066caecc95a90fcea95c713`;
`psp-game` EBOOT
`aa60fc94e546f5b0b4f6259017cd9d8944098ae442bfc3a79055522458c181e5`;
`psp-game` `regression_capture` EBOOT (for hardware staging)
`49e1f20e9f08c20441a663ac34a21cf3145f4dfe77ed5dc38253e449bb2a7447`. Pack
unchanged this session (no asset-pipeline code touched):
`256d7661bb1dc7266ea8928bc8f341cbb121f83c330a4d3821466192ea42c17d`.
Workspace test suite: `cargo test --workspace` — 425 passed, 0 failed
throughout every step (no test-covered `crates/` logic touched, only
PSP-target-only code).
