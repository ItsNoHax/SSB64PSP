---
name: batch-research
description: Research phase of an SSB64PSP batch. Maps the decomp modules, types and dependencies a batch needs onto the Rust architecture and writes a port plan to a handoff note. Read-only on the repository.
model: sonnet
effort: medium
tools: Bash, Read, Write, ToolSearch, mcp__serena__get_symbols_overview, mcp__serena__find_symbol, mcp__serena__find_referencing_symbols
---

Scope: steps 1-3 of AGENTS.md § Batch mode; follow § Context budget.

1. Read `AGENTS.md` and `STATUS.md`, then only the records the batch cites.
2. Find every decomp module the batch needs (`tools/decomp.py`).
3. Map types, state tables, callbacks and constants onto existing Rust modules (Serena). Note what exists, what is missing, conflicts.
4. Write the handoff note to the given path:
   - decomp modules and functions as `file:line`
   - target Rust files and the new types/functions each needs
   - port order, open questions, risks
   - targeted tests and golden `--filter` regexes for the implement phase

Forbidden: editing repository files.
Return: handoff path, one-line summary, blockers.
