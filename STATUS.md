# Project Status

**Last updated:** 2026-09-02

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

`R0.7 — Missing Material Tables`

## Task Status

`IN_PROGRESS`

Continuing R0.7 from the concrete leads RE-057 produced while closing R0.3.
RE-057's own guess (353's table lives in a sibling file, missed by
`PartTables::scan`'s same-file requirement) turned out to be wrong: reading
`353_LinkSpecial2.c` directly showed it already declares its own `DObjDesc`
graphs *and* its own `MObjSub` tables in the same file — retracted, see
RE-058.

RE-058 found something more useful instead: `WPAttributes`
(`refs/ssb-decomp-re/src/wp/wptypes.h:36-45`) is a **second** struct shape
with the same pointer adjacency `PartTables::scan` already looks for,
structurally, used for weapon/projectile sub-objects — and
`crates/ssb-rom/src/mobj.rs` currently only documents/handles
`FTCommonPart` (fighters) and `MPGroundDesc` (stages). This plausibly
explains part of the wider "71 unresolved graphs" figure R0.7 already
tracks, independent of file 353. But the one confirmed `WPAttributes`
instance checked (Link's boomerang, `226_LinkSpecial1.c`) has
`p_mobjsubs = NULL` by design — so a missing record is not automatically the
answer for every graph with zero `mobjs`.

Next step, two threads: (1) check whether `PartTables::scan`'s already-generic
matching logic structurally catches `WPAttributes` instances elsewhere in the
archive without any code change (it doesn't care what struct holds the
pointers, only their shape) — if it already does, the 56-vs-71 count may
just be stale and re-running `romtool mobj`/`romtool scene` is the fix; (2)
find whichever `WPAttributes` instance (if any) names 353's `EntryWave`/
`EntryBeam`/`SpinAttack` sub-graphs and read its `p_mobjsubs` field — if it's
`NULL` there too, file 353's `MissingPalette` cases are not a pairing gap at
all and need to be traced back into `mesh.rs`'s own state handling instead.

## Last Completed Task

`R0.3 — Texture Conversion Completeness` is now `COMPLETE`. Both of its
remaining failure classes were root-caused and reattributed to the tasks
that actually own a fix, the same pattern applied twice in one session:

* 26 segment-0x01 entries → RE-055 → confirmed as `sLBTransitionPhotoHeap`,
  a runtime framebuffer photocopy bound to RSP segment 1, never present in
  any ROM file (`refs/ssb-decomp-re/src/lb/lbtransition.c`) → R0.13's scope
* 4 `MissingPalette` entries → RE-057 (via a temporary instrumented trace of
  `crates/ssb-rom/src/mesh.rs`, reverted, not committed) → confirmed as a
  `PartTables` material-pairing gap in 3 specific files, not a texture/TLUT
  decode bug → R0.7's scope (RE-056's dedup-key theory was a real secondary
  factor but not the root cause; RE-057 corrects it)

RE-054's S2DEX-BG lead (from the prior session) is refuted:
`romtool scan --exhaustive` finds zero `G_BG_1CYC`/`G_BG_COPY` anywhere in
the ROM. See `PLAN.md` R0.3, R0.7 and R0.13.

`R0.2 — N64 Rendering Command Inventory`, `R0.1 — Rendering State
Reconciliation` and `R0.9 — Stage Animation` are also `COMPLETE` — see
`PLAN.md` for each.

## Next Eligible Task

`R0.7` (in progress, see above). `R0.4`'s two remaining open items are
adjacent — one (`all missing palette cases resolved`) is the same RE-057
finding pointing back to R0.7, the other (palette inheritance/state) is
genuinely distinct and tracked toward R0.15. `R0.8` is also eligible
(dependencies R0.1/R0.2 complete, has a lead from RE-054: find the real
matrix-building function for `0x8000`/`RecalcRotRpyRSca` nodes in
`objdisplay.c`) if R0.7's Link trace stalls.

## Blockers

None currently recorded.

---

# 2. Milestone Status

