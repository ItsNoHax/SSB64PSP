# Project Status

**Last updated:** 2026-09-10

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** Framebuffer paths — `PLAN.md` R1's next unchecked
acceptance item ("all required framebuffer paths render"). RE-190 (this
session) scoped it down to exactly one remaining mechanism.

**Status:** `IN_PROGRESS` (scoped, not implemented)

**Dependencies:** RE-172–189 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1's framebuffer bullet; `docs/rendering.md`
"Framebuffer effects" row;
`refs/ssb-decomp-re/src/sc/sc1pmode/sc1pstageclear.c:2119`
(`sc1PStageClearCopyFramebufToWallpaper`);
`refs/ssb-decomp-re/include/PR/sp.h` (`Sprite`/`Bitmap` structs);
`docs/reverse-engineering.md` RE-190.

**First checks:** `git status --short`; `git log -5 --oneline`; re-read
RE-190's open questions before touching code.

**Acceptance:** `PLAN.md` R1's "all required framebuffer paths render"
bullet. RE-190 already ruled out every other `framebuf` decomp reference as
N64-only plumbing with no PSP counterpart; only
`sc1PStageClearCopyFramebufToWallpaper` (1P Mode Stage Clear wallpaper
capture) remains.

**Stop condition:** Two copy-order details are unexplained (odd/even `u32`
chunk swap per row; 2-word destination skip every 6th row) — resolve via a
throwaway ROM probe of relocData file 26's `Sprite`/`Bitmap` header (or a
corroborating second call site) before writing PSP code, per RE-190's
"Remaining scope".

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations and effects now
  have software audits.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is now checked off.
- Next R1 work: framebuffer wallpaper-capture mechanism (RE-190, scoped),
  runtime `MObj` display state, unexplained rendering commands/assets/
  material failures, and remaining regression rows.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-190 — framebuffer-path census; scoped down to one remaining mechanism**

- Exhaustively categorized all 399 `framebuf`/`copyfb`/`G_SETCIMG` hits
  across the decomp. N64 VI swap-chain scheduling, the crash-screen debug
  overlay, and nine files' fixed-VRAM heap-arithmetic are all N64-only
  plumbing with no PSP counterpart. `lb/lbtransition.c` is R0.13, already
  complete.
- Found exactly one remaining content-bearing mechanism:
  `sc1PStageClearCopyFramebufToWallpaper` (1P Mode Stage Clear results
  wallpaper) — copies the same 300×220 active-picture rectangle RE-099/100
  already established into a real ROM sprite buffer (relocData file 26,
  `GRWallpaperTrainingBlack`), not a synthetic segment reference, so it is
  invisible to `romtool textures`'s report.
- Two copy-order details (odd/even `u32` chunk swap per row; 2-word
  destination skip every 6th row) are recorded as open, not guessed;
  resolving them needs a ROM header probe or a corroborating second call
  site before implementation.
- Documentation-only session (matches RE-099's own precedent); no code
  changed.
- Evidence: `docs/reverse-engineering.md` RE-190.
- Commit: pending (this session).

## Verification

Documentation-only change; no source touched. `git diff --stat` for this
session covers `PLAN.md`, `STATUS.md`, `docs/rendering.md`, and
`docs/reverse-engineering.md` only. Prior session's RE-189 verification
(`cargo test --workspace`, strict Clippy, `cargo fmt --check`, on-device
PPSSPP capture) is unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–190.
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
