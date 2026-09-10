# Project Status

**Last updated:** 2026-09-10

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** Framebuffer paths — `PLAN.md` R1's next unchecked
acceptance item ("all required framebuffer paths render"), first eligible
`TODO` now that RE-189 closes the effects item.

**Status:** `TODO`

**Dependencies:** RE-172–189 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1 (framebuffer/`MObj`/unexplained-commands
bullets), `docs/porting-status.md`, `docs/rendering.md`, `TODO.md`.

**First checks:** `git status --short`; `git log -5 --oneline`;
`rg -n "framebuffer|FBObj|G_SETCIMG|copyfb" crates psp docs`.

**Acceptance:** See `PLAN.md` R1's own "all required framebuffer paths
render" bullet; not yet scoped in detail this session.

**Stop condition:** Not yet started.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations and effects now
  have software audits.
- Effects: RE-172–189 cover manager descriptors, transforms, material/
  texture/colour animation, LBParticle decoding/packing, drawing, exhaustive
  audits, spawn-tree execution, `LBGenerator`, and a real manager-effect
  spawn event wired into the PSP runtime and PPSSPP-verified. `PLAN.md`
  R1's "all required effects render" acceptance item is now checked off.
- Next R1 work: framebuffer paths, runtime `MObj` display state, unexplained
  rendering commands/assets/material failures, and remaining regression rows.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-189 — real manager-effect spawn event wired into runtime**

- `efManagerRippleMakeEffect` (`efcommon` script `0x61`) ported as
  `generator::Generator::spawn_at` (host, `crates/ssb-rom`) and `spawn_ripple`
  (PSP runtime, `psp/src/main.rs`).
- New debug-viewer mode `effect_spawn_view` (`C_LEFT`): ticks a live
  `LBGenerator` and its one spawned particle every real frame and draws the
  particle at its own live, re-centred position — not a static frame-4
  snapshot.
- Confirmed this specific target always spawns exactly one particle, once,
  then ejects (`generator_lifetime == 1`, deterministic `update_rate`); the
  debug viewer self-retriggers once both finish so any screenshot shows a
  live effect.
- Refactored `particle_view`'s inline script-conversion into a shared
  `pack_particle_script` helper; no behaviour change (re-verified on-device).
- Evidence: `docs/reverse-engineering.md` RE-189.
- Commit: pending (this session).

## Verification

RE-189 passed `cargo test --workspace` (337 `ssb-rom` tests, was 336, with
and without `SSB64_ROM`), strict Clippy (`cargo clippy --workspace --lib
--tests -- -D warnings`, `cargo clippy -p ssb-rom --no-default-features --
-D warnings`), and `cargo fmt --check` (root workspace and `psp/`
separately). `psp/`'s own strict clippy run was not required (not part of
this project's clippy gate; introduced no new findings versus `main`).
On-device: `cargo psp --release --features effect_spawn_audit_capture` +
`tools/run-ppsspp.sh --no-build` captured a real, non-blank particle sprite
with `gen-alive false particle-alive true` at both 3s and 8s after boot,
confirming the self-retrigger keeps the effect visibly alive indefinitely.
Re-captured `particle_render_audit_capture` unchanged, confirming the
`pack_particle_script` refactor did not regress RE-183's mode.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–189.
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
