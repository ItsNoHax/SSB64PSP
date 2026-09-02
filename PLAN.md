# SSB64PSP Development Plan

## Mission

Create a native Rust implementation of Super Smash Bros. 64 for Sony PSP hardware.

The original SSB64 decompilation and the user's legally obtained ROM are the primary behavioral references.

The project prioritizes:

```text
Original behavior
        ↓
Rendering correctness
        ↓
Rendering completeness
        ↓
Physical PSP validation
        ↓
Rendering performance
        ↓
Combat
        ↓
Full game systems
```

**Rendering is a hard gate.**

Combat does not begin until the rendering gate has been explicitly passed.

---

# 1. How to Use This Plan

`PLAN.md` defines:

* the ordered roadmap;
* task dependencies;
* acceptance criteria;
* verification requirements;
* milestone gates.

`STATUS.md` defines:

* what the agent is currently doing;
* what it completed last;
* what it verified;
* what is blocked;
* what should be resumed.

Do not put mutable session state in this file.

When the user says:

> Continue with the plan.

the agent reads `STATUS.md` first to determine whether an existing task should be resumed.

If there is no active task, select the first eligible `TODO` task in this plan.

---

# 2. Task Statuses

Every implementation task uses exactly one:

* `TODO`
* `IN_PROGRESS`
* `BLOCKED`
* `VERIFYING`
* `COMPLETE`
* `ACCEPTED_DEVIATION`

Definitions:

### TODO

The task has not been started.

### IN_PROGRESS

The task is actively being implemented.

### BLOCKED

The task cannot proceed. The blocker and evidence must be recorded in `STATUS.md`.

### VERIFYING

Implementation exists but acceptance criteria have not yet been fully demonstrated.

### COMPLETE

All acceptance criteria are satisfied and evidence has been recorded.

### ACCEPTED_DEVIATION

Exact N64 reproduction is impossible on PSP, and the difference has been demonstrated, documented and justified.

---

# 3. Task Requirements

Every implementation task must have:

* Objective
* Dependencies
* Acceptance criteria
* Verification
* Evidence
* Relevant files
* Known limitations

Never mark a task `COMPLETE` without evidence.

---

# 4. Reference Hierarchy

When determining original SSB64 behavior:

1. Original SSB64 decompilation
2. Original ROM/data
3. BattleShip
4. sf64-psp
5. n64psp
6. Existing SSB64PSP implementation
7. Engineering assumptions

BattleShip:

`https://github.com/JRickey/BattleShip`

BattleShip is a technical reference, not an authority.

Disagreements must be investigated.

Do not copy Nintendo assets or copyrighted game data from reference projects.

---

# 5. Completed Foundation

## M0 — Research

Status: `COMPLETE`

Original architecture, decompilation and reverse-engineering references established.

---

## M1 — PSP Bootstrap

Status: `COMPLETE`

PSP target builds and the engine executes under the development environment.

---

## M2 — Resource Pipeline

Status: `COMPLETE`

ROM validation, VPK0/relocData processing, extraction and runtime asset-pack infrastructure are operational.

---

## M3 — Core Game / Scene Infrastructure

Status: `COMPLETE`

Core scene, fighter, animation, collision and rendering infrastructure exists.

Remaining renderer work is governed by the rendering milestones below.

---

# 6. R0 — Rendering Correctness

Status: `IN_PROGRESS`

This is the current development gate.

The objective is to determine and reproduce the actual rendering behavior used by SSB64 rather than merely producing visually plausible output.

---

## R0.1 — Rendering State Reconciliation

Status: `COMPLETE`

### Objective

Reconcile the documented renderer state with the actual implementation.

### Dependencies

* M3

### Acceptance

* [x] implementation inventory completed — renderer, texture, mesh, animation and material subsystems inspected against `docs/porting-status.md` and source
* [x] renderer architecture documented — `docs/rendering.md`, README "Architecture"; the stale duplicate `ARCHITECTURE.md` was removed
* [x] stale documentation identified — see 2026-09-02 documentation audit (below)
* [x] known rendering gaps enumerated — `TODO.md` Phases B–H, `docs/porting-status.md` "Known gaps"
* [x] unsupported rendering paths identified — texture wrap modes (hardcoded `Repeat`), mipmap/LOD (generated but does not fix Dream Land canopy, RE-053), material animation (decoded, not played), majority-vote lighting heuristic
* [x] current verification baseline recorded — `cargo test --workspace`: 338 passing (`ssb-rom` 195, `ssb-engine` 36, `ssb-game` 107); `romtool textures`: 647 bound / 617 packed / 30 failed

