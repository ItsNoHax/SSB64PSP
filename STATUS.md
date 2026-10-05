# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** playable campaign Break the Targets (RE-452).
  All twelve courses load ten shared-model targets with independent root
  scripts. Real item hits update objectives; GO, follow camera, completion,
  fall/timeout failure and the campaign result handoff run. Bonus records
  preserve campaign stocks and falls.
- **Next gameplay batch:** Board the Platforms' playable scene controller;
  Race to the Finish follows. Bind objectives, course-specific camera and
  the existing bonus result boundary.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Break the Targets | Separate bonus world, target hits/motion, end rules and result handoffs | RE-452 |
| Authored campaign presentation | Intro cards/cameras, Continue figures/fades, score tables and snapshot | RE-451 |
| PSP 1P session binding | Campaign routing, setup, entry/team bounds, collectors, replacements and HUD | RE-450 |

## Verification baseline

- All 1,911 workspace tests pass (absolute `SSB64_ROM`, Rust 1.98.0,
  one thread), including 1,100 game tests. New ROM checks cover all 120
  placements, attributes and 600 ticks of each moving target script.
  Clippy with warnings denied, workspace rustfmt and docs validation pass.
- Pack v96 rebuilt: 40,789,280 bytes, SHA-256
  `39a52dc24777fcdbab9b0668e8e0cda4b7596e110207f789f1f1a1c458de4593`.
  No ROM-derived assets committed.
- Both production PSP releases pass (nightly-2026-08-26): no game warnings,
  five existing viewer warnings. Production game rebuilt after capture use.
- PPSSPP software: live Kirby course, Mario jab completion, fall and real
  timeout; all three reach RESULT with stocks preserved. Repeated live
  frames are pixel-identical; ordinary Link battle still starts. Production
  20 s startup exits 0 at expected TIMEOUT without reported faults.
  Seeded bonus intros bypass earlier wins; unmodified traversal, full
  campaign and N64 equivalence remain unproven.
- Golden manifest remains 195; existing goldens/full matrix unchanged.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Platforms/Race scenes; bonus pause map zoom, 12-tick fade and audio;
  Master Hand, special fighters, boss wallpaper/fade, ending/challenger/
  message scenes, saves/unlocks, select reconciliation, shade/magnify-ignore
  and 1P tags. Details live in `TODO.md`; later stages remain unvalidated.
- Audio/rumble and event-aligned N64 evidence remain. Selects' spotlight,
  fighter jostling and Yoshi's double-jump apex hang remain. Rendering
  fidelity, performance and VRAM residency belong to P5.
