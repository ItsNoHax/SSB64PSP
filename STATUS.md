# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`IN_PROGRESS`); C3-C7 remain
- Last complete: `RE-242` (2026-09-11) -- `R2.2`/C2, continuing RE-241's
  remainder. RE-241 said closing the external-per-object initial-lighting
  gap needed an archive mesh/costume-file-to-fighter mapping this project
  "does not yet have." That claim was wrong: `crates/ssb-rom/src/
  fighter.rs`'s `common_parts()` already reads exactly that mapping, from
  commit `bea4829` (well before RE-240/RE-241) -- a genuine two-hop
  relocation read (`FTAttributes.commonparts_container` intern reloc to an
  `FTCommonPartContainer`, then each of its two `FTCommonPart` entries'
  extern reloc into the model file), already wired into `romtool
  figatree`'s skeleton pipeline but never cross-checked against ground
  truth on its own. Added `tools/fighter-model-ground-truth.py`, which reads
  the archive-file id straight out of the decompilation's own
  `<id>_<Name>Model.c` relocData source filenames (26 files, matching
  `FighterFile::name`'s own naming convention), and
  `fighter.rs`'s new `real_rom_common_parts_match_every_named_model_file`
  test (`SSB64_ROM`-gated, kept permanently): all 27 fighters' both
  `FTCommonPart` detail levels match. 25 of 27 have their own `*Model.c` and
  match it exactly; Giant DK and NLuigi have none, and the ROM confirms they
  share their base character's file (317, Donkey Kong's; 301, NMario's) --
  a real finding the cross-check produced, not an assumption. Verified the
  test discriminates ("test the test by breaking the code": fed it a wrong
  file id, confirmed failure, reverted). No runtime code changed; this is a
  mapping-verification entry, not the lighting-default fix itself. See
  `docs/reverse-engineering.md` RE-242 for the full entry.
- Previously complete: `RE-241` (2026-09-11) -- `R2.2`/C2 part 1 of 2: fixed
  a real timing bug (vertex normal-vs-colour meaning was read from
  triangle-time `material.lit` instead of `G_VTX` load-time state;
  `MeshVertex::lit` now captures it correctly, measured 0/110,316 real
  archive disagreements) and traced `ftDisplayMainProcDisplay`'s
  unconditional external `G_LIGHTING` as the cause of RE-021's remaining
  36,356-vertex `looks_like_unit_normal` fallback gap.
- Previously complete: `RE-240` (2026-09-11) -- closed `R2.2`/C1
  ("Single-source `prim_color`"): `SHADE * PRIM` was applied twice, and lit
  vertices' normals were being overwritten as if they were colour.
- Next: `R2.2`/C2's remainder -- wire `mesh.rs`'s `State::material.lit` to
  seed `true` when building either of `fighter::common_parts()`'s two
  graphs (matching `ftDisplayMainProcDisplay`'s unconditional external
  `G_LIGHTING`), instead of the current unconditional unlit default. Needs
  threading an initial-lit parameter through `mesh::State`/`convert`/
  `convert_sequence` and `pack.rs`'s per-graph decode loop in `tools/
  romtool`, regression tests (an unlit-by-default non-fighter graph must be
  unaffected; a fighter graph with no in-list `G_LIGHTING` signal must now
  resolve lit), a full pack rebuild, and re-quantifying
  `looks_like_unit_normal`'s remaining archive-wide coverage now that it
  should be a true last resort. If that proves out of reach this session,
  the next eligible fallback is starting C3 ("Independent depth
  compare/write state") while C2's remainder stays tracked, since
  `PLAN.md`'s C1->C7 order is a target sequence, not a hard gate on
  IN_PROGRESS items blocking all later ones from starting.
- Blockers: none for what RE-242 closed. Same non-blocking follow-ups
  RE-240/RE-241 already recorded remain open (visual before/after for
  RE-240's 23 affected files; the `debug_overlay` HUD text bug,
  `task_1bf9bc35`; RE-224's TEXVIEW screenshot; RE-228's clamp-boundary
  deviation; RE-214/RE-236's known screenshot noise floor; R2.1/T1's 164
  cross-node differing-transform vertex reuses) -- none are new, see prior
  snapshot history for detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-242.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `IN_PROGRESS`).
- Decisions: `DECISIONS.md` -- no new revision for RE-242 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes";
  this fix is progress within that, not a change to the decision itself).
- Subsystem: `docs/porting-status.md` -- no change needed for RE-242 (a
  mapping-verification finding, not a subsystem capability change).
- Verification (RE-242): `cargo test --workspace` (`SSB64_ROM` set):
  `ssb-rom` 412 passed (up from 411, one new test), `romtool` 15 passed, 0
  failed overall. `cargo fmt --check` clean. `cargo clippy --workspace
  --all-targets` clean (the same two pre-existing, unrelated warnings as
  RE-240/RE-241). No pack rebuild needed (no runtime/pack-pipeline code
  changed). Code changed: `crates/ssb-rom/src/fighter.rs`
  (`real_rom_common_parts_match_every_named_model_file` test),
  `tools/fighter-model-ground-truth.py` (new).
- Documentation: RE-242, `PLAN.md` (`R2.2`/C2 status + remainder text +
  lighting checklist row), this snapshot.
- Commit: `fec6d24` (RE-242, `R2.2`/C2 mapping-verification).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
