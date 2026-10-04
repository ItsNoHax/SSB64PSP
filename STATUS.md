# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** authored 1P screens (RE-451).
  Packed sprites, one-shot cameras and demo clips replace intro, Continue
  and stage-clear placeholders. Fighter/ally cards, team reveals, dropped
  and stand-up figures, Game Over and score/bonus tables draw over the
  existing clocks. Stage-clear dims the last completed active picture.
  Unsupported gameplay/scenes still block explicitly.
- **Next gameplay batch:** playable 1P bonus-stage scene controllers,
  starting with Break the Targets; bind objectives, completion/failure and
  the existing results boundary. Board the Platforms and Race follow.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Authored campaign presentation | Intro cards/cameras, Continue figures/fades, score tables and snapshot | RE-451 |
| PSP 1P session binding | Campaign routing, setup, entry/team bounds, collectors, replacements and HUD | RE-450 |
| Live 1P battle statistics | Callback identity, damage/item/KO records and percent sync | RE-449 |

## Verification baseline

- All 1,903 workspace tests pass (absolute `SSB64_ROM`, Rust 1.98.0,
  one thread), including 1,094 game tests and five new ROM-backed campaign
  tests. Clippy with warnings denied, workspace rustfmt and docs validation
  pass; generated animation table reproduces exactly.
- Pack v95 rebuilt: 40,788,688 bytes, SHA-256
  `e0ba40a14102d359340548db6b192702b71c67d411a73246d0dc682350969073`.
  No ROM-derived assets committed.
- Both production PSP releases pass (nightly-2026-08-26): no game warnings,
  five existing viewer warnings. Production game rebuilt after capture use.
- PPSSPP software: Link intro/battle; seeded Yoshi/bonus intros, Continue,
  retry/stand-up, Game Over and stage-clear pages inspected. Repeated intro,
  Continue and stage-clear frames are pixel-identical. Production 20 s
  startup exits 0 at expected TIMEOUT without reported faults. Seeded
  fixtures do not prove real win/loss handoffs or N64 equivalence.
- Golden manifest remains 195; existing goldens/full matrix unchanged.
  Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Bonus-stage controllers, Master Hand, special fighters, boss wallpaper/
  fade, ending/challenger/message scenes, saves/unlocks, select reconciliation,
  shade/magnify-ignore, 1P tags and scene audio. Details live in `TODO.md`;
  later stages remain unvalidated.
- Audio/rumble and event-aligned N64 evidence remain. Selects' spotlight,
  fighter jostling and Yoshi's double-jump apex hang remain. Rendering
  fidelity, performance and VRAM residency belong to P5.
