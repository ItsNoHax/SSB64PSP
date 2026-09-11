# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T5 — Linear integer conversion` (next up; not started)
- Status: `TODO`
- Last complete: `RE-228` (2026-09-11), `R2.1/T4 -- shared regular/linear
  reference math`. Built host-testable helpers proving the regular-texgen
  GE lowering matches an independent source-formula reference, and in doing
  so found a real bug.
  **Added** `ssb_rom::psp_texture::regular_texgen_curve` (`(dot+1)/4`) and
  `regular_texgen_uv`, sharing `texgen_dot` and a new
  `texgen_s10_5_addressed` scale-and-addressing step with the existing
  `linear_texgen_uv` -- "the only curve difference... common scale and
  addressing," `PLAN.md`'s own wording. Added `regular_texgen_matrix_coeffs`
  (pulled out of `apply_texture_mapping`'s inline arithmetic) and
  `ssb_engine::math::transform_lookat_basis` (pulled out of
  `texgen_object_basis`'s inline closure), both host-testable; the real
  rendering path now calls this same shared code rather than a private copy.
  **Found and fixed a real bug** via a 20,000-case random property test
  (`regular_texgen_matrix_lowering_matches_the_reference_curve`): the
  shipped texture-matrix translation constant `b` wrongly carried the same
  `128/127` `NORMAL_SCALE_COMPENSATION` the dot-term coefficient `a` needs
  (`a` *is* read through the GE's measured `/128` normal divisor, RE-226;
  `b` -- the curve's zero-crossing constant plus the tile-origin shift --
  is not), overcorrecting by up to `scale/127 - scale/128` S10.5 units
  (hundreds to thousands of units across random cases, pinned by a
  dedicated regression test, `uncorrected_b_coefficient_measurably_
  overcorrects`). Fixed in `regular_texgen_matrix_coeffs`; residual error
  after the fix measures ~1.78 S10.5 units max (< 0.06 texels), traced to
  a separate, documented, *unfixed* effect: an i8-quantized normal is only
  approximately unit length, which can push `dot` a hair past +-1 that the
  reference formula clamps but the GE's real affine matrix does not.
  **Changed real rendered output**: rebuilt and updated two of three texgen
  goldens (`tests/golden/r2-metal-texgen{,-rotated}.png`, 28,240 / 23,624
  differing pixels against the pre-fix goldens, reconfirmed deterministic).
  `regression_capture_scene13` (the one linear-texgen primitive, drawn
  through the untouched CPU path) measured 0 differing pixels, correctly
  unaffected since this fix is scoped to the regular/environment matrix path
  only.
- Next: `R2.1`/T5 -- linear integer conversion. Determine truncate/round/
  other from microcode, faithful HLE implementations and controlled
  original-ROM output, in that order. Add `N+0.49`, `N+0.50` and `N+0.51`
  boundary tests at real scales. Use "source-formula exact" until original
  output proves "bit-exact to N64". Read `PLAN.md`'s full `R2.1` section
  (T1-T10) before starting; T6-T10 remain queued behind it in order.
- Blockers: none for starting T5. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T5. `R2.2`/C1-C7 renderer corrective gate remains behind
  all of `R2.1`. Combat remains gated behind `R2.2`.
  Separately (not blocking): a real bug was found and flagged (not fixed)
  in the `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture. See the spawned follow-up task
  (`task_1bf9bc35`). Also open, non-blocking: a before/after PPSSPP TEXVIEW
  screenshot confirming RE-224's CI4 palette-bank fix (global texture
  indices 187/194/195, object indices 60-102 in file 86) was never obtained
  -- manual follow-up for whoever next has hands on the interactive build.
  Also open, non-blocking, low-confidence: RE-228's residual ~1.78-S10.5-unit
  clamp-boundary deviation is measured but not checked against real `sceGu`
  calls the way RE-226's normal-semantics question was -- minor lead for a
  future task, not currently assigned.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-228.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2/T3/T4 complete, T5 next).
- Decisions: `DECISIONS.md` -- no new revision this task (D-038/D-040 already
  cover the semantics RE-228 builds on).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing; `docs/rendering.md`
  -- texgen rows now reflect the corrected regular-texgen matrix constant
  (renderer behavior changed: `regression_capture_scene11/12` goldens
  updated, `_13` confirmed unaffected).
- Verification: 6 new host tests (`crates/ssb-rom/src/psp_texture.rs`,
  `crates/ssb-engine/src/math.rs`); full `cargo test --workspace
  --all-targets` (pinned 1.98.0 toolchain, `SSB64_ROM` set) -- 574 passing,
  0 failed (568 prior + 6 new); `cargo fmt --check` clean on touched files
  (pre-existing drift in untouched files, e.g. `n64_addressing.rs`, left
  alone -- out of scope); `cargo psp --release` (default features) builds
  clean; goldens rebuilt and re-measured for scenes 11/12, confirmed
  unaffected for scene 13, not assumed either way; `psp/`'s own `clippy` is
  not part of this project's gate (native clippy cannot cross-compile to
  `mipsel-sony-psp`).
- Documentation: RE-228, `PLAN.md` `R2.1`/T4, this snapshot.
- Commit: (pending).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
