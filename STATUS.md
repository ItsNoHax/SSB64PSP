# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** the throwable utilities: Motion-Sensor Bomb,
  Bob-omb, Bumper, both Shells and the Poké Ball's throw and open,
  compared with an N64 Training scene (RE-434, pack v84).
- **Next gameplay batch:** the Poké Ball's 13 Pokémon (`itmonster/`), its
  rays and open animation. Swing/shoot items and the Hammer follow.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Throwable utilities | Six makers and statuses, surface attachment, landing root squaring, Green Shell palette mesh; 2 new goldens, 3 rebaselined | RE-434 |
| Utility contents | Weights, appearance actor, Tomato/Heart/Star, star invincibility and heal | RE-433 |

## Verification baseline

- Workspace: 1,662 tests pass (absolute SSB64_ROM, pinned 1.98.0, one thread).
  Clippy, workspace rustfmt, gameplay/ROM no_std and docs validation pass.
- Both production PSP release builds pass. Game has no warnings; viewer
  retains five existing warnings. Production binaries are restored.
- Full golden matrix: 169 of 169 scenes pass (run `20261003-020526`).
- Final production PPSSPPHeadless 20 s software smoke finishes without
  faults. N64 reference for the throwable scene; no hardware proof.
- Pack v84: 35,437,552 bytes, SHA-256
  `2026b104f3da8d118777f1da801e0b9e98c91c4e98cf0842c60600eb3a137ba3`.
- Production `run` frame 27,920; largest frame `enter_training` 212,416.
  Combined 240,336 of the 256 KiB stack, leaving 21,808 before alignment
  and nested calls. ELF total 3,454,196 bytes. Details: RE-434.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).

## Blockers and remaining scope

- The 13 Pokémon and seven swing/shoot/Hammer makers make nothing yet
  (their draws are kept). Shell spin animation, Bob-omb walk lists, Bumper
  lit palette, item music, arrows and the Star's material animation remain.
- The N64 threw the Poké Ball right where the port throws it left (RE-434).
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
