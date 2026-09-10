# Project Status

**Last updated:** 2026-09-10 (RE-198 session)

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** "Golden/reference renders are established" — `PLAN.md`
R1's last remaining unchecked acceptance item. RE-198 (this session)
identified concrete file/offset evidence for the three test-matrix rows
that had none (CI8 texture: file 52 offset `0x2ee8`; clamp texture mode:
file 22 offset `0x8`; untextured/vertex-coloured geometry: file 52 mesh 4
primitive 0) via a temporary, reverted `romtool census-matrix` walk over
the real `load_all`/`file_meshes` pipeline. No source changed — the census
tool was reverted after use; the counts/offsets are recorded in
`docs/reverse-engineering.md` RE-198 as the evidence of record.

**Status:** `IN_PROGRESS`

**Remaining for this item:** all six non-"Yes" test-matrix rows (CI8
texture, `combiner_texture_blend`, `combiner_flat_color`, translucency,
clamp texture mode, untextured/vertex-coloured geometry) now share one
blocker: a second dedicated `regression_capture`-style frozen scene,
which does not yet exist. Building it requires (a) picking a camera/object
state that puts one or more of these concrete assets on screen — file 52's
graphs are attractive since three of the six examples already live there
— (b) extending `psp/src/main.rs`'s deterministic-freeze mechanism
(`DETERMINISTIC_CAPTURE_TICKS`, currently wired to `Play`/`StageAnimator`/
`MaterialAnimator` specifically) to a non-`Play` object/scene view, (c) an
actual PPSSPP capture + a committed second golden PNG + a
`compare-screenshot.sh` invocation, and (d) updating the test-matrix rows
this touches from "needs a dedicated scene" to "Yes". This is a real
feature addition, not another census — do not treat it as finished by
identification alone.

**Dependencies:** RE-172–198 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `docs/visual-regression.md` ("Test matrix", "The
deterministic test scene", "Capture procedure"); `psp/src/main.rs` (freeze
logic, ~line 56 `DETERMINISTIC_CAPTURE_TICKS` and ~line 1811 HUD
suppression); `psp/Cargo.toml` (`regression_capture` feature); `PLAN.md`
R1's remaining bullet.

**First checks:** `git status --short`; `git log -5 --oneline`; read
`docs/reverse-engineering.md` RE-198 for the concrete assets already
identified; read `psp/src/main.rs`'s existing object/animation-viewer
boot path (used by `--audit-animations`/`--audit-stages`) to see whether it
can be pointed at file 52 directly rather than building new navigation.

**Acceptance:** `PLAN.md` R1's one remaining unchecked bullet (§7).

**Stop condition:** None yet — second scene not started.

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
- Next R1 work: build the second dedicated `regression_capture`-style
  scene the golden-render matrix's six remaining rows all need (CI8
  texture, `combiner_texture_blend`/`combiner_flat_color`, translucency,
  clamp mode, untextured/vertex-coloured geometry). RE-198 identified
  concrete file/offset evidence for three of them; none has an actual
  frozen capture yet. `MObj` display-state parity (RE-194), rendering-
  command coverage (RE-195), missing-assets/material-failures
  reconciliation (RE-196), and the golden regression rerun (RE-197) are
  closed.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-198 — Concrete file/offset evidence for three "needs identification" test-matrix rows**

- Added a temporary `romtool census-matrix <rom>` subcommand (reverted
  after use, not shipped) built on the same `load_all`/`file_meshes`
  pipeline `textures`/RE-100 already use, walking every converted
  primitive archive-wide.
- CI8 texture: 75 CI8-bound primitives archive-wide; concrete example file
  52 (`mvopeningroom.c`'s opening-movie scene, RE-060's "MVCommon", fully
  paired) offset `0x2ee8`, 16×32.
- Clamp texture mode: 2,201 clamp-bound primitives archive-wide; clean
  (non-mirrored) example file 22 offset `0x8`, 32×32, fully paired.
- Untextured/vertex-coloured geometry: file 52 mesh 4 primitive 0, 14
  triangles, unlit, opaque vertex colour `[145,213,213,255]`.
- No hit for RE-102's named fighters (Fox/Falcon/Kirby, files 209/236/229)
  through this raw per-file walk — likely reached via `MObj` runtime-tile
  state (`apply_mobj`, RE-194) rather than a static per-file `G_SETTILE`;
  the file-22/file-52 examples stand on their own regardless.
- Updated `docs/visual-regression.md`'s test matrix: these three rows move
  from "needs identification" to "needs a dedicated scene" — the same
  bucket the other three non-covered rows already occupy. All six now
  share one remaining blocker (see Continuation packet above); none is
  resolved by this entry alone.
- No source code changed; only `docs/reverse-engineering.md` (new RE-198),
  `docs/visual-regression.md` (test matrix), `PLAN.md` and `STATUS.md`.
- Evidence: `docs/reverse-engineering.md` RE-198.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `PLAN.md`, `STATUS.md`,
`docs/reverse-engineering.md` and `docs/visual-regression.md` only — no
source changed (the temporary `romtool` instrumentation was built, run, and
reverted; `git status --short` after revert shows only the four docs
files). `cargo build --release -p romtool` confirmed clean after the
revert. Workspace tests/fmt/clippy were not rerun since no source changed
from RE-197's own clean run.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–198.
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
