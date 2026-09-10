# Project Status

**Last updated:** 2026-09-10 (RE-197 session)

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** "Golden/reference renders are established" — `PLAN.md`
R1's last remaining unchecked acceptance item. RE-197 (this session) closed
"rendering regression suite passes": rebuilt the deterministic capture
EBOOT, reran the golden Dream Land comparison after RE-190–196's code
changes (0 differing pixels, captured screenshot byte-identical to the
committed golden), and reran `fmt`/`clippy`/`cargo test --workspace` (347
passing) clean. Not started yet this session.

**Status:** `TODO` (not started)

**Dependencies:** RE-172–197 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `docs/visual-regression.md` ("Test matrix" — most rows
say "Yes" against the single Dream Land golden scene, but CI8 texture,
`combiner_texture_blend`/`combiner_flat_color` shapes, translucency, clamp
mode and untextured/vertex-coloured geometry are marked "needs
identification"/"needs a dedicated scene"); `PLAN.md` R1's remaining bullet
("golden/reference renders are established").

**First checks:** `git status --short`; `git log -5 --oneline`; read
`docs/visual-regression.md`'s "Test matrix" and "Capture procedure" sections
in full before adding a new scene — the methodology (frozen
`regression_capture` feature + `compare-screenshot.sh`) already generalises,
so this is about identifying a concrete file/offset for each "needs
identification" row and, where none of Dream Land's own camera framing
covers a shape, building one additional dedicated frozen scene, reusing
existing archive-wide census tools before writing new code (RE-196's own
approach).

**Acceptance:** `PLAN.md` R1's one remaining unchecked bullet (§7).

**Stop condition:** None yet — task not started.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations, effects,
  framebuffer paths and rendering-command coverage now have software audits.
  The single-scene golden regression check now passes against post-RE-190–196
  code.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is checked off.
- Framebuffer paths: RE-190–193 cover the exhaustive census, the
  wallpaper-capture mechanism, and a minimal real `SObj` 2D-sprite render
  path (`Gpu::draw_wallpaper_sprite`) that draws the capture back through a
  real GE texture bind, device-verified bounded and correctly dimmed.
  `PLAN.md` R1's "all required framebuffer paths render" acceptance item is
  checked off. Only the real 1P-mode/results-screen G2 trigger remains
  unbuilt — accepted as out of R1 scope, the same split RE-149 already used
  to close R0.13.
- Next R1 work: remaining golden-render matrix rows (CI8 texture,
  `combiner_texture_blend`/`combiner_flat_color`, translucency, clamp mode,
  untextured/vertex-coloured geometry — each "needs identification" or
  "needs a dedicated scene" in `docs/visual-regression.md`'s test matrix).
  `MObj` display-state parity (RE-194), rendering-command coverage
  (RE-195), missing-assets/material-failures reconciliation (RE-196), and
  the golden regression rerun (RE-197) are now closed.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-197 — Full regression stack rerun closes `PLAN.md` R1's "rendering regression suite passes" bullet**

- No source change. Rebuilt the deterministic-capture EBOOT
  (`cargo psp --release --features regression_capture`), ran
  `tools/run-ppsspp.sh --no-build --seconds 6`, and diffed against the
  committed golden with `tools/compare-screenshot.sh`: `differing pixels: 0`,
  `PASS`. The captured screenshot's SHA-256 is byte-identical to the
  committed golden's own hash.
- Confirmed the asset pack used
  (`7647db75dce032048e6ab69a1ada5b6990e8ccfd9c86d36a6c04fe612650b2f0`)
  matches RE-196's own recorded hash — no asset-pipeline drift between
  sessions.
- Reran `cargo fmt --check` (clean), `cargo clippy --workspace --all-targets`
  (clean), `cargo test --workspace` with `SSB64_ROM` set (347 passed, 0
  failed — unchanged from RE-196).
- Rebuilt the plain (non-`regression_capture`) EBOOT afterward per
  `docs/visual-regression.md`'s own rule against leaving that feature
  enabled for normal use.
- Checked off `PLAN.md` R1's "rendering regression suite passes" acceptance
  item. RE-170's 41-stage audit and RE-171's 532-animation audit remain
  valid, separate smoke coverage, not rerun here (nothing in RE-190–196
  touched stage selection or animation playback).
- Evidence: `docs/reverse-engineering.md` RE-197.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `PLAN.md`, `STATUS.md`, and
`docs/reverse-engineering.md` only — no source changed. `cargo fmt --check`,
`cargo clippy --workspace --all-targets`, and `cargo test --workspace` (347,
`SSB64_ROM` set) all clean, unchanged from RE-196. PPSSPP software-render
golden comparison: 0 differing pixels, byte-identical captured screenshot.
Plain (non-`regression_capture`) EBOOT rebuilt afterward. Prior sessions'
verification (RE-190–196) is unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–197.
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
