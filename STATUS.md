# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** portable 1P campaign core: manager, US stage/CPU
  setup, team replacements, entry timing, bonuses and score/record
  accounting; shared 1P KO and COMPLETE announcement support. Race's four
  Bumper root clips and runtime bindings are in pack v92 (RE-447).
- **Next gameplay batch:** 1P campaign frontend and presentation:
  `mnPlayers1PGame*`, intro/continue/stage-clear scene flow and live
  session/stat hooks. Follow the scene requests in `spgame::Session`;
  do not substitute VS battles for missing bonus/Boss behavior.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Portable 1P campaign core | Stage progression, continues, challengers, bonuses, score ledger, enemy replacements; all 40 handicap rows ROM-checked; Race Bumper clips | RE-447 |
| Bonus 3, pipes, stage effects | Barrel and Bumper behavior, pipes, plant notification, vapor and sparkles | RE-446 |

## Verification baseline

- All 1,827 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread). Clippy with warnings denied and workspace rustfmt pass.
- Pack v92: 35,821,024 bytes, SHA-256
  `4b84c7d713dc732d5dac53a584f2605d5256976ec0560fd5478e43e84d245e35`.
  Rebuilt locally; no ROM-derived assets committed.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- Production PPSSPPHeadless 20 s software startup smoke exits 0 at its
  expected timeout without reported faults. It does not enter 1P, which
  has no frontend yet; no physical-PSP or N64-equivalence proof.
- No goldens re-run. Last targeted game goldens: RE-444 (33 scenes);
  larger game baseline RE-443; full viewer matrix RE-438. Manifest: 195.
- Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- 1P is not selectable/playable yet. Bonus-stage controllers, Master
  Hand, boss wallpaper/fade, remaining campaign presentation, persistent
  saves and unlock writes remain in TODO.md.
- Audio and rumble remain; Training/magnifier sounds await the backend.
- Event-aligned N64 evidence remains for CPU item behavior, VS entry
  focus, item damage/trajectory, thrown-body hits, pipes and stage effects.
- Lit primitives without authored light colours retain baked fallback
  shade. Bumper palette, Star flicker and bomb flashes lack N64 traces.
- Fighter jostling and selects' spotlight remain. Yoshi's pre-existing
  double-jump apex hang persists. Performance/VRAM residency belong to P5.
