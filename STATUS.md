# Project Status

**Last updated:** 2026-09-10 (RE-199 session)

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** "Golden/reference renders are established" — `PLAN.md`
R1's last remaining unchecked acceptance item. RE-198 identified concrete
file/offset evidence for three previously-unidentified test-matrix rows.
RE-199 (this session) built the second dedicated `regression_capture_scene2`
frozen scene (file 52, `mvopeningroom.c`'s "MVCommon" graph) and closed two
of the six non-covered rows against a new committed golden,
`tests/golden/r1-mvopeningroom.png`.

**Status:** `IN_PROGRESS`

**Remaining for this item:** four test-matrix rows still uncovered:

- Clamp texture mode — RE-198's clean (non-mirrored) example is file 22, not
  file 52, so RE-199's scene does not cover it. File 52's own clamp
  primitive is a clamp+mirror combination already covered by the "Mirror
  wrap mode" row. Needs a scene (or an extension of `regression_capture_
  scene2`) that puts file 22 on screen.
- `combiner_texture_blend`, `combiner_flat_color`, translucency — RE-198 did
  not tie any of these to a concrete file/offset. They need the same
  identification step RE-198 did for the other three rows (a census-style
  walk over `load_all`/`file_meshes`, filtering on the relevant combiner/
  alpha classification) before a scene can be built for them.

A reasonable next step is identification first (repeat RE-198's approach for
these three), then decide whether one more scene can carry all four
remaining rows or whether file 22's clamp example needs its own.

**Dependencies:** RE-172–199 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `docs/visual-regression.md` ("Test matrix", "The second
deterministic test scene", "Capture procedure"); `psp/src/main.rs`
(`deterministic_capture_frozen`, `stage_view`/`object_index`/`spin` overrides
under `regression_capture_scene2`); `psp/Cargo.toml` (`regression_capture`,
`regression_capture_scene2` features); `tests/golden/r1-mvopeningroom.png`;
`PLAN.md` R1's remaining bullet.

**First checks:** `git status --short`; `git log -5 --oneline`; read
`docs/reverse-engineering.md` RE-198/RE-199 for what is and is not covered;
if extending RE-198's census approach for the remaining three unidentified
rows, the same temporary/reverted `romtool` pattern RE-198 used is the
precedent to follow.

**Acceptance:** `PLAN.md` R1's one remaining unchecked bullet (§7).

**Stop condition:** None yet — four rows remain uncovered.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations, effects,
  framebuffer paths and rendering-command coverage now have software audits.
  Both golden regression scenes (Dream Land and `mvopeningroom`) pass exact
  pixel comparison.
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
- Next R1 work: identify concrete file/offset evidence for
  `combiner_texture_blend`, `combiner_flat_color` and translucency (RE-198's
  approach, not yet repeated for these three), then build whatever scene(s)
  those plus the still-open clamp-texture-mode row (file 22) need. `MObj`
  display-state parity (RE-194), rendering-command coverage (RE-195),
  missing-assets/material-failures reconciliation (RE-196), the golden
  regression rerun (RE-197), and the second scene's two rows (RE-199) are
  closed.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-199 — Second deterministic scene closes two of RE-198's six test-matrix rows**

- Added `regression_capture_scene2`, a second off-by-default Cargo feature
  on `ssb64-psp` (`psp/Cargo.toml`), reusing `regression_capture`'s existing
  tick-240 freeze and HUD-suppression `cfg!(any(...))` lists rather than
  duplicating them.
- `psp/src/main.rs`: disabled the default Dream Land `stage_view` boot under
  this feature; overrode the object viewer's boot heuristic to file 52's
  graph (`ObjectDesc.source_file == 52`, `mvopeningroom.c`'s "MVCommon"
  scene) — the same graph the unmodified heuristic already finds and
  rejects for the *first* golden scene.
- Found and fixed a real determinism gap: the object viewer's idle model
  spin (`spin += 0.02`/frame) was not gated by `deterministic_capture_
  frozen` at all (no prior object-view audit needed exact-match capture, so
  nobody had frozen it). Without the fix, two captures 24 seconds apart
  differed by 126,693 pixels; with it, byte-identical.
- Captured under PPSSPP software rendering; verified byte-identical and 0
  differing pixels across two capture times. Committed
  `tests/golden/r1-mvopeningroom.png`, SHA-256 `db3fd4bce8d3dbbed4534d53fdbea1c3708d708d19298149037676f2628f9ba1`.
- Rebuilt plain `regression_capture` afterward and reconfirmed the original
  Dream Land golden still matches exactly (0 differing pixels) — this
  session's code changes have no effect without the new feature.
- Updated `docs/visual-regression.md` (new "second deterministic test scene"
  section, test-matrix rows for CI8 texture and untextured/vertex-coloured
  geometry now "Yes", clamp-texture-mode row's caveat), `docs/reverse-
  engineering.md` (new RE-199), `PLAN.md` and `STATUS.md`.
- `cargo fmt --check` passed on `psp`. Workspace `cargo test`/`clippy` not
  rerun: `psp` is excluded from the workspace (root `Cargo.toml`) and no
  host-side crate changed.
- Evidence: `docs/reverse-engineering.md` RE-199.
- Commit: pending (this session).

## Verification

`cargo psp --release --features regression_capture_scene2` built clean.
Two PPSSPP captures (`--seconds 6` and `--seconds 30`) were byte-identical
(`cmp`) and 0 differing pixels (`tools/compare-screenshot.sh`). Rebuilt
`cargo psp --release --features regression_capture` and reran the original
Dream Land golden comparison: 0 differing pixels, unchanged. Rebuilt plain
`cargo psp --release` afterward per the documented "always follow a
regression-capture run with a plain build" rule. `cargo fmt --check` passed
on `psp`. No host-side crate changed, so workspace `cargo test`/`clippy`
were not rerun.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–199.
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