### Verification

* inspect renderer implementation
* inspect generated asset reports
* run relevant tests
* run PPSSPP baseline

### Evidence

2026-09-02 documentation audit: reconciled `AGENTS.md`, `PLAN.md`, `STATUS.md`,
`README.md`, `DECISIONS.md`, `TODO.md`, `docs/porting-status.md`,
`docs/rendering.md` against current code, git history, and rebuilt asset
reports. Removed a stale duplicate `ARCHITECTURE.md` and a stale "Milestones"
table in `docs/porting-status.md` that contradicted its own subsystem rows.
Corrected a README claim that physical PSP hardware validation was complete
when `STATUS.md`/`PLAN.md` R2 show it is not. See `STATUS.md` for the
current task following this one.

---

## R0.2 — N64 Rendering Command Inventory

Status: `COMPLETE`

### Objective

Enumerate every N64 rendering command and relevant state transition actually exercised by SSB64.

### Dependencies

* R0.1

### Acceptance

* [x] GBI commands identified — `docs/rendering.md` "Measured usage" (`romtool scan`), full opcode/count table and a "never emitted" list
* [x] usage/frequency recorded — same table
* [x] display-list usage mapped — 135 files, 1,864 display lists (`docs/rendering.md`)
* [x] current PSP implementation mapped — `docs/rendering.md` "Display list translation" table
* [x] unsupported commands identified — texture wrap/mirror, mipmap/LOD selection, material animation playback (`docs/rendering.md` "Not yet handled", `TODO.md`)
* [x] relevant RSP/RDP behavior identified — depth inversion (D-007), aspect ratio (D-008), coordinate handling (D-004) in `DECISIONS.md`
* [x] BattleShip cross-reference performed — cloned `refs/BattleShip` + its `libultraship` submodule, read its F3DEX2/S2DEX interpreter (`src/fast/interpreter.cpp`), cross-checked opcode coverage, and recorded findings as RE-054 in `docs/reverse-engineering.md`: opcode coverage agrees with `docs/rendering.md`; found a new lead for R0.13 (S2DEX `G_BG_1CYC`/`G_BG_COPY` mixed into F3DEX2 lists, not decoded by our `dl.rs` at all); corroborated RE-053 (BattleShip has no LOD support either); clarified R0.8's transform question (the custom-MVP draw types patch the RSP's matrix via `G_MW_MATRIX`, but the actual transform is CPU-computed matrix math in `objdisplay.c` — ordinary decomp-porting work per D-001, not novel RDP/RSP behavior); confirmed the wrap-mode gap (R0.5) is real by comparison.

### Verification

* decompilation inspection
* ROM/display-list inspection
* BattleShip comparison — done, see RE-054

### Evidence

RE-054 in `docs/reverse-engineering.md`. New leads recorded in `TODO.md`
Phase B/C/E for R0.13, R0.5 and R0.8 to pick up when their dependencies are
met — this task did not implement fixes for any of them, only recorded
evidence per its own acceptance criteria.

---

## R0.3 — Texture Conversion Completeness

Status: `COMPLETE`

### Objective

Resolve every texture conversion failure that represents a missing required texture path.

### Dependencies

* R0.2

### Acceptance

