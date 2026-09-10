# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.0/P0a — N64 3-point texture filtering vs PSP bilinear reopening`
- Status: `TODO`
- Last complete: `RE-216` — rebuilt original-ROM harness; corrected Metal Box
  route. `RE-217` reconciled the corrective queues; `RE-218` (2026-09-11)
  reopened R0.5's filtering and mirror/clamp/mask/POT-padding addressing
  claims and added `PLAN.md` R2.0 (P0a/P0b/P1) ahead of the texgen queue.
  No implementation code changed by RE-217 or RE-218 — planning/documentation
  reconciliation only.
- Next: build a host-side N64 3-point (triangular) filtering reference
  sampler from an authoritative RDP source, then measure PSP `Linear`
  against it on representative SSB64 textures (magnified low-res, edges,
  diagonal gradients) per `PLAN.md` R2.0/P0a.
- Blockers: R2.0/P0a–P1 (filtering equivalence; general mirror+clamp/
  mask==0/POT-padding tile-addressing model; `G_SETTILE`
  palette/line/tmem/shift census) must close before R2.1/T1–T10 resumes;
  R2.1 was designated but never implemented, so resuming there loses no
  progress. R2.2/C1–C7 renderer corrective gate remains behind R2.1.
  Combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-216, RE-217, RE-218.
- Plan: `PLAN.md` — R2.0/P0a.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing; `docs/rendering.md`
  — "Texture filtering"/"Texture addressing" rows.
- Verification: ROM identity, deterministic frame stepping, scripted menu route;
  `cargo test --workspace` — 531 passing (unaffected by this documentation-only
  reconciliation).
- Documentation: RE-216/RE-217/RE-218, `docs/porting-status.md`, this snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only.
- Commit: `ec87660`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
