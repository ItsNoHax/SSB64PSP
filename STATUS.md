# Project Status

**Last updated:** 2026-09-10 (RE-201 physical-PSP session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** `R0.5 — Texture Filtering / LOD / Mipmapping` (RE-201).
PSPLink direct framebuffer capture on PSP Slim reproduces the Dream Land
canopy composition; prior hardware FPU faults are fixed.

**Dependencies:** R0.5 and R1 complete. R2 hardware checklist remains.

**Relevant files:** `PLAN.md` R0.5; `docs/reverse-engineering.md` RE-053,
RE-070, RE-124, RE-127, RE-128; `docs/visual-regression.md`; Dream Land
golden and physical capture procedure.

**First checks:** confirm whether physical PSP hardware is available. If not,
no further software-only rendering task is eligible: R2 is blocked by R1 and
R3/combat remain blocked behind R2.

**Acceptance:** `PLAN.md` R2.

**Next:** capture representative fighter/stage/animation/material/texture
paths and record VRAM/environment evidence on this same PSP.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; Dream Land boots, pack loads, stage/material/textures
  render, and no hardware exception remains in the captured regression path.
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

**RE-201 — PSPLink physical validation and FPU-trap fixes**

- Installed PSPLink v3.2.1; connected PSP Slim on firmware 6.61 through
  USBHostFS and captured its native framebuffer.
- Fixed speculative zero-duration divisions in material and joint animation,
  plus invalid scalar math on packed extended RGBA material tracks.
- Dream Land runs stably, with no PSPLink exceptions; direct capture matches
  the documented canopy composition qualitatively.
- Evidence: `docs/reverse-engineering.md` RE-201.

## Verification

Focused material and figatree tests: 22 passed. `cargo fmt --check` passed.
PSP `regression_capture` build ran on PSP Slim without exceptions; direct
framebuffer screenshot and sustained thread run-clock evidence recorded in
RE-201. Pack was unchanged.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–201.
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