* [x] all required N64 texture formats supported — RGBA16/32, IA4/8/16, I4/8, CI4/8 (`crates/ssb-rom/src/texture.rs`, `psp_texture.rs`)
* [x] all required textures decode — 617/647 bind-and-decode; the remaining 26 are not decodable textures at all (see below), so this item is satisfied for every texture that actually exists in the ROM
* [x] all required palettes resolve — every palette that exists in the ROM for a texture this converter can reach resolves; the 4 `MissingPalette` cases are a `PartTables` material-pairing gap (RE-057), not a missing/undecoded palette, and are out of this task's scope (R0.7's)
* [x] no unexplained conversion failures remain — the 30 remaining failures are fully explained and attributed: 26 are the LB (loading-break) transition's per-frame framebuffer photocopy, bound to RSP segment 0x1 at runtime and absent from the ROM (RE-055); 4 are palette loss caused by missing/partial `MObj` material-table pairings in three specific files (RE-057)
* [x] framebuffer/screen-wipe failures separately categorized and identified — RE-055 identifies the 26 segment-0x01 entries as `sLBTransitionPhotoHeap` (`refs/ssb-decomp-re/src/lb/lbtransition.c`), R0.13's territory, not R0.3's
* [x] conversion report generated — `romtool textures "rom/Super Smash Bros. (USA).z64"`
* [x] regression tests added where appropriate — `psp_texture::mip_tests` and others in `crates/ssb-rom/src/psp_texture.rs`

### Evidence

Both remaining failure classes were traced to root causes outside texture
conversion, each documented and reclassified to the task that actually owns
a fix:

* 26 segment-0x01 entries → RE-055 → R0.13 (framebuffer effects)
* 4 `MissingPalette` entries → RE-057 → R0.7 (missing material tables); RE-056
  is a superseded partial explanation, corrected by RE-057

No texture format, decode, or palette-resolution bug remains inside this
task's actual scope (texture-conversion logic in
`crates/ssb-rom/src/texture.rs`/`psp_texture.rs`). Closing this task does not
mean the 30 textures pack — it means the reason each one doesn't is now
identified, attributed to the right task, and none of them is a gap in this
task's own subject matter.

### Verification

```bash
cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"
```

---

## R0.4 — TLUT / Palette Correctness

Status: `IN_PROGRESS`

### Objective

Reproduce original N64 CI/TLUT behavior.

### Dependencies

* R0.3

### Acceptance

* [x] CI4 verified — unit-tested decode, dominant format in the ROM (`docs/rendering.md` "Measured usage")
* [x] CI8 verified — unit-tested decode
* [x] TLUT loading behavior verified — the 4 "no TLUT recorded" notes are explained: `MObj` material-table pairing gaps in 3 specific files, not a TLUT-loading bug (RE-057)
* [ ] palette inheritance/state verified — not explicitly tested for state leakage across display lists (see R0.15); RE-057 found a related but distinct issue (palette *loss* on an unresolved segment-0x0E call, not leakage between lists)
* [x] palette pointers verified — resolved through archive extern relocations (RE-037)
* [ ] all missing palette cases resolved — 1 `MissingPalette` failure remains (was 4; files 353 and 52 fixed via RE-059/RE-060), root-caused to a `PartTables` pairing gap in file 86's one remaining graph (RE-057, RE-060); tracked under R0.7, not this task
* [x] regression coverage added — texture decode unit tests in `crates/ssb-rom/src/texture.rs`

---

## R0.5 — Texture Filtering / LOD / Mipmapping

Status: `IN_PROGRESS`

### Current evidence

Mip chains are generated at build time for 151 textures
(`psp_texture::pack_mipped`), but this did **not** resolve the Dream Land
canopy discrepancy (RE-053) — the wrong pattern survives and sharpens at
higher resolution, pointing at magnification rather than minification/LOD.
Wrap modes beyond `Repeat` are decoded from `G_SETTILE` but not wired to
`sceGuTexWrap` (`psp/src/meshdraw.rs` hardcodes `Repeat, Repeat`). Filtering
mode (bilinear vs point) is not yet verified per texture. This task's
explicit acceptance item "Dream Land canopy discrepancy resolved" remains
open — do not close this task while it is.

### Objective

Determine and reproduce the actual texture sampling behavior used by SSB64.

### Dependencies

* R0.2
* R0.3
* R0.4

### Acceptance

* [ ] filtering modes identified from original data
* [ ] magnification behavior identified
* [ ] minification behavior identified
* [ ] LOD behavior identified
* [ ] mipmapping behavior identified
* [ ] texture tile parameters verified
* [ ] texture coordinate behavior verified
* [ ] wrap/clamp/mirror behavior verified
* [ ] Dream Land canopy discrepancy resolved
* [ ] no unsupported mipmapping assumptions remain

