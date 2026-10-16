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
| Asset pipeline | Tables from the ROM's code segments generated at build time by `ssb-tablegen` ([D-048](docs/decisions/D-048.md)); `romtool` builds pack v109 from the user's ROM: all 2,132 archive files, every reachable model, texture, animation, sprite and table, and the audio section; scenes load their own files ([D-046](docs/decisions/D-046.md)) | Same; trimmed wallpaper padding |
| Rendering | Build-time display-list conversion, GE lowering with measured deviations (3-point filtering); battle draws in the original's display-link passes; CRT overscan crop ([D-047](docs/decisions/D-047.md)) | Remaining effect draws and N64 comparisons; per-scene texture residency in VRAM |
| Gameplay | All 12 fighters, Master Hand, Metal Mario, Giant Donkey Kong and the Polygon Team; hits, shields, grabs, ledges, KO; How to Play matches an N64 trace on every frame (RE-472, RE-473) | Remaining fidelity items below |
| Items | All 20 normal item makers, 13 Poké Ball Pokémon, stage items and hazards (RE-431–RE-446) | Event-aligned N64 comparisons |
| CPU | VS, Training and 1P CPU behaviours, traits and objectives (RE-390, RE-391, RE-437, RE-438, RE-447) | CPUs' special effects drawn; N64 trait checks |
| Game modes | VS (up to four, teams), Training, 1P campaign through the ending, staff roll and challengers, bonus stages and Bonus Practice | Same, with rumble |
| Menus/UI | N64 logo, opening movie, title, attract loop (How to Play, Characters, auto demo), mode/1P/VS menus, selects, options and data menus, HUD | Selects' spotlight; opening's remaining differences |
| Save data | N64 SRAM image in `ssb64.sav` beside the pack ([D-045](docs/decisions/D-045.md)) | Anti-piracy validators |
| Audio | The N64 audio system ported whole: synthesizer, sequences, FGM sound effects and every call site, on a PSP audio thread; sample-exact against N64 captures ([D-049](docs/decisions/D-049.md), RE-479) | Same, within the PSP-2000's CPU budget in every scene |
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
| `MS7` | Audio and rumble | Audio done; rumble planned |
| `MS8` | PSP performance and hardware acceptance | In progress; never gates `MS4`–`MS7` ([D-032](docs/decisions/D-032.md), [D-036](docs/decisions/D-036.md)) |

