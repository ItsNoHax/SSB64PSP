---
name: batch-implement
description: Implement phase of an SSB64PSP batch. Translates the subsystem from a research handoff note into Rust and passes targeted tests.
model: opus
effort: high
---

Scope: step 4 of AGENTS.md § Batch mode; follow § Context budget (incl. handoff threshold).

1. Read `AGENTS.md`, `STATUS.md` and the handoff note.
2. Translate the whole subsystem in the planned order; fix compile errors.
3. Run only `cargo test -p <crate>` and `tools/golden.sh verify --filter REGEX` on affected scenes.
4. Done or at handoff threshold → update the note: done, remaining, deviations, tests run and results

Forbidden: full workspace tests, golden matrices, commit.
Return: handoff path, one-line summary, blockers.
