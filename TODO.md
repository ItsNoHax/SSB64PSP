# TODO

Deferred work that is not the current batch ([`STATUS.md`](STATUS.md)) and not
a milestone in [`PLAN.md`](PLAN.md). Unordered. Delete an entry once a batch
or evidence record covers it.

## Rendering

| Item | Reason deferred | Evidence |
|---|---|---|
| Independent fighter animation validation | Stage animation has a ROM-derived check (RE-050–052, RE-142); fighter costume/material animation does not | — |
| Per-scene texture residency | Training fits the ~700 KiB VRAM pool (171 KiB), but Hyrule Castle alone is 915 KiB and the worst four-fighter VS scene 3,025 KiB, mostly Donkey Kong's 32-bit mipmapped costume textures. Needs a residency design (main-RAM sampling or per-scene streaming) and a look at DK's packed format before VS mode | RE-076, RE-077, RE-341 |
| Material-animation command 22 | `ssb-rom::matanim` rejects it; its writes are never read, so it can be skipped | RE-010 |
| `WPAttributes` pairing shape | Only known instance (Link's boomerang) has no sub-objects; revisit if another appears | RE-058 |

## Gameplay

| Item | Reason deferred | Evidence |
|---|---|---|
| Restore condensed same-valued attack boxes | Some Donkey motion commands attach otherwise identical boxes to different joints; the existing `MoveData` kept one copy. Mario's and Fox's cases were restored (RE-339) | RE-332, RE-339 |
| Draw Samus's Charge Shot and Bomb | Gameplay weapons exist; their meshes (`dSamusSpecial3` and the `SamusModel` bomb display list with palette blink) are not packed or drawn | RE-333 |
| Mario down-air landing | `dFTMarioMotionDescs` has no `LandingAirLw` motion, so the source enters `LandingAirNull` from `AttackAirLw`; the Mario port still enters `LandingAirLw` (Luigi's port follows the source) | RE-334 |
| Select and draw Samus, Luigi, Link, Yoshi, Captain Falcon and Kirby in `psp-game` | The movesets are host-only; Luigi's Fireball needs Mario's mesh with palette frame 1; Link's Boomerang and Spin Attack effect, Yoshi's Egg Throw and Bomb stars, and Kirby's Final Cutter wave, copy hats and stars are not drawn | RE-333–335, RE-337–338, RE-343 |
| Ness map callbacks and shared DownBounce | PK Thunder hold/end cliff catch, blast wall sliding/rebound and grounded ledge stops need the shared map solver; downward self-launch or steep floor impact enters the unported `DownBounceD` | RE-347 |
| Select and draw Ness in `psp-game` | Host moveset and copied PK Fire exist; PK Fire spark/flame, PK Thunder head/trails, Magnet and bat visuals need runtime integration | RE-347 |
| PK Fire incoming item damage and moving floors | The unpickable flame has its own allocation table, gravity, floor/wall response, shrinking hitboxes and rehit clock; source damage reduces life by three times the highest queued damage, and moving map groups need per-frame surface speed | RE-347 |
| Quick Attack wall/ceiling cancellation and cliff catch | The fighter map solver resolves floors only; Quick Attack needs source wall/ceiling normals and cliff flags | RE-345 |
| Aerial Z-cancel window | `ftCommonAttackAirProcMap` checks ten ticks since Z; the shared fighter state has no Z-tap timer, so Pikachu's active landing windows retain their authored lag | RE-345 |
| Select and draw Pikachu and Jigglypuff in `psp-game` | The movesets are host-only; Thunder Jolt, Thunder head/trails, Quick Attack effects and Sing's notes need gameplay render integration | RE-345–346 |
| Kirby copy-ability loss on damage | `ftKirbySpecialNDamageCheckLoseCopy` rolls `syUtilsRandFloat() < 1/12`; the crate has no RNG | RE-343 |
| Kirby ledge, wall and pass-through cases | Final Cutter and suplex ledge catches, star reflections off walls and ceilings, and the Inhale drop-through wiggle need fighter wall/cliff/pass collision | RE-343 |
| Falcon Kick wall rebound and Falcon Dive cliff catch | Fighter map collision resolves floors only; the decomp callbacks need wall and cliff flags | RE-338 |
| Yoshi Egg Lay victim collision and effect | The egg uses the root-sphere hurtbox instead of `dFTCommonYoshiEggDamageCollDescs`; laying omits the wall/ceiling sweep, damaging-floor escape needs hazards, and the break effect is represented by a 10-frame clock | RE-337 |
| Yoshi Bomb aerial ledge catch | The source can catch a ledge during the aerial Bomb; the fighter ledge search is not integrated with this status | RE-337 |
| Weapon map-bound removal | `wpProcessProcWeaponMain` deletes weapons outside `map_bound_*`; the pool keeps a missed Blaster or Charge Shot until a map contact | RE-333 |
| Boomerang off-camera removal | `wpLinkBoomerangCheckOffCamera` needs the battle camera's projection; the pool keeps the Boomerang until its lifetime ends | RE-335 |
| Item system | Link's Bomb (`itLinkBomb`) and the item-throw branch of his down special need held items and `ftCommonItemThrow*` | RE-335 |
| Weapon shield and hop callbacks | Source-specific shield/hop callbacks remain partial; the Boomerang's `ProcShield`/`ProcHop` are not reached | RE-335 |
| Escape (roll) statuses | Samus's Charge Shot loop reads `ftCommonEscapeGetStatus`; `EscapeF`/`EscapeB` are ordinals only | RE-333 |
| Hit-status intangibility | Screw Attack startup, earlier fighters' throws, Luigi's Super Jump Punch/up smash, Quick Attack startup and Jigglypuff's down-smash parts need shared hit status and per-part hurtboxes; Rest's whole-body window and Ness's blast/throw invincibility windows are modelled | RE-333, RE-334, RE-345–347 |
| Same-frame catcher and held hits | `ftCommonDamageUpdateMain`'s simultaneous-hit branches and catcher hitlag need a deferred per-frame damage queue; hits resolve one at a time | RE-339 |
| `recent_damage` for fighter hits | The source passes the frame's `damage_queue`; the hit path passes zero. Needs the same deferred hit collection as the row above | RE-339 |
| Training capture scripts land no hit | RE-295 timed the jab to hit the dummy; on the current build the scripted jab and Fireball never damage it (dummy damage 0 over 3,600 ticks), so no golden covers hit resolution | RE-341 |
| `psp-game` runs Training at 30 Hz on hardware | One tick per loop, and the loop takes two vsyncs (33.4 ms) on the PSP-2000 while simulation takes 1.74 ms; the draw side needs profiling or a fixed-step clock like the viewer's | RE-341 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current pack (v38, with Ness and copy slots) is 23,659,808 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318, RE-327, RE-344–347 |
| 30-minute run on a second unit | Only one unit (Slim) has run 30 minutes with the full pack | RE-273, RE-284 |
| Re-capture current goldens on hardware | RE-320 captured the v32 diagnostic object, RE-326 three v35 stages and RE-341 the six `psp-game` scenes, not the viewer matrix | RE-320, RE-326, RE-341 |
| Hand-input gameplay checks on hardware | R shield and grab, live throws, a held fighter hit by a Fireball and hand costume picks need a person at the controller | RE-339, RE-341 |

## Open questions

| ID | Question | Next step |
|---|---|---|
| RE-009 | PSP nub deadzone (20 units, linear rescale to ±80 is a guess) | Measure on hardware against decomp thresholds |
| RE-011 | How `sGCDetailLevel` is chosen | Trace during `P5` profiling |

## Technical debt

- Use `ssb_rom::reloc_link` from a runtime loader; its layout is checked against the original (RE-341, [D-011](docs/decisions/D-011.md))
- Wire `ssb_engine::memory` arenas and pools into `psp-runtime` ([docs/memory.md](docs/memory.md))
- VFPU math, after `P5` profiling ([D-032](docs/decisions/D-032.md))
- `sceAudio` mixer thread (`P4`)
- Debug HUD uses `sceGuDebugFlush`: software-rasterizer-only in PPSSPP (RE-014) and faults on real hardware (RE-202); replace with GE geometry
