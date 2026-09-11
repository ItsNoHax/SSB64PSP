# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2 (`IN_PROGRESS`); C3-C7 remain
- Last complete: `RE-241` (2026-09-11) -- `R2.2`/C2 ("Load-time lighting
  provenance"), part 1 of 2. Two findings:
  1. **A real timing bug.** `mesh.rs`'s `push_vertex` decided whether a
     vertex's bytes were a normal or a colour from `self.material.lit` --
     the *current* material as of whatever command most recently ran -- not
     from the state active when that vertex's own `G_VTX` loaded it.
     `pack.rs`'s `add_mesh` had the same bug one layer down, reading the
     enclosing primitive's triangle-time `material.lit` instead of a
     per-vertex load-time fact. This is `PLAN.md`'s own named R2.2 stop
     condition ("vertex meaning depends on triangle-time state"). Fixed by
     adding `MeshVertex::lit`, captured at the exact `Cmd::Vtx` command
     (mirroring the existing `space` field's own load-time capture), and
     reading it instead of the triangle-time material in both `push_vertex`
     and `add_mesh`'s per-vertex `lit[]` derivation (which also dropped the
     RE-103 "first primitive touching a shared vertex wins" approximation --
     no longer needed once `lit` is an intrinsic per-vertex fact). Measured
     archive-wide (`tools/romtool`'s new
     `census_g_vtx_vs_triangle_time_lighting_state`, kept permanently):
     **0/110,316** real triangle-corner vertex references actually disagree
     between load time and draw time -- the bug was real (and two of
     RE-240's own tests had unknowingly encoded it, confirmed by
     "test the test by breaking the code": fixing it made both fail for the
     right reason until their command order was corrected to match their
     actual intent) but is not currently observable in any rendered frame.
     `assets/generated/ssb64.pak` rebuilt byte-identical.
  2. **Caller-trace evidence for RE-021's external-lighting gap.**
     `ftDisplayMainProcDisplay` (decompilation) unconditionally sets
     `G_LIGHTING` for every fighter draw before its own node lists run,
     regardless of `colanim.is_use_light`; `grep -rl G_LIGHTING
     refs/ssb-decomp-re/src/` matches only fighter/character-model code
     (`ft`, `mv`, `mn`, `sc`, `db`), never stages (`gr`) or items (`it`).
     This corroborates and narrows RE-021's "external, per-object
     `G_LIGHTING`" finding, but closing it needs an archive mesh/costume-file
     -to-fighter identification this project does not yet have
     (`fighter::FIGHTER_FILES` only names each fighter's `FTAttributes`
     file, not its separate mesh/costume files -- see RE-240's own
     unmapped-file-list note). Not implemented this session; recovering that
     mapping with an explicit-pairing record is `R2.2`/C2's own next step.
- Previously complete: `RE-240` (2026-09-11) -- closed `R2.2`/C1
  ("Single-source `prim_color`"): `SHADE * PRIM` was applied twice, and lit
  vertices' normals were being overwritten as if they were colour. See
  `docs/reverse-engineering.md` for the full entry.
- Previously complete: `RE-239` (2026-09-11) -- closed `R2.1`/T10's one
  remaining item (texgen-without-texture census; `R2.1` closed complete).
- Next: `R2.2`/C2's remainder -- recover a real, ROM-verified mapping from
  archive mesh/costume file to the fighter whose caller sets `G_LIGHTING`
  externally (the same rigor as `DObjDesc`/RE-023 and `MObjSub`/RE-027), then
  make fighter-sequence initial lighting state explicit instead of defaulting
  unlit, then requantify `looks_like_unit_normal`'s remaining archive-wide
  coverage now that it should be a true last resort. If that recovery proves
  out of reach this session, the next eligible fallback is starting C3
  ("Independent depth compare/write state") while C2's remainder stays
  tracked, since `PLAN.md`'s C1->C7 order is a target sequence, not a hard
  gate on IN_PROGRESS items blocking all later ones from starting.
- Blockers: none for what RE-241 closed. Same non-blocking follow-ups RE-240
  already recorded remain open (visual before/after for RE-240's 23 affected
  files; the `debug_overlay` HUD text bug, `task_1bf9bc35`; RE-224's TEXVIEW
  screenshot; RE-228's clamp-boundary deviation; RE-214/RE-236's known
  screenshot noise floor; R2.1/T1's 164 cross-node differing-transform vertex
  reuses) -- none are new, see prior snapshot history for detail.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-241.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 `IN_PROGRESS`).
- Decisions: `DECISIONS.md` -- no new revision for RE-241 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes";
  this fix is progress within that, not a change to the decision itself).
- Subsystem: `docs/porting-status.md` -- no change needed for RE-241 (an
  internal mesh/pack-pipeline correctness fix, not a subsystem capability
  change).
- Verification (RE-241): `cargo test --workspace` (`SSB64_ROM` set):
  `ssb-rom` 411 passed (up from 409, two new tests), `romtool` 14 passed, 0
  failed overall. `cargo fmt --check` clean. `cargo clippy --workspace
  --all-targets` clean (the same two pre-existing, unrelated warnings as
  RE-240). Rebuilt `assets/generated/ssb64.pak` (`romtool pack`):
  byte-identical to the pre-fix build (`git diff`/`git status` both clean),
  matching the 0/110,316 census result. Code changed:
  `crates/ssb-rom/src/mesh.rs` (`MeshVertex::lit` + load-time capture in
  `Cmd::Vtx` + `push_vertex`'s gate + 2 new tests + 2 RE-240 tests reordered
  to match their actual intent), `crates/ssb-rom/src/pack.rs` (`add_mesh`'s
  `lit[]` derivation now per-vertex-only + `sample_mesh` fixture/2 test sites
  updated), `tools/romtool/src/main.rs`
  (`census_g_vtx_vs_triangle_time_lighting_state`).
- Documentation: RE-241, `PLAN.md` (`R2.2` header + C1/C2 status + two
  related checklist rows), this snapshot.
- Commit: pending (RE-241, `R2.2`/C2 partial -- load-time lighting
  provenance fix + caller-trace evidence).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
