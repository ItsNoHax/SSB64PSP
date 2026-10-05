# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All twelve fighters, Master Hand, Metal Mario, Giant Donkey Kong and the
  Polygons have host movesets. Shared machinery and PSP game integration
  remain before `P2` is complete.
- **Completed batch:** 1P special fighters: the variants' motion tables,
  scripts, hurtboxes, parts and sprites; their CPU branches, entries and
  knockback resistance; TopN's `attr->size` scale for every fighter; the
  team stock display and banner-clipped intro cards; stages 6, 10 and 12
  run through STAGE CLEAR (RE-458).
- **Next gameplay batch:** 1P ending, challenger and unlock-message scenes,
  so a campaign runs from the select through the credits without blocking.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Special fighters | Metal Mario, Giant DK, Polygon Team stages; TopN scale | RE-458 |
| Master Hand | Boss fighter, bullets, wallpaper, cameras, defeat, GAME CLEAR | RE-457 |
| Bonus pause and entry fade | Source map zoom, L: Retry and black 12-tick fade | RE-456 |
| Race to the Finish | Gate, hazards, campaign stocks, camera and results | RE-455 |

## Verification baseline

- 1,958 workspace tests pass on one thread with `SSB64_ROM` (Rust
  1.98.0); clippy with warnings denied and rustfmt pass.
- Pack v102: 41,738,400 bytes, SHA-256
  `f67f08ae47c9d5b49348c28d2894f656d0dcd6dac18ec159258bf31e815a30ac`.
  No ROM assets are committed.
- Both production PSP applications build on nightly-2026-08-26: no game
  warnings, five existing viewer warnings. The production game runs under
  PPSSPP software for 20 seconds without faults.
- PPSSPP `onepmetal`/`onepgiant`/`onepzako` captures cover the intros,
  entries, team drops, replacements and STAGE CLEAR; N64 references of the
  three stages.
- Golden manifest remains 195; full matrix not rerun, and its fighter
  goldens predate TopN's scale. Stack last measured RE-440; production ELF
  last measured RE-443. Physical PSP last checked RE-361 (PSP-2000, 6.61
  ARK, pack v43).

## Blockers and remaining scope

- The ending, challenger and message scenes block. Saves/unlocks, select
  reconciliation, shade, 1P tags, audio and the boss's and special
  fighters' fidelity items are in `TODO.md`.
- Rumble, N64 evidence, select spotlight, fighter jostling and Yoshi's
  double-jump apex hang remain. Fidelity, performance and VRAM belong to P5.
