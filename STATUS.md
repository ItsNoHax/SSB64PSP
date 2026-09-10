# Project Status

**Last updated:** 2026-09-10

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** Runtime `MObj` display-state parity — `PLAN.md` R1's next
unchecked acceptance item. RE-193 (this session) closed the previous
bullet ("all required framebuffer paths render"), so this is now the first
open item. Not started yet this session.

**Status:** `TODO` (not started)

**Dependencies:** RE-172–193 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1's `MObj` display-state bullet;
`refs/ssb-decomp-re/src/sys/objdisplay.c` (`gcDrawMObjForDObj`);
`crates/ssb-rom/src/mobj.rs`; `psp/src/meshdraw.rs` (`apply_material` and
related); `TODO.md` (fractional/UV-scroll symptoms this item owns).

**First checks:** `git status --short`; `git log -5 --oneline`; read
`gcDrawMObjForDObj` in full before touching code — this item explicitly
owns the end-to-end state model (`MOBJ_FLAG_NONE` defaults, texture
enable/disable, `scau`/`scav`, `trau`/`trav`, `scrollu`/`scrollv`,
`MOBJ_FLAG_FRAC`), not just the isolated symptoms already listed in
`TODO.md`.

**Acceptance:** `PLAN.md` R1's "runtime `MObj` display-state parity" bullet
— reproduce `gcDrawMObjForDObj`'s emission path completely enough that no
symptom in this state model is treated as fixed in isolation.

**Stop condition:** None yet — task not started.

## Current state

- R0.5: `VERIFYING`; physical PSP validation unavailable/deferred.
- R1: `IN_PROGRESS`; stages, fighters, costumes, animations, effects and
  framebuffer paths now have software audits.
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
- Next R1 work: runtime `MObj` display-state parity (this session's
  selected next task), unexplained rendering commands/assets/material
  failures, and remaining regression rows.
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-193 — minimal `SObj` 2D-sprite port renders the wallpaper capture through a real GE draw**

- Found and corrected a wrong assumption in the prior session's own
  scoping: the wallpaper is drawn in the decompilation as an `SObj` 2D
  screen-space sprite (RDP `gSPTextureRectangle`, no 3D transform, no
  display list), not a `DObj`/`MObj` textured quad like the LB-transition
  precedent. This project had never ported any `SObj` support. Corrected
  `PLAN.md`'s bullet text accordingly (`AGENTS.md` §2: investigate and fix
  disagreeing documentation rather than silently working around it).
- Per explicit user direction, built the real minimal `SObj` slice this one
  caller needs rather than a synthetic 3D-quad shortcut: `SpriteVertex`
  (`GU_TRANSFORM_2D`) and `Gpu::draw_wallpaper_sprite` in `psp/src/gu.rs`,
  reproducing the one real draw call's own fixed parameters (single bitmap,
  `pos = (10,10)`, scale 1.0, opaque, `G_CC_MODULATEI_PRIM`'s 50%-grey prim
  dim) — not a general `Sprite`/`Bitmap` decoder, which RE-190/191 already
  ruled out as unnecessary here.
- `WALLPAPER_PHOTO`'s backing array grew from 220 to a padded 256 rows: the
  GE's texture-height register is a power-of-two exponent field, a
  constraint the buffer never had to satisfy before this session's GE bind
  (the CPU-only capture/blit paths never cared).
- Found and fixed a real on-device bug: `TEXTURE_32BITF` UV with identity
  `sceGuTexScale`/`Offset` is a raw texel address, not a normalized 0..1
  fraction. The first attempt's fractional UVs sampled almost only the
  single top-left (background) texel, smearing it across the whole
  300×220 quad and blacking out the stage every frame past the capture
  trigger — screenshotted before the fix (tick 276) as part of the record.
  Fixed with texel-unit UV corners.
- Added `wallpaper_sprite_audit_capture` (`psp/Cargo.toml`/`main.rs`):
  same tick-240 capture trigger as `wallpaper_audit_capture`, displayed
  through the new real GE draw instead of the raw CPU blit — independent
  evidence of a different mechanism, kept alongside (not replacing) the
  existing feature.
- Verified on device: fixed build measures average luminance 43.3 inside
  the exact 300×220 draw rectangle vs. 72.6 for the same rectangle in a
  same-duration default-build screenshot (consistent with the 50% grey
  modulate), and 23.24 vs. 23.26 strictly outside it (unchanged, proving
  the write is exactly bounded). Re-verified `wallpaper_audit_capture`'s
  original CPU-blit ghosting evidence still reproduces after the buffer
  resize (no regression).
- All three PSP configs (default, `wallpaper_audit_capture`,
  `wallpaper_sprite_audit_capture`) build clean; `cargo fmt --check` in
  `psp/` clean; `cargo clippy --workspace --all-targets` and `cargo test
  --workspace` (337, unaffected) both clean.
- Checked off `PLAN.md` R1's "all required framebuffer paths render"
  acceptance item, on the same "renderer owns mechanism, G2 owns trigger"
  precedent RE-149 used for R0.13 (whose own LB-transition draw path is
  likewise only ever called from its own audit feature today).
- Evidence: `docs/reverse-engineering.md` RE-193.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `psp/src/gu.rs`, `psp/src/main.rs`,
`psp/Cargo.toml`, `PLAN.md`, `STATUS.md`, and `docs/reverse-engineering.md`.
All three PSP configs build clean; `cargo fmt --check` in `psp/` clean;
`cargo clippy --workspace --all-targets` and `cargo test --workspace` (337,
psp/ is outside the workspace, unaffected) both clean. On-device:
`tools/run-ppsspp.sh --no-build --seconds 5` against the broken and fixed
`wallpaper_sprite_audit_capture` builds and against `wallpaper_audit_capture`
post-resize, screenshots measured/compared — see RE-193. Prior sessions'
verification (RE-189, RE-192) is unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–193.
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
