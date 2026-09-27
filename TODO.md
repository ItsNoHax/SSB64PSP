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
| Draw Samus's Charge Shot and Bomb | Gameplay weapons exist; their meshes (`dSamusSpecial3` and the `SamusModel` bomb display list with palette blink) are not packed or drawn | RE-333 |
| Select and draw Samus, Luigi, Link, Yoshi, Captain Falcon and Kirby in `psp-game` | The movesets are host-only; Luigi's Fireball needs Mario's mesh with palette frame 1; Link's Boomerang and Spin Attack effect, Yoshi's Egg Throw and Bomb stars, and Kirby's Final Cutter wave, copy hats and stars are not drawn | RE-333–335, RE-337–338, RE-343 |
| Select and draw Ness in `psp-game` | Host moveset and copied PK Fire exist; PK Fire spark/flame, PK Thunder head/trails, Magnet and bat visuals need runtime integration | RE-347 |
| PK Fire incoming item damage and moving floors | The unpickable flame has its own allocation table, gravity, floor/wall response, shrinking hitboxes and rehit clock; source damage reduces life by three times the highest queued damage, and moving map groups need per-frame surface speed | RE-347 |
| Select and draw Pikachu and Jigglypuff in `psp-game` | The movesets are host-only; Thunder Jolt, Thunder head/trails, Quick Attack effects and Sing's notes need gameplay render integration | RE-345–346 |
| Kirby Inhale downward wiggle | The captured victim's downward mash/drop-through path needs its source floor flags and ignored-line linkage; Final Cutter cliff catches and star reflections now use the shared map solver | RE-343, RE-348 |
| Moving map groups and changing fighter diamonds | Static geometry is resolved with authored vertex identity; moving-surface speed, group attachment and per-frame collision-box changes need match inputs | RE-345, RE-348 |
| Authored cliff poses and clip clocks | Catch queries use packed hand reach and two-fighter occupancy, but root placement still uses the corner; climb/attack/escape need their TransN poses and clip lengths | RE-348 |
| Yoshi Egg Lay victim collision and effect | Laying omits the wall/ceiling sweep, damaging-floor escape needs hazards, and the break effect is represented by a 10-frame clock | RE-337 |
| Weapon map-bound removal | `wpProcessProcWeaponMain` deletes weapons outside `map_bound_*`; the pool keeps a missed Blaster or Charge Shot until a map contact | RE-333 |
| Boomerang off-camera removal | `wpLinkBoomerangCheckOffCamera` needs the battle camera's projection; the pool keeps the Boomerang until its lifetime ends | RE-335 |
| Item system | Link's Bomb (`itLinkBomb`) and the item-throw branch of his down special need held items and `ftCommonItemThrow*` | RE-335 |
| Weapon shield and hop callbacks | Weapons meet shields through the hit log with their source `sd`; the Boomerang's own `ProcShield`/`ProcHop` are not reached | RE-335, RE-350 |
| Catcher branches on a same-frame partner hit | `ftCommonDamageUpdateMain`'s catcher branches read the held partner's same-frame `damage_knockback`; each fighter resolves without the other's state, so the catcher takes the single-hit path | RE-339, RE-350 |
| Pack clips for reaction statuses | Escape, Down (bounce/wait/stand/attack/roll), Passive, ShieldBreak, FuraFura and Rebound have figatree lengths from the motion descs but no pack animation slots; roll and tech root motion comes from those clips. Adding slots changes the pack layout | RE-350 |
| Weapon clank and reflector collisions | Weapon–attack clank (`ftMainSearchHitWeapon`'s attack branch), reflector/absorb special collisions as attack collisions and the one-frame invincibility after a knockback overrun are not ported | RE-350 |
| Training capture scripts land no hit | The RE-341 build never damaged the dummy in 3,600 ticks. Re-measure with joint hurtboxes, scripted attack timing and the per-frame hit log; a matching screenshot alone does not prove hit resolution | RE-341, RE-349, RE-350 |
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
