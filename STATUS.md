# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** playable campaign Board the Platforms (RE-454).
  Twelve courses bind DETECT floors, independent platform child clocks,
  moving parents, course Bumpers, camera, end rules and bonus results.
- **Next gameplay batch:** Race to the Finish's playable scene controller.
  Bind the finish gate, animated Bumpers, barrel draw, camera and results.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Board the Platforms | Landing credits, child trees/materials, Bumpers and results | RE-454 |
| Break the Targets | Bonus world, target hits/motion, end rules and results | RE-452 |
| Branch reconciliation | Retain campaign/Targets and remote scripts/APIs/tests | RE-453 |

## Verification baseline

- All 1,925 workspace/all-target tests pass, including 1,107 game tests
  (absolute `SSB64_ROM`, Rust 1.98.0, one thread). ROM checks cover all
  120 floors and 600 ticks of child/material/Bumper scripts. Clippy with
  warnings denied, workspace rustfmt, diff and docs checks pass.
- Pack v98 rebuilt: 40,912,768 bytes, SHA-256
  `59055e284dce64eeb6a3f76702132fc2961136bea8dfd8f0dfa489feccf4d2ee`.
  Adds platform child/material and course Bumper bindings.
  No ROM-derived assets committed.
- Both production PSP releases pass (nightly-2026-08-26): no game warnings,
  five existing viewer warnings. Game restored to production after captures.
- PPSSPP software: live Platforms repeats are pixel-identical; real landing
  credits reach COMPLETE, ten-task RESULT and Perfect; fall and the
  unmodified two-minute timer reach FAILURE. Link/Targets still start.
  Production game reaches the expected 20 s timeout, exit 0, without
  reported faults. The capture wrapper reports a missing screenshot because
  production emits none. These seeded diagnostics do not prove a full campaign,
  unmodified course traversal, original-N64 equivalence or physical PSP.
- Golden manifest remains 195; full matrix unchanged and not rerun.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Race scene; bonus pause map zoom, 12-tick fade and audio; Master Hand,
  special fighters, boss wallpaper/fade, ending/challenger/message scenes,
  saves/unlocks, select reconciliation, shade and 1P tags.
  Details live in `TODO.md`; later stages remain unvalidated.
- Audio/rumble, N64 evidence, select spotlight, fighter jostling and Yoshi's
  double-jump apex hang remain. Fidelity, performance and VRAM belong to P5.
