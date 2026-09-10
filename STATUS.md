# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.0/P1 — Archive-wide G_SETTILE field census`
- Status: `TODO`
- Last complete: `RE-222` (2026-09-11) closed `R2.0/P0d`: fixed PSP
  power-of-two texture padding vs the N64 logical clamp boundary. Two new
  helpers in `crates/ssb-rom/src/psp_texture.rs`, `pad_edge_repeat`
  (byte-granular) and `pad_edge_repeat_nibbles` (`PsmT4`'s two-texels-per-
  byte packing), fill a non-power-of-two texture's padding region with the
  repeated edge row/column instead of zeros — a no-op for an already-POT
  texture, and never touching a mirrored axis by construction (mirror-
  doubling always lands on a power of two, RE-220). Found `PLAN.md`'s task
  text named the wrong functions (`pack_rgba`/`pack_indexed`): the real
  production padding site is `encode_level` (via `pack_mipped`,
  `convert_texture`'s actual call path); fixed both that and `pack_rgba`/
  `pack_indexed` (the particle-frame path) for consistency. 7 new host
  tests, including one through `pack_mipped` end-to-end. Re-ran
  `tile_addressing_census_against_real_archive_textures`: bullet 3's counts
  are unchanged (124/456/347) as expected, since it measures the
  structural condition, not the padding fix itself.
- Next: `R2.0`/P1 — census every real render-tile-0 `G_SETTILE` archive-wide
  for the `palette`/`line`/`tmem`/`shift_s`/`shift_t` fields `dl.rs`'s
  `Cmd::SetTile` already decodes but `mesh.rs`'s only consumer discards
  behind a `..` wildcard (`mesh.rs:1767-1788`). In particular verify
  whether any CI4 render tile uses a non-zero `palette` bank. For each
  field: if every real value is canonical/irrelevant to the current static
  conversion, pin the invariant with a test; if a non-default value changes
  observable sampling semantics for real content, open a scoped correctness
  task rather than implementing unused complexity speculatively. This is
  the last task before `R2.0` can return to `COMPLETE` and `R2.1`/T1–T10
  resumes.
- Blockers: R2.0/P1 (`G_SETTILE` field census) must close before R2.1/T1–T10
  resumes; R2.1 was designated but never implemented, so resuming there
  loses no progress. R2.2/C1–C7 renderer corrective gate remains behind
  R2.1. Combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-217, RE-218, RE-219, RE-220,
  RE-221, RE-222.
- Plan: `PLAN.md` — R2.0/P1.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing; `docs/rendering.md`
  — "Texture addressing" row.
- Verification: `cargo test -p ssb-rom psp_texture::` (40 tests, 7 new) and
  `cargo test -p romtool tile_addressing_census_against_real_archive_textures -- --nocapture`
  against the real ROM — bullet 3 counts unchanged, as expected;
  `cargo test --workspace --all-targets` (pinned 1.98.0 toolchain) — 555
  passing, 0 failed; `cargo clippy --workspace --all-targets` clean.
- Documentation: RE-222, `docs/rendering.md`, `PLAN.md` R2.0/P0d, this
  snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only. Not re-run for this fix: same
  caveat as RE-221 — a pixel-level before/after is a natural follow-up, not
  required for P0d's acceptance criteria (padding-content fix covered by
  the new unit tests, structural census counts unaffected by design).
- Commit: pending.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
