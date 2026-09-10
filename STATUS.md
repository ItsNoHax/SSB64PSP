# Project Status

**Last updated:** 2026-09-10

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** Framebuffer paths — `PLAN.md` R1's next unchecked
acceptance item ("all required framebuffer paths render"). RE-190 scoped it
to exactly one remaining mechanism; RE-191 resolved its copy-order
questions; RE-192 (this session) implemented and device-verified the
PSP-side capture mechanism itself. Still blocked on real integration (see
Stop condition) — the bullet cannot be checked off yet.

**Status:** `IN_PROGRESS` (mechanism implemented and device-verified; no
real caller)

**Dependencies:** RE-172–191 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1's framebuffer bullet;
`psp/src/gu.rs` (`WALLPAPER_PHOTO*`, `Gpu::request_wallpaper_capture`,
`capture_wallpaper_photo`, `blit_wallpaper_debug`, `wallpaper_photo_data`);
`psp/src/main.rs` (`wallpaper_audit_capture` wiring); `psp/Cargo.toml`
(`wallpaper_audit_capture` feature);
`refs/ssb-decomp-re/src/sc/sc1pmode/sc1pstageclear.c:2119`
(`sc1PStageClearCopyFramebufToWallpaper`);
`docs/reverse-engineering.md` RE-190, RE-191, RE-192.

**First checks:** `git status --short`; `git log -5 --oneline`; re-read
RE-192's "Remaining scope" before touching code.

**Acceptance:** `PLAN.md` R1's "all required framebuffer paths render"
bullet. RE-190 already ruled out every other `framebuf` decomp reference as
N64-only plumbing with no PSP counterpart; only
`sc1PStageClearCopyFramebufToWallpaper` (1P Mode Stage Clear wallpaper
capture) remains. RE-191 confirmed the PSP port needs only a plain 300×220
linear-buffer capture (no N64 tile swizzle/padding required); RE-192 built
that capture and proved it on device, but it still has no real caller and
nothing packed to render it through, so the bullet stays unchecked.

**Stop condition:** Two prerequisites are still missing before this bullet
can be checked off, per RE-192's "Remaining scope": (1) no 1P-mode/
results-screen game state exists yet to call `request_wallpaper_capture`
from for real (same "renderer owns mechanism, G2 owns trigger" split as
RE-149's LB-transition precedent), and (2) no packed asset exists for a
results-screen quad to sample the capture through — a
`TextureDesc::ROLE_FRAMEBUFFER`-shaped pack entry sized 300×220 (RE-192
explicitly ruled out needing a `Sprite`/`Bitmap` asset class for this).
Next session should build one of those two, most likely alongside whatever
first brings 1P-mode/results-screen state into the project at all.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations and effects now
  have software audits.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is now checked off.
- Next R1 work: wallpaper-capture real integration (RE-192 built the
  mechanism; needs 1P-mode/results-screen state and a
  `ROLE_FRAMEBUFFER`-shaped 300×220 pack entry to bind it through), runtime
  `MObj` display state, unexplained rendering commands/assets/material
  failures, and remaining regression rows.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-192 — wallpaper-capture mechanism implemented and device-verified**

- Built `Gpu::request_wallpaper_capture`/`capture_wallpaper_photo` in
  `psp/src/gu.rs`: a plain 300×220 block copy (`WALLPAPER_PHOTO_WIDTH`/
  `_HEIGHT`), no periodic wrap-fill — RE-191 established the PSP port needs
  none of the N64 swizzle/tile padding `TRANSITION_PHOTO_HEIGHT`'s 6→8 pad
  exists for. Same pillarbox-offset and draw-buffer-selection contract as
  the existing `capture_transition_photo`.
- Deliberately did not add `Sprite`/`Bitmap` pack support: RE-191's own
  porting implication already shows the PSP destination only needs the same
  shape `TextureDesc::ROLE_FRAMEBUFFER` already models for the LB
  transition, just larger — a `Sprite` asset class would only matter for a
  byte-level ROM comparison, not for this mechanism.
- Added `wallpaper_audit_capture` (`psp/Cargo.toml`) following the existing
  `*_audit_capture` feature convention (`transition_audit_capture` et al.):
  requests the capture at sim tick 240, then every later frame calls a new
  `Gpu::blit_wallpaper_debug` that writes the capture directly into the
  draw buffer's top-left corner via a raw CPU block copy.
- Verified on device: `cargo psp --release --features
  wallpaper_audit_capture` vs. plain `cargo psp --release`, both screenshot
  via `tools/run-ppsspp.sh`. The audit build's left 300 columns show a
  bounded, visible ghost of an earlier frame's real content (doubled debug
  text, a second stage-silhouette triangle); the right 180 columns are
  pixel-clean and match the baseline exactly — proof the capture holds
  genuine varying pixel data and the write is correctly bounded.
- `cargo psp --release` (default) still builds clean; `cargo test
  --workspace` (337, psp/ is outside the workspace) and `cargo fmt --check`
  in `psp/` both clean.
- Still blocked on the same two prerequisites RE-191 identified, now
  narrower: no 1P-mode/results-screen state to call the capture from for
  real, and no packed `ROLE_FRAMEBUFFER`-shaped 300×220 texture entry for a
  results-screen quad to bind.
- Evidence: `docs/reverse-engineering.md` RE-192.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `psp/src/gu.rs`, `psp/src/main.rs`,
`psp/Cargo.toml`, `PLAN.md`, `STATUS.md`, and `docs/reverse-engineering.md`.
`cargo psp --release` (default) and `cargo psp --release --features
wallpaper_audit_capture` both build clean; `cargo fmt --check` in `psp/`
clean; `cargo test --workspace` 337 passing (psp/ is outside the workspace,
unaffected by this session's changes). On-device: `tools/run-ppsspp.sh
--no-build --seconds 10` against both builds, screenshots compared —
see RE-192. Prior session's RE-189 verification is unaffected and remains
valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–192.
- Rendering methodology: `docs/visual-regression.md`.
- Permanent decisions: `DECISIONS.md`.

## Blockers and caveats

- Physical PSP validation is incomplete; PPSSPP does not prove hardware
  correctness.
- Combat is prohibited until rendering gate passes.
- ROM-derived generated assets remain uncommitted; rebuild pack when asset
  pipeline code changes.

## State update contract

Keep this file as current snapshot, not append-only journal. Update only
current task/status, last completed task, next task, blockers, changes,
verification, evidence, documentation and commit. Put detailed investigations
in `docs/reverse-engineering.md`; keep PLAN acceptance entries as short
evidence links. Older session detail remains available through git history.

## Continuation command

For `Continue with the plan`: read `AGENTS.md`, this file, relevant `PLAN.md`
section, relevant `docs/porting-status.md` row and `RE-*` evidence entry; then
inspect git state/recent commits, resume this task or select first eligible
TODO, implement, verify, document, update this snapshot and commit focused
work.
