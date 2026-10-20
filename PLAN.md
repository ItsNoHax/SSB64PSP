# Plan

- Target: whole original game (every mode, menu, fighter, audio), 60 FPS,
  PSP-2000 or later; N64 frame-for-frame wherever the PSP allows.
- Gameplay: translated from [`ssb-decomp-re`][decomp]. Display lists → GE
  geometry at build time.
- Scope: only roadmap and task list. Current figures and work:
  [`STATUS.md`](STATUS.md). Rules: [`AGENTS.md`](AGENTS.md#documentation).
- "Verified" = PPSSPP unless a physical PSP is named.

## Project State

| Area | Current | Target |
|---|---|---|
| Architecture | Portable `ssb-rom`/`ssb-engine`/`ssb-game`, shared `psp-runtime`, thin `psp-game`, separate `psp-asset-viewer` ([D-044](docs/decisions/D-044.md)) | Runtime loader on `ssb_rom::reloc_link` and `ssb_engine::memory` arenas |
| Asset pipeline | ROM tables generated at build time by `ssb-tablegen` ([D-048](docs/decisions/D-048.md)); `romtool` packs all 2,132 archive files, every reachable asset and the audio section; scenes load their own files ([D-046](docs/decisions/D-046.md)) | Trimmed wallpaper padding |
| Rendering | Build-time display-list conversion; GE lowering with measured deviations; original display-link passes; CRT overscan crop ([D-047](docs/decisions/D-047.md)) | Remaining effect draws and N64 comparisons |
| Gameplay | 12 fighters, Master Hand, Metal Mario, Giant DK, Polygon Team; How to Play matches an N64 trace every frame (RE-472, RE-473) | Fidelity items below |
| Items | 20 normal items, 13 Poké Ball Pokémon, stage items and hazards (RE-431–RE-446) | Event-aligned N64 comparisons |
| CPU | VS, Training and 1P behaviours, traits, objectives (RE-390, RE-391, RE-437, RE-438, RE-447) | N64 trait checks |
| Game modes | VS (four players, teams), Training, 1P to the ending, staff roll, challengers, bonus stages, Bonus Practice | Rumble |
| Menus/UI | N64 logo, opening, title, attract loop, every menu and select, HUD | opening differences |
| Save data | N64 SRAM image in `ssb64.sav` ([D-045](docs/decisions/D-045.md)) | Anti-piracy validators |
| Audio | N64 audio system ported whole, sample-exact ([D-049](docs/decisions/D-049.md), RE-479) | Within the PSP-2000 CPU budget everywhere |
| PSP performance | 60 FPS under PPSSPP; PSP-2000 figures in STATUS | 60 FPS on a PSP-2000 in every scene |
| Platform | PSP-2000 and later (64 MiB); PSP-1000 unsupported (RE-288, RE-475) | Same (PSP-1000 deferred by the user) |
| Testing | Host tests (CI: no ROM, stub tables), ROM tests, PPSSPPHeadless goldens as pixel hashes ([visual regression](docs/visual-regression/README.md)), N64 RDRAM traces | Hardware acceptance matrix |

## Milestones

| ID | Area | State |
|---|---|---|
| `MS1` | Architecture, toolchain, CI | Done |
| `MS2` | Asset pipeline and pack | Done |
| `MS3` | Renderer fidelity | Done; goldens not re-captured on a PSP |
| `MS4` | Fighters, Master Hand, special fighters | Done; fidelity remainders below |
| `MS5` | Match systems: VS, Training, stages, items, CPU, KO | In progress |
| `MS6` | Front end, 1P campaign, save data | In progress |
| `MS7` | Audio and rumble | Audio done; rumble open |
| `MS8` | PSP performance and hardware acceptance | In progress; never gates `MS4`–`MS7` ([D-032](docs/decisions/D-032.md), [D-036](docs/decisions/D-036.md)) |

- Retired labels (`P0`–`P5`, `M0`–`M4`, `R0`–`R3`, `F1`, `G0`–`G5`) →
  [`docs/evidence/INDEX.md`](docs/evidence/INDEX.md#retired-labels).

## Remaining Work

- Row = one open task: what remains / acceptance, then refs. Closed → delete row.


### PSP performance and hardware (`MS8`)

- Figures: STATUS (PSP-2000).
- Link orders: searched with `tools/icache/`, chosen on hardware by busy time
  (`tools/hw-link-order.sh`). Hot code changes size → re-search
  (RE-494–RE-500, RE-539).

| Task | Remaining / acceptance | Refs |
|---|---|---|
| 1P intro | Kirby vs Yoshi team ~21 FPS on PSP-2000 → profile | RE-537, RE-539 |
| `vs4@2400` | Busy time back under RE-500's 13.64 ms; rise = small gameplay and draw additions | RE-533–RE-539 |
| Opening | Profile Clash (link order no effect), each fight's first frame (21–40 ms). Levers: `bind_texture` (~121/frame, ~11 µs each), lit-expand traffic (~1.2 µs/vertex) | RE-469–RE-471, RE-476, RE-532, RE-539 |
| Other scenes | RE-469 scenes, RE-471 worst cases on PSP; explain `vs4@4000` 5.2 ms vs PPSSPP 4.5. Memory stalls 55% of cycles; draw I-cache misses dominate | RE-480, RE-481 |
| D-047 fill | Measure +19% GE fill on PSP-2000: `vs4@4000`, opening, selects (`ge` span) | RE-477 |
| Texture residency | Cost of remaining main-RAM texel reads after photo-region pool (`texture_probe`). More resident textures need VRAM → only via 16-bit colour buffers (changes every golden) | RE-076, RE-486, RE-489–RE-492, RE-539 |
| Read-ahead gaps | 1P next stage, intro → battle (no side-effect-free next-stage prediction); bonus select → bonus battle; Characters scene | RE-501 |
| Select clip files | Players-training entry: 10–33 scattered files (+16–18 ms). `enter` drains unrelated in-flight job (battle waits 28 ms when maps left after 13 ticks) | RE-501 |
| VS wallpapers | Capture a VS stage-select route; goldens cover training maps only | — |
| Kirby copy kind | Absent kind → scene lists; not reproduced (`demand=` in capture logs) | RE-475, RE-476 |
| Wallpaper padding | Trim twelve 5551 wallpapers' 512 × 256 padding (~1.5 MiB) | RE-419 |
| Unpalettized mips | Mip levels never given to GE → drop (memory, not bandwidth) | RE-127, RE-491 |
| GE state batching | Only where profiling shows submission cost. Refs: `sf64-psp` material hash, `oot-PSP` sampler cache | RE-124, [D-036](docs/decisions/D-036.md) |
| VFPU | Only after profiling | [D-032](docs/decisions/D-032.md) |
| VFPU block copies | `lv.q`/`sv.q` 16-byte copies for large aligned transfers (vertex expansion, frozen picture, photo) vs rust-psp word `memcpy` | RE-471 |
| CPU/GE overlap | Double-buffered display lists: +1 frame display latency, never a game tick. `vs4` already overlaps ≥4.8 ms GE; no bus-contention relief. Measure GE time first | RE-469, RE-486, [D-047](docs/decisions/D-047.md) |
| `meshdraw` audit | Indexed draws, transformed-vertex reuse. sf64-psp: non-indexed rebatching beat reuse. Lit primitives stay indexed | RE-514, RE-530 |
| I-cache locality | 16 KB I + 16 KB D; per-mesh path of `draw_object_posed_nodes` dominates misses. Outline only cold blocks. Rejected: per-primitive outlining, `opt-level = "s"`, `#[optimize(size)]`, shadowed render-state table; LTO re-inlines single-call helpers | RE-481, RE-487, RE-488, RE-493, RE-495 |
| Stack depth | Battle entry 239–294 KB of 512 KiB (`enter_training`, `EffectVisuals::sync_ko`) → re-measure | RE-469 |
| `run` branch range | Keep within 128 KiB MIPS reach | RE-431 |
| Debug HUD | `sceGuDebugFlush` (PPSSPP software rasterizer only; faults on PSP) → GE geometry | RE-014, RE-202 |
| Auto-sleep | Unattended release builds (no `profile`/`frame_timer`) auto-sleep → keep PSP awake or document why not | RE-481 |
| Campaign by hand | PSP-2000, current production EBOOT: campaign past Race to the Finish; then RE-476/RE-477 checks | RE-476–RE-478 |
| Hardware acceptance | 30-minute run on second unit; re-capture goldens on PSP; hand-input shield, grab, throws, costume picks | RE-273, RE-284, RE-320, RE-326, RE-339, RE-341 |
| PSP-1000 | Deferred by user; reopen on request. `MEMSIZE=1` ignored; VS results exceed PSP-1000 budget under PPSSPP | RE-288, RE-475 |
| Nub deadzone | Measure on hardware (20 units, linear rescale to ±80: unmeasured) | RE-009 |
| `sGCDetailLevel` | Trace how it is chosen | RE-011 |

Audio cost ([D-049](docs/decisions/D-049.md) amendment, RE-479), in order:

1. A/B audio vs `no_audio` on PSP-2000, current build.
2. VFPU reverb path (exact: constant coefficients, split products).
3. Media Engine probe:
   - throwaway crate; mcidclan `kcall.prx` (me-core-mapper) from user EBOOT,
     ARK 6.61;
   - `Dmem::envmix`, `adpcm_16` (`crates/ssb-engine/src/audio/dsp.rs`) on ME
     via 64-byte-aligned uncached mailbox;
   - accept: outputs equal SC over 10,000 repeats; ME ≤ 1.3× SC's 72 µs per
     voice pull; SC slowdown < 5%; survives sleep/wake;
   - SC fallback mandatory (PPSSPP has no ME);
   - no VME (24-bit; breaks D-049 sample-exactness).

### Gameplay math, tables and goldens

| Task | Remaining / acceptance | Refs |
|---|---|---|
| `r2-mario-entry-pipe` | Golden hash not a function of code alone → find differing input (pack bytes, staging, build path) | RE-506 |
| Slope contour | `explain_movement` parity; golden for motion_id -2 hold (Guard or Down-wait on slope) | RE-510 |
| Thunder Shock spark | Ejection on status change unobserved on N64 → longer Pikachu trace | RE-512 |
| vs-four golden | Optional: narrow change to catch/thrown/relz/pulled | RE-511 |
| Trig N64 traces | N64 traces for Ness jibaku, Kirby Stone slope, Falcon Kick roll, Pikachu `SpecialAirHiStart` cliff catch; Arwing `frame_rotate` ArcSin else branch unreached (needs a scene) | RE-503 |
| 1P goldens | 1P intro, stage clear | RE-548 |
| Fox ledge freeze | Playtest: occasional freeze at a ledge; loop-wrap end fixed, no second cause found → reproduce | RE-553 |
| Undrawn exit frame | Only the opening screen skips a scene's exit update; audit other scene changes (battle to results, menus, movies) | RE-553 |
| Kirby/Purin CliffEscapeQuick | N64 check of the two-lead (`0xC0000000`) clips 483/484 | RE-553 |

### Audio and rumble (`MS7`)

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Attract audio | Compare KO and later hits (97.6–124.5 s), 99 near-full-scale samples with N64; reverb's unapplied low-pass vs hardware recording | RE-479 |
| Rumble | Port Rumble Pak; accept: N64 motor timings per event match decomp | — |

### Match systems (`MS5`)

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Boss-intro camera thread | `BossCameraAnim` is set after the fighter step; the decomp runs it as a priority-5 thread before the camera. Untraced → boss-intro N64 camera trace, compare per tick | RE-543 |
| Captain's car | Battle-camera fallback before TopN in bounds unverified (`f1-vs-car`) → warp-boot capture, focus id 2, Captain | RE-523 |
| Samus fire spark | Up smash, forward air; 84 + 0x2040. Needs matrix kind 0x49 billboard along joint 16, Samus capture | RE-427 |
| Same-frame makes | Process order modelled; N64 two-fighter confirmation needs a cpufx `mkrom.py` per-port input patch (cpufx ROMs mirror controller 0) | RE-538, RE-540, RE-542 |
| Chunk 5 golden | Golden scene reaching wall-damage impact wave / PK Thunder bound / Kirby star bounce; unit tests only today | RE-503 |
| cpufx tick map | cpufx-n/u: 399+el; clean trace: 398+el; cpufx-d: 399 | RE-540 |
| Spin Attack band | Less saturated than N64 at el48-52 | RE-540 |
| cpufx pixel captures | N64 pixels for cpufx-d and yoshi-explosion row (RE-536 compared timelines only); second Kirby costume vs N64 HUD icons in cpufx-k; PSP VS camera frames tighter (global MAE unusable) | RE-536 |
| Kirby | PSP spit-tick capture vs N64 shots 007/008; SpecialAirNCatch af unchecked vs N64 | RE-552 |
| PK Thunder trail texture frame | `Fx::TextureRand(3)` picks sprites 0-2 per frame; PSP always draws sprite 0 (texture 1961), bolt thicker than N64 → per-trail texture selection plus pack data for the 3 sprites | RE-550 |
| Kirby star mesh | Capture star draws the `yoshi_star_mesh` billboard; same ROM object unproven | RE-551 |
| Thunder Jolt burst | Area 23–36% larger than N64 on the el162 crop (~11% framing, ~20% unattributed): check filter kernel and scale at 2x; acceptance: area within 10% | RE-541 |
| PK Fire, Bomb | PK Fire flame on dummy in a capture; Bomb critical flash, held bloat | RE-352, RE-382, RE-383 |
| Battle draw order | Object head-1 lists don't wait for stage head-1 lists of same pass | RE-422 |
| Camera | Special camera modes; cpufx-u el56-70 MAE 23.9-29.7 vs N64 after RE-542 | RE-406, RE-538, RE-542 |
| Yoshi | Tongue and Egg Lay timeline matched (RE-552); remaining: grab vs N64, effect-room flag0 breakout fallback unported by design | RE-337, RE-417, RE-432, RE-466, RE-468, RE-552 |
| Setter first frames | No N64 trace: Barrel/Tornado captures, item throws, Final Cutter landing, Yoshi aerial release, Link aerial Boomerang return | RE-468, RE-473, RE-474 |
| `anim_frame` | Shield pose clock; DK speed-0 cargo jump/fall; idle camera `status_total_tics` | RE-472, RE-474 |
| Fighter entry | vs N64: Fox cockpit, Poké Ball near-camera edge at tick 224, Luigi pipe, Kirby star | RE-402, RE-403, RE-425 |
| Items | Event-aligned N64 damage, hit order, trajectories; remaining Pokémon, Clefairy selections; Bumper lit palette, Star flicker, bomb flashes; lit primitives without authored light colours | RE-431–RE-444 |
| Stages | N64 captures: pipes, plant notification, cloud vapor, scale sparkles, plant KO, Sector Z Arwing, Board the Platforms blocks | RE-428, RE-429, RE-442, RE-446, RE-455 |
| Acid tile 1 | Only sprite 0 draws; pack converts four 384 × 384 variants | RE-364 |
| Window matanim blobs | Script packs whole source file; command 22 rejected, skippable | RE-010, RE-378, RE-381 |
| Results wipe | N64 view | RE-464 |
| CPU | N64 trace of CPU tracking an item; VS entry focus, 1P traits vs N64 | RE-437, RE-447, RE-450 |
| `WPAttributes` pairing | Revisit if another case like Link's boomerang appears | RE-058 |

### Front end and 1P (`MS6`)

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Select CPU colour animation | VS select CPU colour animation; locked shadows' noise | RE-411, RE-448 |
| Title fire, colour grade | Slightly green after fade vs Rice (tic 152: PSP (189,177,117), N64 (193,171,105), equal luma). Open until accurate N64 reference (GLideN64 segfaults in `PluginStartup(gfx)`, 18/18 runs) | RE-529 |
| Battle-stage lights | Fixed-direction light bake (RE-065 deviation) → display-list lights as movies use; A/B battle rows vs N64, PSP CPU cost | RE-065, RE-529 |
| Opening movie | Room falling-figure pose, spotlight cone, close-up overlay shade; N64 refs from accurate video plugin | RE-467, RE-468 |
| 1P ending | Ending room background matanim; N64 compare ending, staff roll, challenger; CP/heart tags; observed loss/win handoffs | RE-459 |
| 1P bonus stages | N64 pause capture, PSP comparison | RE-456 |
| Master Hand | `is_use_fogcolor` darkening; wallpaper effects' matanims; comet ENV tint; defeat-zoom struck joint; boss stock snap; hit-by-player capture; magnifiers hidden before "Go" | RE-457 |
| Special fighters | Intro Polygons darker, Metal Mario reflection duller than N64; unobserved: 30-Polygon clear, DK Defender/Perfect bonuses | RE-458 |
| 1P shade | Yoshi same-costume shade | RE-450, RE-453 |
| Item Switch | No N64 reference (locked on new save) | RE-462 |

### Architecture

| Task | Remaining / acceptance | Refs |
|---|---|---|
| Runtime loader | Use `ssb_rom::reloc_link` (layout matches original) | RE-340, RE-341, [D-011](docs/decisions/D-011.md) |
| Memory arenas | Wire `ssb_engine::memory` arenas, pools into `psp-runtime` | [memory](docs/memory.md#allocators) |
| Costume check | Independent ROM-derived check of fighter costume, matanim (as stages have) | RE-050–RE-052, RE-142 |

## Completed

| Capability | Refs |
|---|---|
| Architecture: portable crates, shared `psp-runtime`, `psp-game`, `psp-asset-viewer`; pinned toolchains; CI without ROM | [D-026](docs/decisions/D-026.md), [D-030](docs/decisions/D-030.md), [D-044](docs/decisions/D-044.md) |
| ROM: validation; VPK0 (499 files); relocData (2,132 files, 61,343 intern, 3,092 extern relocs); F3DEX2 (1,864 lists, 0 failures) | RE-001, RE-002, RE-017 |
| Pack: meshes, textures (RGBA, IA, I, CI + palette banks), mat/joint anims, figatrees, sprites, particles, collision, stage and fighter tables; per-scene files | [D-046](docs/decisions/D-046.md), RE-475 |
| Renderer: geometry, projection, texture addressing, 3-point compensation, combiner, `MObj`, lighting, texgen, alpha/blend/depth, billboards, shadows, framebuffer effects, wipes, UI sprites, GE state cache | [rendering](docs/rendering.md) |
| Physical PSP rendering: PSP-2000, PSP-3000; two 10-minute runs, one 30-minute run | RE-270, RE-271, RE-273, RE-284 |
| Fighters: all 12 (Kirby with all copies), Master Hand, Metal Mario, Giant DK, Polygon Team | RE-344–RE-347, RE-457, RE-458 |
| Fighter systems: physics, collision, status machine, figatree, motion scripts, hits, shields, grabs, throws, ledges, recovery, KO, rebirth, colour animations | RE-330–RE-355, RE-366–RE-371, RE-388, RE-412–RE-414 |
| How to Play, opening battles: N64 RDRAM traces match every frame | RE-466, RE-468, RE-472–RE-474 |
| Weapons and items: all fighter weapons, 20 normal items, Poké Ball Pokémon, stage items, hazards | RE-354, RE-372–RE-381, RE-416, RE-430–RE-446 |
| Stages: 41 maps; 9 VS stages with controllers, hazards; wallpapers; Sector Z Arwing; bonus stages | RE-385, RE-419, RE-428, RE-429, RE-446, RE-452–RE-455 |
| CPU AI: input scripts, objectives, traits, item use | RE-390, RE-391, RE-437, RE-438 |
| VS: rules, countdown, HUD, pause, teams, four fighters, results | RE-389–RE-411, RE-420, RE-464 |
| Training: menu, stats | RE-438–RE-440 |
| 1P: every stage, bonus stages, Master Hand, ending, staff roll, congratulations, challengers, unlocks; full transition run | RE-447–RE-459, RE-478 |
| Save: `LBBackupData` in `ssb64.sav`; options, data menus | [D-045](docs/decisions/D-045.md), RE-460, RE-461 |
| Front end: N64 logo, opening, title, attract loop (How to Play, Characters, auto demo), mode/1P/VS menus, Bonus Practice | RE-462, RE-465, RE-467 |
| Per-scene loading, background reads: no intro load hitch on PSP-2000 | RE-475, RE-476 |
| CRT overscan crop: N64 visible box fills PSP height | [D-047](docs/decisions/D-047.md), RE-477 |
| CPU performance: 60 FPS under PPSSPP | RE-469–RE-471 |
| Audio: synthesizer, sequence player, FGM engine, `syAudio`; all gameplay/menu/scene calls; Sound Test; PSP thread on 32 kHz SRC channel | [D-049](docs/decisions/D-049.md), RE-479 |
| No ROM data committed: scripts, status flags, anim table generated at build time; goldens as pixel hashes | [D-048](docs/decisions/D-048.md) |

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
