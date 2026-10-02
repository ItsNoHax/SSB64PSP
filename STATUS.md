# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** light containers Capsule/Egg, their ROM animation
  and rendering, real Chansey Egg allocation, normal-item switches and
  item–weapon clashes (RE-431, pack v82).
- **Next gameplay batch:** Crate/Barrel and the common heavy-item pickup
  and throw statuses. Utility contents and normal-item appearance follow.
- **Parallel track:** rendering fidelity (`P5`), not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Light containers | Capsule/Egg states, switches, Chansey maker; 2 new goldens | RE-431 |
| Saffron Pokémon | Five real items, two weapons, gate callbacks and ROM clocks | RE-430 |

## Verification baseline

- Workspace: 1,630 tests pass (absolute SSB64_ROM, pinned 1.98.0, one thread).
  Clippy, workspace rustfmt, gameplay/ROM no_std and docs validation pass.
- Both production PSP release builds pass. Game has no warnings; viewer
  retains five existing warnings. Production binaries are restored.
- Twelve relevant game goldens pass twice at four-way concurrency, zero
  pixel differences. Chansey is rebaselined for its Egg and spawn effects
  (6,752 pixels). Manifest: 159 scenes. Last full matrix: RE-429's 152
  scenes; not rerun for this gameplay batch.
- Final production PPSSPPHeadless 20 s software smoke finishes without
  faults. No menu-navigation, N64 visual-equivalence or hardware proof.
- Pack v82: 35,435,568 bytes, SHA-256
  `baecdd64d7b495a4ef1bccea4dad3e784806ac49bb7c5118b2c4cba8c673580f`.
- Production `run` frame 26,144; largest frame `enter_training` 212,384.
  Combined 238,528 of the 256 KiB stack, leaving 23,616 before alignment
  and nested calls. ELF total 3,390,216 bytes. Details: RE-431.
- Physical PSP last checked RE-361 (PSP-2000, 6.61 ARK, pack v43).
  Scripted Training held 16,682 µs/frame in RE-360.

## Blockers and remaining scope

- Heavy containers/statuses, utility makers and normal appearance actor.
  Current switches enable only Capsule/Egg; the utility drop table is
  empty, so containers use the source explosion fallback.
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
