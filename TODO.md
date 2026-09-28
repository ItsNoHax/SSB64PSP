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
| Select and draw Pikachu and Jigglypuff in `psp-game` | The movesets are host-only; Thunder Jolt, Thunder head/trails, Quick Attack effects and Sing's notes need gameplay render integration | RE-345–346 |
| Kirby Inhale downward wiggle | The captured victim's downward mash/drop-through path needs its source floor flags and ignored-line linkage; Final Cutter cliff catches and star reflections now use the shared map solver | RE-343, RE-348 |
| Stage selection | `StageSetup` builds any VS stage's controller from pack v44 (RE-362) and every controller object from v47 (RE-365), but the game has no stage select; Training loads Dream Land. The Dream Land capture script walks off other stages, and Training has no respawn | RE-362, RE-365 |
| Acid tile-1 sprite variants | The acid script's `TextureIDNext` reaches sprites 1–3, but its primitive has no two-tile blend, so only sprite 0 draws. Pack v46 still converts all four 384×384 variants (523 KB unreachable). Converting only sprites a primitive can sample would change every stage's shared `MatAnimDesc` conversion | RE-364 |
| Sector Z Arwing, bonus stages and stage items | The Arwing (weapons, flight patterns), `grbonus3.c`, and the Bumper, POW Block, Piranha Plant and Pokémon items remain; controllers already call `StageObjects::make_item`, and the Castle ground root the Bumper rides now moves (RE-365). The cloud vapor particle and the scale sparkle are not ported | RE-356, RE-365 |
| Twister and Barrel Cannon clips | Both statuses keep the previous pose; `nFTCommonMotionTwister` needs a shared slot, and TaruCann has none (`-1`) | RE-356 |
| Yoshi Egg Lay victim collision and effect | Laying omits the wall/ceiling sweep, the damaging-floor escape is not wired to the ported ground hits (RE-356), and the break effect is represented by a 10-frame clock | RE-337 |
| Other item kinds and item presentation | Bomb and PK Fire use the shared item system; heavy/swing/shoot/consume items, team checks, item models, effects, sound, spin, throw-turn joint yaw and pickup arrows remain | RE-352 |
| Training `grab` scene whiffs on PSP | In PPSSPP the player stays in `Catch` without a catch (RE-351 capture log) while host `romtool jumptest --catch-tick 100` on the same route catches on tick 106; the golden already pins the whiff. Compare the posed catch box and the dummy's grabbable boxes against the unposed fallbacks | RE-341, RE-351 |
| Shield tilt pose | `Guard`/`GuardSetOff` hold `GuardOn`'s last frame. The original layers the per-fighter `shield_anim_joints[angle]` events onto the joints, blended by `shield_rotate_range` (`ftCommonGuardUpdateJoints`, `ftCommonGuardInitJoints`) | RE-366 |
| Clip start frame | `set_status` drops `anim_frame_begin`, so every clip starts at frame 0. `RebirthDown` (100), hammer statuses, gun and Fire Flower shots, walks and `Pass` start later in the original | RE-366 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current pack (v48, with every shared status clip) is 26,905,648 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318, RE-327, RE-344–347, RE-351, RE-355, RE-366 |
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
