# Project Status

**Last updated:** 2026-09-10 (RE-194 session)

## Continuation packet

**Milestone:** `R1 — Rendering Completeness`

**Current task:** "No unexplained rendering commands remain" — `PLAN.md` R1's
next unchecked acceptance item. RE-194 (this session) closed the previous
bullet ("runtime `MObj` display-state parity"), so this is now the first
open item. Not started yet this session.

**Status:** `TODO` (not started)

**Dependencies:** RE-172–194 complete. R0.5 physical PSP comparison remains
`VERIFYING` and is temporarily deferred by explicit user direction.

**Relevant files:** `PLAN.md` R1's remaining bullets ("no unexplained
rendering commands/missing assets/material failures remain", "rendering
regression suite passes", "golden/reference renders are established");
`docs/rendering.md` "Measured usage"/"Not yet handled" tables;
`docs/visual-regression.md` (existing test-matrix rows).

**First checks:** `git status --short`; `git log -5 --oneline`; re-run
`romtool scan` to get a current opcode census and diff it against
`docs/rendering.md`'s existing table before assuming anything is still
accurate — several of these bullets may already be substantially covered by
work recorded elsewhere in `PLAN.md`/`docs/reverse-engineering.md` and just
need the acceptance text reconciled, per `AGENTS.md` §2.

**Acceptance:** `PLAN.md` R1's four remaining unchecked bullets (§7).

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
- Next R1 work: no unexplained rendering commands/assets/material failures
  remain, rendering regression suite passes, remaining golden-render matrix
  rows. `MObj` display-state parity is now closed (RE-194).
- R2/R3/combat: blocked behind R1 and the physical rendering gate.

## Last completed task

**RE-194 — `gcDrawMObjForDObj`'s runtime tile/texture-scale state, measured and reproduced; `MOBJ_FLAG_FRAC` confirmed dead**

- Corrected a wrong assumption standing in `mesh.rs`'s own doc comment
  (`current_texture_shape`), which called `MOBJ_FLAG_TEXTURE` and friends
  "runtime-only, cannot read statically": every input these branches use
  (`flags`, `scau`/`scav`, `trau`/`trav`, `scrollu`/`scrollv`, and the
  tile/scale-formula fields) is an ordinary static `MObjSub` field, never
  mutated anywhere outside `objdisplay.c` in the whole decompilation
  (confirmed by grep). Only `texture_id_curr`/`texture_id_next`/
  `palette_id`/`lfrac` are genuinely runtime state.
- Measured every flag archive-wide (665 real `MObjMaterial`s, via a
  temporary instrumented `romtool mobj` scan, reverted before committing)
  before implementing anything: `flags == NONE` 3, `TEXTURE` 10, tile-0
  `0x20` 35, tile-1/scroll `0x40` 12, `FRAC` **0**.
- Implemented and unit-tested against real ROM values: the `MOBJ_FLAG_NONE`
  default substitution, `MOBJ_FLAG_TEXTURE`'s `gSPTexture` scale
  (`MObjMaterial::tex_scale`), and the tile-0 `gDPSetTileSize` window
  (`MObjMaterial::tile0_uv`), both in `crates/ssb-rom/src/mobj.rs`, wired
  into `crates/ssb-rom/src/mesh.rs`'s `apply_mobj` the same way the
  equivalent real `Cmd::SetTileSize`/`Cmd::Texture` handlers already work.
- `MOBJ_FLAG_FRAC` confirmed dead code for this game's content (0/665 real
  occurrences; never OR'd in at runtime either — the only runtime
  `sub.flags |=` anywhere sets `MOBJ_FLAG_ENVCOLOR`, for shields) — the same
  "measured, not guessed" treatment RE-127 gave RDP LOD blending. Not
  implemented.
- Tile-1/scroll confirmed real but inert: every occurrence's inputs are
  identical to its own tile-0 window, and no packed combiner shape reads
  `TEXEL1` (RE-130) — documented (`mobj.rs`'s `MOBJ_FLAG_TILE1`), not given
  a pack field nothing would read.
- 5 new unit tests in `mobj.rs` (2 lock the tile-0 math against real Dream
  Land/`StageMetalFile2` `MObjSub` values, 1 locks the texture-scale math,
  1 covers the `MOBJ_FLAG_NONE` substitution, 1 is `SSB64_ROM`-gated and
  re-reads all three real `MObjSub`s straight through `read_material`).
  `cargo test --workspace`: 342 passing (was 337). `cargo fmt --check` and
  `cargo clippy --workspace --all-targets` both clean.
- Rebuilt the pack: textures bound `1330 → 1345` (+15, the newly-resolved
  sprites). `cargo psp --release` + `tools/run-ppsspp.sh --seconds 8`: clean
  boot, 60 FPS, no log errors. Pixel-diffed the resulting screenshot against
  an equivalent pre-fix build: of 960×544 pixels, exactly 258 differ, all of
  them inside the on-screen texture-count HUD digits (`1330`→`1345`) — zero
  pixels differ in the rendered 3D geometry, including Dream Land's own
  affected `MObjSub`s. Same "not visible at this camera distance" outcome
  RE-075/RE-081 already recorded for this exact scene.
- Checked off `PLAN.md` R1's "runtime `MObj` display-state parity"
  acceptance item; updated `TODO.md`'s "UV Scroll"/"Implement
  `MOBJ_FLAG_FRAC`" items to reflect the same measured conclusions.
- Evidence: `docs/reverse-engineering.md` RE-194.
- Commit: pending (this session).

## Verification

`git diff --stat` for this session covers `crates/ssb-rom/src/mobj.rs`,
`crates/ssb-rom/src/mesh.rs`, `PLAN.md`, `TODO.md`, `STATUS.md`, and
`docs/reverse-engineering.md`. `cargo fmt --check`, `cargo clippy
--workspace --all-targets`, and `cargo test --workspace` (342, `SSB64_ROM`
set) all clean. Pack rebuilt and reloads cleanly (+15 textures bound, see
above). On-device: `tools/run-ppsspp.sh --seconds 8`, clean log, 60 FPS,
pixel-diffed against an equivalent pre-fix build — see RE-194. Prior
sessions' verification (RE-190–193) is unaffected and remains valid.

## Documentation and evidence map

- Roadmap and acceptance: `PLAN.md`.
- Subsystem status: `docs/porting-status.md`.
- Detailed investigations: `docs/reverse-engineering.md` RE-172–194.
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
