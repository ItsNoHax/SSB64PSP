# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** PSP 1P campaign session binding (RE-450). START on
  the existing 1P select runs the portable frontend, real campaign setup,
  collectors, entry schedule, team replacement and results boundary.
  Link, Yoshi Team and Fox are supported; Break the Targets then blocks.
  Unsupported scenes and missing special-fighter assets block explicitly.
  Intro/continue/stage-clear drawing is interim; authored drawing remains.
- **Next gameplay batch:** authored 1P intro, continue and stage-clear
  presentation over the portable controllers. Pack their authored assets
  and bind scene models/cameras without changing process timing.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| PSP 1P session binding | Campaign routing, setup, entry/team bounds, collectors, replacements and HUD positions | RE-450 |
| Live 1P battle statistics | Callback identity, ownership, damage/item/KO records and percent sync | RE-449 |
| Portable 1P frontend controllers | Selection/settings, clocks and presentation transitions | RE-448 |

## Verification baseline

- All 1,898 workspace tests pass (absolute `SSB64_ROM`, Rust 1.98.0,
  one thread), including 1,094 game tests. Clippy with warnings denied,
  workspace rustfmt and documentation validation pass.
- Pack v94 rebuilt: 37,146,848 bytes, SHA-256
  `465538fdf77087c747eaae06697427b337967bd2327515598fbe376266ea4e94`.
  No ROM-derived assets committed.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- PPSSPPHeadless software `onepgame` reaches Link's battle; two tick-600
  captures are pixel-identical. Select outline, countdown and later live
  timer inspected. Production 20 s startup smoke exits 0 at its expected
  timeout without reported faults. No physical-PSP or N64-equivalence proof.
- Golden manifest remains 195; no full matrix or existing goldens re-run.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Authored campaign presentation, bonus-stage controllers, Master Hand,
  special fighters, boss wallpaper/fade, ending/challenger/message scenes,
  saves/unlocks, select reconciliation, shade/magnify-ignore and 1P tags.
  Details live in `TODO.md`; later-stage PSP behavior remains unvalidated.
- Audio/rumble and event-aligned N64 evidence remain. Selects' spotlight,
  fighter jostling and Yoshi's double-jump apex hang remain. Rendering
  fidelity, performance and VRAM residency belong to P5.
