# TODO

Deferred work that is not the current batch ([`STATUS.md`](STATUS.md)) and not
a milestone in [`PLAN.md`](PLAN.md). Unordered. Delete an entry once a batch
or evidence record covers it.

## Rendering

| Item | Reason deferred | Evidence |
|---|---|---|
| Costume validation and dummy costume | `psp-game` picks the player's costume (RE-340) but no PPSSPP capture shows costumes 1–3; the dummy keeps costume 0 instead of the CPU's first free costume, which needs a golden refresh | RE-096, RE-261, RE-340 |
| Independent fighter animation validation | Stage animation has a ROM-derived check (RE-050–052, RE-142); fighter costume/material animation does not | — |
| Per-scene texture residency | Archive-wide textures exceed the ~700 KiB VRAM budget; the measured worst match scene fits. Re-measure with `romtool scene-deps` | RE-076, RE-077, RE-340 |
| Strict mode over the real pack | `romtool strict` and `strict_render` exist but have not run over pack v36 | RE-340 |
| Material-animation command 22 | `ssb-rom::matanim` rejects it; its writes are never read, so it can be skipped | RE-010 |
| `WPAttributes` pairing shape | Only known instance (Link's boomerang) has no sub-objects; revisit if another appears | RE-058 |

## Gameplay

| Item | Reason deferred | Evidence |
|---|---|---|
| Restore condensed same-valued attack boxes | Some Donkey motion commands attach otherwise identical boxes to different joints; the existing `MoveData` kept one copy. Mario's and Fox's cases were restored (RE-339) | RE-332, RE-339 |
| Draw Samus's Charge Shot and Bomb | Gameplay weapons exist; their meshes (`dSamusSpecial3` and the `SamusModel` bomb display list with palette blink) are not packed or drawn | RE-333 |
| Mario down-air landing | `dFTMarioMotionDescs` has no `LandingAirLw` motion, so the source enters `LandingAirNull` from `AttackAirLw`; the Mario port still enters `LandingAirLw` (Luigi's port follows the source) | RE-334 |
| Select and draw Samus, Luigi, Link, Yoshi and Captain Falcon in `psp-game` | The movesets are host-only; Luigi's Fireball needs Mario's mesh with palette frame 1; Link's Boomerang and Spin Attack effect, and Yoshi's Egg Throw and Bomb stars, are not drawn | RE-333–335, RE-337–338 |
| Falcon Kick wall rebound and Falcon Dive cliff catch | Fighter map collision resolves floors only; the decomp callbacks need wall and cliff flags | RE-338 |
| Yoshi Egg Lay victim collision and effect | The egg uses the root-sphere hurtbox instead of `dFTCommonYoshiEggDamageCollDescs`; laying omits the wall/ceiling sweep, damaging-floor escape needs hazards, and the break effect is represented by a 10-frame clock | RE-337 |
| Yoshi Bomb aerial ledge catch | The source can catch a ledge during the aerial Bomb; the fighter ledge search is not integrated with this status | RE-337 |
| Weapon map-bound removal | `wpProcessProcWeaponMain` deletes weapons outside `map_bound_*`; the pool keeps a missed Blaster or Charge Shot until a map contact | RE-333 |
| Boomerang off-camera removal | `wpLinkBoomerangCheckOffCamera` needs the battle camera's projection; the pool keeps the Boomerang until its lifetime ends | RE-335 |
| Item system | Link's Bomb (`itLinkBomb`) and the item-throw branch of his down special need held items and `ftCommonItemThrow*` | RE-335 |
| Weapon shield and hop callbacks | Weapons pass through shields; the Boomerang's `ProcShield`/`ProcHop` are not reached | RE-335 |
| Escape (roll) statuses | Samus's Charge Shot loop reads `ftCommonEscapeGetStatus`; `EscapeF`/`EscapeB` are ordinals only | RE-333 |
| Hit-status intangibility | `SetHitStatusAll(2)` (Screw Attack start, throws) and Luigi's Super Jump Punch and up-smash intangibility have no effect on the root-sphere hurtbox | RE-333, RE-334 |
| Same-frame catcher and held hits | `ftCommonDamageUpdateMain`'s simultaneous-hit branches and catcher hitlag need a deferred per-frame damage queue; hits resolve one at a time | RE-339 |
| `recent_damage` for fighter hits | The source passes the frame's `damage_queue`; the hit path passes zero | RE-339 |
| Weapon staling | `wpMainGetStaledDamage` and weapon queue updates are not ported | RE-339 |
| Re-run goldens after RE-339 | Handicap rounding moves some knockback by one ulp, and repeated moves now deal staled damage | RE-339 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current pack (v37, with Captain Falcon slots) is 23,037,360 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318, RE-327, RE-338 |
| 30-minute run on a second unit | Only one unit (Slim) has run 30 minutes with the full pack | RE-273, RE-284 |
| Re-capture current goldens on hardware | RE-320 captured the v32 diagnostic object and RE-326 three v35 stages, not the full current golden matrix | RE-320, RE-326 |

## Open questions

| ID | Question | Next step |
|---|---|---|
| RE-009 | PSP nub deadzone (20 units, linear rescale to ±80 is a guess) | Measure on hardware against decomp thresholds |
| RE-011 | How `sGCDetailLevel` is chosen | Trace during `P5` profiling |

## Technical debt

- Check `ssb_rom::reloc_link` against real closures and use it from a runtime loader ([D-011](docs/decisions/D-011.md), RE-340)
- Wire `ssb_engine::memory` arenas and pools into `psp-runtime` ([docs/memory.md](docs/memory.md))
- VFPU math, after `P5` profiling ([D-032](docs/decisions/D-032.md))
- `sceAudio` mixer thread (`P4`)
- Debug HUD uses `sceGuDebugFlush`: software-rasterizer-only in PPSSPP (RE-014) and faults on real hardware (RE-202); replace with GE geometry