| Milestone                             | Status          |
| ------------------------------------- | --------------- |
| M0 — Research                         | `COMPLETE`      |
| M1 — PSP Bootstrap                    | `COMPLETE`      |
| M2 — Resource Pipeline                | `COMPLETE`      |
| M3 — Core Game / Scene Infrastructure | `COMPLETE`      |
| R0 — Rendering Correctness            | `IN_PROGRESS`   |
| R1 — Rendering Completeness           | `BLOCKED_BY_R0` |
| R2 — Physical PSP Validation          | `BLOCKED_BY_R1` |
| R3 — Rendering Performance            | `BLOCKED_BY_R2` |
| G0 — Combat Unlocked                  | `BLOCKED_BY_R3` |

---

# 3. Rendering Gate

Rendering is currently the highest-priority development area.

Combat is intentionally blocked.

Do not begin attacks, hitboxes, damage, knockback, KO, stocks, CPU combat or match gameplay until the rendering milestones explicitly unlock combat.

Movement, physics, collision and animation may continue when required for rendering validation.

---

# 4. Verified Foundation

The following foundation work has been completed:

* ROM validation
* VPK0 decompression
* relocData archive handling
* asset extraction
* runtime asset-pack generation
* F3DEX2 display-list parsing
* N64 texture decoding
* mesh conversion
* scene graph conversion
* fighter animation extraction
* stage animation extraction
* collision extraction
* PSP asset loading
* PSP mesh drawing
* core engine traits
* fixed timestep
* fighter movement infrastructure
* stage collision infrastructure

Detailed subsystem status belongs in:

`docs/porting-status.md`

---

# 5. Known Rendering Work

Known renderer work includes investigation/fixes for:

* texture conversion completeness
* CI4/CI8 palette/TLUT behavior
* texture filtering
* LOD
* mipmapping
* texture tile addressing
* wrap/clamp/mirror behavior
* Dream Land canopy rendering
* material tables
* combiner behavior
* lighting
* alpha/blending
* render state
* unresolved MObj behavior
* transform kind `0x8000`
* stage animation
* material animation
* fighter palettes/costumes
* billboard behavior
* framebuffer effects
* camera/projection behavior
* render-state leakage
* physical PSP compatibility
* VRAM behavior
* rendering performance

The exact ordered task list is in `PLAN.md`.

---

# 6. Current Task State

## R0.7 — Missing Material Tables

Status: `IN_PROGRESS`

### Objective

Resolve every scene graph containing an unresolved material table.

### Required Work

* [x] Determine whether file 353 (`LinkSpecial2`)'s graph is rejected by `PartTables::scan`'s same-file requirement despite a real same-file record existing — no: 353 already declares its own graph and its own `MObjSub` table in the same file, so the same-file requirement isn't the blocker (RE-058, retracts RE-057's guess)
* [x] Identify a second pairing-record shape `PartTables` might be missing — found: `WPAttributes` (`refs/ssb-decomp-re/src/wp/wptypes.h:36-45`), used for weapon/projectile sub-objects, same pointer adjacency as `FTCommonPart` but never documented/verified against `PartTables::scan` (RE-058)
* [ ] Check whether `PartTables::scan`'s existing generic matching already structurally catches `WPAttributes` instances archive-wide (no code change needed to test this — just re-run `romtool mobj`/`romtool scene` and inspect which graphs resolve)
* [ ] Find whichever `WPAttributes` instance (if any) names file 353's `EntryWave`/`EntryBeam`/`SpinAttack` sub-graphs specifically, and read its `p_mobjsubs` field — the one instance checked so far (Link's boomerang) has it `NULL` by design, so this is not guaranteed to resolve 353's `MissingPalette` cases
* [ ] Re-run the material-table search (`romtool mobj`/`romtool scene`) to reconcile the stale 56-vs-71 resolved/unresolved graph counts before trusting either
* [ ] Do the same tracing for files 52 (`MVCommon`) and 86 (`ITCommonObject`) — confirm whether they ever had a pairing record (`FTCommonPart`-, `MPGroundDesc`- or `WPAttributes`-shaped) to find, or are legitimately table-less (non-fighter/non-stage UI containers)

