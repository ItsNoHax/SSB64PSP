# SSB64PSP — Agent Operating Protocol

## 1. Project

SSB64PSP is a native Rust reimplementation of Super Smash Bros. 64 for Sony PSP hardware.

It is **not an emulator**.

The original SSB64 decompilation and the user's legally obtained ROM are the primary sources for original game behavior.

The project goal is faithful reproduction of SSB64 behavior on PSP, not merely a game inspired by SSB64.

---

# 2. Authoritative Repository Files

The agent MUST treat repository documentation as follows:

| File                          | Authority                                                      |
| ----------------------------- | -------------------------------------------------------------- |
| `AGENTS.md`                   | How the agent operates                                         |
| `PLAN.md`                     | Ordered development roadmap and acceptance criteria            |
| `STATUS.md`                   | Current execution state and session continuity                 |
| `docs/porting-status.md`      | Verified subsystem implementation status                       |
| `docs/reverse-engineering.md` | Technical discoveries, investigations and unresolved questions |
| `docs/rendering.md`           | Renderer architecture and rendering-specific behavior          |
| `docs/ssb-architecture.md`    | Original game's architectural understanding                    |
| `DECISIONS.md`                | Permanent architectural and implementation decisions           |
| `TODO.md`                     | Future work not yet incorporated into the ordered plan         |

If documentation disagrees with the code:

1. Investigate the discrepancy.
2. Determine which is correct using source evidence.
3. Update the incorrect documentation.
4. Never silently ignore the discrepancy.

`STATUS.md` is the authority for **current execution state**.

`PLAN.md` is the authority for **what should be done and in what order**.

Do not create another state or planning system.

---

# 3. Autonomous Continuation

When the user says:

> Continue with the plan.

the agent has permission to continue autonomously.

The agent MUST:

1. Read `AGENTS.md`.
2. Read `PLAN.md`.
3. Read `STATUS.md`.
4. Read the relevant sections of `docs/porting-status.md`.
5. Inspect `git status`.
6. Inspect recent commits.
7. Identify the current task from `STATUS.md`.
8. If the current task is `IN_PROGRESS`, resume it.
9. Otherwise select the first eligible `TODO` task from `PLAN.md`.
10. Check task dependencies.
11. Investigate the relevant original decompilation/ROM data before making behavioral assumptions.
12. Implement the smallest appropriate change.
13. Run targeted verification.
14. Run broader verification when appropriate.
15. Compare behavior against the original where possible.
16. Update all affected documentation.
17. Update `STATUS.md`.
18. Record evidence.
19. Commit focused completed work when appropriate.
20. Continue to the next eligible task when doing so is safe.

Do **not** ask the user:

> What should I work on?

if the repository already determines the next task.

Only stop and ask the user when:

* the plan is genuinely ambiguous;
* required information or access is unavailable;
* a destructive decision requires explicit approval;
* the implementation requires information that cannot be determined from the repository, decompilation, ROM or references;
* or the task is genuinely blocked.

If the next task is known and implementable, implement it.

---

# 4. Task Selection

Task order is defined by `PLAN.md`.

Use this priority:

1. Resume the current `IN_PROGRESS` task from `STATUS.md`.
2. Resolve blockers preventing that task.
3. Complete foundational tasks.
4. Select the first eligible `TODO` task in `PLAN.md`.
5. Prefer correctness and evidence over cosmetic improvements.
6. Prefer tasks that unblock multiple later tasks.

Never skip dependencies merely because a later task looks easier.

Never mark a milestone complete while required tasks remain unresolved.

---

# 5. Rendering Is the Hard Gate

Rendering is the project's highest-priority development gate.

**Combat MUST NOT be implemented until the rendering gate in `PLAN.md` has explicitly been passed.**

Do not implement:

* attacks;
* hitboxes;
* hurtboxes;
* damage;
* knockback;
* hitstun;
* stocks;
* KO logic;
* CPU combat;
* combat interactions;
* match gameplay.

Movement, physics, collision and animation may continue to be implemented when they are required to exercise or validate rendering.

Do not use "rendering looks good enough" as a reason to begin combat.

The rendering gate requires correctness, completeness, PPSSPP validation, physical PSP validation, performance measurement and synchronized documentation as defined in `PLAN.md`.

---

# 6. Original Behavior Source Hierarchy

When determining how SSB64 originally behaves, use this order:

1. Original SSB64 decompilation.
2. Original ROM/data extracted from the user's ROM.
3. BattleShip.
4. `sf64-psp`.
5. `oot-PSP`.
6. `n64psp`.
7. Existing SSB64PSP implementation.
8. Engineering assumptions.

Primary references:

* `VetriTheRetri/ssb-decomp-re`
* `JRickey/BattleShip`
* `TheMrIron2/sf64-psp`
* `z2442/oot-PSP`
* `TheMrIron2/n64psp`

BattleShip, `sf64-psp`, `oot-PSP` and `n64psp` are all technical references, not authorities — none of them outranks the decompilation or ROM (D-037).

