# Project Status

**Last updated:** 2026-09-10 (RE-196 session)

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** "Rendering regression suite passes" — `PLAN.md` R1's next
unchecked acceptance item. RE-196 (this session) closed "no unexplained
missing assets remain" and "no unexplained material failures remain" by
reconciling existing archive-wide evidence (RE-055–062/139/162/168) against
the current tree, with no code change. Not started yet this session.

**Status:** `TODO` (not started)

**Dependencies:** RE-172–196 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `docs/visual-regression.md` ("Test matrix" — most rows
say "Yes" against the single Dream Land golden scene, but CI8 texture,
`combiner_texture_blend`/`combiner_flat_color` shapes, translucency, clamp
mode and untextured/vertex-coloured geometry are marked "needs
identification"/"needs a dedicated scene"); `PLAN.md` R1's remaining two
bullets ("rendering regression suite passes", "golden/reference renders are
established").

**First checks:** `git status --short`; `git log -5 --oneline`; read
`docs/visual-regression.md`'s "Test matrix" and "Capture procedure" sections
in full before adding a new scene — the methodology (frozen
`regression_capture` feature + `compare-screenshot.sh`) already generalises,
so this is about identifying a concrete file/offset for each "needs
identification" row and, where none of Dream Land's own camera framing
covers a shape, building one additional dedicated frozen scene, reusing
existing archive-wide census tools before writing new code (RE-196's own
approach).

**Acceptance:** `PLAN.md` R1's two remaining unchecked bullets (§7).

**Stop condition:** None yet — task not started.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations, effects,
  framebuffer paths and rendering-command coverage now have software audits.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is checked off.
- Framebuffer paths: RE-190–193 cover the exhaustive census, the
  wallpaper-capture mechanism, and — this session — a minimal real `SObj`
  2D-sprite render path (`Gpu::draw_wallpaper_sprite`) that draws the
  capture back through a real GE texture bind, device-verified bounded and
  correctly dimmed. `PLAN.md` R1's "all required framebuffer paths render"
  acceptance item is checked off. Only the real 1P-mode/results-screen G2
  trigger remains unbuilt — accepted as out of R1 scope, the same split
  RE-149 already used to close R0.13.
- Next R1 work: rendering regression suite passes, remaining golden-render
  matrix rows. `MObj` display-state parity (RE-194), rendering-command
  coverage (RE-195), and missing-assets/material-failures reconciliation
  (RE-196) are now closed.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-196 — Reconciling `PLAN.md` R1's asset/material bullets against existing evidence (no code change)**

- Re-ran every archive-wide census this project already has against the
  current (post-RE-195) tree, to confirm RE-172–195's work reopened nothing
  RE-055–062/139/162/168 had already closed: `romtool check` (0 load/chain
  failures), `romtool scan --exhaustive` (0 unknown opcodes), `romtool
  textures` (721 bound/695 packed/26 failed — the same 26 RE-055 traced to
  the runtime-only `sLBTransitionPhotoHeap` LB-transition framebuffer
  buffer, no ROM data exists to resolve it under D-001), `romtool mobj` (134
  paired, 0 unreadable/unpaired/mismatched, matching R0.7's `COMPLETE`
  status).
- Temporarily instrumented `romtool pack`'s node-list loop to split the
  "1604/1672 node lists placed" figure into cause: 0 conversion errors, 23
  zero-triangle placements — exactly RE-026's own historical count,
  unchanged. Reverted before committing; rebuilt pack byte-identical with
  and without the instrumentation present (SHA-256
  `7647db75dce032048e6ab69a1ada5b6990e8ccfd9c86d36a6c04fe612650b2f0`).
- For material failures (`PLAN.md` R0.6), confirmed via `git diff
  3b9eb50..HEAD -- crates/ssb-rom/src/mesh.rs` that no commit since RE-168's
  own combiner census touched any combiner-shape/alpha-blend/shade-scale
  classification function, so RE-168's 199-decline breakdown (13 real
  runtime-injected shield colours, 186 already-catalogued unsupported
  combiner/alpha-formula edge cases per RE-139) still describes the current
  pack path exactly — no re-run needed to know it has not changed.
- Checked off `PLAN.md` R1's "no unexplained missing assets remain" and "no
  unexplained material failures remain" acceptance items.
- No source change; instrumentation added and reverted. `cargo fmt --check`,
  `cargo clippy --workspace --all-targets`, and `cargo test --workspace`
  (`SSB64_ROM` set) all pass unchanged at 347 tests.
- Evidence: `docs/reverse-engineering.md` RE-196.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `PLAN.md`, `STATUS.md`, and
`docs/reverse-engineering.md` only — no source changed (temporary
instrumentation in `tools/romtool/src/main.rs` was reverted before
committing). `cargo fmt --check`, `cargo clippy --workspace --all-targets`,
and `cargo test --workspace` (347, `SSB64_ROM` set) all clean, unchanged
from RE-195. Pack rebuild verified byte-identical before/after the reverted
instrumentation (SHA-256 above); no PSP/PPSSPP run needed since nothing
renderer-visible changed. Prior sessions' verification (RE-190–195) is
unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–196.
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
