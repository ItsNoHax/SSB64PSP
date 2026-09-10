# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.0/P0d — Fix PSP POT-padding vs N64 logical clamp boundary`
- Status: `TODO`
- Last complete: `RE-221` (2026-09-11) closed `R2.0/P0c`: fixed mirror+clamp
  tile addressing beyond the first mirrored period.
  `texture::mirror_extend` (`crates/ssb-rom/src/texture.rs`) now takes each
  axis's clamp bit and `drawn_width`/`drawn_height` and, only for a
  mirror+clamp axis, pre-bakes every mask period the tile's real drawn rect
  spans instead of always exactly two — a mirrored axis with no clamp bit
  is unchanged (a doubled bake plus `Repeat` already mirrors forever
  exactly, since wrapping back to the start resumes the same phase).
  `n64_addressing::psp_lowering_axis` gained the matching `drawn` parameter
  so the comparison model tracks the fix. Re-ran
  `tile_addressing_census_against_real_archive_textures` archive-wide
  against the real ROM: divergence between the hardware reference model
  and the PSP lowering dropped from 99/810 (12.22%, RE-220) to **0/810**,
  now asserted in the test itself so a regression fails the build.
- Next: `R2.0`/P0d — implement `PLAN.md`'s own candidate fix for the one
  remaining open gap RE-220 measured: fill `pack_rgba`/`pack_indexed`'s
  padding region with the repeated edge row/column instead of zeros (456
  real clamped-non-mirrored-non-POT axis instances, 347 reaching the last
  logical texel where `Linear` blends into zero-filled padding, 71.4%).
  Confirm the fix is a no-op for an already-power-of-two texture and does
  not interact with a mirrored axis (mirror-doubling always lands on a
  power of two, per RE-220). Add a host test packing a non-power-of-two
  image and asserting the padding equals the repeated edge. Re-run
  `tile_addressing_census_against_real_archive_textures` to confirm the
  affected primitives now sample real edge data. `R2.0`/P1
  (`G_SETTILE` field census) remains queued after it.
- Blockers: R2.0/P0d–P1 (POT-padding fix; `G_SETTILE` palette/line/tmem/
  shift census) must close before R2.1/T1–T10 resumes; R2.1 was designated
  but never implemented, so resuming there loses no progress. R2.2/C1–C7
  renderer corrective gate remains behind R2.1. Combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-217, RE-218, RE-219, RE-220,
  RE-221.
- Plan: `PLAN.md` — R2.0/P0d.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing; `docs/rendering.md`
  — "Texture addressing" row.
- Verification: `cargo test -p ssb-rom n64_addressing` (8 tests) and
  `cargo test -p ssb-rom texture::` (49 tests, 2 new) and
  `cargo test -p romtool tile_addressing_census_against_real_archive_textures -- --nocapture`
  against the real ROM — divergence now 0/810; `cargo test --workspace` —
  548 passing, 0 failed.
- Documentation: RE-221, `docs/rendering.md`, `PLAN.md` R2.0/P0c, this
  snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only. Not re-run for this fix: RE-221's
  confidence note flags a pixel-level before/after on Fox/Captain
  Falcon/Kirby as a natural follow-up, not required for P0c's acceptance
  criteria (archive-wide zero divergence, measured above).
- Commit: pending.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
