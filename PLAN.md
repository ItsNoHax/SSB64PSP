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

Match combat (`G0`–`G2`) does not begin until the rendering gate has been
explicitly passed. `F1` (§6.5) is the one explicit, scoped exception: a
training-mode combat sandbox against a single stationary dummy target, with
no stocks/KO/match loop/CPU AI, built in parallel with `R3`. See
`AGENTS.md`'s non-negotiable constraints and `plans/gameplay/F1.md`.

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
5. oot-PSP
6. n64psp
7. Existing SSB64PSP implementation
8. Engineering assumptions

BattleShip:

`https://github.com/JRickey/BattleShip`

sf64-psp:

`https://github.com/TheMrIron2/sf64-psp`

oot-PSP:

`https://github.com/z2442/oot-PSP`

n64psp:

`https://github.com/TheMrIron2/n64psp`

BattleShip, sf64-psp, oot-PSP and n64psp are all technical references, not authorities (`DECISIONS.md` D-037). `sf64-psp` and `oot-PSP` both target the PSP, which makes their `sceGu`/texture/material translation choices directly comparable — see R0.18.

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

Status: `VERIFYING` — R2.0's filtering/addressing reopening and R2.1's texgen
audit are complete (RE-219–239); R2.2's renderer corrective gate is complete
(RE-240–261). R0.5, R0.6, R0.15 and R0.16 are stable again. R0.10's
explicitly-scoped material-animation remainder and the physical R2 matrix
remain; R2 and R3 are still required before combat unlock.

This is the current development gate.

The objective is to determine and reproduce the actual rendering behavior used by SSB64 rather than merely producing visually plausible output.

## 6.0 Rendering-Correctness Hierarchy (Cross-Reference)

This project already organizes rendering-correctness work as `R0.1`–`R0.18`
below, not as a separate top-level `R1`–`R8` sequence — this repository's own
top-level milestone names `R1`/`R2`/`R3` (§7–§9) already mean *Rendering
Completeness* / *Physical PSP Validation* / *Rendering Performance*. A second,
unrelated `R1`–`R8` would collide with those names. The table below maps each
rendering-correctness category onto the `R0.x` task(s) that actually own it,
so nothing is duplicated and nothing is missing an owner.

