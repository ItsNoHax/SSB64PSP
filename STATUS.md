# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T1 — G_VTX model-space invariance` (next up; not started)
- Status: `TODO`
- Last complete: `RE-224` (2026-09-11) closed `R2.0/P2`, and with it all of
  `R2.0`: threaded `Cmd::SetTile.palette` through `mesh.rs`'s `State`/
  `TextureRef` (new `TextureRef::palette: u8` field) and, at pack time,
  added `tools/romtool`'s `palette_bank_offset` helper to shift the TLUT
  read by `palette * 16` entries -- applied in `convert_texture` (the real
  pack path) and, for consistency, three CLI-only debug commands
  (`texdump`, `textures`, `texgen`). No-op by construction when
  `palette == 0` (1,941/1,948 real CI4 instances); guarded (falls back to
  bank 0) when the loaded TLUT is smaller than the requested bank. Two new
  host tests confirm it, both built from RE-223's own directly-measured
  real shape (three 16-entry banks, bank 1 requested, file 86
  `ITCommonObject`): `ssb-rom`'s
  `tile0_palette_bank_is_carried_onto_the_texture_reference` and
  `romtool`'s `convert_texture_resolves_the_requested_palette_bank` (also
  covers the `palette == 0` no-op and the out-of-range-bank guard).
  **PPSSPP visual confirmation was attempted and not obtained** -- see
  RE-224 for the full account: a temporary (reverted) instrumentation pass
  found the exact coordinates for a manual check (global texture indices
  187/194/195, object-view indices 60-102 for file 86), but driving the
  interactive debug viewer unattended in this environment hit real
  obstacles (no `xdotool`/`xset`, no sudo to install either; a stuck
  synthetic keypress caused several seconds of uncontrolled navigation
  drift; no tested key reached the Triangle/TEXVIEW binding; the
  object-view debug overlay's text renders corrupted/double-exposed in
  every capture, a separate real bug, flagged as its own follow-up, not
  caused by this task). Closed on host-test evidence per an explicit
  user decision, not an oversight. `R2.0` (P0a-P0d, P1, P2) is now fully
  `COMPLETE`.
- Next: `R2.1`/T1 -- `G_VTX` model-space invariance. `R2.1` was previously
  designated (RE-217) but never implemented, so it resumes fresh (no
  progress lost). Read `PLAN.md`'s `R2.1` section in full before starting:
  preserve current known-good texgen behaviour (raw GEN/LINEAR bits stay
  independent, LINEAR never enables generation alone, pack version 27
  keeps texgen scale/tile origin, regular texgen uses the GE texture-matrix
  path, linear texgen stays CPU-generated through authored UVs) while
  executing T1 through T10 in order.
- Blockers: none. `R2.0` is closed; `R2.1`/T1-T10 is unblocked and next.
  `R2.2`/C1-C7 renderer corrective gate remains behind `R2.1`. Combat
  remains gated behind `R2.2`.
  Separately (not blocking): a real bug was found and flagged (not fixed)
  in the `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture, making exact on-screen values
  unreliable to read from a screenshot. See the spawned follow-up task.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` -- RE-217, RE-218, RE-219, RE-220,
  RE-221, RE-222, RE-223, RE-224.
- Plan: `PLAN.md` -- `R2.0` (now `COMPLETE`); `R2.1`/T1 next.
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing; `docs/rendering.md`
  -- "Texture addressing" row (now `VERIFIED`).
- Verification: `cargo test -p romtool settile_field_census_against_real_archive_textures -- --nocapture`
  and `cargo test -p romtool convert_texture_resolves_the_requested_palette_bank`
  and `cargo test -p ssb-rom tile0_palette_bank_is_carried_onto_the_texture_reference`
  against the real ROM; `cargo test --workspace --all-targets` (pinned
  1.98.0 toolchain) -- 558 passing, 0 failed; `cargo clippy --workspace
  --all-targets` clean.
- Documentation: RE-224, `docs/rendering.md`, `PLAN.md` `R2.0`/P2, this
  snapshot.
- Visual verification: PPSSPP debug viewer (`debug_overlay` feature) is
  windowed/interactive-only; attempted to automate for this task via raw
  X11 key injection and did not get reliable results (see `Last complete`
  above and RE-224). A before/after TEXVIEW screenshot of file 86's
  affected item(s) (global texture indices 187/194/195, object indices
  60-102) remains an open manual follow-up, not blocking `R2.1`.
- Commit: `52cdae4`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
