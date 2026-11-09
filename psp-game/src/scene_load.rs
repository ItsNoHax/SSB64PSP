//! Which archive files each scene holds (RE-475, D-046): the N64 scene's
//! roots (`ssb_rom::scene_roots`) handed to
//! `ssb_psp_runtime::scene_files::enter`.
//!
//! A scene sets its own roots with [`scene`]; a battle the scene starts
//! (an opening's fight, How to Play, the title demo, VS, Training, the 1P
//! Game) adds the battle's with [`battle`], keeping the scene's.

use alloc::vec::Vec;

use ssb_game::fighter::FighterKind;
use ssb_game::menu::Scene;
use ssb_game::opening::Kind;
use ssb_psp_runtime::scene_files;
use ssb_rom::pack::Pack;
use ssb_rom::scene_roots as r;

/// The roots of the scene last set with [`scene`].
static mut BASE: Vec<u32> = Vec::new();

/// The opening room's two random figures (`mvOpeningRoomInitVars`), by
/// `FTKind`, once [`set_room_figures`] has named them.
static mut ROOM_FIGURES: Option<(FighterKind, FighterKind)> = None;

/// Names the opening room's pulled and dropped figures, whose files its
/// list then holds (RE-476).
pub fn set_room_figures(figures: (FighterKind, FighterKind)) {
    // SAFETY: the game thread alone calls this module.
    unsafe { ROOM_FIGURES = Some(figures) };
}

/// Whose files [`BASE`] holds, so whether a battle keeps them.
#[derive(Clone, Copy, PartialEq)]
enum Base {
    /// A scene's own: a VS or Training battle leaves its select's and
    /// stage select's files.
    Own,
    /// A scene that is a battle: How to Play, the title demo, the 1P
    /// Game's stages.
    Battle,
    /// The opening, whose fights read their clips with the scene (RE-575).
    Opening,
}

static mut BASE_IN_BATTLE: Base = Base::Own;

fn base() -> &'static mut Vec<u32> {
    // SAFETY: the game thread alone calls this module.
    unsafe { &mut *core::ptr::addr_of_mut!(BASE) }
}

/// A log name for `MScene` `s`.
pub fn name(s: Scene) -> &'static str {
    match s {
        Scene::Title => "title",
        Scene::ModeSelect => "mode-select",
        Scene::Option => "option",
        Scene::ScreenAdjust => "screen-adjust",
        Scene::BackupClear => "backup-clear",
        Scene::Data => "data",
        Scene::VsRecord => "vs-record",
        Scene::Characters => "characters",
        Scene::SoundTest => "sound-test",
        Scene::AutoDemo => "auto-demo",
        Scene::OnePMode => "1p-mode",
        Scene::VsMode => "vs-mode",
        Scene::VsOptions => "vs-options",
        Scene::VsItemSwitch => "item-switch",
        Scene::Players1PGame => "players-1p",
        Scene::Players1PTraining => "players-training",
        Scene::Players1PBonus1 => "players-bonus1",
        Scene::Players1PBonus2 => "players-bonus2",
        Scene::Explain => "explain",
        Scene::Startup => "startup",
        Scene::OnePGame => "1p-game",
        Scene::PlayersVs => "players-vs",
        Scene::Maps => "maps",
        Scene::BonusStage => "bonus-stage",
        Scene::Message => "message",
        Scene::Opening(_) => "opening",
    }
}

