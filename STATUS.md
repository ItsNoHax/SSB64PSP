# Project Status

**Last updated:** 2026-09-10 (RE-206 division-sweep session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** RE-206. Performed the systematic sweep the previous
session flagged: confirmed by exhaustive grep that no fourth raw
`1.0 / payload`-shaped division exists outside the three existing
`reciprocal_or_one` guards (`figatree.rs`/`matanim.rs`/`objanim.rs`). Then
checked every division in on-device runtime code (not `romtool`'s host-side
pack conversion, which cannot hit this fault class) for the same
"guard-then-divide" shape: `ssb-engine::math::Vec3::normalized`,
`::timing::FrameTimings::fps`, `ssb-game::camera::original_tan`,
`ssb-game::status::update_walk` and `ssb-game::collision::check_tilt`.
`check_tilt`'s unguarded `/scale` looked like the strongest candidate (no
guard at all, unlike the others), so investigated it in depth: added, then
reverted, a defensive guard and direct unit test after proving — first
analytically (a segment's height is linear in `x`, so an exactly-parallel
movement keeps an invariant surface offset and can never also satisfy the
function's own "started above, ended below" gate), then empirically (a
scratch probe swept thousands of `(segment, from, to)` combinations,
including i16-range magnitudes chosen to maximise float rounding) — that
`scale == 0.0` is unreachable through this function's own preceding checks.
Confirmed the guard was inert by disabling it and observing the added test
still passed (exercising the pre-existing gate, not the new code), then
reverted both per this project's own evidence-driven rule. No source change
this session; `cargo test --workspace` unchanged at 506 passing. Full
findings and the specific candidates left open (no demonstrated
zero-denominator input yet, so not fixed) are in
`docs/reverse-engineering.md` RE-206.

**Dependencies:** R0.5 and R1 complete. R2's one remaining hardware checklist
row is live analog-stick input (needs a human operator); "no hardware-only
rendering failures remain" stays open pending broader coverage.

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–206;
`docs/psplink.md`; `docs/visual-regression.md`; `tests/golden/*.png`;
`tools/compare-screenshot.sh`; `crates/ssb-game/src/collision.rs`;
`crates/ssb-game/src/camera.rs`; `crates/ssb-game/src/status.rs`;
`crates/ssb-engine/src/math.rs`; `crates/ssb-engine/src/timing.rs`.

**First checks:** physical PSP hardware is present and PSPLink-reachable
this session (`lsusb` shows `054c:01c9`, `pspsh -e ver` → `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` is running). Re-check this at the start of the
next session; if hardware is no longer attached, R2 has no further eligible
software-only task.

**Acceptance:** `PLAN.md` R2.

**Next:** the crate-wide division sweep RE-206 asked for is now done and
clean. The remaining path toward "no hardware-only rendering failures
remain" is broader *scene* coverage, not more static code sweeping: every
fighter, stage and effect the existing scene-finding examples
(`stage_animation_amplitude.rs` and similar) have not yet touched is still
an unaudited risk on real hardware — building and PSPLink-testing new golden
scenes for untouched fighters/stages is the concrete next step. Separately,
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

**RE-206 — Swept the crate for other unguarded/speculatable divisions like RE-201/202/205's FPU traps**

- Grepped the whole crate for the literal `1.0 / payload` shape: only the
  three already-guarded call sites exist (`figatree.rs`, `matanim.rs`,
  `objanim.rs`); no fourth instance.
- Audited every division in on-device runtime code (excluding `romtool`'s
  host-side pack conversion, which cannot hit this fault class) for the
  same "guard checks zero, then divides" shape: `Vec3::normalized`,
  `FrameTimings::fps`, `camera::original_tan`, `status::update_walk`,
  `collision::check_tilt`.
- `check_tilt`'s unguarded `/scale` (no zero check at all, unlike the
  others) looked like the strongest candidate. Investigated it directly:
  proved analytically that its own preceding "started above, ended below"
  gate cannot coexist with `scale == 0.0` (a segment's height is linear in
  `x`, so an exactly-parallel movement keeps an invariant surface offset),
  then confirmed empirically with a scratch brute-force probe across
  thousands of `(segment, from, to)` combinations, including i16-range
  magnitudes chosen to maximise float rounding. No counterexample found.
- Added a defensive guard plus a direct unit test on the private
  `check_tilt`, then disabled the guard and re-ran the test: it still
  passed, proving the test exercised the pre-existing gate, not the new
  code. Reverted both — an inert guard and an unfalsifiable test are not
  evidence-driven work.
- No source change. `cargo test --workspace` unchanged at 506 passing.
- Evidence: `docs/reverse-engineering.md` RE-206.

## Verification

RE-206 was a code-and-analysis investigation, not a hardware session:
`cargo test --workspace` (506 passing, unchanged) confirms no regression
from the revert. The `check_tilt` reachability claim was verified two ways
(analytical proof + brute-force numeric search over representative and
extreme-magnitude inputs, both described in RE-206) rather than asserted.
No PPSSPP or physical-PSP capture was needed since no source changed.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–206.
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
