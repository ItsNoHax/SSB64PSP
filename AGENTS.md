# SSB64PSP Agent Bootstrap

SSB64PSP is a native Rust reimplementation of Super Smash Bros. 64 for PSP,
not an emulator. Original decompilation and ROM behavior outrank all other
references.

## Authoritative files

- `PLAN.md`: ordered roadmap and acceptance criteria.
- `STATUS.md`: short, mutable current snapshot.
- `docs/porting-status.md`: subsystem status.
- `docs/reverse-engineering.md`: investigations and evidence.
- `docs/rendering.md` / `docs/ssb-architecture.md`: renderer and game architecture.
- `DECISIONS.md`: permanent architectural decisions.
- `TODO.md`: future work not yet in the roadmap.

If code and documentation disagree, investigate with source evidence and fix
the incorrect record. Do not create another state or planning system.

## Startup protocol

For `Continue with the plan`:

1. Read this file and `STATUS.md`.
2. Read only current task section in `PLAN.md`.
3. Read referenced subsystem row and evidence IDs.
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

- Rendering gate must pass before combat. No attacks, hitboxes, damage,
  knockback, stocks, KO logic, CPU combat, or combat interactions before it.
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
