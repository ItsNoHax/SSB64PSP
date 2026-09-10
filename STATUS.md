# Project Status

**Last updated:** 2026-09-10 (RE-195 session)

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** "No unexplained missing assets remain" — `PLAN.md` R1's
next unchecked acceptance item. RE-195 (this session) closed "no unexplained
rendering commands remain", so this is now the first open item. Not started
yet this session.

**Status:** `TODO` (not started)

**Dependencies:** RE-172–195 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1's remaining bullets ("no unexplained missing
assets/material failures remain", "rendering regression suite passes",
"golden/reference renders are established"); `docs/rendering.md` "Texture
conversion results"/"Not yet handled" tables; `docs/porting-status.md`
"Known gaps"; `docs/visual-regression.md` (existing test-matrix rows).

**First checks:** `git status --short`; `git log -5 --oneline`; re-run
`romtool textures`/`romtool mobj` to get a current failure census before
assuming anything is still accurate — R0.3/R0.4/R0.7 already resolved every
texture/palette/material-table failure this project's own converter can
explain (see their own `PLAN.md` evidence), so this bullet's actual
remaining scope may be narrower than it looks; reconcile acceptance text
against that existing evidence per `AGENTS.md` §2 before assuming new work
is needed.

**Acceptance:** `PLAN.md` R1's three remaining unchecked bullets (§7).

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
- Next R1 work: no unexplained missing assets/material failures remain,
  rendering regression suite passes, remaining golden-render matrix rows.
  `MObj` display-state parity (RE-194) and rendering-command coverage
  (RE-195) are now closed.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-195 — `G_SETOTHERMODE_H`/`L`'s remaining undecoded fields measured archive-wide; `G_MDSFT_ALPHACOMPARE` found real and partially wired**

- Measured (via RE-124/127's own temporary-census-through-the-real-`romtool
  pack`-build method, reverted before committing) the six `G_SETOTHERMODE_H`
  fields and two `G_SETOTHERMODE_L` fields `mesh.rs` had never decoded:
  `ALPHADITHER`/`RGBDITHER`/`COMBKEY`/`TEXTCONV`/`TEXTLUT`/`TEXTPERSP`/
  `PIPELINE` and `ZSRCSEL`/`ALPHACOMPARE`.
- Six fields match the RDP's own per-frame reset default exactly. `TEXTLUT`
  is real but redundant with `G_SETTILE` format data already read.
  `PIPELINE` deviates from its default (`G_PM_1PRIMITIVE` vs. the reset's
  `G_PM_NPRIMITIVE`) but is a scheduling hint with no visible pixel effect.
  File 73 (`MVOpeningSector`, the opening movie, already known
  not-yet-rendered per RE-120) produced a 415,245-occurrence `TEXTLUT`
  outlier from real display-list call-graph replay, correctly excluded as
  unrepresentative rather than trusted.
- `G_MDSFT_ALPHACOMPARE` is genuinely new and non-default: 29.8% of real
  commands request `G_AC_THRESHOLD`. A second census correlating it against
  the existing `alpha_test` approximation at every real triangle found
  10,334 real vertex-visits with no alpha discard applied today where real
  hardware would apply one — a genuine, previously unmeasured gap, disjoint
  from the 28,859 visits where `alpha_test` already (approximately) covers
  it.
- Implemented and unit-tested the safe, additive case only:
  `MeshMaterial::alpha_compare_threshold` (`mesh.rs`), `flags::
  ALPHA_COMPARE_THRESHOLD`/`PrimDesc::alpha_compare_ref` (`pack.rs`,
  `PrimDesc::SIZE` 48→52, `pack::VERSION` 25→26), consumed by `meshdraw.rs`
  via `sceGuAlphaFunc(GreaterOrEqual, reference, 0xFF)` — only packed when
  `alpha_test` is not already set for that primitive, so the two real gates
  are never combined without a priority decision this session did not
  resolve.
- Also fixed a stale `docs/rendering.md` claim (RE-120 had already
  cross-referenced `G_SHADE`-cleared-with-shade-reading-combiner, but the
  doc still said "not yet cross-referenced").
- 5 new unit tests (2 in `mesh.rs` locking the decode independent of
  `alpha_test`, 3 in `pack.rs` locking the packing guard). `cargo test
  --workspace` (`SSB64_ROM` set): 347 passing (was 342). `cargo fmt --check`
  and `cargo clippy --workspace --all-targets` both clean; `cargo clippy
  --release` inside `psp/` shows no new warnings.
- Rebuilt the pack (1345 textures, unchanged; size 8060.0 → 8089.5 KiB from
  `PrimDesc` growing 4 bytes). `cargo psp --release` +
  `tools/run-ppsspp.sh --seconds 8`: clean boot, 60 FPS, no log errors.
  Pixel-diffed against an equivalent pre-fix build (`git stash`): 257/522240
  pixels differ, all inside the HUD's own frame-timing digits (expected
  run-to-run noise) — zero pixels differ in rendered geometry. The boot
  scene does not contain an affected primitive, so this confirms no
  regression, not a positive visual proof of the fix.
- Checked off `PLAN.md` R1's "no unexplained rendering commands remain"
  acceptance item.
- Evidence: `docs/reverse-engineering.md` RE-195.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `crates/ssb-rom/src/mesh.rs`,
`crates/ssb-rom/src/pack.rs`, `psp/src/meshdraw.rs`, `PLAN.md`,
`docs/rendering.md`, `STATUS.md`, and `docs/reverse-engineering.md`.
`cargo fmt --check`, `cargo clippy --workspace --all-targets`, and `cargo
test --workspace` (347, `SSB64_ROM` set) all clean; `cargo clippy --release`
inside `psp/` shows no new warnings. Pack rebuilt and reloads cleanly (1345
textures unchanged, size +29.5 KiB from the wider `PrimDesc`, see above).
On-device: `tools/run-ppsspp.sh --seconds 8`, clean log, 60 FPS,
pixel-diffed against an equivalent pre-fix build — see RE-195. Prior
sessions' verification (RE-190–194) is unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–195.
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
