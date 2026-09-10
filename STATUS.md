# Project Status

**Last updated:** 2026-09-10 (RE-203 physical-PSP session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** RE-203. Ran all four golden regression-capture scenes
(`regression_capture`, `_scene2`, `_scene3`, `_scene4`) on the same physical
PSP used by RE-201/RE-202: zero exceptions across all four, `main_thread`
alive throughout. Compared each hardware capture (2x nearest-upscaled)
against its existing PPSSPP golden in `tests/golden/`: interiors pixel-
identical, differences confined to expected polygon-edge antialiasing plus
two unrelated known overlays (PSPLink's own corner text, PPSSPP's FPS
counter). Checked off 8 of 12 `PLAN.md` R2 acceptance items on this basis.

**Dependencies:** R0.5 and R1 complete. R2 hardware checklist remains
(framebuffer effects, VRAM measurement, exhaustive no-failures-remain
coverage, stage animation, live analog-stick input).

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201,
RE-202, RE-203; `docs/psplink.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`.

**First checks:** physical PSP hardware is present and PSPLink-reachable
this session (`lsusb` shows `054c:01c9`, `pspsh -e ver` → `PSPLink v3.2.1`).
Re-check this at the start of the next session; if hardware is no longer
attached, R2 has no further eligible software-only task.

**Acceptance:** `PLAN.md` R2.

**Next:** capture framebuffer-effect content and VRAM usage on this same
PSP (no golden-scene feature currently isolates a framebuffer-effect-heavy
graph — one may need to be added, following RE-199/RE-200's pattern).
Broaden hardware coverage beyond the four golden scenes toward "no
hardware-only rendering failures remain." Separately, still open: whether
the analog nub correctly drives the fighter now that RE-202's HUD-crash fix
is live — this requires a human physically operating the device with
PSPLink attached; `pspsh` has no controller-injection command, so an agent
session cannot resolve it alone.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; all four golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`) boot, pack loads,
  stage/fighter/material/texture content matches PPSSPP goldens, and no
  hardware exception remains across any of them (RE-203). Framebuffer
  effects, VRAM measurement, and live analog-stick input remain untested
  on hardware.
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

**RE-203 — All four golden regression scenes verified on physical PSP hardware**

- Built and `ldstart`ed `regression_capture`, `_scene2`, `_scene3`, and
  `_scene4` in turn on the same PSPLink session as RE-201/RE-202. Zero
  exceptions across all four; `main_thread` alive in every case.
- Independently re-verified `regression_capture`'s determinism claim on real
  hardware (not just PPSSPP): two captures 5 s apart differ only inside
  PSPLink's own documented corner overlay.
- Compared each hardware capture (2x nearest-upscaled) against its existing
  PPSSPP golden in `tests/golden/`: interiors pixel-identical; only
  difference is expected polygon-edge antialiasing plus two unrelated known
  overlays (PSPLink corner text, PPSSPP's own FPS counter).
- Checked off 8 of 12 `PLAN.md` R2 acceptance items on this evidence:
  boot, pack load, representative fighter/stage render, fighter animation,
  materials, textures, hardware model, build/environment.
- Rebuilt and redeployed the plain (no-feature) interactive build afterward
  per `docs/psplink.md`, verified stable (no exception, `main_thread` alive).
- Evidence: `docs/reverse-engineering.md` RE-203.

## Verification

All four `regression_capture*` builds ran on PSP Slim (firmware 6.61,
ARK/Infinity, PSPLink v3.2.1) with `exlist` empty and `main_thread` alive
throughout. Native `scrshot` captures compared against `tests/golden/*.png`
via a 2x nearest upscale + pixel diff (not just eyeballed) — see RE-203 for
the exact diff-mask methodology and its conclusion. Native captures taken,
inspected, and discarded per `docs/psplink.md` (never committed); SHA-256
hashes recorded in RE-203. Pack unchanged (hash
`7647db75...650b2f0`); no Rust source changed this session, so
`cargo test --workspace` was not required.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–203.
- Rendering methodology: `docs/visual-regression.md`.
- Hardware crash workflow: `docs/psplink.md`.
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
