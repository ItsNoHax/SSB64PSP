# Plan

SSB64PSP is a native Rust source port of Super Smash Bros. (N64, US) to the
PSP. Gameplay is translated from [`ssb-decomp-re`][decomp] in large subsystem
batches; N64 display lists are converted to PSP GE geometry at build time.
The target is the whole original game — every mode, menu and fighter, with
audio — running at 60 FPS on a PSP-2000 or later, matching the N64 frame for
frame wherever the PSP allows.

This is the only roadmap and task list. Current work is in
[`STATUS.md`](STATUS.md), permanent decisions in [`DECISIONS.md`](DECISIONS.md),
how the system works in [`docs/architecture.md`](docs/architecture.md), and the
evidence behind each `RE-NNN` in [`docs/evidence/`](docs/evidence/INDEX.md).
"Verified" means PPSSPP unless a physical PSP is named.

## Project State

| Area | Current | Target |
|---|---|---|
| Architecture | Portable `ssb-rom`/`ssb-engine`/`ssb-game`, shared `psp-runtime`, thin `psp-game`, separate `psp-asset-viewer` ([D-044](docs/decisions/D-044.md)) | Same; runtime loader on `ssb_rom::reloc_link` and `ssb_engine::memory` arenas |
| Asset pipeline | Tables from the ROM's code segments generated at build time by `ssb-tablegen` ([D-048](docs/decisions/D-048.md)); `romtool` builds pack v107 from the user's ROM: all 2,132 archive files, every reachable model, texture, animation, sprite and table; scenes load their own files ([D-046](docs/decisions/D-046.md)) | Same, plus converted audio; trimmed wallpaper padding |
| Rendering | Build-time display-list conversion, GE lowering with measured deviations (3-point filtering); battle draws in the original's display-link passes; CRT overscan crop ([D-047](docs/decisions/D-047.md)) | Remaining effect draws and N64 comparisons; per-scene texture residency in VRAM |
| Gameplay | All 12 fighters, Master Hand, Metal Mario, Giant Donkey Kong and the Polygon Team; hits, shields, grabs, ledges, KO; How to Play matches an N64 trace on every frame (RE-472, RE-473) | Remaining fidelity items below |
| Items | All 20 normal item makers, 13 Poké Ball Pokémon, stage items and hazards (RE-431–RE-446) | Event-aligned N64 comparisons; item audio |
| CPU | VS, Training and 1P CPU behaviours, traits and objectives (RE-390, RE-391, RE-437, RE-438, RE-447) | CPUs' special effects drawn; N64 trait checks |
| Game modes | VS (up to four, teams), Training, 1P campaign through the ending, staff roll and challengers, bonus stages and Bonus Practice | Same, with audio and rumble |
| Menus/UI | N64 logo, opening movie, title, attract loop (How to Play, Characters, auto demo), mode/1P/VS menus, selects, options and data menus, HUD | Sound Test; selects' spotlight; opening's remaining differences |
| Save data | N64 SRAM image in `ssb64.sav` beside the pack ([D-045](docs/decisions/D-045.md)) | Anti-piracy validators |
| Audio | Not started (engine traits only) | Build-time sequence/VADPCM conversion, `sceAudio` mixer thread |
| PSP performance | 60 FPS under PPSSPP (RE-470, RE-471); on a PSP-2000 three opening scenes run at 45–57 FPS (RE-476) | 60 FPS on a PSP-2000 in every scene |
| Platform | PSP-2000 and later (64 MiB); PSP-1000 unsupported (RE-288, RE-475) | Same; PSP-1000 support deferred by user instruction |
| Testing | Host tests (CI, no ROM, stub tables), ROM-backed tests, 198 PPSSPPHeadless goldens committed as pixel hashes ([visual regression](docs/visual-regression/README.md)), N64 RDRAM traces | Hardware acceptance matrix on a PSP |

## Milestones

