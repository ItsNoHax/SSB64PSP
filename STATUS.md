# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports; shared machinery and PSP game
  integration remain before `P2` is complete.
- **Next batch:** items and the remaining map inputs: general item system
  (`ftMainSearchHitItem`, Link's Bomb, PK Fire flame damage), moving map
  groups and changing diamonds, and authored cliff poses.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Damage map callbacks, reaction clips, weapon clank and reflectors | Damage sweep with `WallDamage`/`StopCeil`/`DownBounce` and techs; 25 reaction pack slots (v39) with TransN rolls; weapon–attack clank, `FTSpecialColl` reflector/absorb, Boomerang set-off/hop; overrun invincibility; same-frame catcher/held hits; `DeadUpFall`; Samus roll length fixed | RE-351 |
| Combat audit 2 | Motion scripts drive attacks for all 12 fighters; per-frame hit log, clank, shields, rebound, knockdown/tech/roll, shared RNG | RE-350 |

## Verification baseline

- `cargo +1.98.0 test --workspace` with `SSB64_ROM`: 1,130 pass.
  Clippy (`--workspace --all-targets -- -D warnings`), rustfmt and the
  `no_std` builds of `ssb-engine`/`ssb-game`/`ssb-rom` pass.
- `romtool anims --verify`: 27 fighters, 1,053 lengths agree with the decomp.
- Pack v39: 24,254,352 bytes, SHA-256
  `2e0ce44b126adf1de7c7d7f0deb6c06c7150443019420712ed65839d87efa984`,
  27/27 fighters, 1,392 clips. Rebuilt locally; no ROM-derived output
  committed.
- Both PSP builds pass; `psp-game` also with `strict_render`.
- PPSSPPHeadless: 73 of 73 goldens match twice. New `f1-training-jab`
  (dummy 2%, `DamageN1` in the capture log); `r2-stage-bonus1-luigi`
  rebaselined (stale since RE-350).
- Physical PSP last checked in RE-341 (PSP-2000 Slim, 6.61 ARK, pack v37).
  No hardware run this batch.

## Blockers

- The Training `grab` scene whiffs in PPSSPP (posed catch boxes) while the
  host catch search lands on the same route; its golden pins the whiff.
- Damage statuses and most normals (Mario's jab included) have no pack
  clips, so hits are not visible in captures.
- Items are unported: no item clank, Link's Bomb, PK Fire flame damage.
- Moving map groups, changing diamonds and authored cliff poses remain.
- Samus through Ness are host-only in `psp-game`; weapon/effect drawing
  remains.
- Four-player VS exceeds the ~700 KiB texture pool; Training runs at 30 Hz
  on hardware (RE-341).