Earlier labels (`P0`–`P5`, `M0`–`M4`, `R0`–`R3`, `F1`, `G0`–`G5`) are retired;
code comments and evidence that cite them resolve through
[`docs/evidence/INDEX.md`](docs/evidence/INDEX.md#retired-labels).

## Remaining Work

Each item cites the evidence that describes it. Delete an item when a batch
closes it.

### PSP performance and hardware (`MS8`)

- [ ] Lit primitives draw through the expanded per-corner path: light each vertex once and keep the indexed draw (RE-514).
- [ ] Confirm the RE-514 CPU lighting cost on a physical PSP-2000 (`vs4@2400` profile; +1.6 ms CPU under PPSSPP) (RE-514).
- [ ] By hand on the PSP-2000, with the installed production EBOOT (`target/release-audio/EBOOT.PBP`, pack v108; the attract loop already runs clean, RE-479): the campaign past Race to the Finish, then RE-476/RE-477's checks (RE-476, RE-477, RE-478).
- [ ] Measure D-047's GE fill cost on a PSP-2000: the battle viewport covers 19% more pixels; profile `vs4@4000`, the opening and the selects (`ge` span) (RE-477).
- [ ] Opening CPU cost on a PSP-2000 (~3.5× PPSSPP): Run, Yoster/Sector and Clash at 45–57 FPS with 12–14 ms of CPU, mostly the movie draw's mesh lists; each opening fight's first frame takes 21–40 ms. Cached, written-back vertex blocks gave +0.89 FPS and -163 us CPU on `vs4@2400` in a same-binary toggle but are neutral in the vsync-bound opening (RE-499). Next: the battle's first frame (RE-469, RE-470, RE-471, RE-476).
- [ ] Measure RE-469's other scenes and RE-471's worst cases on the PSP; `vs4@4000` averages 5.2 ms against RE-471's 4.5, unexplained. Four-player VS runs at 40 FPS on a PSP-2000 without audio: 18.3 ms of CPU a frame against PPSSPP's 5.4, every span 2.5–7× PPSSPP's count, no single hot spot; posing the drawn models was 0.78 ms, part of it the battle's stage layers 0, 1 and 3 each recomposing twice a frame (once per head), now posed once and cached. The PSP's counters (`profile_hw`) put 55% of the frame's cycles in memory stalls and a tenth in load-use and FPU stalls: the draw takes 22,900 I-cache misses a frame against 6,800 D-cache misses (RE-471, RE-476, RE-479, RE-480, RE-481).
- [x] Scene read-ahead: menu to child menus, select to stage previews, stage select to the roster's fighter files, VS end or reset to results. VS to results first frame 130.4 to 27.4 ms, battle 179.3 to 77.3 ms on a PSP-2000; no tick or pixel changes (RE-501).
- [ ] Read-ahead gaps: 1P next-stage and intro to battle (no side-effect-free next-stage prediction); bonus select to bonus battle; the Characters scene; no-contest reset to results (trigger only 3 ticks ahead, no gain).
- [ ] Select clip files are scattered: players-training entry reads 10 to 33 files (+16-18 ms on a PSP-2000); `enter` drains an unrelated in-flight job (battle waits 28 ms on a PSP-2000 when maps is left after 13 ticks) (RE-501).
- [ ] Capture a VS stage-select route to check the VS wallpapers on screen (goldens cover the training maps only).
- [ ] Add the Kirby copy's absent kind to the scene lists (not reproduced, kind unknown; `demand=` in capture logs) (RE-475, RE-476).
- [ ] Flaky test: `crates/ssb-rom/tests/particle_runtime.rs:642` `the_item_spawn_swirl_draws_nine_times_at_the_next_frame_start` failed once in a parallel run and passed on rerun; it passed in the single-threaded workspace run. Find the shared state.
- [ ] Parallel `cargo test -p ssb-game` fails 7-10 global-state tests (RNG, audio, items, wpeffect, fteffect) at HEAD before RE-504 too; CI and baselines run `--test-threads=1`. Give those tests a shared lock or per-test state.
- [ ] Per-scene texture residency: v78 Dream Land with four fighters needs 726,924 bytes low detail against a 704,512-byte pool, and since RE-476 only 180,224 bytes of VRAM are free; the runtime samples from main RAM. The GE's main-memory reads slow the CPU: holding the GE back until `sceGuFinish` cuts four-player VS's CPU frame by 3.0 ms, all memory stall, and texel reads alone are 2.2 ms of it (textures off: 42.0 → 48.5 FPS on a PSP-2000), the largest measured CPU lever. The battle wallpaper was 1.8 ms of it, one 300-texel-wide sprite thrashing the GE's texture cache; drawn in 64-texel strips it now costs about 0.5 ms (42.0 → 45.8 FPS). No other large textured rectangle is drawn in the battle loop. Direct-colour textures with at most 256 colours are now stored as CLUT4/CLUT8, losslessly (fighters' 0.45 ms, 42.0 → 47.3 FPS since RE-489 with the strips). The wallpaper (3,031 colours, no palette) is now sampled from the VRAM photo region, idle in battle (0.32 ms, 47.2 → 48.0 FPS). What remains is about 0.45 ms stage (already CLUT4) and 0.2 ms the rest (`texture_probe`); a second resident texture needs VRAM the photo region does not have (16-bit colour buffers would change every golden) (RE-076, RE-077, RE-341, RE-426, RE-486, RE-489, RE-490, RE-491, RE-492).
- [ ] Trim the twelve 5551 wallpapers from their 512 × 256 padding (~1.5 MiB) (RE-419).
- [ ] State batching where profiling shows GE submission cost: `sf64-psp` hashes material state into a batch pool and `oot-PSP` caches sampler state (RE-124, [D-036](docs/decisions/D-036.md)).
- [ ] VFPU math only after profiling ([D-032](docs/decisions/D-032.md)); `ssb_engine::math::sqrt` is now exact `sqrt.s` (RE-502).
- [ ] Gameplay trig fidelity: passes 1 to 3 done (ArcTan/ArcTan2/ArcSin/ArcCos RE-503; `__sinf`/`__cosf` RE-504; sine table RE-505); `tan` still differs. The ssb-rom arctan copy, the FCR31 fold and the `from_trs` kinds are done (RE-508). Follow-ups: `audio/osc.rs:82` f32 `sinf` could call `math::sinf` (D-049 deviation); `psp-game/link-order.txt` lacks `sinf`/`cosf`/`sinf_f64`/`cosf_f64`, re-search at the next MS8 link-order pass.
- [x] Trig third family `gSYSinTable` phase A: `Camera::project` and staffroll fovy use the ROM table cotangent (RE-505).
- [x] Trig third family phase B: GU fovy (`syMatrixPerspFastF`, cliff `PerspF`) and display matrices (`TraRotRpyR`/`Sca` chains) match the ROM (RE-506).
- [ ] RE-506 follow-ups: `r2-mario-entry-pipe` hash is not a function of the code alone: at d1f21eb built in a /tmp worktree with a HEAD-built pack it captures a stable d766... (3 of 3) yet the committed b089... matches the RE-506 tree 3 of 3 (earlier runs showed a third hash, 6107..., with 0 differing RGB pixels); find the input that differs (pack bytes, staging, build path); frame busy-time of the cheaper display chain unmeasured (needs link-order-pinned builds); (the oracle now has `PerspFast`, RE-507).
- [x] Vortex particle motion, Hyrule Twister and the 0x45/`RotRpyR`/`Sca`/0x44 effect matrices match the ROM; Twister bit-exact against an N64 RDRAM trace (RE-507).
- [x] Pupupu leaves/dust (bit-exact against an N64 trace), ssb-rom vortex decline documented, remaining `from_trs` consumers on their original matrix kinds, magnifier cap resolved (RE-508).
- [x] Fighter animation locks (`is_use_animlocks`): frozen rest-local mask joints, TraRotScaInv/Ncs chains, queried-joint mask, figatree division; masks generated by tablegen (RE-509).
- [x] Slope-contour leg IK (800EBD08), FULL TopN tilt, pending-slope sites, motion_id -2 animation hold; bit-exact against Mario, Samus and Jungle traces (RE-510).
- [ ] Slope-contour follow-ups: Kirby Eat/CatchEat run the slope step immediately before PlayAnimEventsAll (0x80163018, 0x801630A0); `explain_movement` parity with the slope step; no golden covers the motion_id -2 hold (a Guard or Down-wait scene on a slope would) (RE-510).
- [x] Fighter TopN display (kind 0x4B), remaining query marks, item-swing attack joint -2, Kirby inhale/throw timing, Pikachu spark RNG; bit-exact against five N64 traces, 123 goldens rebaselined (RE-511).
- [x] Sword-trail (afterimage) renderer for Link and the Beam Sword, with the magnifier pass, and the Pikachu Thunder Shock spark with its TopN query-mark timing; 4 goldens rebaselined or added (RE-512).
- [ ] Thunder Shock spark ejection on a status change was not observed on the N64 (decomp only), and the Pikachu trace is short; capture a longer one (RE-512).
- [x] Kirby attack100 vulcan puffs (link-15 display, TopN query marks), Link/Captain/Kirby attack100 item-pickup interrupt, Captain rapid-jab flag-1 frames 6/14/22/30/37 with a 40-frame loop, link-15 displays drawn in make order; against N64 traces; 1 golden added (RE-513).
- [x] Fighter lighting from the F3DEX2 microcode (CPU vertex lighting, normalised direction, shade clamp before PRIM; LLE 768/768) and the fighter shadow (matrix regression, no depth test) (RE-514).
- [x] Training draws one stock icon per player (2 stocks, `is_single_stockicon`, the default battle state); icons and HUD match the N64 sprite and position (RE-515).
- [x] Samus Charge Shot PSP crash: FGM LFO divide-by-zero and RSP-light float-to-int traps fixed with N64-faithful helpers, six more trap sites, fighter light rows no longer scaled by MODEL_SCALE, impact wave translucent, samus/samusshot/donkey scripts press B at tick 40 (RE-516, D-050).
- [x] RE-516: XMB Samus charge run on the PSP-2000: no crash (user test, RE-516).
- [x] RE-516: FCR31 at an XMB-style launch (`sctrlKernelLoadExecVSHMs2`, `fcr31_probe`) is `0x0e00` on both threads, as under PSPLink; D-050 holds (RE-516).
- [ ] RE-516: latent FPU trap sites from the audit that no test reaches; extend `with_traps` sweeps (items, stages, effects).
- [x] 2D sprite edge fringe: one-ring nearest-neighbour RGB bleed into transparent texels at pack build (`sprite::psp_image`), filters stay Linear; pack v111 (RE-517).
- [ ] Optional: narrow the vs-four golden change to catch/thrown/relz/pulled; marks-only pixel share of the 27 display-plus-marks rows (RE-511).
- [ ] Tooling: `tools/golden.sh` mem-line regex `[^\r]*` stops at the letter r; `SSB64_ROM` must be an absolute path for golden.sh.
- [ ] D-048: `hidden_parts` (FTAttributes::hiddenparts, RE-468) in modelpart.rs is still a committed per-fighter ROM table: generate it in `ssb-tablegen` `model_parts.rs`; audit ssb-game for others. ssb-rom ROM tests return early without `SSB64_ROM` (use `ssb_tablegen::locate_rom`).
- [ ] `syMatrixFastSin/Cos` (matrix.c:78/90) and lbcommon.c callers audit.
- [x] `f1-vs-four-magnifier`: the raw scale was 2.397 (uncapped) at the old `vs4@2800`, measured before Whispy's RNG draws moved the timeline; the unchanged RE-505 hash is explained. The golden moved to `vs4@1330` to keep magnifier coverage (RE-508).
- [ ] Trig audit follow-ups (RE-503): stick gates reframed as slope compares (status.rs, item_throw.rs vs `ftParamGetStickAngleRads`; `set_utilt` lacks the Hi3F/Hi3B split); `syVectorAngleDiff3D` reframed as dot/cos threshold compares (map.rs, pikachu*.rs, ness.rs, status.rs); dropped ArcTan2 calls (reaction.rs wall-damage wave, capture_kirby.rs, ness.rs bound wave, kirby.rs TopN, captain.rs tilt, UpdateModelPitch family); unported trig sites (`ftMainGetBumperDamageAngle`, mpcommon.c:342/400, ftparam.c:2619-2635, lbparticle.c:2482/2486 vortex, gmcollision.c:946, ftdisplaymain.c:513); reflector rotate normalisation; `Light.l.dir` s8 quantisation; Fox up-special `MAP_VERTEX_COLL_PASS` floor exclusion; `F_CST_DTOR32` vs `F_CLC_DTOR32` constants; add goldens for PK Thunder steering, hazard spin, ArcSin sites.
- [x] VFPU for render-only matrix work: the per-node model-matrix compose (joint world matrices for drawing, also exercised by the movie draw's mesh lists) now issues a raw `vmmul.q` with no `vfpu_context` save/restore, +0.70 FPS on a PSP-2000; the skinned-vertex expansion's real cost was libm's software `sqrtf`, not its matrix math, so a correctly-rounded Allegrex `sqrt.s` replaces it (+0.45 FPS, bit-identical output) while a VFPU `vtfm` rewrite of the same loop was measured and rejected (slower on hardware, adds rounding PPSSPP cannot see). Gameplay joint sampling, collision and hitboxes stay on the scalar FPU; the movie draw's own matrix composition (`movie.rs`) stays scalar, reusing gameplay's shared `Mat4` (RE-497).
- [x] Billboard arm: `load_billboard_matrix` replaced by a closed-form scalar matrix with `sqrt.s`; span 310 → 182 us, +1.14 FPS on a PSP-2000 (RE-498).
- [x] Movie/opening goldens: `f1-opening-mario`, `f1-opening-run`, `f1-opening-clash` cover the animated `Span::MovieMesh` path (RE-498).
- [ ] VFPU 16-byte block copies (`lv.q`/`sv.q`) for large 16-byte-aligned transfers (vertex expansion, frozen-picture and photo copies), measured against `ssb_engine::memops` (RE-471).
- [ ] Overlap CPU and GE: build frame N+1's display list while the GE draws frame N (double-buffered lists, PSPSDK sample). Costs one frame of display latency, never a game tick; measure GE time first ([D-047](docs/decisions/D-047.md)'s +19% fill, RE-469). Four-player VS has at least 4.8 ms of GE work, already overlapped within the frame; double-buffering would not remove the GE's bus contention with the CPU (RE-486).
- [ ] Keep geometry non-indexed: audit `meshdraw` for indexed draws or transformed-vertex reuse; on the PSP rebatching non-indexed vertices beat reuse (sf64-psp: −39% VFPU instructions but +4.6 ms; rebatching won back 1.2 ms).
- [ ] Instruction-cache locality (16 KB I + 16 KB D): the draw's 22,900 I-cache misses a frame are its largest memory cost (RE-481). A cache model in PPSSPP (`tools/icache/`) puts three quarters of them in twelve functions, the per-mesh path of `draw_object_posed_nodes` (`draw_mesh_vertices`, `bind_texture`, `apply_texture_mapping`, `sceGum*`): one pass touches 14.7 KB scattered through ~60 KB, and 39% of the misses are conflicts. `psp-game` now links its hot functions first (`tools/psp-link.py`, −8% draw misses, −0.5 ms); `opt-level = "s"` is slower, and LLVM's hot/cold splitting without a profile does not split the hot functions. Outlining the per-mesh path's cold branches and composing the per-node model matrix on the CPU cut a draw pass's modelled fetches 21% and hardware draw time 4.8% on HEAD's link order; regenerating the link order from the smaller functions made hardware slower, so `link-order.txt` keeps its RE-482 order (RE-487). Outlining `draw_stage_links`' inlined object posing and uncached paths (13.6 KB → 0.9 KB) cut draw time a further 1.7% (RE-488). The rest of the shared pass (`draw_training`, `draw_magnifiers`, `draw_items_weapons_effects`) is under 1% of the misses, and `#[optimize(size)]` on the per-mesh functions is worse; a call to `draw_mesh_vertices` runs 5.1 KB spread over many small steps. LTO re-inlines a single-call-site helper (`apply_material`, `compose_model_matrix`) back into its caller regardless of earlier outlining, and outlining per-primitive code that still runs every mesh loses on hardware even when it shrinks the function and the I-miss count, so only the per-mesh path's genuinely cold blocks (LOD-blend second pass, fade blend, RE-426 sprite lookup, RE-326 animated window) were pulled out: −95 µs CPU frame, 48.0 → 48.27 FPS (RE-493). The link order is now searched, not guessed: `tools/icache/layout.c` replays a model trace for each candidate order (exact against relinked binaries), and on the PSP three layouts ranked as the model ranked them, at ~0.33 µs a miss. On the shipped audio build, the searched order cuts modelled misses 21% (25% in the draw), ~1.5 ms a frame (RE-494). An order fits only the build it was searched on, so search it again when hot code changes size. A shadowed flags-derived render-state table that skips redundant GE writes loses: building and diffing the key costs more than the ~430 redundant writes a frame it saves, ~150 µs a frame slower on hardware at each build's best order. A second anneal seed found a 3% better order that ran 0.6 FPS faster (RE-495). Sixteen seeds at 30,000 iterations and twelve more at 90,000, scored by held-out trace, found an order 3.7% below HEAD's that ran 0.90 FPS faster than the single-seed method on a `profile` proxy build; the shipped `link-order.txt` is now that order (RE-496). The order was re-searched again after the VFPU compose and `sqrt.s` changes resized the hot functions; the new winner (seed 8 of 12, 90,000 iterations) scored 21.6% below the pre-search order on held-out misses, verified byte-identical between a real `cargo psp --release` build and a `relink.py` relink of the same order (RE-497), and once more after the billboard change (RE-498). Model-scored order selection does not predict hardware at +-1.5 FPS: equal-model orders spread 2-3 FPS and HEAD's order was the model's worst yet among the hardware best (RE-499); a release-code frame timer then ranked orders on hardware by busy time and chose seed s7, 170-205 us faster than s3 (RE-500). Next: the next `MS8` lever (RE-480, RE-482, RE-487, RE-488, RE-489, RE-493, RE-494, RE-495, RE-496, RE-497, RE-498, RE-499, RE-500).
- [x] Measure the release build's frame time on the PSP: the `frame_timer` feature times release-code frames, which are vsync-bound (`vs4@2400` 59.3 FPS), so link orders are ranked by busy time; `link-order.txt` is now seed s7, 169-205 us busy faster than s3 on three stages (RE-500). Next: nothing open here; the I-cache model ranks orders weakly (Spearman 0.45), so search new orders but choose on hardware with `tools/hw-link-order.sh`.
- [ ] Add a `tools/icache` scorer that replays a trace against a relinked ELF's real addresses rather than `layout`'s own placement (RE-496 found `layout`'s "as linked" line can't score a relinked ELF directly, and its held-out score slightly overestimates a relinked ELF's true misses; a scratch rewrite confirmed the behaviour again in RE-497 but still lives outside the repo, not in `tools/icache/`).
- [ ] Unpalettized textures (295 of 8888, 29 of 5551 after RE-491) still store mip levels the GE is never given (RE-127); drop them for memory, not bandwidth (RE-491).
- [ ] Audio cost: an exact VFPU path for the custom reverb (constant coefficients; split products stay exact), then the voices; the Media Engine stays out ([D-049](docs/decisions/D-049.md)) unless profiling shows audio is what stops 60 FPS. On a PSP-2000 audio costs 3 FPS in the opening (55.8 against 58.7) and 1.6 in `vs4@2400` (39.0 against 40.6) (RE-479).
- [ ] FPU traps outside PSPLink: check whether an XMB-started game traps divide-by-zero as PSPLink does; keep `math::div_nonzero` on speculated divisions (RE-201, RE-469).
- [ ] Large stack frames: battle entry peaks at 239–294 KB of the 512 KiB stack (`enter_training`, `EffectVisuals::sync_ko`) (RE-469).
- [ ] Keep `run` inside MIPS branch range: 22,884 code bytes against a 128 KiB reach (RE-431).
- [ ] Replace the debug HUD's `sceGuDebugFlush` (software-rasterizer-only in PPSSPP, faults on a PSP) with GE geometry (RE-014, RE-202).
- [ ] Hardware acceptance: a 30-minute run on a second unit (RE-273, RE-284); re-capture current goldens on a PSP (RE-320, RE-326, RE-341); hand-input checks of shield, grab, throws and costume picks (RE-339, RE-341).
- [ ] PSP-1000 (deferred by user instruction): `MEMSIZE=1` is ignored; RE-475 measures the heaviest scenes against a PSP-1000-sized budget under PPSSPP; the VS results do not fit (RE-288, RE-475).
- [ ] Measure the nub deadzone on hardware (20 units with a linear rescale to ±80 is unmeasured) (RE-009).
- [ ] Trace how `sGCDetailLevel` is chosen (RE-011).

### Audio and rumble (`MS7`)

- [ ] Compare the attract capture's KO and later hits (97.6–124.5 s) and 99 near-full-scale samples with the N64; check the reverb's unapplied low-pass against a hardware recording (RE-479).
- [ ] Rumble.

### Match systems (`MS5`)

- [ ] Kirby's late entry poses (N64 frames 326-332) look about one frame ahead of the N64; the star phase is frame-exact. Not diagnosed.
- [ ] No N64 focus-2 capture of Captain Falcon's car, so the battle-camera fallback before his TopN comes in bounds is unverified for that phase (`f1-vs-car`). A warp-boot capture with focus id 2 and Captain closes it (RE-523).
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

- [ ] Both character selects' spotlight under a held puck; the VS select's CPU colour animation; locked shadows' noise (RE-411).
- [x] Intro (user report, PSP): every scene compared against N64 captures; batches 1-5 done (RE-519 to RE-529). F3 floor contrast, the title fire tint and the slash line are Rice artefacts or a PSP-forced deviation (slash-line coverage AA is missing; the geometry and colour match), with evidence in RE-529. The colour grade (R4) stays open until an accurate N64 reference exists.
- [ ] Title fire is slightly green after its fade compared with Rice (tic 152: PSP (189,177,117), N64 (193,171,105), equal luma); no accurate N64 reference exists because GLideN64 segfaults in `PluginStartup(gfx)` 18 of 18 runs.
- [ ] Battle stages still use the fixed-direction light bake (RE-065 deviation); evaluate display-list lights there as the movies now do (RE-529): A/B battle rows against N64 captures and measure the PSP CPU cost.
- [ ] Intro follow-ups: demo-script EFFECT/FGM events on the results and select screens and on joints other than 0 (RE-528); audit every `rgb_lerp` material for runtime image or palette swaps, which would draw un-lerped (RE-527).
- [ ] Opening movie: the room's falling-figure pose, spotlight cone and close-up overlay shade; N64 references from an accurate video plugin (RE-467, RE-468).
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
- [x] Audio: the N64 audio system (synthesizer, sequence player, FGM engine, `syAudio`), every gameplay, menu and scene call, the Sound Test, a PSP audio thread on the 32 kHz SRC channel ([D-049](docs/decisions/D-049.md), RE-479).
- [x] Nothing read from the ROM is committed: motion, colour-animation and CPU scripts, status flags and the animation table are generated at build time from the user's ROM; goldens are pixel hashes ([D-048](docs/decisions/D-048.md)).

[decomp]: https://github.com/VetriTheRetri/ssb-decomp-re