/// The files of `MScene` `s`, without any battle it starts.
pub fn scene_roots(s: Scene) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    // The opening scenes' demo fighters (`mvOpening*FuncStart`'s
    // `ftManagerSetupFilesAllKind` calls), by `FTKind`.
    // SAFETY: as `base`.
    let room = unsafe { ROOM_FIGURES };
    let room_kinds: [u32; 3] = match room {
        Some((pulled, dropped)) => [12, pulled as u32, dropped as u32],
        None => [12; 3],
    };
    // The Run's and Clash's eight, by immediates in the same order as
    // `mvOpeningRunMakeFighters`' `fkinds` (a ROM table, in the pack).
    let run_kinds: [u32; 8] = ssb_game::opening::run::KINDS.map(|k| k as u32);
    let opening_kinds: &[u32] = match s {
        // With its figures once picked (RE-476).
        Scene::Opening(Kind::Room) => &room_kinds,
        Scene::Opening(Kind::Run | Kind::Clash) => &run_kinds,
        Scene::Opening(Kind::Yoster) => &[6],
        Scene::Opening(Kind::Cliff) => &[5],
        Scene::Opening(Kind::Standoff) => &[0, 8],
        Scene::Opening(Kind::Yamabuki) => &[9],
        _ => &[],
    };
    let pack = scene_files::pack();
    if !opening_kinds.is_empty() {
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(opening_kinds.iter().map(|&k| r::FIGHTER_MAIN[k as usize]));
        // The figures' figatrees: the N64 reads each per status into the
        // figure's figatree heap (`desc.figatree_heap`); here each streams
        // on its first use ([`stream_scene`], RE-572 option C).
    }
    // The opening's fights (`mvOpening<Kind>FuncStart`, `mvOpeningJungle`):
    // the stage and fighters, read with the scene rather than when its
    // battle starts a few frames later.
    let opening_fight: Option<(u32, &[u32])> = match s {
        Scene::Opening(Kind::Mario) => Some((0, &[0])),
        Scene::Opening(Kind::Fox) => Some((1, &[1])),
        Scene::Opening(Kind::Donkey) => Some((2, &[2])),
        Scene::Opening(Kind::Samus) => Some((3, &[3])),
        Scene::Opening(Kind::Link) => Some((4, &[5])),
        Scene::Opening(Kind::Yoshi) => Some((5, &[6])),
        Scene::Opening(Kind::Kirby) => Some((6, &[8])),
        Scene::Opening(Kind::Pikachu) => Some((7, &[9])),
        Scene::Opening(Kind::Jungle) => Some((2, &[2, 3])),
        _ => None,
    };
    // The clips the scene's fighters start with, which the N64 reads as
    // `FuncStart` sets their statuses: read with the scene, so its first
    // frames do not wait on them (RE-574). Later statuses stream on their
    // first use, but for the fights': their read traces list them (RE-575).
    if let (Scene::Opening(k), Some(p)) = (s, pack) {
        if let Some((_, kinds)) = opening_fight {
            r::opening_fight_files(p, kinds, &mut v);
        }
        for (kind, row) in ssb_game::opening::first_rows(k, room) {
            if let Some(slot) = ssb_psp_runtime::movie::demo_row_slot(row) {
                for f in r::clip_files(p, [kind as u32], &[slot]) {
                    if !v.contains(&f) {
                        v.push(f);
                    }
                }
            }
        }
    }
    if let Some((gkind, kinds)) = opening_fight {
        v.extend(*r::GM_COMMON);
        v.extend(r::EF_COMMON);
        v.extend(r::IT_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(r::ground_map(gkind));
        v.extend(kinds.iter().map(|&k| r::FIGHTER_MAIN[k as usize]));
        // The battle's own files, which `Session::enter` holds when the
        // fight starts: read with the scene (RE-476).
        if let Some(p) = pack {
            v.extend(battle_roots(p, gkind as u8, kinds));
        }
    }
    let playable = || r::FIGHTER_MAIN[..12].iter().copied();
    // A character select: its files, every playable kind, and the demo
    // clips it poses them with (`Wait`, then `Win1` to `Win4` once
    // picked), which the N64 reads per status into each figatree heap.
    let players = |v: &mut Vec<u32>, files: &[u32]| {
        v.extend_from_slice(files);
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(playable());
        if let Some(p) = pack {
            v.extend(r::clip_files(p, 0..12, &r::SELECT_CLIP_SLOTS));
        }
    };
    match s {
        Scene::Startup => v.extend(r::STARTUP),
        Scene::Title => v.extend(*r::TITLE),
        Scene::ModeSelect => v.extend(*r::MODE_SELECT),
        Scene::OnePMode => v.extend(*r::ONE_P_MODE),
        Scene::VsMode => v.extend(*r::VS_MODE),
        Scene::VsOptions => v.extend(*r::VS_OPTIONS),
        Scene::VsItemSwitch => v.extend(*r::ITEM_SWITCH),
        Scene::Option => v.extend(*r::OPTION),
        Scene::ScreenAdjust => v.extend(*r::SCREEN_ADJUST),
        Scene::BackupClear => v.extend(*r::BACKUP_CLEAR),
        Scene::Data => v.extend(*r::DATA),
        Scene::SoundTest => v.extend(*r::SOUND_TEST),
        Scene::Message => v.extend(*r::MESSAGE),
        Scene::VsRecord => v.extend(*r::VS_RECORD),
        Scene::Characters => {
            v.extend(*r::CHARACTERS);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(playable());
        }
        Scene::PlayersVs => players(&mut v, &*r::PLAYERS_VS),
        Scene::Players1PGame => players(&mut v, &*r::PLAYERS_1P),
        Scene::Players1PTraining => players(&mut v, &*r::PLAYERS_TRAINING),
        Scene::Players1PBonus1 | Scene::Players1PBonus2 => players(&mut v, &*r::PLAYERS_BONUS),
        // The preview of the cursor's stage: `maps`.
        Scene::Maps => v.extend(*r::MAPS),
        Scene::Explain => {
            v.extend(r::EXPLAIN);
            // Its battle's, held from the scene's start (RE-476).
            if let Some(p) = pack {
                v.extend(explain_battle_roots(p));
            }
        }
        Scene::AutoDemo => v.extend(r::AUTO_DEMO),
        Scene::OnePGame | Scene::BonusStage => {}
        Scene::Opening(k) => v.extend_from_slice(match k {
            Kind::Room => &r::OPENING_ROOM[..],
            Kind::Portraits => &*r::OPENING_PORTRAITS,
            Kind::Mario
            | Kind::Donkey
            | Kind::Link
            | Kind::Samus
            | Kind::Yoshi
            | Kind::Kirby
            | Kind::Fox
            | Kind::Pikachu => &*r::OPENING_FIGHTER,
            Kind::Run => &*r::OPENING_RUN,
            Kind::Cliff => &*r::OPENING_CLIFF,
            Kind::Yamabuki => &*r::OPENING_YAMABUKI,
            Kind::Jungle => &*r::OPENING_JUNGLE,
            Kind::Yoster => &*r::OPENING_YOSTER,
            Kind::Sector => &*r::OPENING_SECTOR,
            Kind::Standoff => &*r::OPENING_STANDOFF,
            Kind::Clash => &*r::OPENING_CLASH,
            Kind::Newcomers => &*r::OPENING_NEWCOMERS,
        }),
    }
    v
}

