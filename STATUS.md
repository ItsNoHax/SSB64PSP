# Project Status

**Last updated:** 2026-09-10 (RE-200 session)

## Continuation packet

**Milestone:** `R0 — Rendering Correctness`

**Current task:** `R0.5 — Texture Filtering / LOD / Mipmapping`, physical-PSP
Dream Land canopy comparison.

**Status:** `VERIFYING`

**Remaining for this item:** compare Dream Land canopy on physical PSP against
the documented PPSSPP/reference evidence. Physical hardware remains
unavailable and this verification is deferred by explicit user direction.

**Dependencies:** software investigation complete through RE-200. R1's full
acceptance checklist is checked, but R1 cannot be declared complete before
R0.5 resolves.

**Relevant files:** `PLAN.md` R0.5; `docs/reverse-engineering.md` RE-053,
RE-070, RE-124, RE-127, RE-128; `docs/visual-regression.md`; Dream Land
golden and physical capture procedure.

**First checks:** confirm whether physical PSP hardware is available. If not,
no further software-only rendering task is eligible: R2 is blocked by R1 and
R3/combat remain blocked behind R2.

**Acceptance:** `PLAN.md` R0.5's remaining physical-PSP comparison.

**Stop condition:** physical hardware unavailable; do not substitute PPSSPP.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: every acceptance bullet is checked through RE-200. It remains
  `IN_PROGRESS` only because R0.5's prerequisite physical verification is
  deferred. Four golden scenes pass exact PPSSPP software comparisons.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is checked off.
- Framebuffer paths: RE-190–193 cover the exhaustive census, the
  wallpaper-capture mechanism, and a minimal real `SObj` 2D-sprite render
  path (`Gpu::draw_wallpaper_sprite`) that draws the capture back through a
  real GE texture bind, device-verified bounded and correctly dimmed.
  `PLAN.md` R1's "all required framebuffer paths render" acceptance item is
  checked off. Only the real 1P-mode/results-screen G2 trigger remains
  unbuilt — accepted as out of R1 scope, the same split RE-149 already used
  to close R0.13.
- Golden coverage: RE-200's graph census identifies 119 texture-blend, 12
  flat-colour, and 252 classified translucent graph-backed primitives.
  Scene 3 (file 109 graph `0x44C8`) covers texture blend, translucency, and
  clean clamp; scene 4 (file 84 graph `0x2760`) covers flat colour.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-200 — Final four golden-render rows identified and captured**

- Temporary, reverted graph-backed census measured 119 texture-blend, 12
  flat-colour, and 252 classified translucent primitives.
- Added `regression_capture_scene3` for file 109 graph `0x44C8` and
  `regression_capture_scene4` for file 84 graph `0x2760`.
- Added deterministic Stage Sector and Catch Swirl goldens. Each matched
  byte-for-byte at 6 and 30 seconds under PPSSPP software rendering.
- Rechecked scene 2 and Dream Land: 0 differing pixels.
- Updated roadmap, state, visual-regression, reverse-engineering, rendering,
  and porting-status documentation.
- Evidence: `docs/reverse-engineering.md` RE-200.
- Commit: this commit (`test: complete R1 golden render coverage`).

## Verification

Scenes 3 and 4 built clean. Each produced byte-identical PPSSPP software
captures at 6 and 30 seconds and 0 differing pixels. Scene 2 and Dream Land
were rebuilt and still match their goldens exactly. `cargo fmt --check`
passed on `psp`; final plain PSP build restored normal EBOOT. No host-side
crate or asset conversion changed, so workspace tests and pack rebuild were
not required.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–200.
- Rendering methodology: `docs/visual-regression.md`.
- Permanent decisions: `DECISIONS.md`.

## Blockers and caveats

- Physical PSP validation is incomplete; PPSSPP does not prove hardware
  correctness.
- Combat is prohibited until rendering gate passes.
- ROM-derived generated assets remain uncommitted; rebuild pack when asset
  pipeline code changes.

## State update contract

Keep this file as current snapshot, not append-only journal. Update only
current task/status, last completed task, next task, blockers, changes,
verification, evidence, documentation and commit. Put detailed investigations
in `docs/reverse-engineering.md`; keep PLAN acceptance entries as short
evidence links. Older session detail remains available through git history.

## Continuation command

For `Continue with the plan`: read `AGENTS.md`, this file, relevant `PLAN.md`
section, relevant `docs/porting-status.md` row and `RE-*` evidence entry; then
inspect git state/recent commits, resume this task or select first eligible
TODO, implement, verify, document, update this snapshot and commit focused
work.
