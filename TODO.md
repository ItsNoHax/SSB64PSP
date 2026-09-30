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
| Draw Kirby's stars and copied specials' effects in `psp-game` | Kirby draws in Training with his Final Cutter wave (RE-378) and his copy hats (RE-417); the Inhale/spit stars and the copied Falcon Punch flame (joint 30) are not drawn, and motion-script model-part events (`ftParamSetModelPartID`) are not ported | RE-343, RE-376, RE-378, RE-417 |
| Draw Yoshi's egg/star hit effects in `psp-game` | Yoshi draws in Training with his Egg Throw egg and Bomb stars (RE-375), the Egg Lay egg (RE-417) and his egg shield (RE-418); the shatter and egg explosion remain, and dropping the egg shield makes no egg-break particles (`ftCommonGuardUpdateShieldVars`' `efManagerEggBreakMakeEffect` is not ported) | RE-337, RE-375, RE-417, RE-418 |
| Reach the PK Fire flame in a `psp-game` capture | The flame draws (RE-382) but no capture scene lands a spark on the dummy; the Bomb's critical colour flash and held bloat are not drawn (RE-383) | RE-352, RE-382, RE-383 |
| Keep `run` inside MIPS branch range | `psp-game`'s release `run` is 20,464 bytes after the Training select (its frame 18,176), far inside the ±128 KB branch reach since the screens' logic and draws went out of line (RE-389 onwards); watch it when code moves back into `run`, since growth past the reach brings back `out of range PC16 fixup` | RE-382, RE-385, RE-386, RE-389, RE-419 |
| Window material-animation blobs | A material script packs its whole source file: file 335 (50 KB) came in for PK Thunder's texture blink (RE-381), as weapon animations did before RE-378's window | RE-378, RE-381 |
| Draw Pikachu's Thunder Jolt and Quick Attack effects in `psp-game` | Pikachu draws in Training with both Thunder Jolt forms (RE-379) and Thunder (RE-417); ground-jolt node 4's texture script is declined by `resolve_one_mat_anim` | RE-345, RE-379, RE-417 |
| Kirby Inhale downward wiggle | The captured victim's downward mash/drop-through path needs its source floor flags and ignored-line linkage; Final Cutter cliff catches and star reflections now use the shared map solver | RE-343, RE-348 |
| CPU AI: items, modes and view | The VS CPU fights with `ftComputerProcDefault`, the attack tables and the Attack/Unknown1/Ally/Patrol objectives (RE-391). The item objectives (TrackItem, UseItem), the 1P/team traits and Rush, the Training CPU menu and the weapon/item/Twister/acid reports from `psp-game` remain | RE-391 |
| Fighter entry effects | The pipe, barrel, capsule, Link's wave and beam, egg and leftward star draw (RE-403). Remaining: Fox's Arwing, the Poké Ball and its rays, Captain Falcon's car (each with its own update), Luigi's pipe (his own file) and Kirby's rightward star table (not packed), the entry camera mode and the player tag | RE-402, RE-403 |
| VS battle presentation and players | The VS battle runs (RE-389), and its CPU fights (RE-391), the VS mode menu sets the rule, time and stocks (RE-399) `mnPlayersVS` picks the players (RE-404) and draws its sprites and fighters (RE-411; the spotlight, the locked shadows' noise and the CPU colour animation are not drawn) and up to four fight (RE-405, free-for-all, no handicap; Magnet, Sing, the Spin Attack swirl, Falcon Punch/Kick, the reflector, Charge Shot and the held egg draw for the player only), and team battles run (RE-407), but there is no VS Options, HUD pieces beyond the battle HUD (the damage display, emblems, countdown, timer, stock icons and announcements draw, RE-392–RE-397) or the results' wipe (the wallpaper, tags, text and table draw, RE-410, and the series emblem and confetti, RE-420; the fighters stand, face and play their Win and Lose clips, RE-409, but draw opaque rather than fading in), and three entry effects are not drawn (RE-403). At results tic 457 Kirby's Win pose differs from an N64 capture (mouth open there, closed in `f1-vs-results`); not investigated (RE-420) | RE-389, RE-404, RE-405, RE-409, RE-410, RE-411, RE-420 |
| KO presentation | The blast, screen flash, rebirth halo, rebirth glow, star KO fade (RE-412), the blast's particle streaks and the star sparkle (RE-413) are drawn; the halo's rays, the KO's quake and a star KO past 10,000 units draw (RE-420). The battle projection keeps the GE near plane at 1, not `dGMCameraPerspDefault`'s 256: with 256 the fighter draws over its shield bubble, which the N64 capture covers; why the N64 covers it is not established (RE-420). The camera's weapon interests are not ported (RE-406) | RE-388, RE-406, RE-412, RE-413, RE-420 |
| Remaining effects | The fighters' effects, the display hit effects and set-offs (RE-415), the weapons' own effects, weapon clashes and the quake's camera shake (RE-416) are made. Not ported: Yoshi's egg explosion (his particle bank is not packed), the Thunder Jolt's ground effect and PK Thunder's trail effects (one struct each, no draws), drawing the fire spark; Kirby's copy-bank scripts (0x4C, 0x4D), Donkey Kong's crate pieces, Yoshi's roll egg (`efManagerYoshiEggEscapeMakeEffect`, which hides him); the results scene's fighters' effects; the stages' particle banks. A magnitude-0 quake shakes one frame late (the camera updates before the effect processes) | RE-413, RE-415, RE-416 |
| Stage wallpaper and stage-select leftovers | Every battle draws its wallpaper and the stage select draws `mnMaps` in full (RE-419). Left: the Race to the Finish fill and Final Destination's wallpaper swap (no such stage in `psp-game`); trimming the twelve 5551 wallpapers from their 512 × 256 padding (about 1.5 MiB, `P5`); Yoshi's Island draws grey platforms and the smiling-heart centre where the N64 select shows striped platforms and fruit, in battle too (the stage draw, not the wallpaper) | RE-419 |
| Acid tile-1 sprite variants | The acid script's `TextureIDNext` reaches sprites 1–3, but its primitive has no two-tile blend, so only sprite 0 draws. Pack v46 still converts all four 384×384 variants (523 KB unreachable). Converting only sprites a primitive can sample would change every stage's shared `MatAnimDesc` conversion | RE-364 |
| Sector Z Arwing, bonus stages and stage items | The Arwing (weapons, flight patterns), `grbonus3.c`, and the Bumper, POW Block, Piranha Plant and Pokémon items remain; controllers already call `StageObjects::make_item`, and the Castle ground root the Bumper rides now moves (RE-365). The cloud vapor particle and the scale sparkle are not ported | RE-356, RE-365 |
| Hit-status preservation on map switches | `FTSTATUS_PRESERVE_HITSTATUS` is honoured by Pikachu's special switches (RE-414) and a few common statuses; the other fighters' special map switches still reset the hit status, and Kirby's and Yoshi's captures and Yoshi's throw hold keep intangibility implicitly, so they start no hit-status colour animation | RE-414 |
| Twister and Barrel Cannon clips | Both statuses keep the previous pose; `nFTCommonMotionTwister` needs a shared slot, and TaruCann has none (`-1`) | RE-356 |
| Yoshi Egg Lay victim collision and effect | Laying omits the wall/ceiling sweep, the damaging-floor escape is not wired to the ported ground hits (RE-356), and the break effect is represented by a 10-frame clock | RE-337 |
| Save data and unlocks | No save data: Luigi, Captain Falcon, Ness, Jigglypuff and Mushroom Kingdom stay locked on the selects, which draw their shadows. Both character selects draw their sprites and fighters (RE-411); neither draws the spotlight under a held puck's fighter, and the VS one still leaves out the CPU's colour animation the Training one now draws | RE-385, RE-386, RE-411 |
| Headgear accessories | `FTAttributes.accesspart` (Pikachu's hat, Jigglypuff's bow), which `ftManagerMakeFighter` and `ftParamInitAllParts` attach for any costume but 0, is neither packed nor drawn, so those costumes show without it on the selects and in battle (seen on the Training select's Pikachu) | — |
| Other item kinds and item presentation | Bomb and PK Fire use the shared item system; heavy/swing/shoot/consume items, item models, effects, sound, spin, throw-turn joint yaw and pickup arrows remain | RE-352 |
| Thrown-fighter hits | The motion interpreter skips `SetDamageThrown`, so a thrown fighter makes no attacks and `ftCommonThrownProcStatus`'s throw pointer is not kept. Weapon clashes are ported (RE-416); the item-against-weapon clash (`itprocess.c:931`) waits for an item with `can_setoff` | RE-407, RE-415, RE-416 |

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