/// How to Play's battle files (`demo_screen::start_explain`).
pub fn explain_battle_roots(pack: &Pack<'_>) -> Vec<u32> {
    let kinds = ssb_game::explain::FIGHTERS.map(|k| k as u32);
    battle_roots(pack, ssb_game::explain::GKIND, &kinds)
}

/// The read-aheads [`read_ahead`] started since the scene began, by name
/// and key.
static mut AHEAD: Vec<(&'static str, u32)> = Vec::new();

fn ahead() -> &'static mut Vec<(&'static str, u32)> {
    // SAFETY: as `base`.
    unsafe { &mut *core::ptr::addr_of_mut!(AHEAD) }
}

/// [`prefetch`]es `roots()` for `name` once per scene for each `key` (a
/// stage, say): the files the scene's next step will want, read while it
/// runs. Nothing the game ticks or draws depends on it: a file not yet
/// read when the next scene starts is read then (D-046).
pub fn read_ahead(name: &'static str, key: u32, roots: impl FnOnce() -> Vec<u32>) {
    if ahead().contains(&(name, key)) {
        return;
    }
    ahead().push((name, key));
    prefetch(name, roots());
}

/// Starts reading `roots`, the files of the scene expected next, in the
/// background (`scene_files::prefetch`, RE-476).
pub fn prefetch(name: &'static str, roots: Vec<u32>) {
    let t = ssb_psp_runtime::profile::start();
    scene_files::prefetch(&roots);
    let t = unsafe { psp::sys::sceKernelGetSystemTimeLow() }.wrapping_sub(t);
    if ssb_psp_runtime::profile::ENABLED {
        let line = alloc::format!(
            "prefetch tick={} scene={} roots={} cpu_us={}\n",
            ssb_psp_runtime::profile::tick(),
            name,
            roots.len(),
            t
        );
        ssb_psp_runtime::profile::log(line.as_bytes());
    }
}

/// The files of a battle on stage `gkind` among `kinds` (`FTKind`
/// ordinals): [`fighter_battle_roots`] and [`stage_battle_roots`].
pub fn battle_roots(pack: &Pack<'_>, gkind: u8, kinds: &[u32]) -> Vec<u32> {
    let mut v = fighter_battle_roots(pack, kinds);
    v.extend(stage_battle_roots(gkind));
    v
}

