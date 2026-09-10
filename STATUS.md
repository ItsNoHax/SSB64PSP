# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T1 — texgen G_VTX model-space invariance audit`
- Status: `IN_PROGRESS`
- Last complete: `RE-216` — rebuilt original-ROM harness; corrected Metal Box route.
- Next: extend `romtool texgen` with load-space and normal-transform provenance.
- Blockers: T1–T7 texgen semantics/audits; original Metal comparison (T8); C1–C7
  renderer corrective gate; combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-216, RE-217.
- Plan: `PLAN.md` — R2.1/T1.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing.
- Verification: ROM identity, deterministic frame stepping, scripted menu route;
  `cargo test --workspace` — 531 passing.
- Documentation: RE-216/RE-217, `docs/porting-status.md`, this snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only.
- Commit: `1dd21a2`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
