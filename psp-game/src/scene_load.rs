//! Which archive files each scene holds (RE-475, D-046): the N64 scene's
//! roots (`ssb_rom::scene_roots`) handed to
//! `ssb_psp_runtime::scene_files::enter`.
//!
//! A scene sets its own roots with [`scene`]; a battle the scene starts
//! (an opening's fight, How to Play, the title demo, VS, Training, the 1P
//! Game) adds the battle's with [`battle`], keeping the scene's.

use alloc::vec::Vec;

use ssb_game::menu::Scene;
use ssb_game::opening::Kind;
use ssb_psp_runtime::scene_files;
use ssb_rom::pack::Pack;
use ssb_rom::scene_roots as r;

/// The roots of the scene last set with [`scene`].
static mut BASE: Vec<u32> = Vec::new();

/// The opening room's two random figures (`mvOpeningRoomInitVars`), by
/// `FTKind`, once [`set_room_figures`] has named them.
static mut ROOM_FIGURES: Option<[u32; 2]> = None;

/// Names the opening room's pulled and dropped figures, whose files its
/// list then holds (RE-476).
pub fn set_room_figures(figures: [u32; 2]) {
    // SAFETY: the game thread alone calls this module.
    unsafe { ROOM_FIGURES = Some(figures) };
}

/// Whether a battle keeps [`BASE`]: the scenes that are a battle (an
/// opening fight, How to Play, the title demo, the 1P Game's stages). A VS
/// or Training battle leaves its select's and stage select's files.
static mut BASE_IN_BATTLE: bool = false;

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
        Some([pulled, dropped]) => [12, pulled, dropped],
        None => [12; 3],
    };
    let opening_kinds: &[u32] = match s {
        // With its figures once picked (RE-476).
        Scene::Opening(Kind::Room) => &room_kinds,
        Scene::Opening(Kind::Run | Kind::Clash) => &[0, 1, 2, 3, 5, 6, 8, 9],
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
        // The figures' figatrees, which the N64 reads per status into each
        // figure's figatree heap (`desc.figatree_heap`): read with the
        // scene, as a battle's are (D-046), not on demand mid-scene.
        if let Some(p) = pack {
            for &k in opening_kinds {
                v.extend(figatrees(p, r::anim_kind(k)));
            }
        }
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
    if let Some((gkind, kinds)) = opening_fight {
        v.extend(r::GM_COMMON);
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
    match s {
        Scene::Startup => v.extend(r::STARTUP),
        Scene::Title => v.extend(r::TITLE),
        Scene::ModeSelect => v.extend(r::MODE_SELECT),
        Scene::OnePMode => v.extend(r::ONE_P_MODE),
        Scene::VsMode => v.extend(r::VS_MODE),
        Scene::VsOptions => v.extend(r::VS_OPTIONS),
        Scene::VsItemSwitch => v.extend(r::ITEM_SWITCH),
        Scene::Option => v.extend(r::OPTION),
        Scene::ScreenAdjust => v.extend(r::SCREEN_ADJUST),
        Scene::BackupClear => v.extend(r::BACKUP_CLEAR),
        Scene::Data => v.extend(r::DATA),
        Scene::SoundTest => v.extend(r::SOUND_TEST),
        Scene::Message => v.extend(r::MESSAGE),
        Scene::VsRecord => v.extend(r::VS_RECORD),
        Scene::Characters => {
            v.extend(r::CHARACTERS);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(playable());
        }
        Scene::PlayersVs => {
            v.extend(r::PLAYERS_VS);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(playable());
        }
        Scene::Players1PGame => {
            v.extend(r::PLAYERS_1P);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(playable());
        }
        Scene::Players1PTraining => {
            v.extend(r::PLAYERS_TRAINING);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(playable());
        }
        Scene::Players1PBonus1 | Scene::Players1PBonus2 => {
            v.extend(r::PLAYERS_BONUS);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(playable());
        }
        Scene::Maps => v.extend(r::MAPS),
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
            Kind::Portraits => &r::OPENING_PORTRAITS,
            Kind::Mario
            | Kind::Donkey
            | Kind::Link
            | Kind::Samus
            | Kind::Yoshi
            | Kind::Kirby
            | Kind::Fox
            | Kind::Pikachu => &r::OPENING_FIGHTER,
            Kind::Run => &r::OPENING_RUN,
            Kind::Cliff => &r::OPENING_CLIFF,
            Kind::Yamabuki => &r::OPENING_YAMABUKI,
            Kind::Jungle => &r::OPENING_JUNGLE,
            Kind::Yoster => &r::OPENING_YOSTER,
            Kind::Sector => &r::OPENING_SECTOR,
            Kind::Standoff => &r::OPENING_STANDOFF,
            Kind::Clash => &r::OPENING_CLASH,
            Kind::Newcomers => &r::OPENING_NEWCOMERS,
        }),
    }
    v
}