### Completion Evidence

Record:

* which files were traced, and what `PartTables::scan` actually found or rejected for each
* the fix implemented (new struct shape wired into `PartTables::scan`, newly discovered record, or accepted deviation) with its measured effect on resolved-graph and packed-texture counts
* before/after `romtool mobj`/`romtool scene`/`romtool textures` output
* regression test added

---

# 7. Last Verification

## 2026-09-02 — R0.7 started: WPAttributes found, 353 sibling-file guess retracted

* Read `refs/ssb-decomp-re/src/relocData/353_LinkSpecial2.c` directly — found `dLinkSpecial2_EntryWaveDObjDesc`/`dLinkSpecial2_EntryBeamDObjDesc` (graphs) and `dLinkSpecial2_EntryWaveMObjSub`/`dLinkSpecial2_EntryBeamMObjSub` (tables) both defined in the same file, refuting RE-057's "table lives in a sibling file" guess
* Read `refs/ssb-decomp-re/src/relocData/225_LinkMain.c`'s `FTCommonPartContainer` — confirmed it pairs Link's *main* model (`dLinkModel_JointTree`, both halves in file 324) and has nothing to do with 353's sub-models
* Read `refs/ssb-decomp-re/src/wp/wptypes.h:36-45` — found `WPAttributes`, a second struct with the same `DObjDesc*`/`MObjSub***` adjacency `PartTables::scan` looks for, used for weapon/projectile objects, never documented in `crates/ssb-rom/src/mobj.rs`
* Read `refs/ssb-decomp-re/src/relocData/226_LinkSpecial1.c`'s `dLinkSpecial1_Boomerang_WeaponAttributes` — a real `WPAttributes` instance with `p_mobjsubs = NULL` by design (its `data`/`anim_joints` point into file 325, not 353) — shows a null table can be intentional, tempering how much weight to put on "found the missing struct shape" as a guaranteed fix for 353
* Result: RE-058 recorded in `docs/reverse-engineering.md`, retracting RE-057's specific 353 guess while keeping its zero/partial-`mobjs` mechanism finding intact; `PLAN.md` R0.7, `TODO.md` Phase E updated
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.3 closed: segment-0x01 and MissingPalette investigations

