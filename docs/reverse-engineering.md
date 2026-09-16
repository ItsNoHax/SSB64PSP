# Reverse-Engineering Log

This file is a compatibility pointer. The log used to live here as one
monolithic document; it is now split into one record per investigation under
`docs/evidence/re/`, so an agent can load exactly the investigation it needs
instead of the whole history.

- Index (one line per record, with topic tags): [docs/evidence/INDEX.md](evidence/INDEX.md)
- Individual records: `docs/evidence/re/RE-001.md` … `RE-271.md`

Per Rule 10 (see `PLAN.md`): when the original's behaviour is uncertain,
record the uncertainty rather than guessing. Records use Question / Evidence
(or Setup) / Hypothesis / Implementation / Verification / Conclusion /
Confidence, though older records predate that exact shape — the record body
is preserved verbatim from the original log regardless.

Because the decompilation is **100% complete**, most questions are about
*porting* decisions rather than what the original does. Anything answerable
from the decomp should be answered from the decomp, not guessed.

To add a new investigation: create `docs/evidence/re/RE-NNN.md` following the
existing record shape (see any current record for the metadata header), then
run `python3 tools/docs/gen_evidence_index.py` to refresh the index.

Do not re-inline the full corpus into this file.
