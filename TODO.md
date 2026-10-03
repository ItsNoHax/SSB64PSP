# TODO

Deferred work that is not the current batch ([`STATUS.md`](STATUS.md)) and not
a milestone in [`PLAN.md`](PLAN.md). Unordered. Delete an entry once a batch
or evidence record covers it.

## Rendering

| Item | Reason deferred | Evidence |
|---|---|---|
| Independent fighter animation validation | Stage animation has a ROM-derived check (RE-050–052, RE-142); fighter costume/material animation does not | — |
| Per-scene texture residency | Low detail reduces the v78 Dream Land Mario/Fox/DK/Kirby costume-0 closure, including wallpaper, to 726,924 bytes, 22,412 over the 704,512-byte VRAM pool; high needs 1,051,532. Other stages/costumes can need more. Runtime samples from main RAM; hardware residency and DK format work remain | RE-076, RE-077, RE-341, RE-426 |
| Material-animation command 22 | `ssb-rom::matanim` rejects it; its writes are never read, so it can be skipped | RE-010 |
| `WPAttributes` pairing shape | Only known instance (Link's boomerang) has no sub-objects; revisit if another appears | RE-058 |

## Gameplay

| Item | Reason deferred | Evidence |
|---|---|---|
| Draw Kirby's stars and copied specials' effects in `psp-game` | Kirby draws in Training with his Final Cutter wave (RE-378), his copy hats (RE-417) and his motion-script model parts (RE-425); the Inhale/spit stars and the copied Falcon Punch flame (joint 30) are not drawn | RE-343, RE-376, RE-378, RE-417, RE-425 |
| Reach the PK Fire flame in a `psp-game` capture | The flame draws (RE-382) but no capture scene lands a spark on the dummy; the Bomb's critical colour flash and held bloat are not drawn (RE-383) | RE-352, RE-382, RE-383 |
| Keep `run` inside MIPS branch range | Production run is 22,884 code bytes, inside the 128 KiB branch reach. Stack frames and remaining space belong to STATUS.md and RE-431; watch these when logic moves back into run | RE-382, RE-385, RE-386, RE-389, RE-419, RE-422, RE-426, RE-427, RE-431 |
| Window material-animation blobs | A material script packs its whole source file: file 335 (50 KB) came in for PK Thunder's texture blink (RE-381), as weapon animations did before RE-378's window | RE-378, RE-381 |
| Draw Pikachu's Thunder Jolt and Quick Attack effects in `psp-game` | Pikachu draws in Training with both Thunder Jolt forms (RE-379) and Thunder (RE-417); ground-jolt node 4's texture script is declined by `resolve_one_mat_anim` | RE-345, RE-379, RE-417 |
| Kirby Inhale downward wiggle | The captured victim's downward mash/drop-through path needs its source floor flags and ignored-line linkage; Final Cutter cliff catches and star reflections now use the shared map solver | RE-343, RE-348 |
| CPU AI: modes | The VS CPU fights (RE-391) and tracks, picks up and uses items (RE-437); every trait, Rush and Training's CP option run (RE-438). The 1P Game that sets the traits, an N64 trace of a CPU tracking an item and an N64 check of the VS entry focus remain | RE-391, RE-437, RE-438 |
| Training and magnifier audio | Menu sounds, BGM volume changes and magnifier sound dispatch await the audio backend; the 30-tick request clock is ported | RE-438, RE-439, RE-440 |
| Fighter entry effects | Every entry effect draws, the Arwing, the car and the Poké Ball with its rays sorted onto links 2, 10 and 20 (RE-425). Battle tags now draw (RE-440). Remaining: a Fox-in-cockpit frame the port hides behind the Arwing's hull, the Poké Ball near-camera edge at tick 224, and Luigi's pipe and Kirby's rightward star compared with the N64 | RE-402, RE-403, RE-425, RE-440 |
| VS battle presentation and players | The VS battle runs (RE-389), and its CPU fights (RE-391), the VS mode menu sets the rule, time and stocks (RE-399) `mnPlayersVS` picks the players (RE-404) and draws its sprites and fighters (RE-411; the spotlight, the locked shadows' noise and the CPU colour animation are not drawn) and up to four fight (RE-405, free-for-all, no handicap; Magnet, Sing, the Spin Attack swirl, Falcon Punch/Kick, the reflector, Charge Shot and the held egg draw for the player only), and team battles run (RE-407), but there is no VS Options, HUD pieces beyond the battle HUD (the damage display, emblems, countdown, timer, stock icons and announcements draw, RE-392–RE-397) or the results' wipe (the wallpaper, tags, text and table draw, RE-410, and the series emblem and confetti, RE-420; the fighters stand, face and play their Win and Lose clips, RE-409, but draw opaque rather than fading in), and the entry effects draw (RE-403, RE-425). The demo scripts now run and Kirby's Win mouth matches the N64 (RE-426); fade and wipe remain | RE-389, RE-404, RE-405, RE-409, RE-410, RE-411, RE-420, RE-426 |
| KO presentation | The blast, screen flash, rebirth halo, rebirth glow, star KO fade (RE-412), the blast's particle streaks and the star sparkle (RE-413) are drawn; the halo's rays, the KO's quake and a star KO past 10,000 units draw (RE-420). The battle projection uses `dGMCameraPerspDefault`'s planes, 256 to 39,936, and the shield bubble and the other link-15/18 effect lists draw with no depth test, as on the N64 (RE-421). The camera's weapon interests are not ported (RE-406) | RE-388, RE-406, RE-412, RE-413, RE-420, RE-421 |
| Battle draw order: effects' head-1 lists | The battle draws in the original's display-link passes (RE-422), but an effect, item or fighter draws whole at its link's place: its head-1 lists do not wait for the stage's head-1 lists of that pass. Only translucent lists are reordered among themselves, on Zebes, Sector Z and Saffron City | RE-422 |
| Remaining effects | The fighters' effects, the display hit effects and set-offs (RE-415), the weapons' own effects, weapon clashes and the quake's camera shake (RE-416) are made. Yoshi's roll egg, explosion and shield-release shells now draw (RE-427). Not ported: the Thunder Jolt's ground effect and PK Thunder's trail effects (one struct each, no draws), drawing the fire spark; Kirby's copy-bank scripts (0x4C, 0x4D); the results scene's fighters' effects; the stages' particle banks. A magnitude-0 quake shakes one frame late (the camera updates before the effect processes) | RE-413, RE-415, RE-416, RE-427 |
| Stage wallpaper and stage-select leftovers | Every battle draws its wallpaper and the stage select draws `mnMaps` in full (RE-419). Left: the Race to the Finish fill and Final Destination's wallpaper swap (no such stage in `psp-game`); trimming the twelve 5551 wallpapers from their 512 × 256 padding (about 1.5 MiB, `P5`) | RE-419 |
| Acid tile-1 sprite variants | The acid script's `TextureIDNext` reaches sprites 1–3, but its primitive has no two-tile blend, so only sprite 0 draws. Pack v46 still converts all four 384×384 variants (523 KB unreachable). Converting only sprites a primitive can sample would change every stage's shared `MatAnimDesc` conversion | RE-364 |
| Bonus stages and stage items; Arwing checks | `grbonus3.c` remains. All four normal containers and Chansey's maker are ported (RE-431, RE-432), with real contents and the appearance actor (RE-433). Saffron's five Pokémon, flame/razor weapons and rendering are ported (RE-430). Castle's Bumper, the POW and both Piranha Plants are ported (RE-429); pipe traversal still must notify a plant, and the Bumper's lit hit flash and an N64 plant-knockout comparison remain. The cloud vapor particle and the scale sparkle are not ported. Sector Z's Arwing flies, fires and lands its wing (RE-428); its lasers' look, the 3D patterns' facing, the pilot manoeuvres and the wing collision are not yet compared with the N64, and Board the Platforms' moving blocks, which now follow their `TraI` paths, are not either | RE-356, RE-365, RE-428, RE-429, RE-430, RE-431, RE-433 |
| Hit-status preservation on map switches | `FTSTATUS_PRESERVE_HITSTATUS` is honoured by Pikachu's special switches (RE-414) and a few common statuses; the other fighters' special map switches still reset the hit status, and Kirby's and Yoshi's captures and Yoshi's throw hold keep intangibility implicitly, so they start no hit-status colour animation | RE-414 |
| Twister and Barrel Cannon clips | Both statuses keep the previous pose; `nFTCommonMotionTwister` needs a shared slot, and TaruCann has none (`-1`) | RE-356 |
| Yoshi Egg Lay victim collision and effect | Laying omits the wall/ceiling sweep, the damaging-floor escape is not wired to the ported ground hits (RE-356), and the break effect is represented by a 10-frame clock | RE-337 |
| Save data and unlocks | No save data: Luigi, Captain Falcon, Ness, Jigglypuff and Mushroom Kingdom stay locked on the selects, which draw their shadows. Both character selects draw their sprites and fighters (RE-411), with the accessories (RE-425); neither draws the spotlight under a held puck's fighter, and the VS one still leaves out the CPU's colour animation the Training one now draws | RE-385, RE-386, RE-411 |
| Item presentation and original-game comparisons | Capsule/Egg (RE-431), Crate/Barrel with the heavy pickup, lift and throw statuses (RE-432), the Tomato, Heart and Star with the appearance actor and container contents (RE-433), and the Motion-Sensor Bomb, Bob-omb, Bumper, both Shells and the Poké Ball's throw and open (RE-434) are ported. All thirteen Poké Ball Pokémon and their weapons are ported (RE-435). The Sword, Bat, Fan, Star Rod, Ray Gun, Fire Flower and Hammer lifecycles, swing/shoot/Hammer callbacks, ammo and projectiles are ported (RE-436), completing all 20 normal makers. Remaining: Pokémon status materials/translucency, display link 18, rock texture IDs, FlySparks head-0 state, event-aligned N64 damage/hit-order/trajectory comparisons and observations of the other seven Pokémon remain; the Poké Ball's rays and open animation, the Shells' spin animation and material frames, the Bob-omb's walk display lists, the Bumper's lit palette and flat model, item colour animations (Hammer's warning request is recorded but its flash is not drawn); item and Star music, sound, the item arrow, the Star's material animation, the item-destroy dust, light throw-turn joint yaw and pickup arrows remain. Items made inside the item link run their first main process a frame late. Remaining non-container items, outside the seven held utilities, do not yet take descriptor 1 as their root (`itManagerMakeItem` ejects descriptor 0); RE-383's held Bomb reads `->child` as node 1 | RE-352, RE-383, RE-431, RE-432, RE-433, RE-434, RE-435, RE-436 |
| Yoshi double-jump hang | Without another action Yoshi stays at his `JumpAerialF` apex indefinitely (y 1862 above Dream Land's left platform in `yoshiegg`); predates RE-432 | RE-432 |
| Yoshi tongue and grab N64 comparisons | Joint 31 (the tongue tip) and every Catch/Throw item-heavy joint now animate (RE-432); the tongue mesh is not drawn and neither the grab nor Egg Lay was compared with the N64 | RE-417, RE-432 |
| Thrown-fighter hits | The motion interpreter skips `SetDamageThrown`, so a thrown fighter makes no attacks and `ftCommonThrownProcStatus`'s throw pointer is not kept. Weapon clashes are ported (RE-416); item–weapon clashes are ported with the light containers (RE-431) | RE-407, RE-415, RE-416, RE-431 |

## Hardware acceptance

Deferred by user instruction.

| Item | Reason deferred | Evidence |
|---|---|---|
| PSP-1000 support | MEMSIZE=1 is ignored; the full pack requires 64 MiB mode (see docs/memory.md). Design a reduced or per-scene pack | RE-288, RE-426 |
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
