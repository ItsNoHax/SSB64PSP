# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.2 — Second Renderer Corrective Gate (C1-C7)` (`IN_PROGRESS`)
- Status: `IN_PROGRESS` -- C1 (`COMPLETE`); C2-C7 remain
- Last complete: `RE-240` (2026-09-11) -- closed `R2.2`/C1 ("Single-source
  `prim_color`"). Traced raw vertex RGBA/normal bytes through `CacheEntry`,
  `push_vertex`, `MeshVertex`, `PackWriter`, `PackedVertex` and `meshdraw` per
  the task's own instructions, and found two real defects, not zero:
  1. **`SHADE * PRIM` was applied twice.** `mesh.rs`'s `push_vertex` folds
     `material.prim_color`'s resolved scale into an *unlit* vertex's bytes
     (needed so a shared cache vertex used by two differently-coloured unlit
     primitives dedups correctly); `pack.rs`'s `add_mesh` (RE-106) then
     unconditionally folded the same scale in a second time, squaring any
     non-identity scale (e.g. a 50% grey scale on a 50% grey shade produced
     `32`, not the correct `64`). RE-106's own doc comment ("nothing
     downstream ever multiplied it back in") predates `push_vertex`'s bake
     (present since before RE-106's own commit), so the double application
     was never noticed.
  2. **Normals were being changed as RGB.** `push_vertex`'s three
     colour-baking branches (`prim_color` scale, `texture_blend`, `flat_color`)
     had no `material.lit` gate at all, even though `MeshVertex::rgba` is a
     packed *normal*, not a colour, whenever a primitive is lit. Measured
     archive-wide (`tools/romtool`'s new
     `census_lit_primitives_with_a_colour_baking_branch`): 243 real
     primitives carry `lit` + `prim_color`, 34 carry `lit` + `texture_blend`,
     2 carry `lit` + `flat_color` -- 279 real primitives whose normals
     `push_vertex` was silently overwriting before `pack.rs`'s own
     `shade_normal`/runtime GE lighting ever saw them.

  Fixed by gating `push_vertex`'s three branches on `!self.material.lit`
  (unlit path unchanged) and moving the lit-path equivalent into `pack.rs`'s
  `add_mesh`: new `flat_override`/`blend_override` per-vertex maps alongside
  the existing `prim_scale` one, all applied *after* `shade_normal` computes
  a real shade from the now-intact normal, and `prim_scale` itself now gated
  on `lit[i]` so it no longer re-scales an already-`push_vertex`-baked unlit
  vertex. Added all of C1's named test cases (`crates/ssb-rom/src/mesh.rs`:
  the exact `128*128/255=64` integer-scale case, a SHADE-only case, two
  lit-vertex-keeps-its-raw-normal cases for `prim_color` and `texture_blend`;
  `crates/ssb-rom/src/pack.rs`: replaced the one test that had (unknowingly)
  encoded the old double-scale behaviour with a pair proving no second scale
  for unlit and a correct post-shading scale for lit, including that
  `nx`/`ny`/`nz` still carry the exact original normal). Verified the new
  archive-wide census actually catches the regression it targets (test the
  test by breaking the code): temporarily restoring the unconditional bake
  made the with-branch not-normal-looking rate jump from 60/10,446 to
  6,666/10,446 while an unrelated without-branch baseline stayed fixed at
  808/57,612, then reverted cleanly. The 23 real archive files affected are
  measured and listed in RE-240; per-fighter visual (PPSSPP/physical-PSP)
  recheck is not done this session -- see Blockers below.
- Previously complete: `RE-239` (2026-09-11) -- closed `R2.1`/T10's one
  remaining item (texgen-without-texture census; `R2.1` closed complete).
- Previously complete: `RE-238` (2026-09-11) -- `R2.1`/T10 texgen
  documentation/test cleanup.
- Next: `R2.2`/C2 -- "Load-time lighting provenance". `PLAN.md`'s own
  execution order is `C1 -> C2 -> C3 -> C4 -> C5 -> C6 -> C7`; C1 is now
  `COMPLETE`, so C2 is the first eligible item.
- Blockers: none for `R2.2`/C1 itself. Non-blocking follow-ups: (1) RE-240's
  fix is host-side/measurement-verified only -- a visual before/after
  (PPSSPP or physical PSP) for one of the 23 newly-identified affected
  archive files (`52, 67, 68, 69, 73, 86, 109, 149, 161, 296, 313, 317, 320,
  323, 324, 328, 330, 332, 335, 336, 338, 341, 350`) is a reasonable next
  step for whoever next has hands on the interactive build, not a blocker
  for C2. (2) A real bug was found and flagged (not fixed) in the
  `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture. See the spawned follow-up task
  (`task_1bf9bc35`). (3) A before/after PPSSPP TEXVIEW screenshot confirming
  RE-224's CI4 palette-bank fix (global texture indices 187/194/195, object
  indices 60-102 in file 86) was never obtained -- manual follow-up. (4)
  RE-228's residual ~1.78-S10.5-unit clamp-boundary deviation is measured but
  not checked against real `sceGu` calls -- minor lead, not currently
  assigned; not recorded as `ACCEPTED_DEVIATION` yet. (5) A fresh
  PPSSPP-headless rebuild of scene 11 on this machine diffs from the exact
  committed `r2-metal-texgen.png` bytes by 31,737 pixels (self-consistent
  across rebuilds), the same noise-floor-order pattern already treated as
  expected cross-build/cross-environment variance (RE-214/RE-236 precedent).
  `R2.1`/T1's own finding (164 cross-node differing-transform vertex reuses)
  also remains an open, tracked, known gap, not a blocker.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
  The physical PSP's `/dev/bus/usb/NNN/NNN` node permission can go stale
  after a reconnect; replugging the device re-enumerates it and reapplies
  the rule (RE-236).
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-240.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`COMPLETE`, T1-T10 all
  terminal); `R2.2` (`IN_PROGRESS`, C1 `COMPLETE`, C2 next).
- Decisions: `DECISIONS.md` -- no new revision for RE-240 (D-042 already
  covers "renderer correctness claims stay provisional until R2.2 closes";
  this fix is progress within that, not a change to the decision itself).
- Subsystem: `docs/porting-status.md` -- no change needed for RE-240 (an
  internal pack/mesh-pipeline correctness fix, not a subsystem capability
  change).
- Verification (RE-240): `cargo test --workspace` (`SSB64_ROM` set):
  `ssb-rom` 409 passed (up from 404), `romtool` 14 passed (up from 13), 0
  failed overall. `cargo fmt --check` clean. `cargo clippy --workspace
  --all-targets` clean (two pre-existing, unrelated warnings only). Rebuilt
  `assets/generated/ssb64.pak` (`romtool pack`): mesh/primitive/texture/
  triangle counts unchanged (this fix corrects vertex *colour* bytes only),
  loads back cleanly. Code changed: `crates/ssb-rom/src/mesh.rs`
  (`push_vertex`'s lit gate + 4 new tests), `crates/ssb-rom/src/pack.rs`
  (`add_mesh`'s `flat_override`/`blend_override` maps + gated `prim_scale`,
  1 test replaced with 2), `tools/romtool/src/main.rs`
  (`census_lit_primitives_with_a_colour_baking_branch`).
- Documentation: RE-240, `PLAN.md` (`R2.2` header + C1 status), this
  snapshot.
- Commit: pending (recorded in a follow-up commit once made, per this
  project's own convention).

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
