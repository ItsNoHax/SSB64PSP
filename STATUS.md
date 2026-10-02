# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** Crate/Barrel, heavy pickup/lift/throw statuses,
  hidden-part figatree binding and the item root, matched to an N64
  crate lift and throw (RE-432, pack v83).
- **Next gameplay batch:** utility container contents (the item makers
  the drop table names) and the normal-item appearance actor.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Heavy containers | Crate/Barrel, heavy statuses, item-heavy joint animated; 6 new goldens, 3 rebaselined | RE-432 |
| Light containers | Capsule/Egg states, switches, Chansey maker; 2 new goldens | RE-431 |

## Verification baseline

- Workspace: 1,638 tests pass (absolute SSB64_ROM, pinned 1.98.0, one thread).
  Clippy, workspace rustfmt, gameplay/ROM no_std and docs validation pass.
- Both production PSP release builds pass. Game has no warnings; viewer
  retains five existing warnings. Production binaries are restored.
- Full golden matrix: 165 of 165 scenes pass (run `20261002-152358`).
- Final production PPSSPPHeadless 20 s software smoke finishes without
  faults. N64 reference only for the crate lift/throw; no hardware proof.
- Pack v83: 35,435,760 bytes, SHA-256
  `f980d85b6c126be34698453fef4e1440ca121fe324cbad8ffc6ca22ac373037d`.
- Production `run` frame 27,200; largest frame `enter_training` 212,384.
  Combined 239,584 of the 256 KiB stack, leaving 22,560 before alignment
  and nested calls. ELF total 3,390,560 bytes. Details: RE-432.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Utility makers and normal appearance actor; the utility drop table is
  empty, so containers use the source explosion fallback. Non-container
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
