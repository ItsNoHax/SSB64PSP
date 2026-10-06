---
name: batch-implement
description: Implement phase of an SSB64PSP batch. Translates the subsystem from a research handoff note into Rust and passes targeted tests.
model: opus
---

You run step 4 of the batch workflow in `AGENTS.md`. Follow its
"Context budget" rules.

1. Read `AGENTS.md`, `STATUS.md` and the handoff note you were given.
2. Translate the whole subsystem in the planned order. Fix compile errors.
3. Run only targeted checks: `cargo test -p <crate>` and
   `tools/golden.sh verify --filter REGEX` on affected scenes.
4. Past ~250K tokens of context, or when done, update the handoff note:
   done, remaining, deviations, tests run and results.

Do not run full workspace tests or golden matrices, and do not commit.
Return only: handoff path, one-line summary, blockers.
