---
name: reverse-engineering
description: Original SSB64 (N64) behavior needs to be established from the decompilation or ROM. Activates for "what does the original do", porting-decision questions, and any claim about N64 game behavior.
---

# Reverse engineering

1. Determine the concrete question (a specific struct field, opcode,
   behavior, or numeric value — not "how does rendering work" in general).
2. Search [docs/evidence/INDEX.md](../../docs/evidence/INDEX.md) by ID or
   topic tag for prior work. Do not bulk-read the evidence corpus.
3. Load only the matching `docs/evidence/re/RE-XXX.md` record(s).
4. If existing evidence doesn't answer the question, consult the decomp
   (`refs/ssb-decomp-re/`) or ROM directly — per the reference hierarchy in
   `AGENTS.md`/`PLAN.md` §4: decompilation → ROM/data → BattleShip →
   `sf64-psp` → `oot-PSP` → `n64psp` → existing implementation → assumptions.
   If the question is about *runtime* behavior static code can't settle
   (camera settle timing, live struct values, reaching a specific menu/game
   state), use the [n64-emulator](../n64-emulator/SKILL.md) Skill instead of
   guessing from source alone.
5. Record uncertainty rather than guessing (Rule 10) — an accepted deviation
   needs measurement and documentation, not a plausible-looking default.
6. Create a new `docs/evidence/re/RE-XXX.md` record only for genuinely new
   investigation (next unused ID). Follow the shape of an existing record
   (metadata header: Status/Topics/Related tasks/Relevant files, then the
   investigation body — Question/Evidence/Implementation/Verification/
   Conclusion/Confidence where applicable). Then run
   `python3 tools/docs/gen_evidence_index.py` to refresh the index.
7. Never bulk-read the entire RE corpus (`docs/evidence/re/*.md`) to answer
   one question — that defeats the point of the split.
