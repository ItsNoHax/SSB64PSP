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

Status: `TODO`

### Objective

Reconcile the documented renderer state with the actual implementation.

### Dependencies

* M3

### Acceptance

* [ ] implementation inventory completed
* [ ] renderer architecture documented
* [ ] stale documentation identified
* [ ] known rendering gaps enumerated
* [ ] unsupported rendering paths identified
* [ ] current verification baseline recorded

### Verification

* inspect renderer implementation
* inspect generated asset reports
* run relevant tests
* run PPSSPP baseline

### Evidence

Record in `STATUS.md` and relevant documentation.

---

## R0.2 — N64 Rendering Command Inventory

Status: `TODO`

### Objective

Enumerate every N64 rendering command and relevant state transition actually exercised by SSB64.

### Dependencies

* R0.1

### Acceptance

* [ ] GBI commands identified
* [ ] usage/frequency recorded
* [ ] display-list usage mapped
* [ ] current PSP implementation mapped
* [ ] unsupported commands identified
* [ ] relevant RSP/RDP behavior identified
* [ ] BattleShip cross-reference performed

### Verification

* decompilation inspection
* ROM/display-list inspection
* BattleShip comparison

---

## R0.3 — Texture Conversion Completeness

Status: `TODO`

### Objective

Resolve every texture conversion failure that represents a missing required texture path.

### Dependencies

* R0.2

### Acceptance

* [ ] all required N64 texture formats supported
* [ ] all required textures decode
* [ ] all required palettes resolve
* [ ] no unexplained conversion failures remain
* [ ] framebuffer/screen-wipe failures separately categorized
* [ ] conversion report generated
* [ ] regression tests added where appropriate

### Verification

```bash
cargo run --release -p romtool -- textures "rom/Super Smash Bros. (USA).z64"
```

---

## R0.4 — TLUT / Palette Correctness

Status: `TODO`

### Objective

Reproduce original N64 CI/TLUT behavior.

### Dependencies

* R0.3

### Acceptance

* [ ] CI4 verified
* [ ] CI8 verified
* [ ] TLUT loading behavior verified
* [ ] palette inheritance/state verified
* [ ] palette pointers verified
* [ ] all missing palette cases resolved
* [ ] regression coverage added

---

## R0.5 — Texture Filtering / LOD / Mipmapping

Status: `TODO`

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

Status: `TODO`

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

Status: `TODO`

### Objective

Resolve every scene graph containing an unresolved material table.

### Dependencies

* R0.6

### Acceptance

* [ ] all material-table references traced
* [ ] original material data identified
* [ ] heuristic mapping removed where original data exists
* [ ] affected scenes verified
* [ ] regression coverage added

---

## R0.8 — Transform Correctness

Status: `TODO`

### Objective

Implement every transform kind exercised by SSB64.

### Dependencies

* R0.1
* R0.2

### Acceptance

* [ ] transform kinds enumerated
* [ ] `0x8000` investigated
* [ ] original matrix behavior identified
* [ ] PSP implementation verified
* [ ] affected scene nodes tested
* [ ] billboard-related transforms cross-checked

---

## R0.9 — Stage Animation

Status: `TODO`

### Objective

Reproduce original stage animation behavior.

### Dependencies

* R0.6
* R0.8

### Acceptance

* [ ] all stage animation formats understood
* [ ] event encoding verified
* [ ] timing verified
* [ ] interpolation verified
* [ ] animation playback verified
* [ ] independent ROM comparison exists
* [ ] all stages tested

---

## R0.10 — Material Animation

Status: `TODO`

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

Status: `TODO`

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

Status: `TODO`

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

Status: `TODO`

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
