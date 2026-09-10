# Project Status

**Last updated:** 2026-09-10 (RE-205 physical-PSP session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** RE-205. Built a new persistent example,
`crates/ssb-rom/examples/stage_animation_amplitude.rs`, to find a stage with
visibly animated geometry (no existing golden scene had one); chose stage 9
(Saffron City) over the single largest, uncorroborated amplitude because
RE-142/RE-143 already independently proved its gate moves. Added
`regression_capture_scene5` (stage 9, default `stage_view`, same tick-240
freeze). Confirmed the animated pose differs from rest in PPSSPP (1,780px,
temporary reverted control build) before trusting it. Loading it on physical
PSP hardware crashed with `FPU Exception (IUZ)` in `objanim.rs`'s
`StageJoint::apply` — a third `1.0 / payload` speculative-division trap,
the same class `f111892` already fixed in `figatree.rs`/`matanim.rs` but
never ported to stage animation's structurally identical interpreter. Fixed
with the same `reciprocal_or_one` guard and a new regression test
(`cargo test --workspace`: 506 passing). Rebuilt, redeployed: zero
exceptions, native hardware capture matches
`tests/golden/r2-saffron-city-gate.png` (upscaled 2x) with only the
expected edge-antialiasing/overlay divergence RE-203 already established as
acceptable.
Checked off `PLAN.md` R2's "stage animation works" row (11 of 12 now
checked). Redeployed the plain interactive build afterward, verified stable.

**Dependencies:** R0.5 and R1 complete. R2's one remaining hardware checklist
row is live analog-stick input (needs a human operator); "no hardware-only
rendering failures remain" stays open pending broader coverage.

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–205;
`docs/psplink.md`; `docs/visual-regression.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`; `crates/ssb-rom/src/objanim.rs`;
`crates/ssb-rom/examples/stage_animation_amplitude.rs`.

**First checks:** physical PSP hardware is present and PSPLink-reachable
this session (`lsusb` shows `054c:01c9`, `pspsh -e ver` → `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` is running). Re-check this at the start of the
next session; if hardware is no longer attached, R2 has no further eligible
software-only task.

**Acceptance:** `PLAN.md` R2.

**Next:** broaden hardware coverage beyond the five golden scenes toward "no
hardware-only rendering failures remain" — every fighter, stage and effect
this scene-finding method has not yet touched is still an unaudited risk for
the same class of FPU-trap bug RE-205 just found a third instance of; a
systematic `grep -n '/ payload'`-style sweep across the crate for the same
unguarded-division shape is reasonable next work, not yet done. Separately,
still open: whether the analog nub correctly drives the fighter now that
RE-202's HUD-crash fix is live — this requires a human physically operating
the device with PSPLink attached; `pspsh` has no controller-injection
command, so an agent session cannot resolve it alone.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; all five golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`, Saffron City stage
  animation) boot, pack loads, stage/fighter/material/texture content
  matches PPSSPP goldens, and no hardware exception remains across any of
  them (RE-203, RE-205). The real framebuffer-effect `SObj` sprite path,
  VRAM usage, and stage animation are now hardware-verified too (RE-204,
  RE-205). Exhaustive no-failures-remain coverage and live analog-stick
  input remain untested on hardware.
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

**RE-205 — Stage animation verified on physical PSP hardware; a third FPU-trap site found and fixed**

- Added `crates/ssb-rom/examples/stage_animation_amplitude.rs` to rank
  animated stage nodes by amplitude; chose stage 9 (Saffron City) over the
  single largest, uncorroborated amplitude because RE-142/RE-143 already
  independently proved its gate moves.
- Added `regression_capture_scene5` (stage 9, default `stage_view`, tick-240
  freeze). Confirmed the animated pose differs from rest (1,780px PPSSPP
  diff, temporary reverted control build) before trusting it.
- Physical PSP hardware load crashed: `FPU Exception (IUZ)` in
  `objanim.rs::StageJoint::apply`, a third `1.0 / payload` speculative-
  division trap — the same class `f111892` already fixed in
  `figatree.rs`/`matanim.rs`, never ported to stage animation's own,
  structurally identical interpreter. Fixed with the same
  `reciprocal_or_one` guard; added a regression test
  (`cargo test --workspace`: 506 passing).
- Rebuilt, redeployed: zero exceptions, `main_thread` alive; native capture
  matches `tests/golden/r2-saffron-city-gate.png` (upscaled 2x) with only
  the expected edge-antialiasing/overlay divergence RE-203 already
  established as acceptable.
- Checked off `PLAN.md` R2's "stage animation works" row (11 of 12 now
  checked).
- Rebuilt and redeployed the plain (no-feature) interactive build afterward
  per `docs/psplink.md`, verified stable (no exception, `main_thread` alive).
- Evidence: `docs/reverse-engineering.md` RE-205.

## Verification

The `regression_capture_scene5` build ran on PSP Slim (firmware 6.61,
ARK/Infinity, PSPLink v3.2.1) with `exlist` empty and `main_thread` alive
throughout, after the FPU-trap fix. Two native `scrshot` captures 3 s apart
differ by 101 pixels, entirely inside PSPLink's own documented corner
overlay — otherwise byte-identical. The capture (upscaled 2x
nearest-neighbour) was diffed against `tests/golden/r2-saffron-city-gate.png`
(itself confirmed byte-identical run-to-run in PPSSPP before use): large
interior regions pixel-identical, only the expected antialiasing/overlay
divergence RE-203 already established as acceptable for the other four
scenes. `cargo test --workspace`: 506 passing (2 romtool, 36 engine, 118
game, 350 ROM). Native BMP captures reviewed and discarded per
`docs/psplink.md` (never committed); pack unchanged (hash
`7647db75...650b2f0`). See RE-204 for the prior session's settling-transient
finding and measurement methodology.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–205.
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
