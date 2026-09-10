# Project Status

**Last updated:** 2026-09-10

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** Framebuffer paths — `PLAN.md` R1's next unchecked
acceptance item ("all required framebuffer paths render"). RE-190 scoped it
to exactly one remaining mechanism; RE-191 (this session) resolved both of
RE-190's open copy-order questions via a direct ROM probe. Implementation
still blocked on two prerequisites (see Stop condition).

**Status:** `IN_PROGRESS` (fully understood, not implemented)

**Dependencies:** RE-172–190 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1's framebuffer bullet; `docs/rendering.md`
"Framebuffer effects" row;
`refs/ssb-decomp-re/src/sc/sc1pmode/sc1pstageclear.c:2119`
(`sc1PStageClearCopyFramebufToWallpaper`);
`refs/ssb-decomp-re/src/libultra/sp/sprite.c` (`spDraw`, `drawbitmap`);
`refs/ssb-decomp-re/include/PR/sp.h` (`Sprite`/`Bitmap` structs);
`docs/reverse-engineering.md` RE-190, RE-191.

**First checks:** `git status --short`; `git log -5 --oneline`; re-read
RE-191's "Remaining scope" before touching code.

**Acceptance:** `PLAN.md` R1's "all required framebuffer paths render"
bullet. RE-190 already ruled out every other `framebuf` decomp reference as
N64-only plumbing with no PSP counterpart; only
`sc1PStageClearCopyFramebufToWallpaper` (1P Mode Stage Clear wallpaper
capture) remains, and RE-191 confirms the PSP port needs only a plain
300×220 linear-buffer capture (no N64 tile swizzle/padding required).

**Stop condition:** Two prerequisites are still missing before this can be
implemented and verified, per RE-191's "Remaining scope": (1) no
1P-mode/results-screen game state exists yet to call the capture from
(same "renderer owns mechanism, G2 owns trigger" split as RE-149's
LB-transition precedent — a dev-harness-only call site can substitute), and
(2) `romtool`/pack format has no `Sprite`/`Bitmap` asset representation to
render the captured wallpaper into for on-device verification. Next session
should decide and build one of RE-191's two proposed paths (minimal
`Sprite` pack support, or a dev-harness capture-and-display path) before
writing the actual capture code.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations and effects now
  have software audits.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is now checked off.
- Next R1 work: framebuffer wallpaper-capture mechanism (RE-190/191, fully
  understood, blocked on `Sprite` pack support or a dev-harness call site),
  runtime `MObj` display state, unexplained rendering commands/assets/
  material failures, and remaining regression rows.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-191 — wallpaper-capture copy-order fully explained by ROM probe + `spDraw` source**

- Resolved RE-190's stop condition with a direct ROM probe:
  `romtool dump` on relocData file 26, decoded the `Sprite`/`Bitmap` header
  at the offset the file's own `.spritelist` names (`sprite_0x20718`).
- Confirmed hypothesis (a): destination is tiled into 37 `Bitmap`s of 6
  rows each (`bmheight=6`, `nbitmaps=37`), and consecutive tiles' `buf`
  pointers are offset by exactly 3600 (pixel data) + 8 (2-word gap) bytes —
  an exact match to the copy loop's `wallpaper_pixels += 2` every 6th row.
- Explained the odd/even chunk swap from the decompiled `spDraw`/
  `drawbitmap` (`libultra/sp/sprite.c`): each tile is loaded into TMEM via
  `gDPLoadTextureBlock` (hardware `LoadBlock`), which requires the standard
  N64 16-bit-texture row swizzle this copy routine is pre-baking.
- Established the PSP-side porting implication: no swizzle or tile padding
  needed at all — PSP GE just needs a plain linear 300×220 capture.
- Documentation-only session (matches RE-099/190 precedent); no code
  changed. Probe output written to gitignored `assets/generated/dump/`.
- Still blocked on two prerequisites before implementation: no
  1P-mode/results-screen state to call it from, and no `Sprite`/`Bitmap`
  asset representation in `romtool`/pack format to verify against.
- Evidence: `docs/reverse-engineering.md` RE-191.
- Commit: pending (this session).

## Verification

Documentation-only change; no source touched. `git diff --stat` for this
session covers `PLAN.md`, `STATUS.md`, and `docs/reverse-engineering.md`
only. Prior session's RE-189 verification (`cargo test --workspace`,
strict Clippy, `cargo fmt --check`, on-device PPSSPP capture) is
unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–191.
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