| ID | Area | State |
|---|---|---|
| `MS1` | Architecture, toolchain, CI | Done |
| `MS2` | Asset pipeline and pack | Done |
| `MS3` | Renderer fidelity | Done; current goldens not re-captured on a PSP |
| `MS4` | Fighters, including Master Hand and the special fighters | Done; fidelity remainders below |
| `MS5` | Match systems: VS, Training, stages, items, CPU, KO | In progress |
| `MS6` | Front end, 1P campaign, save data | In progress |
| `MS7` | Audio and rumble | Planned |
| `MS8` | PSP performance and hardware acceptance | In progress; never gates `MS4`–`MS7` ([D-032](docs/decisions/D-032.md), [D-036](docs/decisions/D-036.md)) |

Earlier labels (`P0`–`P5`, `M0`–`M4`, `R0`–`R3`, `F1`, `G0`–`G5`) are retired;
code comments and evidence that cite them resolve through
[`docs/evidence/INDEX.md`](docs/evidence/INDEX.md#retired-labels).

## Remaining Work

Each item cites the evidence that describes it. Delete an item when a batch
closes it.

### PSP performance and hardware (`MS8`)

- [ ] Run the RE-478 production EBOOT (`target/release-re478/EBOOT.PBP`) on the PSP-2000: the campaign past Race to the Finish, then RE-476/RE-477's checks (RE-476, RE-477, RE-478).
- [ ] Measure D-047's GE fill cost on a PSP-2000: the battle viewport covers 19% more pixels; profile `vs4@4000`, the opening and the selects (`ge` span) (RE-477).
- [ ] Opening CPU cost on a PSP-2000 (~3.5× PPSSPP): Run, Yoster/Sector and Clash at 45–57 FPS with 12–14 ms of CPU, mostly the movie draw's mesh lists; each opening fight's first frame takes 21–40 ms. Next: expanded vertices through the cache into a line-aligned, written-back block, then the battle's first frame (RE-469, RE-470, RE-471, RE-476).
- [ ] Measure RE-469's other scenes and RE-471's worst cases on the PSP; `vs4@4000` averages 5.2 ms against RE-471's 4.5, unexplained (RE-471, RE-476).
- [ ] Read ahead for scenes outside the attract loop (menus, selects, VS, 1P): they still load in their first frame, 0.1–0.6 s on a PSP-2000; a select could read its stage's and fighters' files once picked (RE-475, RE-476).
- [ ] Add the few on-demand files to scene lists (select/results figatrees, Characters' figatree, a Kirby copy's absent kind; `demand=` in capture logs) (RE-475, RE-476).
- [ ] Per-scene texture residency: v78 Dream Land with four fighters needs 726,924 bytes low detail against a 704,512-byte pool, and since RE-476 only 180,224 bytes of VRAM are free; the runtime samples from main RAM (RE-076, RE-077, RE-341, RE-426).
- [ ] Trim the twelve 5551 wallpapers from their 512 × 256 padding (~1.5 MiB) (RE-419).
- [ ] State batching where profiling shows GE submission cost: `sf64-psp` hashes material state into a batch pool and `oot-PSP` caches sampler state (RE-124, [D-036](docs/decisions/D-036.md)).
- [ ] VFPU math only after profiling; `ssb_engine::math::sqrt`'s four divisions are a candidate (a `sqrt.s` changes PSP pixels) ([D-032](docs/decisions/D-032.md)).
- [ ] VFPU for render-only matrix work: joint world matrices for drawing, skinned-mesh vertex expansion and the movie draw's mesh lists (one `vmmul.q` replaces ~112 scalar instructions per 4×4 product; sf64-psp measured 8.8× on matrices, 2.5× on a scene). Gameplay joint sampling, collision and hitboxes stay on the scalar FPU so How to Play keeps matching the N64; goldens decide whether a render path may change precision.
- [ ] VFPU 16-byte block copies (`lv.q`/`sv.q`) for large 16-byte-aligned transfers (vertex expansion, frozen-picture and photo copies), measured against `ssb_engine::memops` (RE-471).
- [ ] Overlap CPU and GE: build frame N+1's display list while the GE draws frame N (double-buffered lists, PSPSDK sample). Costs one frame of display latency, never a game tick; measure GE time first ([D-047](docs/decisions/D-047.md)'s +19% fill, RE-469).
- [ ] Keep geometry non-indexed: audit `meshdraw` for indexed draws or transformed-vertex reuse; on the PSP rebatching non-indexed vertices beat reuse (sf64-psp: −39% VFPU instructions but +4.6 ms; rebatching won back 1.2 ms).
- [ ] Instruction-cache locality (16 KB I + 16 KB D): profile hot functions with PSPLink's `profmode t` hardware counters and `psp-gprof`, then group the hottest code (link-section ordering); compare `opt-level` 3 against `s` on the PSP rather than assuming 3 (GBAdhoc measured 7–25%).
- [ ] Texture bandwidth: every GE texture is swizzled already (`psp_texture`); measure whether CLUT4/CLUT8 for textures that fit their palette cuts main-memory bandwidth enough to matter. Earlier CLUT work was a red herring for RE-300's alpha bug, not a bandwidth measurement.
- [ ] Media Engine for audio mixing, POPS-style: the mixer runs on the ME with the Allegrex posting commands through shared uncached memory. PPSSPP doesn't run custom ME code, so keep an Allegrex-thread fallback and select it at runtime (audio batch decision).
- [ ] FPU traps outside PSPLink: check whether an XMB-started game traps divide-by-zero as PSPLink does; keep `math::div_nonzero` on speculated divisions (RE-201, RE-469).
- [ ] Large stack frames: battle entry peaks at 239–294 KB of the 512 KiB stack (`enter_training`, `EffectVisuals::sync_ko`) (RE-469).
- [ ] Keep `run` inside MIPS branch range: 22,884 code bytes against a 128 KiB reach (RE-431).
- [ ] Replace the debug HUD's `sceGuDebugFlush` (software-rasterizer-only in PPSSPP, faults on a PSP) with GE geometry (RE-014, RE-202).
- [ ] Hardware acceptance: a 30-minute run on a second unit (RE-273, RE-284); re-capture current goldens on a PSP (RE-320, RE-326, RE-341); hand-input checks of shield, grab, throws and costume picks (RE-339, RE-341).
- [ ] PSP-1000 (deferred by user instruction): `MEMSIZE=1` is ignored; RE-475 measures the heaviest scenes against a PSP-1000-sized budget under PPSSPP; the VS results do not fit (RE-288, RE-475).
- [ ] Measure the nub deadzone on hardware (20 units with a linear rescale to ±80 is unmeasured) (RE-009).
- [ ] Trace how `sGCDetailLevel` is chosen (RE-011).

