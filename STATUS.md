# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Current batch:** 1P campaign frontend. Done: `mnPlayers1PGame`'s
  select (`ssb_game::players_1p`), drawn in `psp-game` from the menu's
  third entry; pack v93 adds its sprites. Remaining: intro/continue/
  stage-clear scene flow and live session/stat hooks. START on the select
  saves its data and returns to the menu until then. Follow the scene
  requests in `spgame::Session`; do not substitute VS battles for missing
  bonus/Boss behavior.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| 1P select | Logic and presentation ported; sprite offsets ROM-checked on the user's machine | — |
| Portable 1P campaign core | Stage progression, continues, challengers, bonuses, score ledger, enemy replacements; all 40 handicap rows ROM-checked; Race Bumper clips | RE-447 |

## Verification baseline

- Without the ROM, every workspace test passes (pinned 1.98.0, one
  thread); clippy with warnings denied and workspace rustfmt pass. The
  ROM-gated tests, including the new sprite decodes, were last run at
  RE-447.
- Pack v93 is not rebuilt yet. Pack v92: 35,821,024 bytes, SHA-256
  `4b84c7d713dc732d5dac53a584f2605d5256976ec0560fd5478e43e84d245e35`.
  No ROM-derived assets committed.
- The `psp-game` release build passes without warnings
  (nightly-2026-08-26); the viewer was last built at RE-447.
- Production PPSSPPHeadless 20 s software startup smoke exits 0 at its
  expected timeout without reported faults. It does not enter 1P, which
  has no frontend yet; no physical-PSP or N64-equivalence proof.
- No goldens re-run. Last targeted game goldens: RE-444 (33 scenes);
  larger game baseline RE-443; full viewer matrix RE-438. Manifest: 195.
- Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- 1P's select runs, but the campaign is not playable yet. Bonus-stage controllers, Master
  Hand, boss wallpaper/fade, remaining campaign presentation, persistent
  saves and unlock writes remain in TODO.md.
- Audio and rumble remain; Training/magnifier sounds await the backend.
- Event-aligned N64 evidence remains for CPU item behavior, VS entry
  focus, item damage/trajectory, thrown-body hits, pipes and stage effects.
- Lit primitives without authored light colours retain baked fallback
  shade. Bumper palette, Star flicker and bomb flashes lack N64 traces.
- Fighter jostling and selects' spotlight remain. Yoshi's pre-existing
  double-jump apex hang persists. Performance/VRAM residency belong to P5.