If a reference project disagrees with the decompilation or ROM:

1. Identify the discrepancy.
2. Determine why it exists.
3. Prefer the original game's evidence.
4. Document the conclusion.

Do not blindly copy another project's implementation.

Do not copy Nintendo assets or copyrighted game data into this repository.

---

# 7. Evidence-Driven Development

Every meaningful implementation must have evidence.

Acceptable evidence includes:

* decompilation source;
* ROM data;
* display-list inspection;
* extracted asset reports;
* unit tests;
* integration tests;
* generated reports;
* screenshots;
* frame captures;
* PPSSPP behavior;
* physical PSP behavior;
* BattleShip comparison;
* numerical comparisons against original data.

Statements such as:

> It looks correct.

are not sufficient evidence for rendering correctness.

For rendering work, prefer measurable comparisons whenever possible.

---

# 8. Rendering Investigation Protocol

When investigating a rendering discrepancy:

```text
IDENTIFY SYMPTOM
        ↓
IDENTIFY AFFECTED ASSET / SCENE / DISPLAY LIST
        ↓
TRACE ORIGINAL DECOMPILATION
        ↓
INSPECT ROM DATA
        ↓
INSPECT DISPLAY LIST / GBI STATE
        ↓
CHECK BATTLESHIP
        ↓
FORM TESTABLE HYPOTHESIS
        ↓
IMPLEMENT SMALLEST CHANGE
        ↓
RUN TARGETED TEST
        ↓
COMPARE AGAINST ORIGINAL
        ↓
RUN BROADER REGRESSION TESTS
        ↓
DOCUMENT RESULT
        ↓
UPDATE STATUS
```

Do not repeatedly tweak rendering parameters until screenshots look better.

Determine what the N64 actually does first.

---

# 9. No Unsupported Heuristics

Do not introduce a heuristic simply because it makes a screenshot look better.

Examples include guessing:

* material tables;
* palettes;
* texture formats;
* texture filtering;
* LOD behavior;
* mipmapping;
* transforms;
* animation timing;
* lighting;
* combiner behavior;
* alpha behavior;
* depth behavior.

If a heuristic is genuinely required because the PSP cannot directly reproduce an N64 behavior:

1. Document the original behavior.
2. Explain why direct reproduction is impossible.
3. Explain the approximation.
4. Measure its effect.
5. Add regression coverage where practical.
6. Record it as a documented deviation.

Never disguise an approximation as an exact implementation.

---

# 10. BattleShip / sf64-psp / oot-PSP Usage

BattleShip, `sf64-psp` and `oot-PSP` should be actively consulted for N64 rendering and runtime behavior.

Use them particularly for:

* F3DEX/F3DEX2;
* GBI semantics;
* RSP/RDP concepts;
* texture handling;
* TMEM;
* display lists;
* material/render state;
* combiner translation to a target GPU;
* framebuffer behavior;
* N64 rendering architecture;
* PSP `sceGu` usage patterns (`sf64-psp`, `oot-PSP` specifically — both already target the PSP);
* debugging methodology and performance technique (`sf64-psp`, `oot-PSP`).

All three must be treated as reference implementations, per D-037.

Do not copy BattleShip's PC/desktop renderer architecture, or `sf64-psp`'s/`oot-PSP`'s specific state-translation choices, into this project's PSP renderer without a concrete technical reason recorded in `PLAN.md` R0.18 or `docs/reverse-engineering.md`.

`sf64-psp` and `oot-PSP` both target the PSP, which makes their `sceGu` usage and texture/material handling directly comparable to this project's — but Star Fox 64 and Ocarina of Time are not Smash 64. A technique either project uses is only adopted here once it is confirmed SSB64 actually needs it, per D-037's four-way classification.

The goal is correct SSB64 behavior on PSP, not architectural similarity to any reference project.

---

# 11. Documentation Is Part of the Implementation

A task is not complete if its documentation is knowingly stale.

After every significant implementation, determine whether these need updating:

* `PLAN.md`
* `STATUS.md`
* `docs/porting-status.md`
* `docs/rendering.md`
* `docs/reverse-engineering.md`
* `docs/ssb-architecture.md`
* `DECISIONS.md`
* `README.md`

Update documentation in the same work cycle as the implementation.

Do not make claims in the README that are contradicted by the current implementation or validation state.

---

# 12. STATUS.md Rules

`STATUS.md` is the persistent execution state.

It MUST contain enough information for a completely fresh agent session to continue safely.

At minimum it records:

* current milestone;
* current task;
* task status;
* last completed task;
* next eligible task;
* blockers;
* changes made;
* verification performed;
* evidence;
* documentation updated;
* relevant commit;
* important discoveries;
* hardware validation state.

When a task becomes `IN_PROGRESS`, update `STATUS.md`.

When a task becomes `COMPLETE`, update `STATUS.md`.

When a task becomes `BLOCKED`, record the exact reason and evidence.

Never rely on conversation history for important execution state.

---

# 13. Task Completion Semantics

Tasks use these statuses:

