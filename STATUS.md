# Project Status

**Last updated:** 2026-09-10 (RE-202 physical-PSP session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** RE-202. The plain interactive `cargo psp --release`
build (no features) crashed real hardware within seconds inside
`sceGuDebugFlush`, content-independent; RE-201 had only ever hardware-tested
`regression_capture`, which never draws the HUD. Fixed by gating the
interactive HUD behind a new `debug_overlay` feature, off by default;
`tools/run-ppsspp.sh` opts it back in for PPSSPP dev use. Verified: 60+ s
sustained on-device run, no exception, after the fix.

**Dependencies:** R0.5 and R1 complete. R2 hardware checklist remains.

**Relevant files:** `PLAN.md` R0.5/R2; `docs/reverse-engineering.md` RE-201,
RE-202; `docs/psplink.md`; `psp/src/main.rs`, `psp/Cargo.toml`,
`tools/run-ppsspp.sh`.

**First checks:** confirm whether physical PSP hardware is available. If not,
no further software-only rendering task is eligible: R2 is blocked by R1 and
R3/combat remain blocked behind R2.

**Acceptance:** `PLAN.md` R2.

**Next:** with the interactive build now hardware-stable, capture
representative fighter/stage/animation/material/texture paths and record
VRAM/environment evidence on this same PSP. Separately open: confirm whether
the original bug report (fighter unresponsive to input) was fully explained
by the HUD crash, or whether the D-pad-cycles-stages-not-movement design
(analog nub drives the fighter, not D-pad) also needs revisiting once a user
can interact with a stable build.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; Dream Land boots, pack loads, stage/material/textures
  render, and no hardware exception remains in the captured regression path.
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
- Golden coverage: RE-200's graph census identifies 119 texture-blend, 12
  flat-colour, and 252 classified translucent graph-backed primitives.
  Scene 3 (file 109 graph `0x44C8`) covers texture blend, translucency, and
  clean clamp; scene 4 (file 84 graph `0x2760`) covers flat colour.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-202 — Interactive HUD crash on real hardware, fixed**

- Reproduced live over the same PSPLink/`host0:` setup as RE-201: plain
  `cargo psp --release` (no features) crashed `main_thread` inside
  `sceGuDebugFlush` (`AdEL`, misaligned `BadVAddr`) within seconds of
  `ldstart`, mapped via `llvm-addr2line`/`llvm-objdump`.
- Proved content-independence: a one-word HUD crashed just as fast as the
  full 19-line one, ruling out a string-length/overflow theory.
- Fix: new `debug_overlay` Cargo feature, off by default, gating the
  interactive viewer's `gpu.debug_text` HUD call; `tools/run-ppsspp.sh`'s
  default build passes it explicitly so PPSSPP dev keeps the HUD.
- Verified fixed build sustains 60+ s on-device with `exlist` empty and
  `main_thread` alive; native `scrshot` capture shows Dream Land rendering
  normally.
- Evidence: `docs/reverse-engineering.md` RE-202.

## Verification

`cargo psp --release` (both with and without `debug_overlay`) builds clean.
Pre-fix build reproducibly crashed on PSP Slim (firmware 6.61, PSPLink
v3.2.1) inside `sceGuDebugFlush` within seconds, twice, with two different
HUD contents. Post-fix build ran 60+ s with no exception, `main_thread`
alive, verified twice (35 s and 60+ s). Native framebuffer captures taken
before and after, not committed per `docs/psplink.md`. Pack unchanged; no
Rust unit tests affected (the change is a feature gate around an existing
call, not new logic), so `cargo test --workspace` was not required.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–202.
- Rendering methodology: `docs/visual-regression.md`.
- Hardware crash workflow: `docs/psplink.md`.
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
