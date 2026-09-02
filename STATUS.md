# Project Status

**Last updated:** 2026-09-02

---

# 1. Execution State

## Current Milestone

`R0 — Rendering Correctness`

## Current Task

`R0.2 — N64 Rendering Command Inventory`

## Task Status

`VERIFYING`

Every acceptance item except one is satisfied with evidence already recorded
in `docs/rendering.md`. The outstanding item is the BattleShip cross-reference
required by `AGENTS.md` §10: `refs/BattleShip` is not cloned in this
checkout, and `docs/reverse-engineering.md` has zero references to it. Next
step: clone it per README "Local reference setup" and cross-reference its
GBI/RDP handling against the opcode inventory in `docs/rendering.md`
"Measured usage," then close R0.2.

## Last Completed Task

`R0.1 — Rendering State Reconciliation` (documentation audit: reconciled
`AGENTS.md`/`PLAN.md`/`STATUS.md`/`README.md`/`DECISIONS.md`/`TODO.md` and
`docs/porting-status.md`/`docs/rendering.md` against actual code, removed a
stale duplicate `ARCHITECTURE.md`, corrected a README claim that physical PSP
validation was complete, refreshed test/texture-conversion counts). See
`PLAN.md` R0.1 for full evidence.

`R0.9 — Stage Animation` is also `COMPLETE`, validated three independent ways
(RE-050/051/052) — see `PLAN.md` R0.9. It was already substantially done
before this documentation pass; this pass only recorded that fact.

## Next Eligible Task

`R0.2` (in progress, `VERIFYING`) — see above. After it closes, the next
eligible `TODO` tasks in dependency order are `R0.3` (texture conversion
completeness — 26 segment-0x01 cross-file failures, 4 missing palettes) and
`R0.13`/`R0.15` (framebuffer rendering, render-state isolation), which have
no implementation started yet. `R0.4` through `R0.12` and `R0.14` are
`IN_PROGRESS`/`VERIFYING` with real but partial progress — see `PLAN.md` for
each task's current evidence.

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

## R0.2 — N64 Rendering Command Inventory

Status: `VERIFYING`

### Objective

Enumerate every N64 rendering command and relevant state transition actually exercised by SSB64.

### Required Work

* [x] GBI commands identified, usage/frequency recorded, display-list usage mapped, current PSP implementation mapped, unsupported commands identified, relevant RSP/RDP behavior identified — all done, see `docs/rendering.md` "Measured usage"
* [ ] BattleShip cross-reference — clone `refs/BattleShip` (README "Local reference setup") and cross-reference its GBI/RDP handling against `docs/rendering.md`'s opcode inventory; record findings in `docs/reverse-engineering.md` as a new `RE-XXX` entry

### Completion Evidence

Record once the BattleShip cross-reference is done:

* which BattleShip source files were consulted
* what agreed / disagreed with the decompilation or ROM evidence
* any documentation updates that result

---

# 7. Last Verification

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
