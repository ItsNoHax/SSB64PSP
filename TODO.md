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
| Draw Kirby's copy hats, stars and copied specials' effects in `psp-game` | Kirby draws in Training with his Final Cutter wave (RE-378); copy hats, the Inhale/spit stars and the copied Falcon Punch flame (joint 30) are not drawn | RE-343, RE-376, RE-378 |
| Draw Yoshi's Egg Lay egg and his egg/star hit effects in `psp-game` | Yoshi draws in Training with his Egg Throw egg and Bomb stars (RE-375); the captured victim's egg and the shatter, egg-break and sparkle effects remain | RE-337, RE-375 |
| Draw Link's Bomb and reach the PK Fire flame in `psp-game` | The flame draws (RE-382) but no capture scene lands a spark on the dummy; the Bomb needs the held-item hand parent (kind 0x52) and a look at why its strips draw as an orange smear | RE-352, RE-374, RE-382 |
| Keep `run` inside MIPS branch range | `psp-game`'s `run` is 163 KB after RE-382's split, over the ±128 KB branch reach; more growth can bring back `out of range PC16 fixup` | RE-382 |
| Window material-animation blobs | A material script packs its whole source file: file 335 (50 KB) came in for PK Thunder's texture blink (RE-381), as weapon animations did before RE-378's window | RE-378, RE-381 |
| Draw Pikachu's Thunder head/trails and the Thunder Jolt, Quick Attack effects in `psp-game` | Pikachu draws in Training with both Thunder Jolt forms (RE-379); ground-jolt node 4's texture script is declined by `resolve_one_mat_anim` | RE-345, RE-379 |
| Kirby Inhale downward wiggle | The captured victim's downward mash/drop-through path needs its source floor flags and ignored-line linkage; Final Cutter cliff catches and star reflections now use the shared map solver | RE-343, RE-348 |
| Stage selection | `StageSetup` builds any VS stage's controller from pack v44 (RE-362) and every controller object from v47 (RE-365), but the game has no stage select; Training loads Dream Land. The Dream Land capture script walks off other stages, and Training has no respawn | RE-362, RE-365 |
| Acid tile-1 sprite variants | The acid script's `TextureIDNext` reaches sprites 1–3, but its primitive has no two-tile blend, so only sprite 0 draws. Pack v46 still converts all four 384×384 variants (523 KB unreachable). Converting only sprites a primitive can sample would change every stage's shared `MatAnimDesc` conversion | RE-364 |
| Sector Z Arwing, bonus stages and stage items | The Arwing (weapons, flight patterns), `grbonus3.c`, and the Bumper, POW Block, Piranha Plant and Pokémon items remain; controllers already call `StageObjects::make_item`, and the Castle ground root the Bumper rides now moves (RE-365). The cloud vapor particle and the scale sparkle are not ported | RE-356, RE-365 |
| Twister and Barrel Cannon clips | Both statuses keep the previous pose; `nFTCommonMotionTwister` needs a shared slot, and TaruCann has none (`-1`) | RE-356 |
| Yoshi Egg Lay victim collision and effect | Laying omits the wall/ceiling sweep, the damaging-floor escape is not wired to the ported ground hits (RE-356), and the break effect is represented by a 10-frame clock | RE-337 |
| Fireball spin | `wpMarioFireballProcUpdate` adds `rotate_speed` (20° Mario, 25° Luigi) to the DObj X rotation each frame; the port draws only the ±90° yaw. The DObj's second transform kind (0x47) is an undecoded battle-scene custom matrix function | RE-372 |
| Training fighter select | Only capture scenes pick Fox or Luigi; real pad input always spawns Mario | RE-372 |
| Other item kinds and item presentation | Bomb and PK Fire use the shared item system; heavy/swing/shoot/consume items, team checks, item models, effects, sound, spin, throw-turn joint yaw and pickup arrows remain | RE-352 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | Pack did not fit in 32 MiB and `MEMSIZE=1` is ignored. The current pack (v51, with Luigi's Fireball) is 27,086,256 bytes; re-measure before designing a reduced or streaming pack | RE-288, RE-318, RE-327, RE-344–347, RE-351, RE-355, RE-366–368, RE-372 |
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