* `TODO` — not started.
* `IN_PROGRESS` — actively being implemented.
* `BLOCKED` — cannot currently proceed.
* `VERIFYING` — implementation exists but acceptance criteria are not yet satisfied.
* `COMPLETE` — acceptance criteria satisfied and evidence recorded.
* `ACCEPTED_DEVIATION` — exact reproduction is impossible on PSP, and the deviation has been demonstrated and justified.

Never mark a task `COMPLETE` merely because:

* it compiles;
* tests pass;
* PPSSPP boots;
* a screenshot looks plausible;
* the agent believes it is correct.

Completion requires the task's acceptance criteria in `PLAN.md`.

---

# 14. Verification

Use the smallest relevant verification first.

Typical sequence:

```text
targeted test
    ↓
relevant crate tests
    ↓
workspace tests
    ↓
asset / ROM verification
    ↓
PPSSPP
    ↓
physical PSP when required
    ↓
broader CI-equivalent verification
```

Do not run the entire test suite after every tiny change if a targeted test is sufficient.

Before marking a milestone complete, perform the full verification required by the milestone.

---

# 15. Asset Pack Discipline

Whenever ROM extraction or asset-pipeline code changes, rebuild:

```text
assets/generated/ssb64.pak
```

Never assume an existing generated pack reflects the current source code.

Verify that PPSSPP/device execution is using the newly generated pack.

Generated ROM-derived assets must remain excluded from Git as intended by the repository.

Never commit copyrighted ROMs or extracted copyrighted game assets.

---

# 16. PPSSPP Is Not Physical PSP Hardware

PPSSPP is a development and regression environment.

PPSSPP is **not proof of physical PSP correctness**.

A milestone requiring hardware validation must be tested on a physical PSP.

Record:

* PSP model;
* firmware/environment where relevant;
* build used;
* asset pack version;
* relevant runtime configuration;
* observed behavior;
* failures.

Do not claim "works on PSP hardware" based solely on PPSSPP.

---

# 17. Git Discipline

Prefer focused commits.

Examples:

```text
render: implement transform 0x8000
render: fix CI palette inheritance
render: implement stage material animation
test: add Dream Land texture regression
docs: update rendering status
```

Avoid giant mixed commits.

Before committing:

```text
git status
git diff
relevant tests
documentation review
```

Do not commit:

* ROMs;
* copyrighted extracted assets;
* unrelated generated files;
* temporary debugging artifacts.

---

# 18. Avoid Destructive Changes

Do not:

* delete working systems without evidence;
* rewrite large subsystems unnecessarily;
* replace verified code with speculative architecture;
* remove tests because they fail;
* weaken acceptance criteria to make progress appear faster;
* silently discard existing implementation.

If a rewrite is justified:

1. document the problem;
2. identify why incremental correction is insufficient;
3. preserve useful tests;
4. implement incrementally where practical;
5. verify against the previous behavior.

---

# 19. One Primary Task

Maintain exactly one primary implementation task at a time.

Related investigation is allowed, but the agent must be able to identify the primary task from `STATUS.md`.

Do not simultaneously claim to be implementing:

```text
texture system
+
combat
+
audio
+
menus
+
renderer rewrite
```

as one task.

Keep work traceable.

---

# 20. Session / Compaction Safety

Before ending a session or reaching context compaction:

1. Update `STATUS.md`.
2. Record the current task.
3. Record what changed.
4. Record what was verified.
5. Record what remains.
6. Record blockers.
7. Record important discoveries.
8. Update affected documentation.
9. Commit completed work when appropriate.
10. Leave the repository in a state another agent can safely continue.

A fresh agent must be able to execute:

> Continue with the plan.

and resume without reconstructing the previous conversation.

---

# 21. Failure Recovery

If an implementation approach fails:

1. Reproduce the failure.
2. Identify the exact subsystem.
3. Inspect the original decompilation.
4. Inspect ROM data.
5. Inspect display lists/assets where relevant.
6. Inspect BattleShip/reference implementations.
7. Document the investigation in `docs/reverse-engineering.md` when appropriate.
8. Form a new hypothesis.
9. Implement the smallest testable change.
10. Verify again.

Do not repeatedly retry an unsupported approach.

---

# 22. Do Not Stop at Recommendations

A successful autonomous session should end with one of:

### Progress

A planned task was implemented and verified.

### Continued Progress

A task remains `IN_PROGRESS`, but meaningful implementation and verification occurred and `STATUS.md` was updated.

### Blocked

The task is genuinely blocked and the exact reason/evidence is recorded.

It should NOT end with:

> I investigated the issue and recommend doing X next.

If X is the next planned task and can be implemented, implement X.

---

# 23. Final Rule

The repository itself must always contain enough information to determine:

* what has been completed;
* what is currently being worked on;
* what remains;
* what should happen next;
* what evidence exists;
* what is blocked;
* how success is verified;
* when rendering is complete;
* when combat is unlocked.

The user's default command is:

> **Continue with the plan.**

The agent's responsibility is to maintain the repository so that command remains sufficient.
