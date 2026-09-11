# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`COMPLETE`); C3-C7 remain
- Last complete: `RE-243` (2026-09-11) -- `R2.2`/C2's remainder and closes C2.
  RE-242 found the fighter mesh/costume-file mapping already existed
  (`fighter::common_parts()`); this entry wires it in. `mesh::convert_sequence`
  gained an `initial_lit: bool` parameter (threaded through `State::new` and
  `convert`), seeded `true` in `tools/romtool`'s `pack`, `file_meshes` and
  `scene` for exactly the `(model_file, graph_offset)` pairs a fighter's two
  `common_parts` skeleton graphs name across all 27 fighters
  (`fighter_skeleton_graphs`), matching `ftDisplayMainProcDisplay`'s
  unconditional external `G_LIGHTING` (RE-021/RE-241). Two new `mesh.rs` unit
  tests cover both seed states directly. Measuring the real impact
  (`tools/romtool`'s `census_initial_lit_seed_measured_impact_on_skeleton_
  graphs`, kept permanently) found **zero** of 10,958 vertices across all 37
  distinct skeleton graphs change `lit` state: every one of these graphs'
  node lists that draws a vertex already carries its own `G_MW_LIGHTCOL`
  (`gSPLightColor`) command ahead of its first `G_VTX`, and RE-105's existing
  handler already resolves `lit` correctly from that alone. The seed is still
  correct and necessary in principle (a list lacking its own `G_MW_LIGHTCOL`
  would need it) but redundant for every fighter this archive ships -- a
  genuine, permanently-censused null result, not a wiring bug.
  `assets/generated/ssb64.pak` rebuilt byte-identical, consistent with that
  measurement. `looks_like_unit_normal`'s remaining fallback usage
  (`census_looks_like_unit_normal_fallback_after_skeleton_lighting_seed`,
  kept permanently: 383/2293 skeleton-graph and 1668/20769 other-graph unlit
  vertices still look like a normal) is therefore unrelated to this specific
  external-per-object case. See `docs/reverse-engineering.md` RE-243 for the
  full entry.
- Previously complete: `RE-242` (2026-09-11) -- `R2.2`/C2, continuing RE-241's
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
- Next: `R2.2`/C3 -- "Independent depth compare/write state". Census
  `Z_CMP`, `Z_UPD`, `ZMODE_OPA/INTER/XLU/DEC`; represent independent
  `depth_test`, `depth_write` and `depth_mode` instead of the current single
  `z_buffer` flag; keep `G_ZBUFFER` as geometry/RSP state; map writes
  through `sceGuDepthMask` (`true` disables PSP writes). Needs a depth-test/
  no-write translucent-front/opaque-behind scene and an ON->OFF->ON switch
  regression, with PPSSPP or physical-PSP evidence. `PLAN.md`'s C1->C7 order
  is a target sequence, not a hard gate, but C3 is the next eligible item
  now that C1 and C2 are both closed.
- Blockers: none for what RE-243 closed. Same non-blocking follow-ups
  RE-240/RE-241/RE-242 already recorded remain open (visual before/after for
  RE-240's 23 affected files; the `debug_overlay` HUD text bug,
  `task_1bf9bc35`; RE-224's TEXVIEW screenshot; RE-228's clamp-boundary
  deviation; RE-214/RE-236's known screenshot noise floor; R2.1/T1's 164
  cross-node differing-transform vertex reuses) -- none are new, see prior
  snapshot history for detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-243.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `COMPLETE`, C3-C7
  remain).
- Decisions: `DECISIONS.md` -- no new revision for RE-243 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes";
  this closes one sub-part of that, not the decision itself).
- Subsystem: `docs/porting-status.md` -- updated the "Mesh conversion" and
  "Model conversion" rows: `G_VTX` load-time lighting provenance (`R2.2`/C2)
  is now closed alongside single-source `PRIM * SHADE` (`R2.2`/C1);
  independent depth state and primitive reordering (`R2.2`/C3-C4) remain
  open.
- Verification (RE-243): `cargo test --workspace --all-targets` (`SSB64_ROM`
  set): `ssb-rom` 414 passed (412 prior + 2 new unit tests), `romtool` 17
  passed (15 prior + 2 new census tests), 0 failed overall. `cargo fmt
  --check` clean. `cargo clippy --workspace --all-targets` clean (the same
  pre-existing, unrelated warnings as RE-240/RE-241/RE-242). Pack rebuilt
  (`romtool pack`): byte-identical checksum to the pre-fix build. Code
  changed: `crates/ssb-rom/src/mesh.rs` (`State::new`/`convert`/
  `convert_sequence` signatures, `MeshVertex::lit` doc comment, two new unit
  tests), `tools/romtool/src/main.rs` (`fighter_skeleton_graphs`,
  `convert_graph_at` signature, `pack`/`scene`/`file_meshes` wiring, two new
  census tests).
- Documentation: RE-243, `PLAN.md` (`R2.2`/C2 status, cross-reference table,
  acceptance checklist), `docs/porting-status.md`, this snapshot.
- Commit: `113b6ff` (RE-243, `R2.2`/C2 external-lighting seed + closure).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
