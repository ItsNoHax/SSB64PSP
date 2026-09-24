# Agent Instructions

SSB64PSP is a native Rust source port of Super Smash Bros. 64 to PSP, not an
emulator. Gameplay is translated from `ssb-decomp-re` in large coherent
subsystems, not rebuilt mechanic by mechanic.

## Reference order

1. `ssb-decomp-re` — gameplay behavior: types, state tables, callbacks,
   constants, timing.
2. Original ROM and data — ground truth when the decomp is ambiguous.
3. BattleShip — SSB-specific native-port adaptations.
4. `sf64-psp`, `oot-PSP`, `n64psp` — PSP platform technique only (GU,
   memory, input, timing, audio, cache, asset loading).
5. Existing SSB64PSP code, then engineering assumptions.

Investigate disagreements; do not guess around them. Never copy Nintendo
assets or copyrighted data from reference projects
([D-037](docs/decisions/D-037.md)).

## Crate ownership

| Crate | Owns | Must not contain |
|---|---|---|
| `crates/ssb-game` | Portable gameplay | PSP code |
| `crates/ssb-engine` | Engine systems: math, animation, collision, traits | PSP code |
| `crates/ssb-rom` | ROM, archive, formats, asset pack | PSP code |
| `crates/ssb-capture` | Golden-capture scene specs shared by both PSP binaries and host tests | PSP code, scene behaviour |
| `psp-runtime` | All shared PSP code: GE, input, timing, audio, memory, asset loading | Gameplay logic |
| `psp-game` | Thin game application: orchestration only | Gameplay or PSP backend code |
| `psp-asset-viewer` | Debug and render-validation tool | Game logic, match state, player features |

`ssb-game`, `ssb-engine` and `ssb-rom` never depend on `psp-runtime`.

## Documentation map

One owner per kind of state. Update the owner; link to it elsewhere.

| State | Owner |
|---|---|
| Current batch, blockers, verification baseline | `STATUS.md` (replace, never append; 1–3 KB) |
| Milestones | `PLAN.md` |
| Per-subsystem status | `docs/porting-status.md` |
| Deferred work | `TODO.md` |
| Investigation evidence | `docs/evidence/INDEX.md` + `docs/evidence/re/RE-NNN.md` |
| Permanent decisions | `DECISIONS.md` + `docs/decisions/D-NNN.md` |
| Renderer model | `docs/rendering.md` + `docs/rendering/*.md` |
| Golden captures | `docs/visual-regression/README.md` |
| Original-game and memory reference | `docs/ssb-architecture.md`, `docs/memory.md` |
| Project overview | `README.md` (no progress detail) |
| Archived specs | `plans/**` (history only; add nothing) |

When code and docs disagree, verify against source and fix the wrong record.

## Batch mode

- Work one coherent subsystem per batch (e.g. "shield system"). Translate a
  group of related decomp modules before stopping; dozens of files per batch
  is normal.
- Do not ask for confirmation between functions or ask what to work on when
  `STATUS.md` determines it. Stop only for genuine ambiguity, missing
  evidence or access, destructive decisions, or a real blocker.
- During a batch run only compile checks and the cheapest relevant tests.
- At the end of a batch: workspace tests, both PSP builds, one PPSSPP smoke
  test, then update docs once and commit the batch.
- Physical-PSP validation and full golden matrices belong at milestone
  boundaries or PSP-specific investigations.
- Preserve original ordering, constants, state transitions, callback
  semantics and timing. Document any deviation the PSP forces.

### Batch workflow

1. Find the relevant decomp modules.
2. Map types and dependencies onto the Rust architecture.
3. Check BattleShip or PSP ports only where integration or platform questions
   need them.
4. Translate the whole subsystem and fix compile errors.
5. Targeted tests → workspace tests → PSP builds → one integration smoke.
6. Update `STATUS.md` and affected docs once, then commit.

## Evidence

Write an `RE-NNN` record only for ambiguous original behavior, RE
discoveries, PSP deviations, rendering discrepancies or major architectural
decisions — not routine ports. Hardware records include PSP model, firmware,
build, pack version and observations. Then run
`python3 tools/docs/gen_evidence_index.py` and
`python3 tools/docs/validate_docs.py`.

## Constraints

- Rendering performance is milestone `P5`, profiled on real workloads; it
  never blocks gameplay.
- No unsupported rendering heuristics. Measure and document unavoidable PSP
  deviations.
- PPSSPP is not physical-PSP proof.
- Rebuild `assets/generated/ssb64.pak` when asset-pipeline code changes.
  Never commit ROM-derived assets.
- Preserve user changes. Use `apply_patch` for edits. Avoid destructive
  commands; resolve exact targets first. Never weaken acceptance
  criteria or delete failing tests to show progress.

## Context budget

- Start from `STATUS.md` and the indexes (`docs/evidence/INDEX.md`,
  `DECISIONS.md`, `docs/porting-status.md`). Open only the records the batch
  needs.
- Never bulk-read `docs/evidence/re/` or `plans/`.
- Prefer symbol-level code search (Serena, when available) over reading
  whole files. Semantic/vector search is deliberately not used; reconsider
  only if the indexes stop being navigable.

## Skill routing

| Need | Skill |
|---|---|
| Resume work | `continue-plan` |
| Original-game behavior from decomp or ROM | `reverse-engineering` |
| Observe the running original game | `n64-emulator` |
| Renderer bug or fidelity question | `rendering` (protocol: `docs/agent-rendering.md`) |
| Goldens and PPSSPPHeadless | `visual-regression` |
| Physical PSP, PSPLink | `psp-hardware` |
| Asset pipeline, pack format, `romtool` | `asset-pipeline` |
| Updating docs | `documentation` |

## Git

Inspect status and diff before committing. Commit a completed batch as one
coherent unit. Do not rewrite others' history.