* `cargo run --release -p romtool -- scan "rom/Super Smash Bros. (USA).z64" --exhaustive` — 0 occurrences of opcode 0x09/0x0A anywhere in the ROM's display lists; refutes RE-054's S2DEX BG lead
* `cargo run --release -p romtool -- dump "rom/Super Smash Bros. (USA).z64" 39` (and 40/41/45/50/51) — dumped raw file bytes, located the failing `G_SETTIMG` (file 39 offset 0x0E10: `fd10012b 01000000`), confirmed identical bytes recur across files 40/41/45/50/51
* Cross-checked address `0x01000000` (segment 1) against `refs/ssb-decomp-re/src/lb/lbtransition.c` — `gSPSegment(..., 0x1, sLBTransitionPhotoHeap)`, a per-frame `300x220` 16-bit framebuffer photocopy for the loading-break transition system; texture dims from `romtool textures --file 39` (`300x5`, `300x6` `Rgba/Bits16`) match
* Temporarily instrumented `crates/ssb-rom/src/mesh.rs` with `eprintln!`s (in `convert_sequence`, the segment-0x0E `Call` handler, `SetTimg`, `LoadTlut`), ran `romtool textures --file 52/86/353`, then reverted the instrumentation (`git checkout -- crates/ssb-rom/src/mesh.rs`) — confirmed files 52 and 353 get zero `MObj` materials for every graph node, file 86 gets them for most but not all nodes; every unresolved segment-0x0E call clears an otherwise-valid palette via `forget_texture()`
* Identified file names via `refs/ssb-decomp-re/src/relocData/`: 52=`MVCommon`, 86=`ITCommonObject`, 353=`LinkSpecial2` (one of Link's own model files, unlike 52/86)
* Result: RE-057 recorded in `docs/reverse-engineering.md`, correcting RE-056's dedup-key theory. `PLAN.md` R0.3 marked `COMPLETE`; R0.4, R0.7 updated; `TODO.md` Phase B/E, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: documentation/investigation only — `crates/ssb-rom/src/mesh.rs` was temporarily modified for tracing and fully reverted before commit; `git diff` confirms no net code change
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below
* Result: RE-055 and RE-056 recorded in `docs/reverse-engineering.md`; `PLAN.md` R0.3/R0.13, `TODO.md` Phase B, `docs/rendering.md`, `docs/porting-status.md` updated to match
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — R0.2 BattleShip cross-reference

* Cloned `refs/BattleShip` + `libultraship` submodule (`ssb64` branch); read `src/fast/interpreter.cpp`
* Cross-checked against `refs/ssb-decomp-re/src/sys/objdisplay.c` (already present, no clone needed)
* Result: RE-054 recorded in `docs/reverse-engineering.md`; three new leads filed in `TODO.md` (R0.13 S2DEX BG, R0.5 LOD corroboration, R0.8 transform clarification)
* Affected subsystem: documentation/investigation only, no code changed
* PPSSPP: not run this pass
* Physical PSP: not tested this pass — see §8 below

## 2026-09-02 — Documentation audit (R0.1)

* `cargo test --workspace` — 338 passing (`ssb-rom` 195, `ssb-engine` 36, `ssb-game` 107), 0 failed
* `cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"` — 647 bound / 617 packed / 30 failed (26 segment-0x01, 4 `MissingPalette`); matches what `docs/rendering.md` now documents
* Affected subsystem: documentation only, no code changed
* PPSSPP: not re-run this pass; prior baseline stands (see `docs/porting-status.md` "M1 verification")
* Physical PSP: not tested this pass — see §8 below

Future entries must include:

* command/test;
* result;
* relevant output;
* affected subsystem;
* whether PPSSPP was tested;
* whether physical PSP was tested.

---

# 8. Physical PSP Validation

## Status

`R2 ACCEPTANCE NOT YET PERFORMED`

The project has been booted and smoke-tested on physical PSP hardware earlier
in development. That testing was not captured against `PLAN.md` R2's
acceptance criteria (hardware model, build, asset-pack version, per-feature
observed behavior, VRAM usage) and predates the rendering work tracked under
R0, so it does not stand in for R2. Treat R2 as not started until a hardware
session records the fields below.

PPSSPP testing does not count as physical PSP validation.

When R2 hardware validation begins, record:

* PSP model
* firmware/environment
* EBOOT/build
* runtime asset pack
* test scene
* observed FPS/frame time
* VRAM behavior
* rendering failures
* screenshots/video where useful

---

# 9. Important Rules

* `PLAN.md` determines task order.
* `STATUS.md` determines current execution state.
* `docs/porting-status.md` determines verified subsystem status.
* `AGENTS.md` determines agent behavior.
* Rendering is the hard gate.
* Do not begin combat because rendering appears "good enough".
* Do not mark work complete without evidence.
* Do not treat PPSSPP as physical PSP validation.
* Do not invent original N64 behavior when the decompilation/ROM can answer it.

---

# 10. Session Continuity

Before ending a session or context compaction, update this file with:

```text
Current milestone
Current task
Task status
Last completed task
Next eligible task
Blockers
Changes made
Verification performed
Evidence
Important discoveries
Documentation updated
Relevant commit
```

A fresh agent must be able to read this file and continue without relying on conversation history.

---

# 11. Continuation Command

The intended workflow is:

> Continue with the plan.

The agent should then:

1. Read `AGENTS.md`.
2. Read `PLAN.md`.
3. Read this file.
4. Inspect the repository.
5. Resume the current task.
6. Implement.
7. Verify.
8. Document.
9. Update this file.
10. Commit.
11. Continue to the next eligible task.
