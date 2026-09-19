# SSB64PSP Agent Bootstrap

SSB64PSP is a native Rust source port of Super Smash Bros. 64 to PSP, not an
emulator. Gameplay is translated from `ssb-decomp-re` in large coherent
subsystems, not rebuilt mechanic-by-mechanic from scratch.

## References

1. `ssb-decomp-re` — primary source of gameplay behavior (types, state
   tables, callbacks, constants, timing).
2. Original ROM/data — ground truth when decomp intent is ambiguous.
3. BattleShip — used to understand SSB-specific native-port adaptations,
   not as a gameplay authority.
4. `sf64-psp`, `oot-PSP`, `n64psp` — PSP platform patterns: GU rendering,
   memory, controller input, timing, audio, cache handling, asset loading,
   PSP-specific optimization. Not gameplay authorities.
5. Existing SSB64PSP implementation, then engineering assumptions.

Disagreements get investigated, not guessed around. Do not copy Nintendo
assets or copyrighted game data from reference projects.

## Crate ownership

- `crates/ssb-game` — portable SSB64 gameplay (fighters, moves, match logic).
  No PSP-specific code.
- `crates/ssb-engine` — reusable engine/runtime-independent systems (scene
  graph, animation, collision, math, traits). No PSP-specific code.
- `psp-runtime` — all shared PSP-specific implementation (GU rendering,
  input, timing, audio, memory, asset loading). The only place PSP code may
  live outside the two apps' thin entry points.
- `psp-game` — the actual game application. Thin: orchestration only, no
  gameplay logic that belongs in `ssb-game`/`ssb-engine`, no PSP backend
  code that belongs in `psp-runtime`.
- `psp-asset-viewer` — debugging/render-validation application only. Must
  never grow game logic, match state, or player-facing features.

`crates/ssb-game`, `crates/ssb-engine` and `crates/ssb-rom` must never
depend on `psp-runtime`.

## Canonical file map

- `PLAN.md`: index/roadmap (milestones `P0`–`P5`). Not a detailed journal.
- `STATUS.md`: small, mutable current snapshot — milestone, batch, what
  finished, next batch, real blockers. The only place for session state.
- `docs/porting-status.md`: per-subsystem status table.
- `docs/evidence/INDEX.md` + `docs/evidence/re/RE-XXX.md`: evidence records.
  Required only for ambiguous original behavior, reverse-engineering
  discoveries, PSP deviations, important rendering discrepancies, or major
  architectural decisions — not for routine source ports.
- `DECISIONS.md` (index) + `docs/decisions/D-XXX.md`: permanent
  architectural decisions.
- `docs/rendering.md` + `docs/rendering/*.md`: renderer technical reference.
- `docs/visual-regression.md` + `docs/visual-regression/{README,scenes/*}.md`:
  golden-capture methodology.
- `docs/ssb-architecture.md`, `docs/memory.md`: standalone domain references.
- `TODO.md`: genuinely deferred work not part of the current or next batch.
- `plans/rendering/*.md`, `plans/gameplay/*.md`: archived task specs from
  the pre-batch-mode process. History/evidence only — not the active
  tracker. Do not add new specs here; new work is tracked as milestones/
  batches in `PLAN.md`/`STATUS.md`.

If code and documentation disagree, investigate with source evidence and fix
the incorrect record. There is exactly one authoritative source for each
kind of state above — update that file, not a copy of it.

## BATCH MODE

Development runs in batches, not per-function edits.