/// A battle's files that do not depend on its stage: the interface,
/// effects, items, the fighter manager's files, and each fighter's main
/// closure and pinned figatrees ([`pinned_figatrees`]); its other
/// figatrees stream ([`stream_scene`]).
pub fn fighter_battle_roots(pack: &Pack<'_>, kinds: &[u32]) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    v.extend(*r::GM_COMMON);
    v.extend(r::EF_COMMON);
    v.extend(r::IT_COMMON);
    v.extend(r::FT_MANAGER);
    for &k in kinds {
        if let Some(&main) = (*r::FIGHTER_MAIN).get(k as usize) {
            v.push(main);
        }
        v.extend(pinned_figatrees(pack, k));
    }
    v
}

/// The figatrees of kind `kind` a battle holds from its start
/// ([`r::PINNED_CLIP_SLOTS`]); the N64 reads every status's as the status
/// starts (`ftMainSetStatus`), the port the others on first use.
pub fn pinned_figatrees(pack: &Pack<'_>, kind: u32) -> Vec<u32> {
    r::clip_files(pack, [r::anim_kind(kind)], &r::PINNED_CLIP_SLOTS)
}

/// A battle's files for stage `gkind`: the map's closure and the stage
/// kind's own files.
pub fn stage_battle_roots(gkind: u8) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    v.extend(r::ground_map(u32::from(gkind)));
    match gkind {
        17..=28 => v.extend(r::BONUS1),
        29..=40 => v.extend(r::BONUS2),
        11 => v.extend(r::EXPLAIN),
        14 => v.extend(r::ZAKO),
        _ => {}
    }
    v
}

/// The stage select's files as it starts (`mnMapsFuncStart`): its listed
/// files and the preview of the stage its cursor opens on, `gkind`
/// ([`maps_preview_roots`]). The N64 reads no other stage's preview until
/// the cursor lands on it (RE-572).
pub fn maps_roots(training: bool, gkind: u8) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::from(*r::MAPS);
    v.extend(maps_preview_roots(training, gkind));
    v
}

/// `mnMapsMakePreview`'s files for `gkind` (`ssb_rom::scene_roots::
/// maps_preview`): none for the random slot.
pub fn maps_preview_roots(training: bool, gkind: u8) -> Vec<u32> {
    r::maps_preview(gkind, training).map_or_else(Vec::new, |f| f.to_vec())
}

/// `mnMapsFuncStart`: the stage select holds its own files, and the
/// preview of `gkind`, the stage its cursor opens on.
pub fn maps(training: bool, gkind: u8) {
    *base() = Vec::from(*r::MAPS);
    // SAFETY: as `base`.
    unsafe { BASE_IN_BATTLE = Base::Own };
    load("maps", &maps_preview_roots(training, gkind), Base::Own);
}

/// `mnMapsMakePreview` after a cursor move: reads `gkind`'s preview now,
/// in the tick, as `lbRelocGetForceExternHeapFile` does on the N64 (the
/// frame runs long and the last picture stays on screen until it ends),
/// and drops the last preview's files at the frame's safe point, as the
/// N64's next preview reuses that heap. The battle files read ahead
/// carry on (`scene_files::enter_within`).
pub fn maps_preview(training: bool, gkind: u8) {
    let mut roots = base().clone();
    roots.extend(maps_preview_roots(training, gkind));
    enter("maps-preview", &roots, true, false);
}

/// The VS results' files: `mnVSResultsFuncStart`'s, all twelve kinds, the
/// wipes, and the demo clips its fighters play (`Win1` to `Win4`, `Lose`).
pub fn results_roots() -> Vec<u32> {
    let mut v: Vec<u32> = Vec::from(*r::VS_RESULTS);
    v.extend(r::EF_COMMON);
    v.extend(r::FT_MANAGER);
    v.extend(&r::FIGHTER_MAIN[..12]);
    v.extend(*r::TRANSITIONS);
    if let Some(p) = scene_files::pack() {
        v.extend(r::clip_files(p, 0..12, &r::RESULTS_CLIP_SLOTS));
    }
    v
}

