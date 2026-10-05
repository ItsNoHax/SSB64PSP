# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** playable campaign Break the Targets (RE-452).
  Twelve courses run target hits/motion, GO, follow camera, end rules and
  campaign result handoffs with stocks preserved.
- **Branch reconciliation:** retain three local and five remote commits.
  Preserve campaign screens/Targets alongside remote demo scripts,
  APIs/tests, Kirby copy initialization and magnify-ignore bindings.
  The colliding remote RE-450 is preserved as RE-453.
- **Next gameplay batch:** Board the Platforms' playable scene controller;
  Race to the Finish follows. Bind objectives, camera and bonus results.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Break the Targets | Bonus world, target hits/motion, end rules and results | RE-452 |
| Campaign presentation | Intro cards/cameras, Continue figures/fades, scores and snapshot | RE-451 |
| PSP 1P session binding | Routing, entry/team bounds, collectors, replacements and HUD | RE-450, RE-453 |

## Verification baseline

- Reconciled tree: all 1,919 workspace/all-target tests pass, including
  1,104 game tests (absolute `SSB64_ROM`, Rust 1.98.0, one thread).
  Every test function from both branches is retained. Clippy with warnings
  denied, workspace rustfmt, diff checks and docs validation pass.
- Pack v97 rebuilt: 40,847,232 bytes, SHA-256
  `836e9a48d5434053e57abfde5e6841da19e33cb50091c72a4870cc1106b144de`.
  Adds fifteen demo-script rows and reachable model/texture parts to v96.
  Previous pack preserved locally; no ROM-derived assets committed.
- Both production PSP releases pass (nightly-2026-08-26): no game warnings,
  five existing viewer warnings. Viewer restored to production after capture.
- PPSSPP software: production game reaches the expected 20 s timeout,
  exit 0, without reported faults. Viewer `stage 17` renders the target
  course and exits after capture. Prior gameplay captures remain RE-452;
  full campaign and N64 equivalence remain unproven.
- Golden manifest remains 195; full matrix unchanged and not rerun.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Platforms/Race scenes; bonus pause map zoom, 12-tick fade and audio;
  Master Hand, special fighters, boss wallpaper/fade, ending/challenger/
  message scenes, saves/unlocks, select reconciliation, shade and 1P tags.
  Details live in `TODO.md`; later stages remain unvalidated.
- Audio/rumble, N64 evidence, select spotlight, fighter jostling and Yoshi's
  double-jump apex hang remain. Fidelity, performance and VRAM belong to P5.
