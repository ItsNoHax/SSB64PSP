# Project Status

**Last updated:** 2026-09-02

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

`R0.3 — Texture Conversion Completeness`

## Task Status

`IN_PROGRESS`

617/647 textures convert. 30 remain: 26 are the LB (loading-break) transition
system's runtime framebuffer photocopy, bound to RSP segment 0x1 and never
present in any ROM file — confirmed against the decompilation and accepted as
out of scope for this task (RE-055; real work belongs to R0.13). RE-054's
S2DEX-BG lead is refuted (`romtool scan --exhaustive` finds zero `G_BG_1CYC`/
`G_BG_COPY` anywhere in the ROM). The only remaining R0.3 gap is 4
`MissingPalette` cases; RE-056 has an unconfirmed lead (a valid palette load
exists for at least one occurrence of the failing texture, but `romtool`'s
dedup key may be evaluating a different, palette-less occurrence). Next step:
confirm which occurrence `mesh::convert_sequence` visits first for each of
the 4 cases and whether `State::forget_texture()` is what clears the palette
in between.

## Last Completed Task

R0.3 is not yet complete, but its segment-0x01 question (the concrete next
step recorded after R0.2) is now resolved: RE-055 identifies all 26
segment-0x01 texture failures as the LB transition system's runtime
framebuffer photocopy (`sLBTransitionPhotoHeap`, RSP segment 0x1, decomp
`refs/ssb-decomp-re/src/lb/lbtransition.c`), refuting RE-054's S2DEX-BG lead
(`romtool scan --exhaustive`: zero `G_BG_1CYC`/`G_BG_COPY` in the whole ROM).
RE-056 records an unconfirmed lead on the remaining 4 `MissingPalette`
cases. See `PLAN.md` R0.3 and R0.13.

`R0.2 — N64 Rendering Command Inventory`, `R0.1 — Rendering State
Reconciliation` and `R0.9 — Stage Animation` are `COMPLETE` — see `PLAN.md`
for each.

## Next Eligible Task

`R0.3` (in progress) — finish the 4 `MissingPalette` cases per RE-056's lead.
`R0.8` is also eligible (dependencies R0.1/R0.2 complete, has a lead from
RE-054) and `R0.13` now has a concrete, decomp-grounded target (RE-055) but
remains blocked on `R0.6`, which is only `IN_PROGRESS`.

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

## R0.3 — Texture Conversion Completeness

Status: `IN_PROGRESS`

### Objective

Resolve every texture conversion failure that represents a missing required texture path.

### Required Work

* [x] Check whether the 26 segment-0x01 failures are S2DEX `G_BG_1CYC`/`G_BG_COPY` background draws (RE-054 lead) — refuted; they are `sLBTransitionPhotoHeap` (RE-055), an RSP-segment-bound runtime framebuffer copy with no ROM presence
* [x] Accept the 26 segment-0x01 entries as out of scope for R0.3, with evidence (RE-055) — no texture-conversion fix applies; real work is R0.13
* [ ] Resolve or accept-with-evidence the 4 `MissingPalette` cases — RE-056 has a lead (dedup key may be evaluating a palette-less occurrence of a texture that has a valid occurrence elsewhere in the same file), not yet confirmed or fixed

### Completion Evidence

Record:

* which hypothesis the raw display-list bytes support, and how that was checked — done, see RE-055/RE-056
* the fix implemented (or the accepted-deviation writeup if no fix is warranted) — accepted-deviation writeup done for the 26 segment-0x01 entries (PLAN.md R0.3 "Remaining gap"); the 4 `MissingPalette` cases still need either a fix or their own accepted-deviation writeup
* before/after `romtool textures` output — no change yet; investigation only, no code touched
* regression test added — none yet; nothing has been fixed in code

---

# 7. Last Verification

## 2026-09-02 — R0.3 segment-0x01 investigation

* `cargo run --release -p romtool -- scan "rom/Super Smash Bros. (USA).z64" --exhaustive` — 0 occurrences of opcode 0x09/0x0A anywhere in the ROM's display lists; refutes RE-054's S2DEX BG lead
* `cargo run --release -p romtool -- dump "rom/Super Smash Bros. (USA).z64" 39` (and 40/41/45/50/51) — dumped raw file bytes, located the failing `G_SETTIMG` (file 39 offset 0x0E10: `fd10012b 01000000`), confirmed identical bytes recur across files 40/41/45/50/51
* Cross-checked address `0x01000000` (segment 1) against `refs/ssb-decomp-re/src/lb/lbtransition.c` — `gSPSegment(..., 0x1, sLBTransitionPhotoHeap)`, a per-frame `300x220` 16-bit framebuffer photocopy for the loading-break transition system; texture dims from `romtool textures --file 39` (`300x5`, `300x6` `Rgba/Bits16`) match
* Investigated the 4 `MissingPalette` cases (files 52, 86, 353): found a valid `G_LOADTLUT`-preceded occurrence of one failing texture (file 52, offset 0x1960, one of 6 occurrences) but did not confirm this explains the failure — recorded as RE-056, a lead not a fix
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
