# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1 — Final Texgen Fidelity (T1-T10)` (complete)
- Status: `COMPLETE`
- Last complete: `RE-239` (2026-09-11) -- closed `R2.1`/T10's one remaining
  item. Measured whether a real texgen-bound primitive with no bound texture
  exists in the archive (`tools/romtool`'s new
  `no_real_texgen_primitive_is_missing_a_bound_texture`, walking every real
  file's converted meshes via the existing `file_meshes` helper, the same
  path `pack` uses): `0` of `202` real texgen primitives lack a texture --
  the "textured->untextured->texgen" mapping transition is real syntax but
  not real content in this ROM. Added the synthetic transition test the T10
  test minimum names anyway, following the same "reference case not seen in
  the real archive" precedent `R2.1`/T7 used
  (`crates/ssb-rom/src/mesh.rs`'s
  `a_texgen_primitive_after_an_untextured_primitive_has_no_stale_texture`):
  binds a real texture, draws textured, disables texturing and draws
  untextured, then enables `G_TEXTURE_GEN` with no further `G_SETTIMG` and
  draws again -- asserts three primitives split by material (textured/no
  texgen, untextured/no texgen, untextured/texgen carrying the live
  `G_TEXTURE` scale). Broke `current_texture`'s `texture_enabled` gate
  locally to confirm the test actually fails without it (test-the-test),
  then reverted. This closes `R2.1`'s T1-T10 gate entirely; `PLAN.md`'s
  `R2.1` header and the two stale texgen checklist rows (`G_TEXTURE_GEN`/
  `G_TEXTURE_GEN_LINEAR` "compared against original output") updated to
  match.
- Previously complete: `RE-238` (2026-09-11) -- `R2.1`/T10 texgen
  documentation/test cleanup: stale `docs/porting-status.md` reconciled,
  `romtool texgen ROM --verify` built and run clean against the real ROM,
  zero-normal test added.
- Previously complete: `RE-237` (2026-09-11) -- closed `R2.1`/T9's two
  remaining matrix items on physical PSP hardware (raw normal diagnostic,
  camera-rotation scene), closing `R2.1`/T9.
- Next: `R2.2 — Second Renderer Corrective Gate (C1-C7)` is now unblocked
  (`PLAN.md`'s own dependency: "depends on R2.1/T1-T10"). Not started this
  session -- `R2.1` closing and `R2.2` starting are kept as separate
  sessions' work per `AGENTS.md`'s "maintain exactly one primary
  implementation task." First eligible item is `C1 — Single-source
  prim_color`.
- Blockers: none for closing `R2.1`. `R2.1`/T1's own finding (164
  cross-node differing-transform vertex reuses) remains an open, tracked,
  *known* gap -- not a blocker, and does not block `R2.2`'s start.
  `R2.2`/C1-C7 renderer corrective gate is `TODO`, gating combat (combat
  remains gated behind `R2.2`). Separately (not blocking): a real bug was
  found and flagged (not fixed) in the `debug_overlay` PSP viewer --
  object-view HUD text renders corrupted/double-exposed in every capture.
  See the spawned follow-up task (`task_1bf9bc35`). Also open, non-blocking:
  a before/after PPSSPP TEXVIEW screenshot confirming RE-224's CI4
  palette-bank fix (global texture indices 187/194/195, object indices
  60-102 in file 86) was never obtained -- manual follow-up for whoever next
  has hands on the interactive build. Also open, non-blocking,
  low-confidence: RE-228's residual ~1.78-S10.5-unit clamp-boundary
  deviation is measured but not checked against real `sceGu` calls the way
  RE-226's normal-semantics question was -- minor lead for a future task,
  not currently assigned; deliberately not recorded as `ACCEPTED_DEVIATION`
  yet since "unavoidable" is not yet established. Also open, non-blocking,
  low-confidence: a fresh PPSSPP-headless rebuild of scene 11 on this
  machine diffs from the exact committed `r2-metal-texgen.png` bytes by
  31,737 pixels, even though two fresh rebuilds against each other are
  byte-identical (self-consistent) -- the same noise-floor-order pattern
  this project already treats as expected cross-build/cross-environment
  variance (RE-214/RE-236 precedent), not investigated further.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-239.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`TODO`, next).
- Decisions: `DECISIONS.md` -- no new revision for RE-239 (test-coverage
  work only; RE-238 already surveyed D-039/D-042 for staleness and found no
  revision needed).
- Subsystem: `docs/porting-status.md` -- last updated RE-238 (texture-
  conversion row and "Known gaps" §1); no further change needed for RE-239
  (test coverage only, no behavior or subsystem-status change).
- Verification (RE-239): `cargo test --workspace` (`SSB64_ROM` set):
  `ssb-rom` 404 passed (up from 403), `romtool` 13 passed (up from 12), 0
  failed overall. `cargo fmt --check` clean in `tools/romtool/` and
  `crates/ssb-rom/`. `romtool texgen "<rom>" --verify`: `PASS`, unchanged
  from RE-238. Code changed: `tools/romtool/src/main.rs`
  (`no_real_texgen_primitive_is_missing_a_bound_texture`),
  `crates/ssb-rom/src/mesh.rs`
  (`a_texgen_primitive_after_an_untextured_primitive_has_no_stale_texture`).
  No PSP-target or asset-pipeline code touched; pack hash unchanged.
- Documentation: RE-239, `PLAN.md` (`R2.1` header + T10 status +
  `G_TEXTURE_GEN`/`G_TEXTURE_GEN_LINEAR` checklist rows), this snapshot.
- Commit: `e31c967` (RE-239, `R2.1`/T10 close -- textured/untextured/texgen
  mapping-transition census + synthetic test, `R2.1` complete).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