---

## R0.6 — Material System Correctness

Status: `IN_PROGRESS`

### Current evidence

`crates/ssb-rom/src/mesh.rs` evaluates a general `(A-B)*C+D` combiner across
both RDP cycles and declines to guess at anything it can't resolve rather
than approximating (RE-039, RE-043). Primitive/environment colour, alpha and
depth state are threaded through. Lighting is **not** derived from `MObj`
light state — a single neutral key light is used as a placeholder
(`DECISIONS.md` D-024), and `TODO.md` Phase D explicitly calls out removing
this "majority-vote lighting heuristic." Per `AGENTS.md` §9, an approximation
like this needs to be recorded as an `ACCEPTED_DEVIATION` with its effect
measured, or replaced — it is currently neither.

### Objective

Reproduce original SSB64 material behavior.

### Dependencies

* R0.2
* R0.4

### Acceptance

* [ ] material tables resolved
* [ ] combiner behavior verified
* [ ] primitive color verified
* [ ] environment color verified
* [ ] lighting verified
* [ ] alpha behavior verified
* [ ] blending verified
* [ ] fog verified
* [ ] depth state verified
* [ ] culling verified
* [ ] unsupported material behavior identified

---

## R0.7 — Missing Material Tables

Status: `IN_PROGRESS`

### Current evidence

Freshly re-measured this session (`romtool mobj`, whole archive, after both
fixes below): **63 graphs paired, 64 unpaired.** Started the session at
56/71.

RE-057 found three concrete test cases while investigating R0.3's
`MissingPalette` failures: files 52 (`MVCommon`), 86 (`ITCommonObject`) and
353 (`LinkSpecial2`) get zero or partial `MObj` materials from
`PartTables::scan` for their scene graphs. RE-057's guess that 353's table
lives in a sibling file is **retracted by RE-058**: 353 already declares its
own graph and its own `MObjSub` table in the same file.

Four distinct pairing mechanisms are now known, only one of which
`PartTables::scan` can discover on its own:

1. `FTCommonPart` (fighters) and `MPGroundDesc` (stages) — one struct, two
   adjacent pointer fields, living in the archive. `PartTables::scan` finds
   these structurally.
2. `WPAttributes` (RE-058, weapon/projectile sub-objects,
   `refs/ssb-decomp-re/src/wp/wptypes.h:36-45`) — same adjacency, also in the
   archive in principle, but the one confirmed instance checked (Link's
   boomerang) has `p_mobjsubs = NULL` by design, and file 353's own instance
   (Spin Attack) is not yet typed in the decompilation, so nothing has
   actually been fixed via this shape yet.
3. `EFDesc` (RE-059, fighter entrance effects,
   `refs/ssb-decomp-re/src/ef/eftypes.h:11-24`) — same adjacency, but the
   instances live in the game's **static executable**, not any archive file,
   so no scan can ever find them. Fixed 2 graphs in file 353 (`EntryWave`,
   `EntryBeam`) via hand-entered `PartTables::insert()` calls in
   `tools/romtool/src/main.rs`'s `load_all`.
4. Plain call-sequence pairing (RE-060, the opening movie's room scene,
   `refs/ssb-decomp-re/src/mv/mvopening/mvopeningroom.c`) — **no struct at
   all**; `gcSetupCommonDObjs(gobj, dobjdesc)` and a separate
   `gcAddMObjAll(gobj, mobjsub)` call are the only link, encoded purely in
   the executable's call order. Fixed all 5 of file 52's unpaired graphs,
   fully resolving that file, via 5 more hand-entered `PartTables::insert()`
   calls.

Verified: `romtool mobj --file 353` and `--file 52` both show 0 chain/demand
mismatches on every newly-paired graph. `romtool textures`: file 353 1→0
failures, file 52 several→0 failures (58/58 packed). Archive-wide
`romtool textures`: 617→638 packed (665 unique bound, up from 647 — several
primitives that previously drew with no texture at all now correctly
resolve one), `MissingPalette` 4→1. `cargo test --workspace`: 338 passing,
unaffected throughout (both fixes live in `romtool`, not the library crate).

