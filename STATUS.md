# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T8 — Original-ROM Metal comparison` (next up; not started)
- Status: `TODO`
- Last complete: `RE-232` (2026-09-11), `R2.1/T7a -- fix mask-narrowed
  clamp-without-mirror texgen addressing divergence` (status `COMPLETE`).
  RE-231 (T7) had measured 9 of 34 real texgen axis instances diverging from
  the hardware addressing model at exactly the sweep's `dot = +1` extreme,
  always on a `Regular`-mode axis whose mask genuinely narrows below the
  tile's drawn rect, and pinned it as a regression baseline pending a fix.
  Root cause: `n64_addressing::psp_lowering_axis`'s `!mirror` clamp branch
  clamped the raw texel index to `period - 1` (the narrowed mask period's
  own last texel) directly, instead of clamping to the drawn rect's real
  far edge (`drawn - 1`) and only then folding through the mask period the
  way `address_axis` (the hardware model) does. `texture::mirror_extend`'s
  matching real bake had the identical bug: `mirror_axis_len` baked only
  `period` texels for every non-mirror axis regardless of the clamp bit, so
  `sceGuTexWrap(Clamp)` in `meshdraw::bind_texture` held at the same
  one-period-early edge on real hardware -- confirming RE-231's
  medium-confidence hypothesis that this was a real PSP rendering
  divergence, not only a host-model gap. Fixed both: `psp_lowering_axis`'s
  `!mirror` clamp branch now clamps to `drawn - 1` before folding by the
  mask period (a no-op when the mask does not narrow below the drawn rect,
  the common case), and `mirror_axis_len` now bakes to `drawn` for any
  clamped axis, mirrored or not, with `mirror_fold` generalized to always
  wrap by `% period` so the wider non-mirror bake still reads valid source
  texels. Mirrors the mirror+clamp fix RE-220/RE-221 already made for the
  mirrored case; applies uniformly to `Regular` and `Linear` texgen since
  the fix lives in the bound texture/wrap state, not the UV source.
  Re-running `texgen_addressing_census_against_real_archive_materials`
  against the real ROM measured the divergence at a strict `0`/34 (down
  from `9`/34), and the test's pinned baseline was dropped to `0`
  accordingly; no other real texgen or authored-UV tile shape regressed.
  `assets/generated/ssb64.pak` rebuilt (gitignored, not committed) since
  this changed asset-pipeline code (`texture::mirror_extend`). No new
  `DECISIONS.md` entry: this was a correctness fix to already-decided
  addressing semantics (D-038), not a new architectural decision, matching
  RE-220/RE-221's own precedent.
- Next: `R2.1`/T8 -- original-ROM Metal comparison. Use the rebuilt
  Mupen64Plus harness to reach legitimate stage-8 Metal content (prefer a
  faithful RAM-level warp that still executes original fighter
  construction, camera, display-list, lighting and material setup; never
  fake registers/material state). Compare original N64/emulator, PPSSPP
  software and physical PSP at equivalent states and at least two object
  rotations, checking model/camera reflection response, diffuse-light
  independence, ordinary-versus-linear behavior, tile-origin phase and
  generated span using a texgen-focused ROI. This is the first `R2.1` task
  since T7/T7a to need an actual rendered comparison (T7a itself changed no
  golden -- it fixed a bake bug proven correct by exhaustive archive
  measurement, not by a new capture) -- consider whether it should also
  cover the `StageMetalFile2`-family materials RE-231/RE-232's fix targeted
  at a reflection-aligned camera angle, to see the corrected texel in
  practice.
- Blockers: none for starting T8. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker. `R2.2`/C1-C7 renderer corrective gate remains behind all of
  `R2.1`. Combat remains gated behind `R2.2`.
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
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-232.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1
  measured, T2/T3/T4/T5/T6/T7a complete, T7 measured (T7a closed it), T8
  next).
- Decisions: `DECISIONS.md` -- no new revision this task (correctness fix
  to already-decided addressing semantics, not a decision premise change).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing row updated to
  record RE-232's fix and re-measurement.
- Verification: 3 new host tests
  (`crates/ssb-rom/src/n64_addressing.rs::tests::psp_lowering_no_longer_diverges_from_hardware_for_clamp_without_mirror_past_the_first_period`,
  `crates/ssb-rom/src/texture.rs::tests::mirror_extend_with_clamp_and_no_mirror_bakes_every_period_the_drawn_rect_spans`,
  `..::mirror_extend_with_clamp_and_no_mirror_and_no_narrowing_is_a_plain_copy`);
  updated `tools/romtool/src/main.rs::tests::texgen_addressing_census_against_real_archive_materials`
  baseline from `9` to `0`; full `cargo test --workspace --all-targets`
  (pinned 1.98.0 toolchain, `SSB64_ROM` set) -- passing (578 prior + 3 new =
  581, 0 failed); `cargo clippy --all-targets -D warnings` (pinned 1.98.0)
  clean; `rustfmt --check` clean; `cargo psp --release` (default features)
  builds clean; `romtool pack` re-run against the real ROM to rebuild
  `assets/generated/ssb64.pak` (gitignored, not committed) since
  asset-pipeline code changed. No PPSSPP/physical capture taken -- the
  archive-wide census (exhaustive over every real texgen tile) already
  confirms the fix, a stronger check than a single visual angle; T8 is the
  next task actually needing a rendered comparison.
- Documentation: RE-232, `PLAN.md` `R2.1`/T7a marked `COMPLETE`,
  `docs/porting-status.md`, this snapshot.
- Commit: (uncommitted at snapshot time).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
