# SSB64PSP Agent Bootstrap

SSB64PSP is a native Rust reimplementation of Super Smash Bros. 64 for PSP,
not an emulator. Original decompilation and ROM behavior outrank all other
references.

## Canonical file map

- `PLAN.md`: ordered roadmap/index. Detailed task specs: `plans/rendering/*.md`, `plans/gameplay/*.md`.
- `STATUS.md`: short, mutable current snapshot. The only place for session state.
- `docs/porting-status.md`: concise per-subsystem status table.
- `docs/evidence/INDEX.md` + `docs/evidence/re/RE-XXX.md`: investigation evidence, one file per record.
- `DECISIONS.md` (index) + `docs/decisions/D-XXX.md`: permanent architectural decisions.
- `docs/rendering.md` (entry point) + `docs/rendering/*.md`: renderer technical reference by domain.
- `docs/visual-regression.md` (entry point) + `docs/visual-regression/{README,scenes/*}.md`: golden-capture methodology.
- `docs/ssb-architecture.md`, `docs/memory.md`: standalone domain references.
- `TODO.md`: future work not yet in the roadmap.

If code and documentation disagree, investigate with source evidence and fix
the incorrect record. Do not create another state or planning system. There
is exactly one authoritative source for each kind of state above — update
that file, not a copy of it.

## Context-budget rule

Never read `PLAN.md`, an evidence/decision archive, or a large subsystem
document in full merely to establish context. Start from `STATUS.md`,
indexes (`docs/evidence/INDEX.md`, `DECISIONS.md`), task/evidence IDs, and
Skill routing (below). Expand only the specific records the active task
needs. A new session determines the active task from `AGENTS.md` + `STATUS.md`
+ one task spec file — not from reading the whole roadmap or evidence corpus.

## Code-discovery rule

For Rust/code investigation, prefer Serena's symbol discovery/reference tools
(find symbol, find references, go to definition) over reading whole source
files. Read a whole file only when symbol-level retrieval is inappropriate
(e.g. understanding module-level structure or a short file end to end).

## Context routing

| Need | Route to |
|---|---|
| Current work | `STATUS.md` → current PLAN task's spec file only |
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
2. Identify the active task ID and read only its spec file under `plans/`.
3. Read only the evidence/decision IDs `STATUS.md` or that spec references.
4. Inspect `git status` and recent commits.
5. Resume `IN_PROGRESS`, otherwise select first eligible `TODO` task.
6. Check dependencies, inspect original behavior, implement smallest change.
7. Run targeted verification, then broader verification when appropriate.
8. Update evidence, affected docs, and `STATUS.md`.
9. Commit focused completed work when appropriate.

Never ask what to work on when repository state determines task. Stop only for
ambiguity, unavailable required evidence/access, destructive decisions, or a
genuine blocker.

## Non-negotiable constraints

- Rendering gate must pass before match combat (`G0`–`G2`): no CPU combat,
  stocks, KO logic, multiplayer/match combat interactions before it.
  Exception: `F1`'s training-mode sandbox (attacks/hitboxes/hurtboxes/
  damage/knockback/hitstun against a single stationary dummy target, no
  stocks/KO/match loop/CPU AI/items) is explicitly carved out — see
  `plans/gameplay/F1.md`. Do not widen this exception without updating this
  file.
- Use original decompilation, ROM, BattleShip, `sf64-psp`, `oot-PSP`,
  `n64psp`, implementation, then assumptions—in that order.
- No unsupported rendering heuristics. Document and measure unavoidable PSP
  deviations.
- PPSSPP is not physical PSP proof.
- Rebuild `assets/generated/ssb64.pak` when asset-pipeline code changes; never
  commit ROM-derived assets.
- Maintain exactly one primary implementation task.
- Preserve user changes; use `apply_patch` for edits; avoid destructive commands.

Detailed operating rules: `docs/agent-protocol.md`.
Rendering investigation protocol: `docs/agent-rendering.md`.

Semantic search (embeddings/vector DB) is deliberately not part of this
project's retrieval stack yet — ID/topic-indexed routing above and Serena
symbol search are expected to cover normal development. Reconsider only if
the evidence corpus becomes hard to navigate even with those in place.
