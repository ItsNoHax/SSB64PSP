# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports; shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** general item system (`ftMainSearchHitItem`, Link's
  Bomb, PK Fire flame damage, item throws and pickup).
- **Next batch:** moving map groups, changing fighter diamonds and authored
  cliff poses. These map inputs were explicitly deferred from this batch.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| General item system | Shared 16-slot item pool, fighter/item/weapon hit searches, Bomb lifecycle and self-hit explosions, PK Fire damage/lifetime, script-driven throws/pickup, guard/escape windows, damage drops and death destruction; PSP frame integration | RE-352 |
| Damage map callbacks, reaction clips, weapon clank and reflectors | Damage sweep and reactions; pack v39; weapon clank/reflectors; overrun invincibility; same-frame catcher/held hits; `DeadUpFall` | RE-351 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with absolute `SSB64_ROM`: **1,152 pass**,
  including 491 gameplay tests. Clippy (`--workspace --all-targets --
  -D warnings`), rustfmt and the engine/game/ROM no_std target builds pass.
- Both PSP release builds pass; game also with
  `regression_capture,strict_render`. Item process boundaries remain out of
  line to avoid PSP LTO's MIPS branch-range overflow (RE-352).
- Scripted Training smoke via `tools/run-ppsspp.sh`: valid software-rendered
  Dream Land screenshot with both fighters; PPSSPP terminated afterwards.
  Interactive game EBOOT rebuilt. No hardware or full golden run this batch.
- Pack v39 unchanged: 24,254,352 bytes, SHA-256
  `2e0ce44b126adf1de7c7d7f0deb6c06c7150443019420712ed65839d87efa984`.
  Last animation verification and full 73-scene golden matrix: RE-351.
- Physical PSP last checked in RE-341 (PSP-2000 Slim, 6.61 ARK, pack v37).

## Blockers and remaining scope

- Moving map groups, changing diamonds and authored cliff poses are next.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Damage statuses and most normals lack pack clips; hits are not visible.
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341). Rendering performance remains `P5`.
