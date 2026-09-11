# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T7a — Fix mask-narrowed clamp-without-mirror texgen addressing
  divergence` (next up; not started)
- Status: `TODO`
- Last complete: `RE-231` (2026-09-11), `R2.1/T7 -- texgen addressing phase`
  (status `MEASURED`, not `COMPLETE` -- a real gap was found and opened as
  `T7a` rather than fixed inline, matching `R2.0`/P0b's own
  "measure, then open a follow-up" precedent). Consumed `R2.0`/P0b's
  `n64_addressing` hardware/PSP-lowering reference model rather than
  building a second one. Added `tools/romtool`'s `texgen_materials_by_mode`
  census (the real `(mode, scale, tile)` triple as it co-occurs at a draw,
  joining what T6 kept as two separate maps) and two new host tests:
  `texgen_addressing_census_against_real_archive_materials` sweeps every
  `i8`-quantized dot product through `regular_texgen_uv`/`linear_texgen_uv`
  for every real texgen tile and compares the addressed texel against
  `address_axis`;
  `texgen_addressing_reference_cases_for_material_combinations_not_seen_in_the_real_archive`
  adds the plan's own requested zero/nonzero-origin, repeat+mask,
  mirror+repeat, mirror+clamp, padded-PSP-dimension and
  partial-uploaded-scale reference cases synthetically, since real texgen
  content never combines mask with a clear clamp bit. **Measured,
  archive-wide, real ROM** (17 real `(mode, scale, tile)` pairings, 34 axis
  instances, every one clamped on both axes per RE-230): 25 of 34 agree with
  the hardware model across the full quantized dot sweep. 9 of 34 diverge,
  always at exactly the sweep's `dot = +1` extreme and nowhere else -- a
  mask-narrowed (`period << drawn`, real `RE-044` narrowing) clamp-without-
  mirror `Regular`-mode axis wraps to the next period's start on real
  hardware at that boundary, but the current PSP-lowering model (and very
  likely the real GE `Clamp` wrap mode `meshdraw::bind_texture` installs)
  instead holds at the narrowed period's own last texel. This is very
  likely a real PSP rendering divergence, not only a host-model gap:
  `psp_lowering_axis` is host-only and has no other real caller to have
  exercised this branch before. Pinned as a regression baseline (`9`, not
  `0`) with a second assertion that no divergence reaches beyond the
  `dot = +1` extreme. No renderer/pack code was touched (measurement only,
  entirely in `tools/romtool`), so no PPSSPP/physical capture was needed and
  no golden changed.
- Next: `R2.1`/T7a -- fix the divergence RE-231 found. Real hardware wraps a
  clamp-without-mirror axis periodically (via mask) all the way to the
  tile's drawn-rect far edge, only clamping once that far edge is reached --
  not at the narrowed mask period's own edge. The fix needs to reach the
  actual rendering path (`meshdraw::bind_texture`'s wrap-mode selection
  and/or a pack-time texture bake that tiles the mask period across the
  drawn rect before binding), not only `n64_addressing::psp_lowering_axis`'s
  host-side comparison model. Applies to both `Regular` (GE texture-matrix,
  live per-frame coordinate -- cannot be pre-baked the way authored UVs are)
  and `Linear` (CPU per-vertex, shares the authored-UV binding path) texgen,
  since both bind through the same `t.wrap`/tile machinery. Re-run
  `texgen_addressing_census_against_real_archive_materials` after the fix
  and drop its pinned baseline back to `0`; confirm no other real texgen
  tile shape regresses. Consider whether a PPSSPP/physical capture of the
  affected `StageMetalFile2`-family materials at a reflection-aligned camera
  angle is needed to see the one-texel difference in practice.
- Blockers: none for starting T7a. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T7a. `R2.2`/C1-C7 renderer corrective gate remains
  behind all of `R2.1`. Combat remains gated behind `R2.2`.
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
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-231.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2/T3/T4/T5/T6 complete, T7 measured (opens T7a), T7a next).
- Decisions: `DECISIONS.md` -- no new revision this task (this task was a
  measurement against already-decided semantics, not a decision premise
  change; the divergence it found is a rendering-fidelity gap for T7a to
  close, not a semantics question).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing row updated to
  record RE-231's addressing agreement/divergence measurement.
- Verification: 2 new host tests
  (`tools/romtool/src/main.rs::tests::texgen_addressing_census_against_real_archive_materials`,
  `..::texgen_addressing_reference_cases_for_material_combinations_not_seen_in_the_real_archive`,
  the first `SSB64_ROM`-gated); full `cargo test --workspace --all-targets`
  (pinned 1.98.0 toolchain, `SSB64_ROM` set) -- passing (576 prior + 2 new =
  578, 0 failed); `cargo clippy --all-targets -D warnings` (pinned 1.98.0)
  clean on the touched crate; `rustfmt --check` clean on the touched file;
  `cargo psp --release` (default features) builds clean; no goldens touched
  (no renderer/pack code changed this task).
- Documentation: RE-231, `PLAN.md` `R2.1`/T7 and new T7a section,
  `docs/porting-status.md`, this snapshot.
- Commit: pending.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
