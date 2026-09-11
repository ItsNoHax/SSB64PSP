# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T4 — Shared regular/linear reference math` (next up; not started)
- Status: `TODO`
- Last complete: `RE-227` (2026-09-11), `R2.1/T3 -- original LookAt
  quantization`. The decomp's `syMatrixLookAtReflectF`
  (`refs/ssb-decomp-re/src/sys/matrix.c:337-342`) quantizes the camera's
  `right`/`up` basis to signed bytes via `FTOFRAC8`
  (`refs/ssb-decomp-re/include/PR/gu.h:37`) once per camera, strictly before
  any per-object model transform runs -- not the continuous per-frame float
  this project's texgen path previously fed `DrawState::texgen_object_basis`.
  `refs/BattleShip`'s `Interpreter::CalculateNormalDir` confirmed the
  consuming order: dequantize by `/127` first, then transform, then
  normalize.
  **Added host-testable helpers** to `crates/ssb-engine/src/math.rs`:
  `ftofrac8` (bit-exact macro port -- positive saturation at 127, asymmetric
  negative range down to -128, documented negative-overflow wraparound
  beyond any real basis component), `quantize_lookat_component` (quantize
  then `/127` reconstruct -- exact for `0.0`/`+1.0`, inexact for `-1.0`:
  `-128/127 ~= -1.00787`), `quantize_lookat_basis`. 8 new host tests cover
  zero, saturation, the asymmetric range, the ±1/128 quantum boundary
  (truncation not rounding), exact/inexact round-trips, and a realistic 45
  degree basis angle where quantized and full-float measurably diverge.
  **Fixed `meshdraw::DrawState::texgen_object_basis`**: now calls
  `quantize_lookat_basis` on `right`/`up` before the existing `M^T v`
  model-transform step, matching source order. Both texgen paths (regular
  GE texture-matrix in `apply_texture_mapping`, linear CPU-generated in
  `draw_mesh`) call this one method, so both receive the identical
  quantized-then-transformed basis automatically -- no separate wiring.
  Revises D-040 (its LUT-rejection premise named the basis "continuous
  float"; the rejection's conclusion is unaffected).
  **No goldens needed rebuilding**: every current texgen regression scene
  (`regression_capture_scene11/12/13`) uses the identity (camera-less)
  basis `([1,0,0], [0,1,0])`, whose only components (`0.0`, `+1.0`) round-trip
  exactly, so quantizing it is a no-op. Measured, not assumed: re-captured
  scenes 11 and 12 through `tools/run-ppsspp-headless.sh` and diffed against
  their existing goldens -- 0 differing pixels both. The fix is currently
  dormant pixel-wise; it activates once a rotated real-camera basis reaches
  a texgen primitive, which no current scene exercises.
- Next: `R2.1`/T4 -- shared regular/linear reference math. Create readable
  host-testable helpers for quantized LookAt, transformed basis, raw
  signed-byte dot and S10.5 conversion. Prove the regular GE lowering
  matches the reference across thousands of random normals/bases/
  rotations/scales; require exact S10.5 equality where possible, else
  document max error. Read `PLAN.md`'s full `R2.1` section (T1-T10) before
  starting; T5-T10 remain queued behind it in order.
- Blockers: none for starting T4. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T4, which is a prerequisite for fixing it, not blocked
  by it. `R2.2`/C1-C7 renderer corrective gate remains behind all of
  `R2.1`. Combat remains gated behind `R2.2`.
  Separately (not blocking): a real bug was found and flagged (not fixed)
  in the `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture. See the spawned follow-up task
  (`task_1bf9bc35`). Also open, non-blocking: a before/after PPSSPP TEXVIEW
  screenshot confirming RE-224's CI4 palette-bank fix (global texture
  indices 187/194/195, object indices 60-102 in file 86) was never obtained
  -- manual follow-up for whoever next has hands on the interactive build.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-227.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2/T3 complete, T4 next).
- Decisions: `DECISIONS.md` D-040 revised by RE-227's measurement (LookAt
  basis is signed-byte quantized, not continuous float; LUT-rejection
  conclusion unaffected).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing; `docs/rendering.md`
  -- texgen rows unchanged in substance (identity-basis scenes measured
  bit-identical; no renderer-visible change yet for any current scene).
- Verification: 8 new host tests in `crates/ssb-engine/src/math.rs`; full
  `cargo test --workspace --all-targets` (pinned 1.98.0 toolchain,
  `SSB64_ROM` set) -- 568 passing, 0 failed (560 prior + 8 new); `cargo fmt
  --check` clean on touched files in both the host workspace and `psp/`;
  `cargo psp --release` (default features) builds clean; re-captured
  `regression_capture_scene11`/`_12` via `tools/run-ppsspp-headless.sh`,
  diffed against existing goldens -- 0 differing pixels both, no golden
  rebuild needed; `psp/`'s own `clippy` is not part of this project's gate
  (native clippy cannot cross-compile to `mipsel-sony-psp`).
- Documentation: RE-227, `PLAN.md` `R2.1`/T3, `DECISIONS.md` D-040, this
  snapshot.
- Commit: `e22cb86`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