/// The 1P manager's presentation scenes (`campaign_screen`'s scene ids):
/// intro, continue, stage clear, ending, staff roll, congratulations,
/// challenger and unlock message, for player kind `fkind`. A battle (id 3)
/// loads its own files.
pub fn campaign(id: u8, fkind: u32) {
    let fighters = |v: &mut Vec<u32>| {
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend((*r::FIGHTER_MAIN).get(fkind as usize).copied());
    };
    // The player's demo clips the scene poses it with.
    let clips = |v: &mut Vec<u32>, slots: &[usize]| {
        if let Some(p) = scene_files::pack() {
            v.extend(r::clip_files(p, [fkind], slots));
        }
    };
    let mut v: Vec<u32> = Vec::new();
    let name = match id {
        0 => {
            v.extend(*r::ONE_P_INTRO);
            fighters(&mut v);
            "1p-intro"
        }
        1 => {
            v.extend(*r::ONE_P_CONTINUE);
            fighters(&mut v);
            clips(&mut v, &r::CONTINUE_CLIP_SLOTS);
            "1p-continue"
        }
        2 => {
            v.extend(*r::ONE_P_STAGE_CLEAR);
            "1p-stage-clear"
        }
        4 => {
            v.extend(*r::ENDING);
            fighters(&mut v);
            clips(&mut v, &r::ENDING_CLIP_SLOTS);
            "ending"
        }
        5 => {
            v.extend(*r::STAFFROLL);
            "staffroll"
        }
        6 => {
            v.extend((*r::CONGRA).get(fkind as usize).into_iter().flatten().copied());
            "congra"
        }
        7 => {
            v.extend(*r::CHALLENGER);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            "challenger"
        }
        8 => {
            v.extend(*r::MESSAGE);
            "message"
        }
        _ => return,
    };
    scene(name, v);
}

/// Replaces the scene's own files, so the next [`battle`] holds `roots`
/// and its own.
pub fn set_base(roots: Vec<u32>) {
    *base() = roots;
    // SAFETY: as `base`.
    unsafe { BASE_IN_BATTLE = Base::Battle };
}

/// Kinds `kinds`' main files and pinned figatrees ([`pinned_figatrees`]).
pub fn fighter_roots(pack: &Pack<'_>, kinds: core::ops::RangeInclusive<u32>) -> Vec<u32> {
    let mut v = Vec::new();
    for k in kinds {
        v.extend((*r::FIGHTER_MAIN).get(k as usize).copied());
        v.extend(pinned_figatrees(pack, k));
    }
    v
}

/// Starts scene `name` holding `roots`.
pub fn scene(name: &'static str, roots: Vec<u32>) {
    *base() = roots;
    // SAFETY: as `base`.
    unsafe {
        BASE_IN_BATTLE = match name {
            "opening" => Base::Opening,
            "explain" | "auto-demo" => Base::Battle,
            _ => Base::Own,
        }
    };
    load(name, &[], Base::Own);
}

/// Starts battle `name` holding its files and the scene's.
pub fn battle(name: &'static str, roots: &[u32]) {
    // SAFETY: as `base`.
    let held = unsafe { BASE_IN_BATTLE };
    if held == Base::Own {
        base().clear();
    }
    load(name, roots, held);
}

/// `held`: whose files [`BASE`] holds; a battle a scene starts leaves the
/// next scene's prefetch running (`scene_files::enter_within`).
fn load(name: &'static str, extra: &[u32], held: Base) {
    ahead().clear();
    let mut roots = base().clone();
    roots.extend_from_slice(extra);
    let slabs = name == "battle" && held != Base::Opening;
    enter(name, &roots, held != Base::Own, slabs);
}

/// Per anim group (`r::anim_kind`), each figatree file its rows play, with
/// the first such row: built once ([`prepare`]). Scanning the pack's anim
/// rows cost every fight start about 3 ms on a PSP-2000 (RE-476).
static mut GROUP_FILES: Vec<Vec<(u32, u32)>> = Vec::new();

/// Builds the tables the scene loads use, about 10 ms on a PSP-2000: at a
/// boot's audio start, in the logo scene's held picture, not in the
/// opening's first frame (RE-476).
pub fn prepare() {
    if let Some(pack) = scene_files::pack() {
        group_files(&pack);
    }
}

