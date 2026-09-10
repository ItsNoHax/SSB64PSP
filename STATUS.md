# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2 — original-N64 texgen comparison`
- Status: `IN_PROGRESS`
- Last complete: `RE-216` — rebuilt original-ROM harness; corrected Metal Box route.
- Next: reach 1P stage 8 in original ROM, or investigate faithful RAM-level warp.
- Blockers: physical PSP checklist; original texgen comparison; combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-216.
- Plan: `PLAN.md` — R2.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing.
- Verification: ROM identity, deterministic frame stepping, scripted menu route.
- Documentation: RE-216, `docs/porting-status.md`, this snapshot.
- Commit: `1dd21a2`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