### Audio and rumble (`MS7`)

- [ ] Audio backend: build-time conversion of the 47 sequences and two VADPCM banks, a `sceAudio` mixer thread, FGM sound effects ([architecture](docs/architecture.md#audio)).
- [ ] Wire the ported request points: menu and Training sounds, BGM volume, magnifier sounds, results BGM and voices, item and Star music, scene audio (RE-438, RE-439, RE-440).
- [ ] Rumble.

### Match systems (`MS5`)

- [ ] Draw the CPUs' special effects: Magnet, Sing, Spin Attack swirl, Falcon Punch/Kick, reflector, Charge Shot and the held egg draw for the player only (RE-389, RE-404, RE-405).
- [ ] Remaining effects: Thunder Jolt ground effect, PK Thunder trail effects, the fire spark, Kirby's copy-bank scripts (0x4C, 0x4D), the results' fighter effects, stage particle banks; a magnitude-0 quake shakes one frame late (RE-413, RE-415, RE-416, RE-427).
- [ ] Kirby: Inhale/spit stars and the copied Falcon Punch flame; Inhale's downward wiggle (RE-343, RE-348, RE-376, RE-378, RE-417).
- [ ] Pikachu: ground Thunder Jolt node 4's texture script is declined by `resolve_one_mat_anim`; Quick Attack effects (RE-345, RE-379).
- [ ] Reach the PK Fire flame on the dummy in a capture; draw the Bomb's critical flash and held bloat (RE-352, RE-382, RE-383).
- [ ] Battle draw order: an object's head-1 lists do not wait for the stage's head-1 lists of the same pass (RE-422).
- [ ] Camera: weapon interests and special camera modes (RE-406).
- [ ] Hit-status preservation on the other fighters' special map switches; capture/hold intangibility starts no colour animation (RE-414).
- [ ] Twister and Barrel Cannon clips: `nFTCommonMotionTwister` needs a shared slot; TaruCann has none (RE-356).
- [ ] Yoshi Egg Lay victim: wall/ceiling sweep, damaging-floor escape, break effect (RE-337); re-observe the double-jump hang (RE-432, RE-466); compare the tongue, grab and Egg Lay with an N64 view (RE-417, RE-432, RE-468).
- [ ] Setter first-frame audit remainder without an N64 trace: Barrel/Tornado captures, item throws, Final Cutter landing, Yoshi aerial release, Link's aerial Boomerang return (RE-468, RE-473, RE-474).
- [ ] `anim_frame` remainder: the shield pose's clock, Donkey Kong's speed-0 cargo jump/fall, the idle camera's `status_total_tics` (RE-472, RE-474).
- [ ] Fighter entry: Fox in the cockpit, the Poké Ball's near-camera edge at tick 224, Luigi's pipe and Kirby's star against the N64 (RE-402, RE-403, RE-425).
- [ ] Items: event-aligned N64 damage, hit-order and trajectory comparisons; the remaining Pokémon and Clefairy selections; Bumper lit palette, Star flicker and bomb flashes against the N64; lit item primitives without authored light colours (RE-431–RE-444).
- [ ] Stages: compare pipes, plant notification, cloud vapor, scale sparkles, a plant knockout, Sector Z's Arwing and Board the Platforms' blocks with N64 captures (RE-428, RE-429, RE-442, RE-446, RE-455).
- [ ] Acid tile-1 sprite variants: only sprite 0 draws; the pack converts four 384 × 384 variants (RE-364).
- [ ] Window material-animation blobs: a script packs its whole source file (RE-378, RE-381); material-animation command 22 is rejected and can be skipped (RE-010).
- [ ] Results: an N64 view of the wipe (RE-464).
- [ ] CPU: an N64 trace of a CPU tracking an item; N64 check of the VS entry focus and 1P traits (RE-437, RE-447, RE-450).
- [ ] `WPAttributes` pairing: revisit if another instance like Link's boomerang appears (RE-058).

### Front end and 1P (`MS6`)

- [ ] Sound Test.
- [ ] Both character selects' spotlight under a held puck; the VS select's CPU colour animation; locked shadows' noise (RE-411).
- [ ] Opening movie: the room's falling-figure pose, spotlight cone and close-up overlay shade; Fox's laser under the rolled camera; the standoff ground's dropped triangles; Yoster clouds' fringes and tint; the run's dust puff; N64 references from an accurate video plugin (RE-467, RE-468).
- [ ] Characters' fighter runs no motion-script events (effects, colour animations, part changes) (RE-461).
- [ ] Anti-piracy validators that set `error_flags` (RE-460, RE-461).
- [ ] 1P: ending room's background material animation; N64 comparisons of the ending, staff roll and challenger; 1P CP/heart tags; real loss/win handoffs observed (RE-459).
- [ ] 1P bonus stages: N64 pause capture and PSP comparison (RE-456).
- [ ] Master Hand: `is_use_fogcolor` darkening, wallpaper effects' material animations, comet ENV tint, the defeat zoom's struck joint, the boss stock snap, a hit-by-player capture; magnifiers should hide before "Go" in every battle (RE-457).
- [ ] Special fighters: the intro's Polygons draw darker and Metal Mario's reflection duller than the N64; a full 30-Polygon clear and DK Defender/Perfect bonuses unobserved (RE-458).
- [ ] Reconcile the 1P select controllers (`players_1p` against `spgame::select`) (RE-448, RE-450).
- [ ] 1P shade: Yoshi's same-costume shade (RE-450, RE-453).
- [ ] The Item Switch has no N64 reference (locked on a new save) (RE-462).

### Architecture

- [ ] Rewrite Git history to drop the golden PNGs, the old documentation images and the five generated tables from earlier commits (the user's step; path list in `target/history-purge-paths.txt`, outside Git) ([D-048](docs/decisions/D-048.md)).

- [ ] Use `ssb_rom::reloc_link` from a runtime loader; its layout matches the original's (RE-340, RE-341, [D-011](docs/decisions/D-011.md)).
- [ ] Wire `ssb_engine::memory` arenas and pools into `psp-runtime` ([memory](docs/memory.md#allocators)).
- [ ] Independent ROM-derived check of fighter costume and material animation, as stages have (RE-050–RE-052, RE-142).

## Completed

- [x] Architecture: portable crates, shared `psp-runtime`, `psp-game`, `psp-asset-viewer`; pinned toolchains; CI with no ROM ([D-026](docs/decisions/D-026.md), [D-030](docs/decisions/D-030.md), [D-044](docs/decisions/D-044.md)).
- [x] ROM validation, VPK0 (499 files), relocData (2,132 files, 61,343 intern and 3,092 extern relocations), F3DEX2 parsing (1,864 lists, 0 failures) (RE-001, RE-002, RE-017).
- [x] Pack: meshes, textures (RGBA, IA, I, CI with palette banks), material and joint animations, figatrees, sprites, particles, collision, stage and fighter tables; per-scene archive files (pack v107, [D-046](docs/decisions/D-046.md), RE-475).
- [x] Renderer: geometry, projection, textures and addressing, 3-point filter compensation, combiner and `MObj` state, lighting, texgen, alpha/blend/depth, billboards, shadows, framebuffer effects and wipes, UI sprites, GE state cache ([docs/rendering.md](docs/rendering.md)).
- [x] Physical PSP rendering validation on PSP-2000 and PSP-3000, including two 10-minute runs and one 30-minute run (RE-270, RE-271, RE-273, RE-284).
- [x] Fighters: Mario, Fox, Donkey Kong, Samus, Luigi, Link, Yoshi, Captain Falcon, Kirby (with all copies), Pikachu, Jigglypuff, Ness; Master Hand; Metal Mario, Giant Donkey Kong, the Polygon Team (RE-344–RE-347, RE-457, RE-458).
- [x] Fighter systems: physics, collision, status machine, figatree animation, motion scripts, hit resolution, shields, grabs and throws, ledges, recovery, KO and rebirth, colour animations (RE-330–RE-355, RE-366–RE-371, RE-388, RE-412–RE-414).
- [x] How to Play and the opening's battles match N64 RDRAM traces frame for frame (RE-466, RE-468, RE-472, RE-473, RE-474).
- [x] Weapons and items: every fighter's weapons, all 20 normal items, Poké Ball Pokémon, stage items and hazards (RE-354, RE-372–RE-381, RE-416, RE-430–RE-446).
- [x] Stages: all 41 maps, nine VS stages with controllers and hazards, wallpapers, Sector Z's Arwing, bonus stages (RE-385, RE-419, RE-428, RE-429, RE-446, RE-452–RE-455).
- [x] CPU AI: input scripts, objectives, traits, item use (RE-390, RE-391, RE-437, RE-438).
- [x] VS mode: rules, countdown, HUD, pause, teams, four fighters, results (RE-389–RE-411, RE-420, RE-464).
- [x] Training mode with its menu and stats (RE-438, RE-439, RE-440).
- [x] 1P campaign: every stage, bonus stages, Master Hand, the ending, staff roll, congratulations, challengers and unlocks; a run through every stage transition (RE-447–RE-459, RE-478).
- [x] Save data: `LBBackupData` in `ssb64.sav`, options and data menus ([D-045](docs/decisions/D-045.md), RE-460, RE-461).
- [x] Front end: N64 logo, opening movie, title, attract loop with How to Play, Characters and the auto demo; mode, 1P and VS menus; Bonus Practice (RE-462, RE-465, RE-467).
- [x] Per-scene loading and background reads: no load hitch in the intro on a PSP-2000 (RE-475, RE-476).
- [x] CRT overscan crop: the N64's visible box fills the PSP's height ([D-047](docs/decisions/D-047.md), RE-477).
- [x] CPU performance: 60 FPS under PPSSPP (RE-469, RE-470, RE-471).
- [x] Nothing read from the ROM is committed: motion, colour-animation and CPU scripts, status flags and the animation table are generated at build time from the user's ROM; goldens are pixel hashes ([D-048](docs/decisions/D-048.md)).

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