/// How to Play's battle files (`demo_screen::start_explain`).
pub fn explain_battle_roots(pack: &Pack<'_>) -> Vec<u32> {
    let kinds = ssb_game::explain::FIGHTERS.map(|k| k as u32);
    battle_roots(pack, ssb_game::explain::GKIND, &kinds)
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
/// ordinals): the interface, effects, items, the fighter manager's files,
/// the stage's map closure, and each fighter's main closure and figatrees
/// (all of them: the N64 streams one per status from the cartridge,
/// which a memory stick is too slow for).
pub fn battle_roots(pack: &Pack<'_>, gkind: u8, kinds: &[u32]) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    v.extend(r::GM_COMMON);
    v.extend(r::EF_COMMON);
    v.extend(r::IT_COMMON);
    v.extend(r::FT_MANAGER);
    v.extend(r::ground_map(u32::from(gkind)));
    match gkind {
        17..=28 => v.extend(r::BONUS1),
        29..=40 => v.extend(r::BONUS2),
        11 => v.extend(r::EXPLAIN),
        14 => v.extend(r::ZAKO),
        _ => {}
    }
    for &k in kinds {
        if let Some(&main) = r::FIGHTER_MAIN.get(k as usize) {
            v.push(main);
        }
        v.extend(figatrees(pack, r::anim_kind(k)));
    }
    v
}

/// [`figatrees`] of every kind, from one scan of the animation table the
/// first time any is asked for: the scan decodes all 6,311 rows, a few
/// milliseconds a pass (RE-476).
static mut FIGATREES: Option<Vec<(u32, Vec<u32>)>> = None;

/// The archive files of `kind`'s animation rows.
pub fn figatrees(pack: &Pack<'_>, kind: u32) -> Vec<u32> {
    // SAFETY: as `base`.
    let cache = unsafe { &mut *core::ptr::addr_of_mut!(FIGATREES) };
    let table = cache.get_or_insert_with(|| scan_figatrees(pack));
    table
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or_else(Vec::new, |(_, v)| v.clone())
}

fn scan_figatrees(pack: &Pack<'_>) -> Vec<(u32, Vec<u32>)> {
    let mut table: Vec<(u32, Vec<u32>)> = Vec::new();
    for a in (0..pack.anim_count()).filter_map(|i| pack.anim(i)) {
        if a.script_len == 0 {
            continue;
        }
        match table.iter_mut().find(|(k, _)| *k == a.fighter) {
            Some((_, v)) => v.push(a.source_file),
            None => table.push((a.fighter, alloc::vec![a.source_file])),
        }
    }
    for (_, v) in &mut table {
        v.sort_unstable();
        v.dedup();
    }
    table
}

/// The 1P manager's presentation scenes (`campaign_screen`'s scene ids):
/// intro, continue, stage clear, ending, staff roll, congratulations,
/// challenger and unlock message, for player kind `fkind`. A battle (id 3)
/// loads its own files.
pub fn campaign(id: u8, fkind: u32) {
    let fighters = |v: &mut Vec<u32>| {
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(r::FIGHTER_MAIN.get(fkind as usize).copied());
    };
    let mut v: Vec<u32> = Vec::new();
    let name = match id {
        0 => {
            v.extend(r::ONE_P_INTRO);
            fighters(&mut v);
            "1p-intro"
        }
        1 => {
            v.extend(r::ONE_P_CONTINUE);
            fighters(&mut v);
            "1p-continue"
        }
        2 => {
            v.extend(r::ONE_P_STAGE_CLEAR);
            "1p-stage-clear"
        }
        4 => {
            v.extend(r::ENDING);
            fighters(&mut v);
            "ending"
        }
        5 => {
            v.extend(r::STAFFROLL);
            "staffroll"
        }
        6 => {
            v.extend(r::CONGRA.get(fkind as usize).into_iter().flatten().copied());
            "congra"
        }
        7 => {
            v.extend(r::CHALLENGER);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            "challenger"
        }
        8 => {
            v.extend(r::MESSAGE);
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
    unsafe { BASE_IN_BATTLE = true };
}

/// Kinds `kinds`' main files and figatrees.
pub fn fighter_roots(pack: &Pack<'_>, kinds: core::ops::RangeInclusive<u32>) -> Vec<u32> {
    let mut v = Vec::new();
    for k in kinds {
        v.extend(r::FIGHTER_MAIN.get(k as usize).copied());
        v.extend(figatrees(pack, r::anim_kind(k)));
    }
    v
}

/// Starts scene `name` holding `roots`.
pub fn scene(name: &'static str, roots: Vec<u32>) {
    *base() = roots;
    // SAFETY: as `base`.
    unsafe { BASE_IN_BATTLE = matches!(name, "opening" | "explain" | "auto-demo") };
    load(name, &[], false);
}

/// Starts battle `name` holding its files and the scene's.
pub fn battle(name: &'static str, roots: &[u32]) {
    // SAFETY: as `base`.
    let within = unsafe { BASE_IN_BATTLE };
    if !within {
        base().clear();
    }
    load(name, roots, within);
}

/// `within`: a battle the scene starts, which leaves the next scene's
/// prefetch running (`scene_files::enter_within`).
fn load(name: &'static str, extra: &[u32], within: bool) {
    let mut roots = base().clone();
    roots.extend_from_slice(extra);
    let last = scene_files::report();
    let last_demand = scene_files::demand_files();
    if last.demand_files > 0 {
        crate::boot_log::log_args(format_args!(
            "scene {} loaded on demand: {} files {}B {:x?}",
            last.scene, last.demand_files, last.demand_bytes, last_demand
        ));
    }
    let t = ssb_psp_runtime::profile::start();
    let entered = if within {
        scene_files::enter_within(name, &roots)
    } else {
        scene_files::enter(name, &roots)
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
