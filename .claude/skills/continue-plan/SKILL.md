---
name: continue-plan
description: Resume or continue SSB64PSP development from repository state. Activates for "continue with the plan", "continue development", "resume current work", "what's next", or /continue-plan.
---

# Continue plan

1. Read `AGENTS.md` and `STATUS.md`.
2. Current work from `STATUS.md`; none named → next open item under the in-progress milestone in `PLAN.md` "Remaining Work"
3. Open only `RE-NNN`/`D-NNN` records the item cites (`docs/evidence/INDEX.md`, `DECISIONS.md`)
4. Check `git status` and recent commits for work in progress; confirm any `STATUS.md` blocker still holds
5. Run the batch workflow (AGENTS.md § Batch mode)
6. Finish per AGENTS.md § Documentation and § Git: `STATUS.md`, `PLAN.md`, affected docs, one commit

## Orchestrating

- Role: dispatch and review only; no batch work
- Per batch, run phase agents in order:
  1. `batch-research`
  2. `batch-implement` (repeat while the handoff lists remaining work)
  3. `batch-validate`
- Each prompt: the batch plus handoff path `<scratchpad>/<batch>-handoff.md` (outside the repository)
- Validate failure → send the note back to `batch-implement`
- After each batch: `tools/token-report.py`
- Never give batch work to `general-purpose`
- Out-of-plan fixes (crash, hardware fault, performance): same three phases
- Agent reports more work → new agent with its handoff note
  - why: `SendMessage` re-bills its whole context

### Model routing

- Phase agents: model and effort set in `.claude/agents/`
- Other subagents: `CLAUDE_CODE_SUBAGENT_MODEL` from `.claude/settings.json`
- Orchestrating session: Sonnet; Opus only where a weaker model causes rework

| Case | Route |
|---|---|
| Mechanical batch (constant tables, straight ports of small modules, no new types or control flow) | `batch-implement` with `model: "sonnet"` |
| Leftover failures or validate failure after Sonnet | rerun `batch-implement` with no override (Opus) |
| Validate failure needing diagnosis | `batch-implement` on Opus, never a second validate run |
| Read-only lookup | `Explore`, one narrow question each (200K window), not a batch |

- Before changing a phase's model: compare one batch's per-agent cost in `tools/token-report.py`
- Running session keeps the skills and settings it started with (`autoCompactWindow` included)
- After editing `.claude/settings.json` or a skill → tell the user to restart the orchestrating session
