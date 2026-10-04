# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** portable 1P frontend controllers: single-puck
  select/settings, intro announcements/entrances, Continue and Game Over,
  timed stage-clear ledger and bonus pages; `spgame::Frontend` connects
  these controllers to the campaign session (RE-448). `mnPlayers1PGame`'s
  select (`ssb_game::players_1p`) is drawn in `psp-game` from the menu's
  third entry; pack v93 adds its sprites. START on that select saves its
  data and returns to the menu until the session is wired.
- **Next gameplay batch:** PSP 1P frontend/session binding: authored
  intro/continue/stage-clear draws, session wiring from the select's
  START, live battle and attack/damage/item-stat hooks, percent sync after
  healing/rebirth. Use `spgame::Frontend` and the real 1P setup. Bonus/Boss
  requests still require their own controllers; never substitute VS
  battles or skip them.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Portable 1P frontend controllers | Selection and settings, source process clocks, intro/continue/stage-clear transitions; 24 new regressions | RE-448 |
| 1P select | Logic and presentation ported; sprite offsets ROM-checked on the user's machine | — |
| Portable 1P campaign core | Manager, stage/CPU setup, replacements, bonus/record accounting; handicap ROM checks and Race bindings | RE-447 |

## Verification baseline

- All 1,851 workspace tests pass at RE-448 (absolute SSB64_ROM, pinned
  1.98.0, one thread). All 53 campaign tests also pass on the final
  source. Clippy with warnings denied and workspace rustfmt pass.
- Pack v93 is not rebuilt yet. Pack v92: 35,821,024 bytes, SHA-256
  `4b84c7d713dc732d5dac53a584f2605d5256976ec0560fd5478e43e84d245e35`.
  No ROM-derived assets committed.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- Production PPSSPPHeadless 20 s software startup smoke exits 0 at its
  expected timeout without reported faults. It exercises startup, not
  the portable 1P frontend; no physical-PSP or N64-equivalence proof.
- No goldens re-run. Last targeted game goldens: RE-444 (33 scenes);
  larger game baseline RE-443; full viewer matrix RE-438. Manifest: 195.
- Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- 1P's select runs, but the campaign is not playable on PSP yet. PSP
  frontend drawing/session wiring, bonus-stage controllers, Master Hand,
  boss wallpaper/fade, ending and challenger/message presentation, saves
  and unlock writes remain.
- Audio and rumble remain; Training/magnifier sounds await the backend.
- Event-aligned N64 evidence remains for CPU item behavior, VS entry
  focus, item damage/trajectory, thrown-body hits, pipes and stage effects.
- Lit primitives without authored light colours retain baked fallback
  shade. Bumper palette, Star flicker and bomb flashes lack N64 traces.
- Fighter jostling and selects' spotlight remain. Yoshi's pre-existing
  double-jump apex hang persists. Performance/VRAM residency belong to P5.
