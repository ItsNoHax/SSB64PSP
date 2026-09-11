# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T10 — Texgen documentation cleanup` (in progress)
- Status: `IN_PROGRESS`
- In progress: `RE-238` (2026-09-11) -- `R2.1`/T10 texgen documentation/test
  cleanup, started after T1-T9 (RE-225-237) closed. Read-only survey found
  `docs/rendering.md`, `docs/visual-regression.md` and `DECISIONS.md` already
  current (each already threads RE-225 through RE-237 inline); only
  `docs/porting-status.md` was stale (its texture-conversion row and "Known
  gaps" §1 still read as if T1-T9 and physical-hardware validation were
  open, citing only RE-201-215) -- fixed. Built `romtool texgen ROM
  --verify`: factored the addressing sweep
  `texgen_addressing_census_against_real_archive_materials` already proved
  into a shared `verify_texgen_addressing`, added `load_draw_mode_mismatch`/
  `load_draw_scale_mismatch`/texgen-tile-`shift` checks (RE-225/RE-230's own
  pinned baselines), and wired a `--verify` CLI flag that prints PASS/FAIL
  and returns a non-zero exit on any violation -- ran clean against the real
  ROM (17 real (mode, scale, tile) pairings, 34 axis instances, 0
  divergence, the same RE-232 numbers). Added a synthetic host test
  (`verify_texgen_passes_on_a_clean_census_and_fails_on_each_named_violation`)
  proving each failure branch actually fires. Added the texgen test
  minimum's missing zero-normal case
  (`texgen_dot_of_the_zero_normal_is_zero_for_any_basis`). Surveyed
  LookAt saturation/near-zero and raw GEN/LINEAR mapping-transition coverage
  against the test minimum and found both already adequately covered by
  existing tests (`crates/ssb-engine/src/math.rs`'s `ftofrac8_*` tests;
  `crates/ssb-rom/src/mesh.rs`'s `texture_gen_follows_raw_geometry_mode_bits`).
  Full account in RE-238. Still open for T10: a dedicated
  textured->untextured->texgen mapping-transition test (needs an archive
  check for whether that combination is real before writing one), and the
  rest of T10's own completion-gate bookkeeping.
- Previously in progress: `RE-237` (2026-09-11) -- closed `R2.1`/T9's two
  remaining matrix items on physical PSP hardware (raw normal diagnostic,
  camera-rotation scene), closing `R2.1`/T9.
- Last complete: `RE-236` (2026-09-11) -- physical-PSP leg of T8 captured,
  closing `R2.1`/T8.
- Previously complete: `RE-235` (2026-09-11) -- PPSSPP leg of T8 done.
- Next: finish `R2.1`/T10. Remaining: decide whether a real texgen-bound
  primitive with no bound texture exists in the archive and, if so, add the
  textured->untextured->texgen mapping-transition test; then close out T10's
  documentation/test-minimum gate per `PLAN.md`'s own text (source semantics,
  addressing, host, PPSSPP, physical and original-Metal gates all already
  rest on T1-T9's evidence, `--verify` now covers the addressing/host gate
  directly).
- Blockers: none currently known for T10. `R2.1`/T1's own finding (164
  cross-node differing-transform vertex reuses) is an open, tracked,
  *known* gap -- not a blocker. `R2.2`/C1-C7 renderer corrective gate
  remains behind all of `R2.1`. Combat remains gated behind `R2.2`.
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
  future task, not currently assigned; deliberately not recorded as
  `ACCEPTED_DEVIATION` yet since "unavoidable" is not yet established.
  Also open, non-blocking, low-confidence: a fresh PPSSPP-headless rebuild of
  scene 11 on this machine diffs from the exact committed
  `r2-metal-texgen.png` bytes by 31,737 pixels, even though two fresh
  rebuilds against each other are byte-identical (self-consistent) -- the
  same noise-floor-order pattern this project already treats as expected
  cross-build/cross-environment variance (RE-214/RE-236 precedent), not
  investigated further.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-238.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1
  measured, T2/T3/T4/T5/T6/T7a/T8/T9 complete, T7 measured (T7a closed it),
  T10 in progress).
- Decisions: `DECISIONS.md` -- no new revision for RE-238 (documentation and
  test-coverage work; surveyed D-039/D-042 for staleness against RE-225 and
  confirmed no revision needed, see RE-238).
- Subsystem: `docs/porting-status.md` -- updated this session (RE-238):
  texture-conversion row and "Known gaps" §1 reconciled against T1-T9's
  completion.
- Verification (RE-238): `romtool texgen "<rom>" --verify` -- PASS against
  the real ROM. Host workspace: `cargo test --workspace` -- `ssb-rom` 403
  passed (up from 402), `romtool` 12 passed (up from 11), 0 failed overall.
  `cargo fmt --check` clean in `tools/romtool/` and `crates/ssb-rom/`. Code
  changed: `tools/romtool/src/main.rs` (`verify_texgen_addressing`,
  `verify_texgen`, `--verify` CLI flag, refactored existing test to share
  the extracted sweep, new synthetic verify test),
  `crates/ssb-rom/src/psp_texture.rs` (zero-normal test). No PSP-target or
  asset-pipeline code touched; pack hash unchanged.
- Documentation: RE-238, `docs/porting-status.md` (texture-conversion row +
  Known gaps §1), `PLAN.md` (T10 progress note), this snapshot.
- Commit: `824a6f0` (RE-238, `R2.1`/T10 progress -- `romtool texgen --verify`,
  zero-normal test, doc reconciliation).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