fn group_files(pack: &ssb_rom::pack::Pack<'_>) -> &'static [Vec<(u32, u32)>] {
    // SAFETY: as `base`.
    let cache = unsafe { &mut *core::ptr::addr_of_mut!(GROUP_FILES) };
    if cache.is_empty() {
        cache.resize_with(r::FIGHTER_MAIN.len(), Vec::new);
        // `nFTKindEnumCount`: rows past it are stage, effect and item clips.
        for i in 0..pack.anim_count() {
            let Some(fighter) = pack.anim_fighter(i) else { continue };
            if fighter >= r::FIGHTER_MAIN.len() as u32 {
                continue;
            }
            let Some(a) = pack.anim(i) else { continue };
            if a.script_len > 0 {
                cache[r::anim_kind(fighter) as usize].push((i, a.source_file));
            }
        }
        // Each file once, at its first row.
        for files in cache.iter_mut() {
            files.sort_unstable_by_key(|&(row, file)| (file, row));
            files.dedup_by_key(|&mut (_, file)| file);
            files.sort_unstable_by_key(|&(row, _)| row);
            files.shrink_to_fit();
        }
    }
    cache
}

/// Marks the figatrees scene `name` streams: in a battle or an opening
/// scene, those of every fighter kind whose main file `roots` holds,
/// grouped by the kind whose set it is (`r::anim_kind`). A file the scene
/// does not hold is read on its first use into its group's slab
/// (`scene_files::stream_files`, RE-572 option C, RE-573). A battle makes
/// its slabs when it starts, as the N64 allocates each fighter's figatree
/// heap at scene setup (`scvsbattle.c:199`); the opening, whose fighters
/// come and go, and its fights (`slabs` false: they read their clips with
/// the scene, RE-575), on their first miss. Other scenes stream none:
/// their few clips load on demand, as the scene's files.
fn stream_scene(name: &str, roots: &[u32], slabs: bool) {
    let Some(pack) = scene_files::pack() else {
        return;
    };
    let mut table: Vec<(u32, u32, u8)> = Vec::new();
    if matches!(name, "battle" | "opening") {
        let mut groups: Vec<u32> = (0u32..)
            .zip(*r::FIGHTER_MAIN)
            .filter(|(_, main)| roots.contains(main))
            .map(|(k, _)| r::anim_kind(k))
            .collect();
        groups.sort_unstable();
        groups.dedup();
        for g in groups {
            if let Some(files) = group_files(&pack).get(g as usize) {
                table.extend(files.iter().map(|&(row, file)| (row, file, g as u8)));
            }
        }
        // Row order across the groups, as the table always ran: a file two
        // groups play keeps the first row's group.
        table.sort_unstable_by_key(|&(row, ..)| row);
    }
    let table: Vec<(u32, u8)> = table.into_iter().map(|(_, file, g)| (file, g)).collect();
    scene_files::stream_files(&table, slabs);
}

/// `scene_files::enter` (or `enter_within`) with `roots`, logged.
fn enter(name: &'static str, roots: &[u32], within: bool, slabs: bool) {
    let last = scene_files::report();
    let last_demand = scene_files::demand_files();
    if last.demand_files > 0 {
        crate::boot_log::log_args(format_args!(
            "scene {} loaded on demand: {} files {}B {:x?}",
            last.scene, last.demand_files, last.demand_bytes, last_demand
        ));
    }
    stream_scene(name, roots, slabs);
    let t = ssb_psp_runtime::profile::start();
    let entered = if within {
        scene_files::enter_within(name, roots)
    } else {
        scene_files::enter(name, roots)
    };
    let t = unsafe { psp::sys::sceKernelGetSystemTimeLow() }.wrapping_sub(t);
    match entered {
        Ok(rep) => {
            crate::boot_log::log_args(format_args!(
                "scene {} files={} bytes={} read={}/{}B reads={} us={} pre={}/{}B wait_us={}",
                name,
                rep.files,
                rep.bytes,
                rep.loaded_files,
                rep.loaded_bytes,
                rep.reads,
                rep.micros,
                rep.prefetched_files,
                rep.prefetched_bytes,
                rep.wait_micros
            ));
            if ssb_psp_runtime::profile::ENABLED {
                let line = alloc::format!(
                    "load tick={} scene={} files={} bytes={} read={}/{}B reads={} us={} prefetched={}/{}B wait_us={} cpu_us={} last_demand={}/{}B {:x?}\n",
                    ssb_psp_runtime::profile::tick(),
                    name,
                    rep.files,
                    rep.bytes,
                    rep.loaded_files,
                    rep.loaded_bytes,
                    rep.reads,
                    rep.micros,
                    rep.prefetched_files,
                    rep.prefetched_bytes,
                    rep.wait_micros,
                    t,
                    last.demand_files,
                    last.demand_bytes,
                    last_demand
                );
                ssb_psp_runtime::profile::log(line.as_bytes());
            }
        }
        Err(e) => {
            crate::boot_log::log_args(format_args!("scene {} load failed: {}", name, e.as_str()));
            ssb_psp_runtime::memory::fatal(&[
                "Could not load the game's data from ssb64.pak:",
                e.as_str(),
            ]);
        }
    }
}