| Correctness category | Owning task(s) | Status |
| --- | --- | --- |
| Geometry (vertex positions/colors/normals, triangle topology, culling, matrix transforms, projection, viewport/scissor, coordinate conventions) | R0.8 (transforms), R0.14 (camera/projection), R0.6 (culling/geometry-mode defaults) | `COMPLETE` |
| N64 render-state model (faithful intermediate representation; must not collapse to `mesh + texture + basic colour`) | **R0.16**, R0.15 (render-state isolation), R0.6 (state threading) | `COMPLETE` — RE-217's findings closed by R2.2/C1–C7 (RE-240–261) |
| Texture correctness (formats, CI4/CI8, TLUT/palette lifetime, relocation, dimensions, coordinate scaling, filtering, LOD, mipmaps, clamp/mirror/repeat, masks/shifts) | R0.3, R0.4, R0.5, **R2.0** | `COMPLETE` — R2.0 (RE-219–224) measured filtering, addressing and remaining `G_SETTILE` fields; N64 three-point vs PSP bilinear is an explicit accepted deviation, while the measurable addressing/palette gaps were fixed |
| Combiner correctness (`G_SETCOMBINE` shapes, TEXEL0/TEXEL1/SHADE/PRIMITIVE/ENVIRONMENT, RGB/alpha, interpolation/modulation) | R0.6 | `COMPLETE` for classified static paths; runtime shield colours deferred with their effect path (RE-168) |
| Lighting correctness (`G_LIGHTING`, shading, normals, vertex colors, material interaction, ambient/directional lights) | R0.6 / R2.2-C1/C2 | `COMPLETE` — C1/C2 (RE-240–243) close PRIM ownership and load-time provenance; RE-261 preserves costume light tracks and passes the integrated 16-scene matrix |
| Alpha/blending correctness (alpha compare/test, source/destination blending, translucent vs. opaque, depth writes, render ordering) | R0.6 | `COMPLETE` for the classified single-cycle formulas (RE-129/130); rare `PRIM_ALPHA` and two-cycle cases remain documented declines |
| Depth/culling correctness (depth direction/range/function/writes, polygon culling, winding, clipping) | R0.6 / R0.14 / R2.2-C3 | `COMPLETE` for `R2.2`/C3 — RE-244 through RE-250 (parts 1-7) built and measured independent `depth_test`/`depth_write`/`depth_mode` fields, traced and seeded every external wrapper found (fighter skeleton, stage render-layer 1, the 11 loading-break transitions, layer 1's list-1 translucent entries), and confirmed the remainder is real archive content, not a missing seed; RE-251 (part 8) wired `psp/src/meshdraw.rs`'s `apply_material` to that state directly (`GuState::DepthTest`/`sceGuDepthMask`, superseding the interim `z_buffer` proxy), measured the golden-scene impact against a same-environment pre/post rebuild (9/13 existing scenes byte-identical, 4 change by a small, visually-explainable, localized amount), and added a self-validating synthetic `depth_mask_diagnostic` regression (`psp/src/depth_diag.rs`) proving the translucent-front/opaque-behind ON→OFF→ON `sceGuDepthMask` switch with PPSSPP evidence; RE-271 physically ran it, found the diagnostic's own `quad()` helper reused one static vertex buffer across three sequential `draw_triangles` calls in the same frame — the same GE-race class `draw_line_strip` was already fixed for — fixed `Gpu::draw_triangles` to copy into arena memory the same way, and re-confirmed the diagnostic matches its golden on physical PSP hardware |
| Render-pass completeness (transparency, particles, shadows, framebuffer effects, UI, other passes) | R0.12 (billboards), R0.13 (framebuffer), top-level R1 §7 (completeness gate) | R1 renderer scope `COMPLETE`: transparency, particles, billboards and framebuffer effects pass RE-261; shadows and real UI remain future systems |
| Visual-regression methodology (deterministic test scenes; reference vs. PPSSPP-software vs. PPSSPP-hardware vs. physical PSP; test matrix) | **R0.17** | `COMPLETE` |
| Reference-port comparative audit (sf64-psp, oot-PSP) | **R0.18** | `COMPLETE` |

`R0.16`, `R0.17` and `R0.18` were added below to close the gaps this
table identifies: this project already has extensive, evidence-driven
per-feature correctness work (`R0.1`–`R0.15`), but no task previously owned
(a) auditing whether the intermediate representation itself is faithful
rather than merely "whatever the current code happens to carry through", (b)
a deterministic, repeatable visual-regression methodology, or (c) a
systematic comparison against `sf64-psp`/`oot-PSP` beyond the ad hoc
BattleShip cross-checks already recorded in `docs/reverse-engineering.md`.

---

### R0 task table

| Task | Status | Task spec | Evidence |
|---|---|---|---|
| **R0.1** — Rendering State Reconciliation | `COMPLETE` | [plans/rendering/R0.1.md](plans/rendering/R0.1.md) | RE-201 |
| **R0.2** — N64 Rendering Command Inventory | `COMPLETE` | [plans/rendering/R0.2.md](plans/rendering/R0.2.md) | RE-053, RE-054 |
| **R0.3** — Texture Conversion Completeness | `COMPLETE` | [plans/rendering/R0.3.md](plans/rendering/R0.3.md) | RE-055, RE-056, RE-057 |
| **R0.4** — TLUT / Palette Correctness | `COMPLETE` | [plans/rendering/R0.4.md](plans/rendering/R0.4.md) | RE-037, RE-057, RE-064, RE-162 |
| **R0.5** — Texture Filtering / LOD / Mipmapping | `COMPLETE` | [plans/rendering/R0.5.md](plans/rendering/R0.5.md) | RE-044–RE-262 (25 records) |
| **R0.6** — Material System Correctness | `COMPLETE` | [plans/rendering/R0.6.md](plans/rendering/R0.6.md) | RE-021–RE-271 (44 records) |
| **R0.7** — Missing Material Tables | `COMPLETE` | [plans/rendering/R0.7.md](plans/rendering/R0.7.md) | RE-057–RE-172 (14 records) |
| **R0.8** — Transform Correctness | `COMPLETE` | [plans/rendering/R0.8.md](plans/rendering/R0.8.md) | RE-049, RE-062, RE-063 |
| **R0.9** — Stage Animation | `COMPLETE` | [plans/rendering/R0.9.md](plans/rendering/R0.9.md) | RE-050, RE-051, RE-052, RE-142 |
| **R0.10** — Material Animation | `VERIFYING` | [plans/rendering/R0.10.md](plans/rendering/R0.10.md) | RE-048–RE-211 (16 records) |
| **R0.11** — Fighter Palettes / Costumes | `COMPLETE` | [plans/rendering/R0.11.md](plans/rendering/R0.11.md) | RE-040–RE-098 (7 records) |
| **R0.12** — Billboard Correctness | `COMPLETE` | [plans/rendering/R0.12.md](plans/rendering/R0.12.md) | RE-049–RE-145 (24 records) |
| **R0.13** — Framebuffer Rendering | `COMPLETE` | [plans/rendering/R0.13.md](plans/rendering/R0.13.md) | RE-055–RE-149 (19 records) |
| **R0.14** — Camera / Projection Correctness | `COMPLETE` | [plans/rendering/R0.14.md](plans/rendering/R0.14.md) | RE-034–RE-151 (7 records) |
| **R0.15** — Render-State Isolation | `COMPLETE` | [plans/rendering/R0.15.md](plans/rendering/R0.15.md) | RE-064–RE-254 (7 records) |
| **R0.16** — N64 Render-State Model Fidelity | `COMPLETE` | [plans/rendering/R0.16.md](plans/rendering/R0.16.md) | RE-067–RE-261 (18 records) |
| **R0.17** — Visual Regression Methodology | `COMPLETE` | [plans/rendering/R0.17.md](plans/rendering/R0.17.md) | RE-123, RE-125 |
| **R0.18** — Reference-Port Comparative Audit (sf64-psp, oot-PSP) | `COMPLETE` | [plans/rendering/R0.18.md](plans/rendering/R0.18.md) | RE-054, RE-066, RE-067, RE-124 |

---

## R1 — Rendering Completeness

Status: `COMPLETE`
Dependencies: R0.1–R0.18
Task spec: [plans/rendering/R1.md](plans/rendering/R1.md)
Current state: [STATUS.md](STATUS.md)
Latest evidence: RE-026–RE-261 (30 records)

---

## R2 — Physical PSP Rendering Validation

Status: `COMPLETE` — PSP-1000 real-content confirmation and a second unit at
the 30-minute sustained-run duration are deferred to a future hardware-
acceptance pass once real game UI/scene loading replaces the current debug
asset viewer (see `TODO.md`); they do not block R2's own acceptance
criteria, all of which are met.
Dependencies: R0, R1
Task spec: [plans/rendering/R2.md](plans/rendering/R2.md)
Current state: [STATUS.md](STATUS.md)
Latest evidence: RE-021–RE-288

---

## R3 — Rendering Performance

Status: `NOT_STARTED`
Dependencies: R0, R1, R2 (leads from R0.18's reference-port audit; not yet actioned)
Task spec: [plans/rendering/R3.md](plans/rendering/R3.md)
Current state: [STATUS.md](STATUS.md)
Latest evidence: RE-124

---

## 6.5 F1 — Front End & Training Mode

Status: `TODO` — current active task (see `STATUS.md`)
Dependencies: R2 (`COMPLETE`). Explicitly independent of R3 — runs in
parallel, must not block or be blocked by it.
Task spec: [plans/gameplay/F1.md](plans/gameplay/F1.md)
Current state: [STATUS.md](STATUS.md)
Latest evidence: none yet

Builds the intro screen, main menu, minimal character/stage select, and a
Training Mode combat sandbox (single stationary dummy target, real hitbox/
damage/knockback, no stocks/KO/CPU AI/items) as a **new, separate PSP
application** from the existing debug asset viewer (`psp/`). This is the one
explicit, scoped exception to the rendering-gate-before-combat rule — see
`AGENTS.md` and `plans/gameplay/F1.md` for the exact boundary.

---

## G0 — Combat Unlocked

Status: `BLOCKED_BY_R3`
Dependencies: R0, R1, R2, R3
Task spec: [plans/gameplay/G0.md](plans/gameplay/G0.md)

Full match combat (opponent, stocks, match loop, CPU) becomes eligible only
after R0, R1, R2 and R3 are complete. `F1`'s training-mode sandbox is a
separate, already-scoped exception (§6.5) and does not advance this gate.

---

## Post-Combat Roadmap

After G0:

| Task | Title | Status | Task spec |
|---|---|---|---|
| **G1** | Full Combat | `TODO` | [plans/gameplay/G1.md](plans/gameplay/G1.md) |
| **G2** | Complete Match Systems | `TODO` | [plans/gameplay/G2.md](plans/gameplay/G2.md) |
| **G3** | Menus and Persistence | `TODO` | [plans/gameplay/G3.md](plans/gameplay/G3.md) |
| **G4** | Audio | `TODO` | [plans/gameplay/G4.md](plans/gameplay/G4.md) |
| **G5** | Final Optimization | `TODO` | [plans/gameplay/G5.md](plans/gameplay/G5.md) |

`G3`'s title/menu screens are delivered early by `F1`; `G3`'s remaining
scope after `F1` is options, save/persistence, results screen and credits
(see `plans/gameplay/G3.md`). `G2`'s Training mode entry is delivered early
by `F1`; `G2` still owns VS/1P mode selection, stocks, timers and match
transitions built on `F1`'s combat core.

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
26. The N64 render-state intermediate representation is audited as faithful, not merely convenient (R0.16).
27. The visual-regression methodology (R0.17) has been run end-to-end with recorded results across the full test matrix.
28. The reference-port comparative audit against `sf64-psp` and `oot-PSP` (R0.18) is complete, with every material difference classified and recorded.

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
