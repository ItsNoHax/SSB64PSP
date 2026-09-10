# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.0/P0c — Fix mirror+clamp beyond the first mirrored period`
- Status: `TODO`
- Last complete: `RE-220` (2026-09-11) closed `R2.0/P0b`: built a host-side
  N64 tile-addressing reference model
  (`crates/ssb-rom/src/n64_addressing.rs`, transcribed from
  `angrylion-rdp-plus`'s `tcshift_cycle`/`TRELATIVE`/`tcclamp_cycle`/
  `tcmask_coupled`) plus a model of the current PSP lowering
  (`psp_lowering_axis`) for direct comparison, and measured all three P0b
  questions archive-wide (2,484 real authored-UV primitives):
  mirror+clamp beyond the first mirrored period is a **real, material,
  still-open gap** (810 real axis instances, 99 measurably diverge,
  12.22%); `mask == 0` **never occurs** on any real drawn primitive
  archive-wide, pinned as a tested invariant, no fix needed; PSP
  power-of-two texture padding vs the N64 logical clamp boundary is a
  **real, material, still-open gap** (456 real axis instances, 347 reach
  the last logical texel where `Linear` blends into zero-filled padding,
  71.4%). `TextureRef` gained raw `mask_s`/`mask_t`/`drawn_width`/
  `drawn_height` fields to support the model; no rendering behavior
  changed. Opened `R2.0`/P0c and P0d as scoped correctness tasks for the
  two open gaps, per this queue's own "measure, don't fix speculatively"
  rule.
- Next: `R2.0`/P0c — implement addressing that keeps mirroring at every
  mask period up to the tile's drawn-rect far edge rather than the current
  "mirror once then clamp" approximation; re-run
  `tile_addressing_census_against_real_archive_textures` and require the
  divergence count to reach zero. `R2.0`/P0d (POT-padding fix) and P1
  (`G_SETTILE` field census) remain queued after it.
- Blockers: R2.0/P0c–P1 (mirror+clamp fix; POT-padding fix; `G_SETTILE`
  palette/line/tmem/shift census) must close before R2.1/T1–T10 resumes;
  R2.1 was designated but never implemented, so resuming there loses no
  progress. R2.2/C1–C7 renderer corrective gate remains behind R2.1. Combat
  remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-217, RE-218, RE-219, RE-220.
- Plan: `PLAN.md` — R2.0/P0c.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing; `docs/rendering.md`
  — "Texture addressing" row.
- Verification: `cargo test -p ssb-rom n64_addressing` (8 new tests) and
  `cargo test -p romtool tile_addressing_census_against_real_archive_textures -- --nocapture`
  against the real ROM; `cargo test --workspace` — 546 passing.
- Documentation: RE-220, `docs/rendering.md`, `PLAN.md` R0.5/R2.0, this
  snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only.
- Commit: `942b744`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