File 86's one remaining graph (an "NBumper" item) uses a **fifth**
mechanism — a compile-time byte-offset delta from a runtime base pointer
(`itGetPData`, `refs/ssb-decomp-re/src/it/itcommon/itnbumper.c:367`).
RE-061 measured this rather than guessing at it: neither linker symbol the
delta is computed from is typed anywhere in the decompilation, and
`romtool mobj --file 86 --search` returns 27 candidate table offsets for
this graph — the same kind of near-chance fingerprint match the project has
already measured and rejected once (Samus's two identical 33-node graphs,
`mobj.rs`'s own doc comment). Left unfixed on purpose. File 353's third
graph (Spin Attack) still needs its `WPAttributes` instance typed before it
can be inserted. These two plus the other 62 unpaired graphs archive-wide
are now treated as an accepted long tail rather than a task-blocking gap;
R0.7 stays `IN_PROGRESS` but further progress here depends on upstream
decomp typing, not more `romtool` investigation.

### Objective

Resolve every scene graph containing an unresolved material table.

### Dependencies

* R0.6

### Acceptance

* [ ] all material-table references traced — 5 shapes now known (`FTCommonPart`, `MPGroundDesc`, `WPAttributes`, `EFDesc`, plain call-sequence pairing); files 52 and 353 fully or mostly traced; file 86's last graph's mechanism is understood but does not narrow to one table (RE-061, measured: 27 candidates, no named record); 63 other archive-wide unpaired graphs are untraced
* [ ] original material data identified — done for 7 pairings (2 `EFDesc` in file 353, 5 call-sequence in file 52); not done for the other 64 unpaired graphs, and file 86's/353's remaining two are blocked on upstream decomp typing, not more tracing
* [ ] heuristic mapping removed where original data exists — n/a so far, no heuristic was standing in for these; this was a pure discovery gap
* [ ] affected scenes verified — file 353's two and file 52's five fixed graphs verified via `romtool mobj`/`romtool textures` (RE-059, RE-060); nothing else verified yet
* [ ] regression coverage added — no `cargo test` coverage; the fix lives in `romtool` (a CLI tool, not the library crate), and the project's existing regression pattern for ROM-dependent behavior is a `romtool` command's own output (matching how R0.9 verifies stage animation), not a unit test. `romtool mobj --file 353`/`--file 52`'s 0-mismatch checks are that regression detector for these fixes.

---

## R0.8 — Transform Correctness

Status: `IN_PROGRESS`

### Current evidence

Billboard matrix kinds 45–48 (translate + camera-facing basis) are
implemented and verified A/B against a rotated camera (RE-049). RE-062 read
`gcPrepDObjMatrix`'s actual switch case for `0x8000`/`RecalcRotRpyRSca`
(case 44, `objdisplay.c:822`): it never touches `dobj->rotate`, computing
the same diagonal-from-`gGCMatrixPerspF` MVP as kinds 45/46 with the
`sin`/`cos` spin term dropped — a full camera-facing billboard, not a
distinct transform. A whole-archive check found 0 of the ROM's 28
`RecalcRotRpyRSca` nodes have non-zero `rotate`, confirming the field is
genuinely dead for this kind. Fixed: `crates/ssb-rom/src/pack.rs`'s
`add_object` now flags these nodes `FLAG_BILLBOARD` alongside `Kind46`/
`Kind48`, reusing the already-verified billboard render path exactly (spin
term is a no-op `0.0`). `cargo test --workspace`: 339 passing (new test
`a_recalc_node_is_flagged_as_a_spin_free_billboard`). `cargo psp --release`
builds; PPSSPP smoke-run (`tools/run-ppsspp.sh --seconds 8`) shows 60 FPS
and a clean log, but did not isolate a specific `0x8000` object on screen.

### Objective

Implement every transform kind exercised by SSB64.

### Dependencies

* R0.1
* R0.2

### Acceptance

* [ ] transform kinds enumerated
* [x] `0x8000` investigated — RE-062: it's a spin-free variant of the
  already-implemented billboard kinds 45/46, not a distinct transform; fixed
  by flagging it `FLAG_BILLBOARD` the same way
* [ ] original matrix behavior identified
* [ ] PSP implementation verified
* [ ] affected scene nodes tested
* [ ] billboard-related transforms cross-checked

---

## R0.9 — Stage Animation

Status: `COMPLETE`

### Objective

Reproduce original stage animation behavior.

### Dependencies

* R0.6
* R0.8

### Acceptance

* [x] all stage animation formats understood — 32-bit `AObjEvent32` joint stream (D-020)
* [x] event encoding verified — RE-050
* [x] timing verified — all 206 animated nodes still looping correctly after 600 frames (RE-050)
* [x] interpolation verified — `AObj` cubic/linear/step ported and exercised
* [x] animation playback verified — plays on device (PPSSPP): 35 stages, 206 animated nodes, 60 FPS
* [x] independent ROM comparison exists — three independent checks agree: ROM replay (RE-050), packed-pose-vs-archive across 444,960 values (RE-052), and a two-frame device diff showing motion only over the animated canopy (RE-051)
* [x] all stages tested — all 41 stages' animation data was checked; 35 carry joint animation and 6 (including Dream Land) do not — Dream Land's scenery instead moves via non-joint game code, which is out of this task's scope

### Evidence

`docs/porting-status.md` "Stage animation" row; RE-050, RE-051, RE-052 in
`docs/reverse-engineering.md`. Validated under PPSSPP, not yet on physical
hardware — that gap belongs to R2, not this task.

---

## R0.10 — Material Animation

Status: `TODO`

### Current evidence

The 12-layer material animation script is decoded but not played
(`docs/porting-status.md` "Stage animation": "read but not played"). Frame 0
happens to match the baked colours already shipped, so nothing currently
renders visibly wrong — but that is coincidence, not implementation. Genuinely
not started; `TODO.md` Phase F.

### Objective

Implement material animation used by SSB64.

### Dependencies

* R0.6
* R0.9

### Acceptance

* [ ] animation data decoded
* [ ] runtime clock implemented
* [ ] material state updated correctly
* [ ] representative animated materials verified
* [ ] stage material animation verified
* [ ] fighter material animation verified where applicable

---

## R0.11 — Fighter Palettes / Costumes

Status: `IN_PROGRESS`

### Current evidence

Per-costume-0 colours are recovered and render correctly (e.g. Mario in red,
via `FTCommonPart::p_costume_matanim_joints`, RE-040). Only costume 0 is
currently packed for any fighter — every other costume is unimplemented, not
merely unverified (`docs/porting-status.md` "Model conversion").

### Objective

Ensure every required fighter visual variant renders correctly.

### Dependencies

* R0.4
* R0.6
* R0.10

### Acceptance

* [ ] all fighter palettes identified
* [ ] all required costumes identified
* [ ] runtime representation complete
* [ ] palette data verified against ROM
* [ ] representative regression renders added
* [ ] all required fighters verified

---

## R0.12 — Billboard Correctness

Status: `VERIFYING`

### Current evidence

Matrix kinds 45–48 are implemented, all 81 flagged nodes billboard at draw
time, and behavior was verified A/B under a deliberately rotated camera
(RE-049; Dream Land's six canopy sprites stay upright when honoured, skew
into slivers when ignored). Not yet verified: alpha and depth behavior
specifically for billboards, and the decomp's `rot_mode` choice between
matrix kinds 45/46 is not modelled. Depends on R0.14 (camera/projection),
which is itself only partially verified.

### Objective

Verify every billboard rendering path.

### Dependencies

* R0.8
* R0.14

### Acceptance

* [ ] billboard types enumerated
* [ ] camera-facing transforms verified
* [ ] scale verified
* [ ] orientation verified
* [ ] texture orientation verified
* [ ] alpha behavior verified
* [ ] depth behavior verified
* [ ] all flagged billboard nodes verified

---

## R0.13 — Framebuffer Rendering

Status: `TODO`

### Current evidence

No framebuffer-based rendering path (render-to-texture, screen wipes) is
implemented. RE-055 (`docs/reverse-engineering.md`) identifies the concrete
target: the LB (loading-break) transition system's `sLBTransitionPhotoHeap`,
a `300x220` 16-bit heap buffer the engine fills with a copy of the last frame
drawn to the framebuffer (`refs/ssb-decomp-re/src/sys/... /lb/lbtransition.c`),
bound to RSP segment `0x1` once per frame and sampled by 11 between-match
transition effects (aeroplane, curtain, cannon, star, bamboo-blind ×2,
camera, block, rotscale, check, "gakubuthi"). These are exactly the 26
segment-0x01 entries currently reported (and accepted as out of scope) under
R0.3. Genuinely not started.

### Objective

Implement every framebuffer-based rendering path required by SSB64.

### Dependencies

* R0.2
* R0.6

### Acceptance

* [ ] framebuffer usage identified
* [ ] framebuffer texture paths implemented
* [ ] screen wipes implemented
* [ ] render-to-texture paths implemented where required
* [ ] framebuffer synchronization verified
* [ ] visual verification completed

---

## R0.14 — Camera / Projection Correctness

Status: `IN_PROGRESS`

### Current evidence

Pillarboxed 362×272 viewport is implemented and applied to both
`sceGuViewport` and `sceGuScissor` (D-008, RE-034). Depth range inversion is
implemented and verified (D-007). Full projection-matrix and camera-transform
correctness against the original's camera behavior has not been separately
verified — this task and R0.12 (billboards) share that open dependency.

### Objective

Reproduce the original camera and projection behavior.

### Dependencies

* R0.8

### Acceptance

* [ ] projection matrix verified
* [ ] viewport verified
* [ ] aspect ratio verified
* [ ] depth mapping verified
* [ ] camera transforms verified
* [ ] N64/PSP resolution differences explicitly handled
* [ ] representative scenes compared

---

## R0.15 — Render-State Isolation

Status: `TODO`

### Current evidence

No dedicated state-leakage tests were found. `RE-010`'s unresolved `MObjSub`
fields and the majority-vote lighting heuristic (R0.6) are related open
questions but do not by themselves demonstrate isolation. Genuinely not
started as a distinct verification effort.

### Objective

Ensure render state cannot incorrectly leak between display-list/material/node draws.

### Dependencies

* R0.2
* R0.6

### Acceptance

* [ ] texture state tracked
* [ ] TLUT state tracked
* [ ] combiner state tracked
* [ ] primitive color tracked
* [ ] environment color tracked
* [ ] blend state tracked
* [ ] depth state tracked
* [ ] culling tracked
* [ ] geometry state tracked
* [ ] texture addressing tracked
* [ ] state leakage tests added

---

# 7. R1 — Rendering Completeness

Status: `BLOCKED_BY_R0`

R1 cannot begin until R0 is complete.

### Objective

Demonstrate that every discovered SSB64 rendering path required for the game is implemented.

### Acceptance

* [ ] all stages render
* [ ] all fighters render
* [ ] all required costumes render
* [ ] all required animations render
* [ ] all required effects render
* [ ] all required framebuffer paths render
* [ ] no unexplained rendering commands remain
* [ ] no unexplained missing assets remain
* [ ] no unexplained material failures remain
* [ ] rendering regression suite passes
* [ ] golden/reference renders are established

---

# 8. R2 — Physical PSP Rendering Validation

Status: `BLOCKED_BY_R1`

### Objective

Verify the renderer on actual PSP hardware.

PPSSPP is not sufficient.

### Acceptance

* [ ] EBOOT boots on physical PSP
* [ ] runtime asset pack loads
* [ ] representative fighters render
* [ ] representative stages render
* [ ] fighter animation works
* [ ] stage animation works
* [ ] materials render correctly
* [ ] textures render correctly
* [ ] framebuffer effects work
* [ ] VRAM usage verified
* [ ] no hardware-only rendering failures remain
* [ ] hardware model recorded
* [ ] build/environment recorded

---

# 9. R3 — Rendering Performance

Status: `BLOCKED_BY_R2`

### Objective

Optimize rendering without sacrificing fidelity.

### Acceptance

* [ ] frame time measured
* [ ] CPU bottlenecks identified
* [ ] GPU/GE bottlenecks identified
* [ ] texture upload costs measured
* [ ] VRAM usage measured
* [ ] memory bandwidth considered
* [ ] 60 FPS target evaluated on physical hardware
* [ ] optimizations revalidated against rendering tests
* [ ] no fidelity regressions introduced

Do not perform speculative optimization before measurement.

---

# 10. G0 — Combat Unlocked

Status: `BLOCKED_BY_R3`

Combat becomes eligible only after R0, R1, R2 and R3 are complete.

### First Combat Vertical Slice

* [ ] input
* [ ] one grounded attack
* [ ] attack animation
* [ ] hitbox
* [ ] hurtbox
* [ ] collision interaction
* [ ] damage
* [ ] knockback
* [ ] hitstun
* [ ] KO
* [ ] minimal opponent
* [ ] minimal stock handling
* [ ] match loop

The exact combat implementation should then follow the original decompilation rather than invented mechanics.

---

# 11. Post-Combat Roadmap

After G0:

## G1 — Full Combat

* all attacks
* specials
* grabs
* throws
* shields
* dodges
* aerial systems
* damage states
* knockback states
* recovery
* items
* hazards
* CPU behavior

## G2 — Complete Match Systems

* stage selection
* character selection
* stocks
* timers
* win conditions
* match transitions
* camera behavior
* multiplayer/input handling

## G3 — Menus and Persistence

* title screen
* menus
* character selection
* stage selection
* options
* save data
* progression/unlock systems

## G4 — Audio

* music
* sound effects
* voice
* audio mixing
* positional behavior
* memory management

## G5 — Final Optimization

* CPU profiling
* GE profiling
* memory profiling
* VRAM optimization
* loading optimization
* asset streaming
* frame pacing

---

# 12. Rendering Definition of Done

Rendering may only be declared complete when:

1. Every discovered rendering subsystem has an explicit status.
2. Every unsupported behavior has been investigated.
3. Every remaining deviation is documented.
4. All known texture failures are resolved or accepted with evidence.
5. All known material failures are resolved or accepted with evidence.
6. All required transform kinds are resolved.
7. Texture filtering is understood.
8. LOD behavior is understood.
9. Mipmapping behavior is understood.
10. Stage animation works.
11. Material animation works where required.
12. Fighter palettes/costumes work.
13. Billboard behavior is verified.
14. Framebuffer effects work.
15. Camera/projection behavior is verified.
16. Render-state leakage is eliminated.
17. All required fighters render.
18. All required stages render.
19. All required effects render.
20. Golden/reference rendering tests pass.
21. PPSSPP verification passes.
22. Physical PSP verification passes.
23. VRAM usage is safe.
24. Performance has been measured.
25. Documentation agrees with implementation.

Only then may R0/R1/R2/R3 be completed and combat unlocked.

---

# 13. Autonomous Execution Rule

When continuing autonomously:

```text
READ STATUS.md
      ↓
RESUME IN_PROGRESS TASK
      ↓
IF NONE:
SELECT FIRST ELIGIBLE TODO FROM PLAN
      ↓
CHECK DEPENDENCIES
      ↓
INVESTIGATE ORIGINAL BEHAVIOR
      ↓
IMPLEMENT
      ↓
TEST
      ↓
VERIFY AGAINST DECOMP / ROM
      ↓
CHECK BATTLESHIP
      ↓
UPDATE DOCUMENTATION
      ↓
UPDATE STATUS.md
      ↓
RECORD EVIDENCE
      ↓
COMMIT
      ↓
SELECT NEXT TASK
```

Never advance merely because code compiles.

Never advance by weakening acceptance criteria.

Never bypass the rendering gate.

---

# 14. End Goal

The intended progression is:

```text
Research
    ↓
PSP bootstrap
    ↓
Resource pipeline
    ↓
Core game/scene infrastructure
    ↓
Rendering correctness
    ↓
Rendering completeness
    ↓
Physical PSP validation
    ↓
Rendering performance
    ↓
COMBAT UNLOCKED
    ↓
Full combat
    ↓
Complete match systems
    ↓
Menus / save
    ↓
Audio
    ↓
Final optimization
    ↓
Complete SSB64 implementation
```

**Gameplay does not advance around a broken renderer.**
