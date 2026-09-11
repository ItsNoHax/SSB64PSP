# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.0/P2 — Fix ignored CI4 palette bank (G_SETTILE.palette)`
- Status: `TODO`
- Last complete: `RE-223` (2026-09-11) closed `R2.0/P1`: censused every real
  render-tile-0 `G_SETTILE` archive-wide (2,238 instances, 1,948 CI4) for
  the `palette`/`line`/`tmem`/`shift_s`/`shift_t` fields `dl.rs` decodes but
  `mesh.rs`'s only consumer discards. Wrote a standalone raw-`Cmd` walker in
  `tools/romtool` (`settile_field_census_against_real_archive_textures`,
  following `Cmd::Call`/`Cmd::Branch` itself, matching `texgen`'s own
  `TexgenWalk` pattern) rather than adding permanent instrumentation fields
  to `TextureRef`. `shift_s`/`shift_t`/`tmem` measure zero archive-wide,
  pinned with assertions; `tmem`/`line` are also structurally unconsumed
  (texels are read straight from the ROM file, never through TMEM
  addressing). **`palette` is a real, material, still-open gap**: 7/1,948
  CI4 instances (0.36%), all in file 86 (`ITCommonObject`, real shipped
  item content), request bank 1 of a 48-entry loaded TLUT that `mesh.rs`
  currently always resolves as bank 0 — a real wrong-colour bug. Opened
  `R2.0`/P2 rather than fixing speculatively, per this queue's own rule.
  No production code changed; only the new romtool test.
- Next: `R2.0`/P2 — thread `Cmd::SetTile.palette` through `mesh.rs`'s
  `State`/`TextureRef` (RE-220's own precedent for adding a raw
  `G_SETTILE` field once it's known to matter) and offset
  `palette_offset` by `palette * 16` entries at pack time
  (`tools/romtool`'s `convert_texture`). Confirm no-op when `palette == 0`
  (the overwhelming common case) or when the loaded TLUT is smaller than
  `(palette + 1) * 16` entries (guard rather than panic/index out of
  bounds; should not occur on real content per RE-223's own measurement).
  Add a host test constructing a multi-bank `LoadTlut` + non-zero-`palette`
  `SetTile` and asserting the resolved `palette_offset` lands at the
  correct bank. Confirm visually, e.g. a PPSSPP screenshot of file 86's
  affected item(s) before/after. This is the last task before `R2.0` can
  return to `COMPLETE` and `R2.1`/T1–T10 resumes.
- Blockers: R2.0/P2 (ignored CI4 palette bank) must close before R2.1/T1–T10
  resumes; R2.1 was designated but never implemented, so resuming there
  loses no progress. R2.2/C1–C7 renderer corrective gate remains behind
  R2.1. Combat remains gated.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` — RE-217, RE-218, RE-219, RE-220,
  RE-221, RE-222, RE-223.
- Plan: `PLAN.md` — R2.0/P2.
- Subsystem: `docs/porting-status.md` — PSP mesh drawing; `docs/rendering.md`
  — "Texture addressing" row.
- Verification: `cargo test -p romtool settile_field_census_against_real_archive_textures -- --nocapture`
  against the real ROM — see numbers above; `cargo test --workspace
  --all-targets` (pinned 1.98.0 toolchain) — 556 passing, 0 failed;
  `cargo clippy --workspace --all-targets` clean.
- Documentation: RE-223, `docs/rendering.md`, `PLAN.md` R2.0/P1, this
  snapshot.
- Visual verification: PPSSPPHeadless via `tools/run-ppsspp-headless.sh`;
  windowed PPSSPP is interactive-only. Not run for this task (measurement
  only, no rendering-affecting code changed); P2's own acceptance criteria
  calls for a before/after screenshot of file 86's affected item(s) once
  the fix lands.
- Commit: `671a23b`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
