# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Race to the Finish's ground (`grbonus3.c`) with its
  bomb barrel (`ittarubomb.c`), Mushroom Kingdom's pipes
  (`ftcommondokan.c`) with the plant notification, the cloud vapor and
  the scale sparkles (RE-446). Pack remains v91.
- **Next gameplay batch:** 1P Game scene flow (`sc1pgame*`), which enters
  the bonus stages; packing Race to the Finish's ground belongs with it
  (TODO.md).
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Bonus 3, pipes, stage effects | ROM-checked barrel table, Bumper scripts, barrel point and gate; pipe check in six interrupt chains, 30-frame trip, 1-in-4 wall exit; vapor on Yoshi's bank 5 | RE-446 |
| Thrown-fighter attacks | Damage-thrown table, one-shot owner, thrower credit | RE-445 |

## Verification baseline

- All 1,794 workspace tests pass (absolute SSB64_ROM, pinned 1.98.0,
  one thread). Clippy with warnings denied and workspace rustfmt pass.
- Pack v91: 35,818,544 bytes, SHA-256
  `731f7593aec0e4cb8596f6f6756f3793efded012cdd517782ae5337ab0132e53`.
  No asset-pipeline change this batch.
- Both production PSP release builds pass (nightly-2026-08-26).
  Game has no warnings; viewer retains five existing warnings.
- No goldens re-run (no golden scene covers the pipes, Yoshi's vapor or
  the scales' fall). Last targeted game goldens: RE-444 (33 scenes);
  larger game baseline RE-443; full viewer matrix RE-438. Manifest: 195.
- Production PPSSPPHeadless 20 s software smoke exits 0 at its expected
  timeout without reported faults. Production has no screenshot hook.
  This is not physical-PSP or N64-equivalence proof.
- Stack last measured RE-440; production ELF last measured RE-443.
  Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Training menu sounds/BGM volume and magnifier sound dispatch await
  the audio backend. Music, audio and rumble remain.
- VS entry focus, CPU item behaviour, item damage/trajectory and
  thrown-body hits need event-aligned original-game evidence (RE-434,
  RE-437–440, RE-445). Pipes, vapor and sparkles lack N64 captures.
- Lit primitives without authored light colours retain baked fallback
  shade; Bumper's lit palette, Star flicker and the Bombs' critical
  flashes lack N64 observations.
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- Fighter jostling is not ported. 1P Game, the bonus stages' scenes,
  selects' spotlight and no-save-data unlocks remain in TODO.md. Stage
  performance and VRAM residency belong to `P5`.
