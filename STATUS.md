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

617/647 textures convert. 30 remain: 26 segment-0x01 cross-file references,
4 `MissingPalette`. Next step: confirm/refute the RE-054 lead that some or
all of the 26 segment-0x01 failures are S2DEX `G_BG_1CYC`/`G_BG_COPY`
full-screen background draws (not decoded by `crates/ssb-rom/src/dl.rs` at
all today) rather than ordinary cross-file `DObjDLLink`/`DObjMultiList`
texture references. Check which hypothesis the actual failing display lists
support before writing a fix for either.

## Last Completed Task

`R0.2 — N64 Rendering Command Inventory` — closed the one open acceptance
item (BattleShip cross-reference, `AGENTS.md` §10) by cloning
`refs/BattleShip` + its `libultraship` submodule and reading its F3DEX2/S2DEX
interpreter. Recorded as RE-054 in `docs/reverse-engineering.md`. Produced
three new leads, now tracked in `TODO.md`: a candidate mechanism for R0.13
(S2DEX BG commands), corroboration that R0.5's Dream Land canopy issue isn't
LOD-related (BattleShip has no LOD support either), and clarification that
R0.8's `0x8000`/`RecalcRotRpyRSca` transform is CPU-computed matrix math to
port from `objdisplay.c`, not novel RDP/RSP behavior. See `PLAN.md` R0.2.

`R0.1 — Rendering State Reconciliation` and `R0.9 — Stage Animation` are also
`COMPLETE` — see `PLAN.md` for each.

## Next Eligible Task

`R0.3` (in progress) — see above. `R0.8` is also newly eligible (its
dependencies R0.1/R0.2 are both now complete) and has a fresher lead than
before (RE-054); either is a reasonable next task once R0.3's immediate
question is answered. `R0.13`/`R0.15` remain genuinely untouched but are
blocked on `R0.6`, which is only `IN_PROGRESS`.

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

* [ ] Check whether the 26 segment-0x01 failures are S2DEX `G_BG_1CYC`/`G_BG_COPY` background draws (RE-054 lead) — pull one failing display list by file/offset from `romtool textures` and inspect its raw opcode bytes at the failing `G_SETTIMG`'s containing list, rather than assuming either hypothesis
* [ ] If confirmed: decode `G_BG_1CYC`/`G_BG_COPY` in `crates/ssb-rom/src/dl.rs` (opcodes 0x09/0x0a under F3DEX2 numbering — verify against our own ROM, BattleShip's numbering may not be load-bearing here) and figure out what the PSP-side draw should be
* [ ] If refuted: continue tracing the `DObjDLLink`/`DObjMultiList` cross-file path per the original Phase B plan
* [ ] Resolve or accept-with-evidence the 4 `MissingPalette` cases

### Completion Evidence

Record:

* which hypothesis the raw display-list bytes support, and how that was checked
* the fix implemented (or the accepted-deviation writeup if no fix is warranted)
* before/after `romtool textures` output
* regression test added

---

# 7. Last Verification

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
