---
name: reverse-engineering
description: Original SSB64 (N64) behavior needs to be established from the decompilation or ROM. Activates for "what does the original do", porting-decision questions, and any claim about N64 game behavior.
---

# Reverse engineering

Rules: AGENTS.md § Reference order, § Evidence, § Context budget.

1. Concrete question (struct field, opcode, behaviour, numeric value), not "how does rendering work"
2. `rg` [docs/evidence/INDEX.md](../../../docs/evidence/INDEX.md) by ID or topic tag; open only the matching `docs/evidence/re/RE-XXX.md`
3. Not answered → read decomp (`refs/ssb-decomp-re/`, `tools/decomp.py`) or ROM
4. Runtime-only (settle timing, live struct values, reaching a menu state) → [n64-emulator](../n64-emulator/SKILL.md)
5. Record uncertainty; accepted deviation needs measurement and documentation
6. Durable finding → new `RE-XXX.md` shaped like an existing one
   - header: Status, Topics, Relevant files
   - sections as applicable: Question, Evidence, Implementation, Verification, Conclusion, Confidence
   - then regenerate the index (AGENTS.md § Evidence)
