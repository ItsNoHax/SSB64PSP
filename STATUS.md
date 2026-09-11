# Current State

- Milestone: `R2 — Physical PSP Rendering Validation`
- Task: `R2.1/T3 — Original LookAt quantization` (next up; not started)
- Status: `TODO`
- Last complete: `RE-226` (2026-09-11), `R2.1/T2 -- raw signed-byte normal
  semantics`. Measured how `GU_NORMAL_8BIT` feeds
  `TextureProjectionMapMode::Normal` two ways: PPSSPP source
  (`GPU/Common/VertexReader.h`'s `ReadNrm` divides every `GE_PROJMAP_NORMAL`
  axis by 128.0 unconditionally, never normalizing) and a new headless
  measurement rig driving this project's own real `sceGu` calls
  (`psp/src/normal_diag.rs`, `texgen_normal_diagnostic_0`-`_6` in
  `psp/Cargo.toml`, all default-off): one full-viewport quad per build, a
  hand-picked normal, a "coordinate ramp" texture that turns the GE's
  computed texcoord into a directly-decodable screenshot pixel.
  **All seven cases matched `/128`-divisor predictions exactly**
  (`[127,0,0]`, `[64,0,0]` under both old `NormalizedNormal` and new raw
  `Normal`, `[-128,0,0]`, `[90,90,0]`, `[73,-41,99]`). The decisive pair: old
  `NormalizedNormal` mode collapsed `[64,0,0]` and `[127,0,0]` to identical
  output (magnitude discarded); raw `Normal` mode produced different,
  magnitude-proportional output for the same pair, matching the RSP's own
  never-normalizes behavior.
  **Fixed `meshdraw::apply_texture_mapping`**: `sceGuTexProjMapMode` now
  `Normal` instead of `NormalizedNormal`, dot-product term scaled by
  `NORMAL_SCALE_COMPENSATION = 128.0/127.0` to correct the GE's measured
  `/128` against the original hardware's `/127`, reproducing `(normal .
  LookAt) / 127` exactly. This revises D-038 (which named `NormalizedNormal`
  outright), not D-042 (T1's cross-node finding, still open, unaffected).
  Rebuilt and updated all three texgen goldens (`tests/golden/
  r2-metal-texgen{,-rotated,-linear}.png`) via `regression_capture_scene11/
  12/13` -- 45,484 / 29,874 / 27,570 differing pixels against the pre-fix
  goldens, reconfirmed deterministic (0 differing pixels between two fresh
  captures of the same build).
  Four incidental bugs surfaced and fixed while building the measurement
  rig itself (documented in `normal_diag.rs`'s module doc): stack-local
  vertex data not surviving to the caller's `sceGuSync`; `alloc::Vec<u8>`
  lacking the GE's required 16-byte DMA alignment; the GE's guard-band clip
  silently discarding an oversized primitive instead of clipping it; and the
  texture-matrix generator's `(s, t)` output being `[0, 1]`-normalized on
  the `TRANSFORM_3D` path (not a raw texel address, unlike the unrelated
  `TRANSFORM_2D` through-mode sprite path).
- Next: `R2.1`/T3 -- original LookAt quantization. Implement
  host-testable `FTOFRAC8`-equivalent helpers with positive saturation at
  127, negative cast/range behavior, and values at/around zero and ±1/128
  and ±1. Quantize before model transformation, reconstruct the basis,
  preserve source-derived rounding rules. Read `PLAN.md`'s full `R2.1`
  section (T1-T10) before starting; T4-T10 remain queued behind it in order.
- Blockers: none for starting T3. `R2.1`/T1's own finding (164 cross-node
  differing-transform vertex reuses) is an open, tracked, *known* gap --
  not a blocker for T3/T4, which are prerequisites for fixing it, not
  blocked by it. `R2.2`/C1-C7 renderer corrective gate remains behind all of
  `R2.1`. Combat remains gated behind `R2.2`.
  Separately (not blocking): a real bug was found and flagged (not fixed)
  in the `debug_overlay` PSP viewer -- object-view HUD text renders
  corrupted/double-exposed in every capture. See the spawned follow-up task
  (`task_1bf9bc35`). Also open, non-blocking: a before/after PPSSPP TEXVIEW
  screenshot confirming RE-224's CI4 palette-bank fix (global texture
  indices 187/194/195, object indices 60-102 in file 86) was never obtained
  -- manual follow-up for whoever next has hands on the interactive build.
- Hardware note: run `pspsh -e reset` after every killed PSPLink module.
- Evidence: `docs/reverse-engineering.md` -- RE-217 through RE-226.
- Plan: `PLAN.md` -- `R2.0` (`COMPLETE`); `R2.1` (`IN PROGRESS`, T1 measured,
  T2 complete, T3 next).
- Decisions: `DECISIONS.md` D-038 revised by RE-226's measurement (raw,
  un-normalized `Normal` projection mode, `128/127` compensation).
- Subsystem: `docs/porting-status.md` -- PSP mesh drawing; `docs/rendering.md`
  -- texgen rows now reflect the raw-normal generator (renderer behavior
  changed: `regression_capture_scene11/12/13` goldens updated).
- Verification: seven-case headless measurement via
  `tools/run-ppsspp-headless.sh --feature texgen_normal_diagnostic_N` (`N` in
  `0..=6`), each cross-checked against its predicted texel by direct pixel
  decode; `cargo psp --release` (default features) builds clean; `cargo fmt
  --check` clean in `psp/` on touched files; full `cargo test --workspace
  --all-targets` (pinned 1.98.0 toolchain, `SSB64_ROM` set) -- 560 passing, 0
  failed, unaffected since `psp/` is a separate workspace with no
  host-testable logic touched; `psp/`'s own `clippy` is not part of this
  project's gate (native clippy cannot cross-compile to `mipsel-sony-psp`).
- Documentation: RE-226, `PLAN.md` `R2.1`/T2, `DECISIONS.md` D-038, this
  snapshot.
- Commit: `b6ece51`.

## Continuation

Read `AGENTS.md`, this file, current `PLAN.md` section, referenced subsystem
row/evidence, then inspect Git state. Resume current task; otherwise select
first eligible TODO. Implement, verify, document, update this snapshot, and
commit focused work when appropriate.
