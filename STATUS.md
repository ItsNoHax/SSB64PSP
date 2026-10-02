# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** stage item weights, the appearance actor, real
  container contents and the Tomato, Heart and Star, matched to an N64
  Training scene (RE-433, pack v84).
- **Next gameplay batch:** the throwable utilities: Motion-Sensor Bomb,
  Bob-omb, Green/Red Shell, Bumper and the Poké Ball's throw and open
  (its 13 Pokémon after). Swing/shoot items and the Hammer follow.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Utility contents | Weights, appearance actor, Tomato/Heart/Star, star invincibility and heal; 2 new goldens, 10 rebaselined | RE-433 |
| Heavy containers | Crate/Barrel, heavy statuses, item-heavy joint animated; 6 new goldens, 3 rebaselined | RE-432 |

## Verification baseline

- Workspace: 1,647 tests pass (absolute SSB64_ROM, pinned 1.98.0, one thread).
  Clippy, workspace rustfmt, gameplay/ROM no_std and docs validation pass.
- Both production PSP release builds pass. Game has no warnings; viewer
  retains five existing warnings. Production binaries are restored.
- Full golden matrix: 167 of 167 scenes pass (run `20261002-203351`).
- Final production PPSSPPHeadless 20 s software smoke finishes without
  faults. N64 reference for the utility scene; no hardware proof.
- Pack v84: 35,436,736 bytes, SHA-256
  `1623a2d589be5f07a73c8b6249a7949a9c253cdda85af837e1707310ed3ded2b`.
- Production `run` frame 27,344; largest frame `enter_training` 212,416.
  Combined 239,760 of the 256 KiB stack, leaving 22,384 before alignment
  and nested calls. ELF total 3,411,304 bytes. Details: RE-433.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Thirteen utility makers make nothing yet (their draws are kept). Item
  music, arrows and the Star's material animation remain. Non-container
  items do not yet use descriptor 1 as their root (RE-432).
- Yoshi hangs at his double-jump apex until interrupted (pre-existing).
- Bonus stages and pipe traversal/plant notification. Bumper lit flash,
  audio, Pokémon and launched-plant N64 comparisons remain.
- Both selects' spotlight is absent; no-save-data unlocks remain locked.
- Arwing laser appearance, 3D facing, pilot manoeuvres and wing collision
  remain unverified against the N64 (RE-428).
- Entry cockpit (264), near-camera Poké Ball (224), focus 2, Luigi pipe and
  Kirby rightward-star comparisons remain (RE-425).
- Four-fighter Dream Land texture closure exceeds VRAM by 22,412 bytes
  at low detail (RE-426). Results fade/wipe and broader scene equality
  remain unverified. Stage draw performance belongs to `P5`.