- Work one coherent subsystem at a time (e.g. "fighter movement", "shield
  system"), not one function at a time.
- Modifying dozens of files in one batch is expected and fine.
- Translate a group of related decomp modules before stopping.
- Do not stop after every function to ask for confirmation.
- Do not run the full test suite after every small change. During
  implementation, use compile/check and only the cheapest relevant tests.
- At the end of a batch: run workspace tests, build PSP, and run one
  appropriate PPSSPP smoke/integration test.
- Physical PSP validation, full visual-regression matrices, and exhaustive
  testing happen at milestone boundaries or when investigating PSP-specific
  behavior — not every batch.
- No evidence record for routine source ports. Evidence is for ambiguous
  original behavior, RE discoveries, PSP deviations, rendering
  discrepancies, or major architectural decisions.
- Update documentation once, at the end of the batch — not per file.
- Prefer translating original behavior over inventing a cleaner/custom
  design. Preserve original ordering, constants, state transitions,
  callback semantics and timing unless the PSP architecture makes this
  impossible (document the deviation when it does).

## Source-port workflow (one batch, internal steps — not separate tasks)

1. Identify relevant `ssb-decomp-re` modules for the subsystem.
2. Map dependencies/types against the existing Rust architecture.
3. Check BattleShip only where integration interpretation is useful.
4. Check PSP ports (`sf64-psp`/`oot-PSP`/`n64psp`) only when platform-specific
   implementation is involved.
5. Translate the full coherent subsystem.
6. Resolve compile errors across the batch.
7. Run targeted host tests.
8. Run workspace tests.
9. Build PSP.
10. Run one integration smoke test.
11. Fix failures.
12. Update `STATUS.md`/porting documentation once.
13. Commit the completed batch.

Never ask what to work on when repository state determines the next batch.
Stop only for genuine ambiguity, unavailable required evidence/access,
destructive decisions, or a real blocker.

## Non-negotiable constraints

- Gameplay lives only in `crates/ssb-game`/`crates/ssb-engine`. PSP-specific
  code lives only in `psp-runtime`. `psp-game` stays a thin application.
  `psp-asset-viewer` stays a debugging/render-validation tool, never the game.
- Use `ssb-decomp-re`, ROM, BattleShip, `sf64-psp`/`oot-PSP`/`n64psp`,
  existing implementation, then assumptions — in that order.
- Rendering performance is not a blocker for gameplay development. It is a
  parallel/later profiling task (milestone `P5`), measured against real
  full-game workloads, not speculative pre-optimization.
- No unsupported rendering heuristics. Document and measure unavoidable PSP
  deviations.
- PPSSPP is not physical PSP proof.
- Rebuild `assets/generated/ssb64.pak` when asset-pipeline code changes;
  never commit ROM-derived assets.
- Preserve user changes; use `apply_patch` for edits; avoid destructive
  commands.

## Context-budget rule

Never read `PLAN.md`, an evidence/decision archive, or a large subsystem
document in full merely to establish context. Start from `STATUS.md`,
indexes (`docs/evidence/INDEX.md`, `DECISIONS.md`, `docs/porting-status.md`)
and Skill routing (below). Expand only the specific records the active
batch needs.

## Code-discovery rule

For Rust/code investigation, prefer Serena's symbol discovery/reference
tools (find symbol, find references, go to definition) over reading whole
source files. Read a whole file only when symbol-level retrieval is
inappropriate (module-level structure, or a short file end to end).

## Context routing

| Need | Route to |
|---|---|
| Current work | `STATUS.md` |
| Roadmap/milestones | `PLAN.md` |
| Per-subsystem status | `docs/porting-status.md` |
| Why was something implemented? | `docs/evidence/INDEX.md` → specific `RE-xxx.md` only |
| Permanent architectural choice? | `DECISIONS.md` index → specific `D-xxx.md` only |
| Rendering behavior? | `rendering` Skill → relevant `docs/rendering/*.md` |
| Original-game behavior unknown? | `reverse-engineering` Skill |
| Need to observe/navigate the live original N64 game? | `n64-emulator` Skill (headless Mupen64Plus) |
| Visual/rendering testing (screenshots, goldens)? | `visual-regression` Skill (PPSSPPHeadless) |
| Physical-hardware testing (real PSP, PSPLink)? | `psp-hardware` Skill |
| Asset/pack/ROM format? | `asset-pipeline` Skill |
| Updating project docs/state? | `documentation` Skill |
| Resuming work generally? | `continue-plan` Skill / `/continue-plan` |

Do not broadly read when a narrower, ID-scoped retrieval is available.

## Startup protocol

For `Continue with the plan`:

1. Read this file and `STATUS.md`.
2. Identify the current milestone/subsystem batch from `STATUS.md`.
3. Read only the evidence/decision IDs `STATUS.md` references.
4. Inspect `git status` and recent commits.
5. Resume the in-progress batch, or start the next batch under the current
   milestone (`PLAN.md`).
6. Run the source-port workflow above for the batch.
7. Update `STATUS.md` once at the end of the batch and commit.

Detailed operating rules: `docs/agent-protocol.md`.
Rendering investigation protocol: `docs/agent-rendering.md`.

Semantic search (embeddings/vector DB) is deliberately not part of this
project's retrieval stack yet — ID/topic-indexed routing above and Serena
symbol search are expected to cover normal development. Reconsider only if
the evidence corpus becomes hard to navigate even with those in place.
