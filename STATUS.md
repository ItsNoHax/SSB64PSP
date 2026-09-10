# Project Status

**Last updated:** 2026-09-10 (RE-204 physical-PSP session)

## Continuation packet

**Milestone:** `R2 — Physical PSP Rendering Validation`

**Current task:** `R2 — Physical PSP Rendering Validation`

**Status:** `IN_PROGRESS`

**Last completed:** RE-204. Hardware-tested RE-193's real `SObj`
framebuffer-effect sprite path (`regression_capture` +
`wallpaper_sprite_audit_capture`) on the same physical PSP: zero exceptions,
measured luminance ratio (0.59) matches RE-193's PPSSPP evidence (0.60)
within noise, deterministic once settled (a several-second double-buffer
settling transient right after the tick-240 freeze was observed and
excluded). Verified VRAM usage by grep-enumerating every `get_vram_allocator`
call site (exactly three, all in `Gpu::init`): 1,360 KiB of the runtime-
reported 2 MiB EDRAM, ~688 KiB headroom, matching an existing independent
code comment; every zero-exception hardware boot to date is a live runtime
check this budget holds (the allocator panics on overflow). Checked off the
two remaining software-testable `PLAN.md` R2 acceptance items (10 of 12 now
checked). Redeployed the plain interactive build afterward, verified stable.

**Dependencies:** R0.5 and R1 complete. R2 hardware checklist remains (stage
animation coverage, exhaustive no-failures-remain coverage, live
analog-stick input).

**Relevant files:** `PLAN.md` R2; `docs/reverse-engineering.md` RE-201–204;
`docs/psplink.md`; `tests/golden/*.png`; `tools/compare-screenshot.sh`.

**First checks:** physical PSP hardware is present and PSPLink-reachable
this session (`lsusb` shows `054c:01c9`, `pspsh -e ver` → `PSPLink v3.2.1`
once `usbhostfs_pc -v "$PWD"` is running). Re-check this at the start of the
next session; if hardware is no longer attached, R2 has no further eligible
software-only task.

**Acceptance:** `PLAN.md` R2.

**Next:** broaden hardware coverage beyond the four golden scenes toward "no
hardware-only rendering failures remain," and find or build a scene that
isolates visibly animated stage geometry for the "stage animation works" row
(no current golden scene's camera view shows moving stage geometry within
its frozen window). Separately, still open: whether the analog nub correctly
drives the fighter now that RE-202's HUD-crash fix is live — this requires a
human physically operating the device with PSPLink attached; `pspsh` has no
controller-injection command, so an agent session cannot resolve it alone.

## Current state

- R0.5: `COMPLETE`; RE-201's PSPLink capture resolves its physical Dream Land
  canopy comparison.
- R1: `COMPLETE`; every acceptance bullet is checked through RE-200 and its
  R0.5 prerequisite is now satisfied.
- R2: `IN_PROGRESS`; all four golden regression scenes (Dream Land/Mario,
  `MVOpeningRoom`, `StageSectorFile2`, `CatchSwirl`) boot, pack loads,
  stage/fighter/material/texture content matches PPSSPP goldens, and no
  hardware exception remains across any of them (RE-203). The real
  framebuffer-effect `SObj` sprite path and VRAM usage are now hardware-
  verified too (RE-204). Stage animation coverage, exhaustive no-failures-
  remain coverage, and live analog-stick input remain untested on hardware.
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

**RE-204 — Framebuffer-effect sprite path and VRAM budget verified on physical PSP hardware**

- Built and `ldstart`ed `regression_capture,wallpaper_sprite_audit_capture`
  (RE-193's real `SObj` wallpaper-sprite draw) on the same PSPLink session as
  RE-201–203. Zero exceptions; `main_thread` alive throughout.
- Found and characterised a several-second post-freeze settling transient
  (double-buffer catch-up), then confirmed steady-state captures 2 s apart
  are byte-identical except the same known PSPLink corner overlay RE-203
  already excludes.
- Measured luminance ratio 0.59 (sprite rectangle vs. just outside it),
  matching RE-193's own PPSSPP-measured 0.60 ratio for the same 50%-grey
  modulate, within noise.
- Verified VRAM usage by grep-enumerating every `get_vram_allocator` call
  site (exactly three, all framebuffers/depth in `Gpu::init`): 1,360 KiB of
  the runtime-reported 2 MiB EDRAM, ~688 KiB headroom, matching an existing
  independent code comment. Every zero-exception hardware boot to date is a
  live runtime confirmation this budget holds (the allocator panics on
  overflow).
- Checked off the two remaining software-testable `PLAN.md` R2 acceptance
  items: framebuffer effects work, VRAM usage verified (10 of 12 now
  checked).
- Rebuilt and redeployed the plain (no-feature) interactive build afterward
  per `docs/psplink.md`, verified stable (no exception, `main_thread` alive).
- Evidence: `docs/reverse-engineering.md` RE-204.

## Verification

The `regression_capture,wallpaper_sprite_audit_capture` build ran on PSP
Slim (firmware 6.61, ARK/Infinity, PSPLink v3.2.1) with `exlist` empty and
`main_thread` alive throughout. Native `scrshot` captures measured for
in-rectangle vs. outside-rectangle luminance and cross-diffed against each
other for determinism — see RE-204 for the exact settling-transient finding
and measurement methodology. Native captures taken, inspected, and discarded
per `docs/psplink.md` (never committed); SHA-256 hashes recorded in RE-204.
Pack unchanged (hash `7647db75...650b2f0`); no Rust source changed this
session, so `cargo test --workspace` was not required.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–204.
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
