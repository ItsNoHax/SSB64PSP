# Project Status

**Last updated:** 2026-09-10

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** Wire one real manager-effect spawn event into runtime. This
is the remaining scope after RE-188's `LBGenerator` implementation.

**Status:** `IN_PROGRESS`

**Dependencies:** RE-172–188 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `crates/ssb-rom/src/particle.rs`, `psp/src/main.rs`,
`psp/src/meshdraw.rs`, `crates/ssb-rom/src/effect.rs`, `PLAN.md` R1,
`docs/porting-status.md`, and `docs/reverse-engineering.md` RE-172–188.

**First checks:** `git status --short`; `git log -5 --oneline`;
`rg -n "spawn|ParticleTree|Generator|MAKEGENERATOR|EFDesc" crates psp`.

**Acceptance:** A source-backed real effect path creates and draws its
particle/spawn tree in runtime; host regression and PPSSPP audit cover it; no
physical-PSP claim is made.

**Stop condition:** Stop after one real spawn path is implemented and
verified, or record an evidence-backed blocker. Do not begin combat.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes and animations have software
  audits. Effects remain open.
- Effects: RE-172–188 cover manager descriptors, transforms, material/texture/
  colour animation, LBParticle decoding/packing, drawing, exhaustive audits,
  spawn-tree execution and `LBGenerator`.
- Remaining effects scope: real spawn-event wiring. See RE-188.
- Next R1 work: framebuffer paths, runtime `MObj` display state, unexplained
  rendering commands/assets/material failures, and remaining regression rows.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-188 — LBGenerator spawn subsystem**

- Cone/line generator math and frame/lifetime handling implemented.
- Vortex declines to existing `VortexUnsupported`; unknown kinds decline.
- Archive census: 65 targets; 60 visible spawns, 4 vortex declines, 0 unknown.
- Evidence: `docs/reverse-engineering.md` RE-188.
- Commit: `c801cde`.

## Verification

RE-188 passed `cargo test --workspace` (336 `ssb-rom` tests, with and without
`SSB64_ROM`), strict Clippy including `--no-default-features`, and
`cargo fmt --check`. `romtool particles` matches regression results. No PSP
file changed; no new PPSSPP run was required.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–188.
- Rendering methodology: `docs/visual-regression.md`.
- Permanent decisions: `DECISIONS.md`.
- Archived status journal: `docs/status-history.md`.

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
evidence links. Older session detail is preserved in `docs/status-history.md`.

## Continuation command

For `Continue with the plan`: read `AGENTS.md`, this file, relevant `PLAN.md`
section, relevant `docs/porting-status.md` row and `RE-*` evidence entry; then
inspect git state/recent commits, resume this task or select first eligible
TODO, implement, verify, document, update this snapshot and commit focused
work.
