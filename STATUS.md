# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.0/P0b — General N64 tile-addressing reference model`
- Status: `TODO`
- Last complete: `RE-219` (2026-09-11) closed `R2.0/P0a`: built a host-side
  N64 3-point (triangular) filtering reference sampler
  (`crates/ssb-rom/src/n64_filter.rs`, transcribed from
  `angrylion-rdp-plus`) and measured it archive-wide against PSP's
  symmetric four-tap `Linear` filter — 684 real textures, 5.73% of interior
  sample points differ by ≥8/255, the Dream Land canopy highlight texture
  reaches the maximum 128/255 diff. Recorded `ACCEPTED_DEVIATION`: the PSP
  GE has no third filter mode and no programmable shader stage to
  reproduce the RDP's formula exactly. No renderer code changed.
- Next: build the general N64 tile-addressing reference model (coordinate →
  `G_TEXTURE` scale → tile shift → tile origin → mask → mirror → clamp)
  covering both authored UVs and texgen UVs, then measure the
  mirror+clamp-beyond-first-period, `mask == 0`, and PSP POT-padding
  questions against real archive content, per `PLAN.md` R2.0/P0b.
- Blockers: R2.0/P0b–P1 (general mirror+clamp/mask==0/POT-padding
  tile-addressing model; `G_SETTILE` palette/line/tmem/shift census) must
  close before R2.1/T1–T10 resumes; R2.1 was designated but never
  implemented, so resuming there loses no progress. R2.2/C1–C7 renderer
  corrective gate remains behind R2.1. Combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-217, RE-218, RE-219.
- Plan: `PLAN.md` — R2.0/P0b.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing; `docs/rendering.md`
  — "Texture filtering" (now `ACCEPTED_DEVIATION`) / "Texture addressing" rows.
- Verification: `cargo test -p ssb-rom n64_filter` (5 new tests) and
  `cargo test -p romtool filter_reconstruction_census_against_real_archive_textures`
  against the real ROM; `cargo test --workspace` — 537 passing.
- Documentation: RE-219, `docs/rendering.md`, `PLAN.md` R0.5/R2.0, this
  snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only.
- Commit: `942b744`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
