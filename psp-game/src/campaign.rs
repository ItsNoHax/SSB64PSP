//! The 1P Game on the PSP: the portable campaign frontend
//! (`ssb_game::spgame::Frontend`, RE-448) between the 1P select and real
//! battles (`sc1PGameFuncStart`, RE-450).
//!
//! The select's START begins a campaign; its intro, continue and
//! stage-clear controllers run here, and a requested battle runs on the
//! shared Training/VS world with the campaign's `Session` collecting its
//! callbacks, falls and enemy replacements. Master Hand's stage runs its
//! boss scene ([`BossScene`]: `sc1pgameboss.c`'s wallpaper and fades and
//! `sc1pgame.c`'s camera animations and defeat). After the last stage the
//! ending movie, staff roll and congratulations run, then a challenger's
//! warning, battle and unlock message (`ssb_game::spgame`). A scene the PSP
//! cannot run yet (a staff roll whose ROM data the pack lacks, or a battle
//! whose fighters the pack lacks) stops the campaign with an explicit blocked screen: it is never
//! replaced by a VS battle or skipped. Authored presentation is bound by
//! campaign_screen.

use super::*;
#[path = "campaign_screen.rs"]
mod presentation;
use ssb_game::spgame::{
    self,
    frontend::{Event, Frontend, Screen as Scene1P},
    manager::Scene,
    session::Session as Campaign1P,
    wait::Action,
    Stage,
};

/// The VS results' unlock message, drawn as the 1P Game's.
pub(crate) unsafe fn draw_message(gpu: &mut Gpu, p: &Pack<'_>, st: &mut meshdraw::DrawState, m: &spgame::message::Message) {
    presentation::draw_message(gpu, p, st, m);
}

/// The campaign the PSP is running, across its scenes.
pub(crate) struct Campaign {
    pub bonus: Option<alloc::boxed::Box<spgame::bonus_stage::BonusStage>>,
    /// Final Destination's boss scene while Master Hand's battle runs.
    pub boss: Option<alloc::boxed::Box<BossScene>>,
    pub frontend: alloc::boxed::Box<Frontend>,
    /// The current overlay's scheduler clock; `sc1PIntroFuncStart` and each
    /// scene start reset it.
    tic: u32,
    /// Why the campaign stopped, at a scene it cannot run yet.
    blocked: Option<Blocked>,
    presentation: presentation::Presentation,
    /// Bonus 1 or 2 Practice (`scene_prev` an `nSCKind1PBonus*Players`):
    /// the course runs alone and goes back to its select.
    pub practice: Option<ssb_game::players_1p_bonus::BonusKind>,
}

/// A scene the host cannot run yet.
#[derive(Clone, Copy)]
enum Blocked {
    /// No PSP controller yet: ending, challenger, message.
    Scene(Scene),
    /// A stage, fighter attributes or fighter model is missing from the pack.
    Assets(Stage),
}

impl Campaign {
    fn session(&mut self) -> &mut Campaign1P {
        self.frontend.session.as_mut().expect("campaign session")
    }
}

/// `nSCKind1PGame` from the 1P select's ready START, after the select
/// applied its scene and backup data.
pub(crate) fn start(s: &mut Session, pack: Option<&Pack<'_>>) {
    let mut frontend = alloc::boxed::Box::new(Frontend::campaign(s.spgame_scene.clone(), &s.backup));
    // The staff roll's tables and name motion, from the pack.
    frontend.staffroll_assets = pack.and_then(ssb_psp_runtime::ending::staffroll_assets);
    s.campaign = Some(Campaign {
        bonus: None,
        boss: None,
        frontend,
        tic: 0,
        blocked: None,
        presentation: presentation::Presentation::default(),
        practice: None,
    });
    s.screen = Screen::Campaign;
}

/// `nSCKind1PBonusStage` from a Bonus Practice select
/// (`sc1PBonusStageFuncStart`'s practice branch): the practice fighter
/// and costume on the course, with no time limit.
pub(crate) fn start_practice(
    s: &mut Session,
    pack: Option<&Pack<'_>>,
    saved: ssb_game::players_1p_bonus::Saved,
    bonus: ssb_game::players_1p_bonus::BonusKind,
) {
    use ssb_game::players_1p_bonus::BonusKind;
    let mut data = s.spgame_scene.clone();
    data.player = saved.player;
    data.fkind = saved.bonus_fkind.unwrap_or(ssb_game::fighter::FighterKind::Mario);
    data.costume = saved.bonus_costume;
    let fkind = data.fkind;
    let mut frontend = alloc::boxed::Box::new(Frontend::campaign(data, &s.backup));
    if let Some(sp) = frontend.session.as_mut() {
        // `sc1PBonusStageSetupFiles`' practice branch: the course of the
        // fighter, `SCBATTLE_TIMELIMIT_INFINITE`, the practice's costume
        // (the error flag's Mario does not apply).
        sp.data.player = saved.player;
        sp.data.fkind = fkind;
        sp.data.costume = saved.bonus_costume;
        sp.data.stage = if bonus == BonusKind::Targets { Stage::Bonus1 } else { Stage::Bonus2 } as u8;
        sp.data.time_limit = ssb_game::battle::TIMELIMIT_INFINITE;
        sp.data.is_reset = false;
        sp.manager.scene = Scene::BonusStage;
    }
    s.campaign = Some(Campaign {
        bonus: None,
        boss: None,
        frontend,
        tic: 0,
        blocked: None,
        presentation: presentation::Presentation::default(),
        practice: Some(bonus),
    });
    s.screen = Screen::Campaign;
    on_host(s, pack, Scene::BonusStage);
}

/// Ends the campaign for `next`: the 1P mode menu or the N64 logo (which
/// is skipped to the title).
// Out of line so `on_host`'s frame holds only one scene's locals: the
// game thread's stack is 512 KiB (RE-469).
#[inline(never)]
fn leave(s: &mut Session, pack: Option<&Pack<'_>>, next: ssb_game::menu::Scene) {
    s.campaign = None;
    s.leave_battle();
    s.vs_battle = None;
    crate::go_scene(s, pack, next, ssb_game::menu::Scene::OnePGame, false);
}

/// One frame of the campaign's own scenes: the intro, continue and
/// stage-clear controllers, or the blocked screen.
#[inline(never)]
pub(crate) fn frame(
    s: &mut Session,
    pack: Option<&Pack<'_>>,
    controller: ControllerState,
    pressed: N64Buttons,
) {
    let Some(c) = s.campaign.as_mut() else {
        crate::go_scene(s, pack, ssb_game::menu::Scene::OnePMode, ssb_game::menu::Scene::OnePGame, false);
        return;
    };
    if c.blocked.is_some() {
        if pressed.contains(N64Buttons::START) || pressed.contains(N64Buttons::B) {
            leave(s, pack, ssb_game::menu::Scene::OnePMode);
        }
        return;
    }
    let before = core::mem::discriminant(&c.frontend.screen);
    c.tic += 1;
    let mut host = None;
    c.frontend
        .tick(c.tic, controller, pressed, &mut s.backup, |e| {
            if let Event::Host(scene) = e {
                host = Some(scene);
            }
        });
    if core::mem::discriminant(&c.frontend.screen) != before {
        c.tic = 0;
    }
    if let (Some(p), Some(sp)) = (pack, c.frontend.session.as_ref()) {
        c.presentation.tick(p, &c.frontend.screen, sp);
    }
    if let Some(scene) = host {
        on_host(s, pack, scene);
    }
}

/// A scene request from the frontend.
fn on_host(s: &mut Session, pack: Option<&Pack<'_>>, scene: Scene) {
    match scene {
        Scene::BonusStage => match enter_bonus(s, pack) {
            Ok(()) => s.screen = Screen::Training,
            Err(blocked) => s.campaign.as_mut().expect("campaign").blocked = Some(blocked),
        },
        Scene::Battle => match enter_battle(s, pack) {
            Ok(()) => s.screen = Screen::Training,
            Err(blocked) => {
                if let Some(c) = s.campaign.as_mut() {
                    c.blocked = Some(blocked);
                }
            }
        },
        Scene::ModeMenu => leave(s, pack, ssb_game::menu::Scene::OnePMode),
        Scene::Startup => leave(s, pack, ssb_game::menu::Scene::Startup),
        // After Luigi's challenge (`sc1PManagerUpdateScene`): the Bonus 1
        // Practice select, entered from the bonus stage.
        Scene::Bonus1Select => {
            s.campaign = None;
            s.leave_battle();
            s.vs_battle = None;
            crate::go_scene(s, pack, ssb_game::menu::Scene::Players1PBonus1, ssb_game::menu::Scene::BonusStage, false);
        }
        other => {
            if let Some(c) = s.campaign.as_mut() {
                c.blocked = Some(Blocked::Scene(other));
            }
        }
    }
}

/// `sc1PGameFuncStart`: the manager's stage setup and the fighters it
/// describes, on the shared battle world.
// Out of line so `on_host`'s frame holds only one scene's locals: the
// game thread's stack is 512 KiB (RE-469).
#[inline(never)]
fn enter_battle(s: &mut Session, pack: Option<&Pack<'_>>) -> Result<(), Blocked> {
    let c = s.campaign.as_mut().expect("campaign");
    let sp = c.session();
    let stage = sp.data.stage().unwrap_or(Stage::Link);
    sp.start_battle(&s.backup);
    let p = pack.ok_or(Blocked::Assets(stage))?;
    let gkind = sp.state.gkind;
    let index =
        ssb_psp_runtime::scene::common_stage_index(p, gkind).ok_or(Blocked::Assets(stage))?;
    let desc = p.stage(index).ok_or(Blocked::Assets(stage))?;
    let game = sp.game.as_ref().expect("active game");
    // The pad drives the scene's player on port 0.
    if sp.data.player != 0 {
        return Err(Blocked::Assets(stage));
    }
    let mut roster: Roster = [None; 4];
    for (i, b) in sp.state.present() {
        let setup = game.setups[i];
        if p.fighter(b.fkind as u32).is_none()
            || ssb_psp_runtime::scene::fighter_object(p, b.fkind as u32).is_none()
            || p.spawn(&desc, setup.mapobj_kind).is_none() {
            return Err(Blocked::Assets(stage));
        }
        roster[i] = Some(Entrant {
            kind: b.fkind,
            costume: b.costume,
            shade: b.shade,
            // `sc1PGameSetupEnemyPlayer`'s CP, an ally's heart
            // (`sc1pgame.c:965,1075`), the player's port.
            tag: Some(b.tag as u8),
            level: b.level,
            handicap: b.handicap,
            spawn: setup.mapobj_kind,
            team: b.team,
            color: b.color,
            human: b.pkind == spgame::PlayerKind::Man,
        });
    }
    let setups = game.setups;
    let flash_alpha = game.screenflash_alpha();
    let single = sp.state.players.map(|b| b.is_single_stockicon);
    let switches = ssb_game::item::normal::Switches {
        appearance: sp.state.item_appearance,
        toggles: sp.state.item_toggles,
    };
    let count = sp.state.pl_count + sp.state.cp_count;
    let positions = spgame::wait::interface_positions(sp.data.player, count);
    let rules = VsRules {
        rule: ssb_game::battle::Rule::Time,
        time_limit: sp.state.time_limit,
        stocks: 0,
        team_rules: ssb_game::team::TeamRules {
            is_team_battle: true,
            is_team_attack: sp.state.is_team_attack,
        },
        items: switches,
        damage_ratio: ssb_game::stale::DAMAGE_RATIO_DEFAULT,
        demo: false,
        skip_entry: false,
    };
    // The 1P manager's battle drops the last presentation scene's files.
    // `sc1PGameFuncStart`'s extras: the Kirby team's copies and the twelve
    // Polygons, whatever the roster holds now (RE-475).
    crate::scene_load::set_base(match stage {
        Stage::Kirby => alloc::vec::Vec::from(ssb_rom::scene_roots::KIRBY_TEAM),
        Stage::Zako => crate::scene_load::fighter_roots(p, 14..=25),
        _ => alloc::vec::Vec::new(),
    });
    s.enter(pack, gkind, &roster, Some(rules));
    s.scene_gkind = gkind;
    let Some(w) = s.world.as_deref_mut() else {
        return Err(Blocked::Assets(stage));
    };
    // `itManagerInitItems` with the 1P stage's switches.
    w.items.normal_switches = switches;
    w.items.normal_drops =
        ssb_game::item::normal::DropWeights::new(switches, desc.item_weights.as_ref());
    let team_bounds = ssb_game::status::BlastZone {
        top: f32::from(desc.team_bounds.top),
        bottom: f32::from(desc.team_bounds.bottom),
        left: f32::from(desc.team_bounds.left),
        right: f32::from(desc.team_bounds.right),
    };
    let c = s.campaign.as_mut().expect("campaign");
    let sp = c.frontend.session.as_mut().expect("campaign session");
    // The campaign's battle runs in the world's slot; `Session` takes it
    // back for the calls that read it.
    s.vs_battle = sp.battle.take();
    let Some(pl) = w.play_state.as_mut() else {
        return Err(Blocked::Assets(stage));
    };
    pl.camera.bonus_follow = stage == Stage::Bonus3;
    for f in scenes(pl, &mut w.dummies).into_iter().flatten() {
        let port = usize::from(f.fighter.port);
        configure(sp, f, setups[port], team_bounds);
    }
    for d in w.dummies.iter_mut().flatten() {
        let setup = setups[usize::from(d.fighter.port)];
        d.is_1p_game = true;
        d.computer.trait_kind = setup.cp_trait;
    }
    w.damage_hud.single_stock = Some(single);
    let kinds = sp.state.players.map(|b| b.fkind);
    for (port, d) in w.damage_hud.damage.iter_mut().enumerate() {
        if positions[port] != 0 {
            let is_boss = kinds[port] == ssb_game::fighter::FighterKind::Boss;
            *d = ssb_game::hud::DamageDisplay::at_kind(port, d.damage, positions[port], is_boss);
        }
    }
    // `sc1PGameFuncStart`'s `ifScreenFlashMakeInterface`: the stage's
    // flash alpha (0x80 on the Yoshi and Kirby teams' and the Polygons' stages).
    w.damage_hud.ko.flash_alpha = flash_alpha;
    c.boss = None;
    if stage == Stage::Boss {
        c.boss = Some(BossScene::new_boxed(p, &desc, &mut w.dummies)?);
    }
    c.tic = 0;
    // `sc1PGameFuncStart`: Metal Mario's and the Polygon Team's music waits
    // for Go (`sc1PGameSetGameStart`).
    if matches!(stage, Stage::MMario | Stage::Zako) {
        ssb_game::music::start_battle_prologue(desc.bgm_id);
    } else {
        ssb_game::music::start_battle(desc.bgm_id);
    }
    Ok(())
}

/// The boss stage's scene state (`sc1pgameboss.c`, `sc1pgame.c`'s boss
/// functions): the wallpaper controller, the two camera animations and
/// the point the defeat zooms on.
pub(crate) struct BossScene {
    pub wallpaper: spgame::boss::BossWallpaper,
    /// The effects' trees and joint clocks on the PSP.
    pub effects: Option<ssb_psp_runtime::boss::BossEffects>,
    intro_camera: ssb_psp_runtime::scene::CameraAnim,
    defeat_camera: ssb_psp_runtime::scene::CameraAnim,
    /// `sSC1PGameBossDefeatZoomPosition`.
    zoom: ssb_engine::math::Vec3,
    /// `sSC1PGameBossMain.bossplayer`.
    pub port: u8,
    extents: spgame::boss::Extents,
    /// `sc1PGameBossDefeatInterfaceProcUpdate` ran.
    defeat_update: bool,
}

impl BossScene {
    /// [`Self::new`] on the heap, out of line: the scene is too large for
    /// `enter_battle`'s frame on the game thread's 512 KiB stack (RE-469).
    #[inline(never)]
    fn new_boxed(
        p: &Pack<'_>,
        stage: &ssb_rom::pack::StageDesc,
        dummies: &mut Dummies,
    ) -> Result<alloc::boxed::Box<Self>, Blocked> {
        Self::new(p, stage, dummies).map(alloc::boxed::Box::new)
    }

    /// `sc1PGameBossInitWallpaper`, and `ftManagerInitFighter`'s Boss
    /// case for the enemy (`ftBossCommonSetNextAttackWait`,
    /// `...SetDefaultLineID`).
    fn new(
        p: &Pack<'_>,
        stage: &ssb_rom::pack::StageDesc,
        dummies: &mut Dummies,
    ) -> Result<Self, Blocked> {
        let missing = Blocked::Assets(Stage::Boss);
        let intro_camera = ssb_psp_runtime::scene::CameraAnim::load(
            p,
            ssb_rom::campaign::BOSS_INTRO_CAMERA_SLOT,
        )
        .ok_or(missing)?;
        let defeat_camera = ssb_psp_runtime::scene::CameraAnim::load(
            p,
            ssb_rom::campaign::BOSS_DEFEAT_CAMERA_SLOT,
        )
        .ok_or(missing)?;
        let surfaces = || ssb_psp_runtime::scene::MapSegments::new(p, stage);
        let bounds = ssb_game::computer::behave::geometry_bounds(surfaces());
        let mut port = None;
        for d in dummies.iter_mut().flatten() {
            if d.fighter.kind != ssb_game::fighter::FighterKind::Boss {
                continue;
            }
            let level = d.computer.level;
            ssb_game::boss::init(&mut d.fighter, &surfaces, level, false);
            port = Some(d.fighter.port);
        }
        let port = port.ok_or(missing)?;
        Ok(BossScene {
            wallpaper: spgame::boss::BossWallpaper::new(),
            effects: Some(ssb_psp_runtime::boss::BossEffects::new(p)),
            intro_camera,
            defeat_camera,
            zoom: ssb_engine::math::Vec3::ZERO,
            port,
            extents: spgame::boss::Extents {
                left: bounds.left,
                right: bounds.right,
                map_top: f32::from(stage.bounds.top),
                map_bottom: f32::from(stage.bounds.bottom),
            },
            defeat_update: false,
        })
    }

    /// The camera animation `gmCameraSetStatusAnim` last started: the
    /// intro's, or the defeat's once it began.
    fn camera_anim(&self) -> &ssb_psp_runtime::scene::CameraAnim {
        if self.defeat_update {
            &self.defeat_camera
        } else {
            &self.intro_camera
        }
    }

    /// `sc1PGameBossWallpaperProcUpdate` and the effects' processes, on a
    /// tick `gcRunAll` runs them.
    fn run_wallpaper(&mut self, dummies: &Dummies) {
        let damage = dummies
            .iter()
            .flatten()
            .find(|d| d.fighter.port == self.port)
            .map_or(0, |d| i32::from(d.fighter.damage));
        self.wallpaper.update(damage, self.extents);
    }
}

/// The boss stage's half of the battle frame before the world runs:
/// `sc1PGameBossDefeatInterfaceProcSet` when the battle ran it. Returns
/// nothing; the defeat's paused ticks run [`boss_frozen_frame`].
pub(crate) fn boss_frame_start(
    boss: &mut BossScene,
    pl: &mut play::FighterScene,
    b: &ssb_game::battle::Battle,
) {
    if b.boss_set {
        // `sc1PGameBossSetChangeWallpaper`, then the defeat camera from
        // the zoom point.
        boss.wallpaper.set_change();
        ssb_psp_runtime::scene::start_camera_anim(pl, &boss.defeat_camera, boss.zoom);
    }
}

/// A paused tick of Master Hand's defeat: during the zoom every process but
/// the camera's, the interface's, the effects' and the boss wallpaper's is
/// paused (`sc1PGameBossDefeatInterfaceProcUpdate`); in the slow motion the
/// camera alone runs (`ifCommonBattleBossDefeatUpdateInterface`).
pub(crate) fn boss_frozen_frame(
    p: &Pack<'_>,
    stage_index: u32,
    boss: &mut BossScene,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    b: &ssb_game::battle::Battle,
) {
    if b.boss_defeat.is_none() {
        return;
    }
    if b.boss_defeat == Some(ssb_game::battle::BossDefeat::Zoom) {
        boss.run_wallpaper(dummies);
    }
    boss_camera(p, stage_index, boss, pl, dummies);
}

/// The camera's process: a camera animation's play, or the camera status
/// the battle camera ticks.
fn boss_camera(
    p: &Pack<'_>,
    stage_index: u32,
    boss: &BossScene,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
) {
    if !ssb_psp_runtime::scene::play_camera_anim(pl, boss.camera_anim()) {
        if let Some(stage) = p.stage(stage_index) {
            ssb_game::matchtick::camera_pass(
                &mut scenes(pl, dummies),
                &ssb_psp_runtime::scene::camera_stage(&stage),
            );
        }
    }
}

/// The boss stage's half of a tick the world ran: the wallpaper's
/// processes, the camera animation's play, and Master Hand's defeat
/// (`sc1PGameBossDefeatInitInterface`, then its first interface update).
pub(crate) fn boss_after_world(
    boss: &mut BossScene,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    b: &mut ssb_game::battle::Battle,
) {
    boss.run_wallpaper(dummies);
    ssb_psp_runtime::scene::play_camera_anim(pl, boss.camera_anim());
    if boss.wallpaper.done {
        // `ifCommonBattleEndSetBossDefeat`.
        b.boss_wallpaper_done();
    }
    let defeated = dummies.iter_mut().flatten().find_map(|d| {
        core::mem::take(&mut d.fighter.boss.defeated).then_some(d.fighter.port)
    });
    if let Some(port) = defeated {
        boss_defeat_init(boss, pl, dummies, b, port);
    }
}

/// `sc1PGameBossDefeatInitInterface`: the tags hide, the hit points
/// break apart, the camera zooms on the joint last struck and the battle
/// ends into the defeat; then `sc1PGameBossDefeatInterfaceProcUpdate`
/// locks every fighter inside the stage.
fn boss_defeat_init(
    boss: &mut BossScene,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    b: &mut ssb_game::battle::Battle,
    port: u8,
) {
    let Some(f) = dummies.iter().flatten().find(|d| d.fighter.port == port) else {
        return;
    };
    // `sc1PGameBossSetCameraZoom`: `fp->joints[fp->damage_joint_id]`,
    // queried before the dead status posed the joints.
    let world = f.fighter.boss.defeat_zoom;
    boss.zoom = world;
    pl.camera.set_status_map_zoom(
        world,
        world + ssb_engine::math::Vec3::new(0.0, 0.0, ssb_game::boss::DEFEAT_ZOOM_EYE_Z),
    );
    for f in scenes(pl, dummies).into_iter().flatten() {
        // `sc1PGameBossHidePlayerTagAll`.
        f.fighter.interface.tag_bossend = true;
        // `sc1PGameBossLockPlayerControl`, `...SetIgnorePlayerMapBounds`.
        f.fighter.interface.control_disable = true;
        f.fighter.dead.is_limit_map_bounds = true;
    }
    boss.defeat_update = true;
    b.boss_defeat();
}

/// `sc1PBonusStageFuncStart`: a separate bonus world, one fighter and ten
/// objectives. Campaign stocks and accumulated battle records stay put.
// Out of line so `on_host`'s frame holds only one scene's locals: the
// game thread's stack is 512 KiB (RE-469).
#[inline(never)]
fn enter_bonus(s: &mut Session, pack: Option<&Pack<'_>>) -> Result<(), Blocked> {
    let c = s.campaign.as_mut().expect("campaign");
    let data = &c.frontend.session.as_ref().expect("campaign session").data;
    let which = data.stage().filter(|s| matches!(s, Stage::Bonus1 | Stage::Bonus2)).ok_or(Blocked::Scene(Scene::BonusStage))?;
    if data.player != 0 { return Err(Blocked::Assets(which)); }
    let p = pack.ok_or(Blocked::Assets(which))?;
    let gkind = (if which == Stage::Bonus1 { 17 } else { 29 }) + data.fkind as u8;
    let index = ssb_psp_runtime::scene::common_stage_index(p, gkind).ok_or(Blocked::Assets(which))?;
    let stage = p.stage(index).ok_or(Blocked::Assets(which))?;
    let kind = data.fkind as usize;
    let positions: alloc::vec::Vec<_> = if which == Stage::Bonus1 {
      let placement = (0..p.object_count()).filter_map(|i| p.object(i)).find(|o|
        o.source_file == 124 + kind as u32 && o.source_offset == ssb_rom::bonus1::COURSES[kind].placements)
        .ok_or(Blocked::Assets(which))?;
      if placement.node_count != 11 { return Err(Blocked::Assets(which)); }
      (1..11).filter_map(|i| p.node(placement.first_node + i))
        .map(|n| ssb_engine::math::Vec3::new(n.rest_translate[0], n.rest_translate[1], n.rest_translate[2])).collect()
    } else { alloc::vec::Vec::new() };
    if p.fighter(data.fkind as u32).is_none() || p.spawn(&stage, spgame::setup::mapobj::PLAYER).is_none() {
        return Err(Blocked::Assets(which));
    }
    let floors = ssb_rom::bonus2::platforms(p, &stage);
    let bonus = if which == Stage::Bonus1 { spgame::bonus_stage::BonusStage::targets(data) } else {
        if floors.len() != 10 || (0..6).any(|i| ssb_rom::bonus2::object(p, i).is_none() || p.item_anim(ssb_rom::bonus2::FIRST_ANIM + i as u32).is_none()) {
            return Err(Blocked::Assets(which));
        }
        let platforms: alloc::vec::Vec<_> = floors.iter().map(|f| spgame::bonus_stage::Platform {
            group: f.group, kind: spgame::bonus_stage::platform_kind(f.width), boarded: false,
        }).collect();
        spgame::bonus_stage::BonusStage::platforms(data, &platforms)
    };
    let bumpers = if which == Stage::Bonus2 {
        if let Some((graph, _)) = ssb_rom::bonus2::BUMPERS[kind] {
            let placements = (0..p.object_count()).filter_map(|i| p.object(i))
                .find(|o| (o.source_file, o.source_offset) == (137 + kind as u32, graph)).ok_or(Blocked::Assets(which))?;
            (1..placements.node_count).filter_map(|i| p.node(placements.first_node + i))
                .map(|n| ssb_engine::math::Vec3::new(n.rest_translate[0], n.rest_translate[1], n.rest_translate[2])).collect()
        } else { alloc::vec::Vec::new() }
    } else { alloc::vec::Vec::new() };
    let mut roster: Roster = [None; 4];
    roster[0] = Some(Entrant { kind: data.fkind, costume: data.costume, shade: 0, tag: None, level: 1,
        handicap: ssb_game::stale::HANDICAP_DEFAULT, spawn: spgame::setup::mapobj::PLAYER,
        team: 0, color: 0, human: true });
    let gkind = bonus.state.gkind;
    crate::scene_load::set_base(alloc::vec::Vec::new());
    s.enter(pack, gkind, &roster, Some(VsRules { rule: ssb_game::battle::Rule::Time,
        time_limit: bonus.state.time_limit, stocks: 0, team_rules: ssb_game::team::TeamRules::FREE_FOR_ALL,
        skip_entry: true,
        ..VsRules::DEFAULT }));
    s.scene_gkind = gkind;
    s.vs_battle = Some(bonus.battle());
    let Some(w) = s.world.as_deref_mut() else {
        return Err(Blocked::Assets(which));
    };
    w.items.appear = None;
    if which == Stage::Bonus1 { bonus.make_targets(&positions, &mut w.items); }
    else {
        let map = w.stage_map.as_mut().ok_or(Blocked::Assets(which))?;
        // One exact allocation: doubling growth would copy each 27 KB `Visual`
        // up to four times and hold two buffers at the last regrowth.
        map.platforms.reserve_exact(bonus.platforms.iter().flatten().count());
        for platform in bonus.platforms.iter().flatten() {
            let group = map.groups.get_mut(platform.group as usize).ok_or(Blocked::Assets(which))?;
            if !group.animated { group.status = ssb_game::map::GroupStatus::On; }
            map.platforms.push(ssb_rom::bonus2::Visual::new(p, platform.group, platform.kind).ok_or(Blocked::Assets(which))?);
        }
        for (i, pos) in bumpers.into_iter().enumerate() {
            use ssb_game::stage::StageItems;
            w.items.make_item(ssb_game::stage::StageItem::Bonus2Bumper(i as u8), pos).ok_or(Blocked::Assets(which))?;
        }
    }
    let pl = w.play_state.as_mut().ok_or(Blocked::Assets(which))?;
    ssb_game::status::set_wait_or_fall(&mut pl.fighter);
    pl.fighter.facing = if pl.fighter.pos.x >= 0.0 { ssb_game::fighter::Facing::Left } else { ssb_game::fighter::Facing::Right };
    pl.fighter.stats.enable();
    pl.fighter.dead.stock_rule = false;
    pl.fighter.dead.spgame_rule = false;
    pl.fighter.dead.bonus_rule = true;
    pl.camera.bonus_follow = true;
    w.damage_hud.damage[0] = ssb_game::hud::DamageDisplay::at(0, 0, 55);
    w.damage_hud.bonus_tasks = Some(10);
    let c = s.campaign.as_mut().expect("campaign");
    let mut bonus = bonus;
    bonus.practice = c.practice.is_some();
    // `sc1PBonusStageProcUpdate`'s new-record check reads the course's
    // completed best time.
    {
        let rec = &s.backup.spgame_records[bonus.state.players[s.spgame_scene.player as usize].fkind as usize];
        let (count, time) = if bonus.platforms[0].is_some() { (rec.bonus2_task_count, rec.bonus2_time) } else { (rec.bonus1_task_count, rec.bonus1_time) };
        bonus.record_time = (count == spgame::BONUSGAME_TASK_MAX).then_some(time);
    }

    // `sc1PBonusStageMakeTimer`'s practice digits.
    w.damage_hud.bonus_timer = bonus.practice.then(spgame::bonus_stage::PracticeTimer::default);
    c.bonus = Some(alloc::boxed::Box::new(bonus));
    // `sc1PBonusStageFuncStart`'s `mpCollisionSetPlayBGM` and crowd.
    ssb_game::music::start_battle(stage.bgm_id);
    Ok(())
}

pub(crate) fn bonus_frame(p: &Pack<'_>, hud: &mut Hud, b: &ssb_game::battle::Battle, bonus: &mut spgame::bonus_stage::BonusStage) {
    hud.bonus_tasks = Some(bonus.tasks_remain);
    if bonus.practice {
        // `sc1PBonusStageTimerProcUpdate`.
        bonus.timer.tick(b.time_passed);
        hud.bonus_timer = Some(bonus.timer);
    }
    if b.clock() == 61 {
        let mut c = ssb_game::countdown::Countdown::sudden_death();
        c.start_go();
        hud.countdown = Some(c);
    }
    if let Some(c) = hud.countdown.as_mut() { c.tick(&game_status_sizes(p)); }
}

/// `ftManagerMakeFighter`'s 1P Game fields and `Session`'s KO machinery.
fn configure(
    sp: &mut Campaign1P,
    f: &mut play::FighterScene,
    setup: spgame::setup::PlayerSetup,
    team_bounds: ssb_game::status::BlastZone,
) {
    sp.configure_fighter(&mut f.fighter, team_bounds);
    if ssb_game::kirby::is_kirby(f.fighter.kind) {
        ssb_game::kirby::init_copy(&mut f.fighter, setup.copy_kind);
    }
    f.fighter.interface.magnify_ignore = setup.is_magnify_ignore;
    f.camera_zoom_frame *= setup.camera_frame_mul;
    if setup.is_skip_entry {
        ssb_game::status::set_wait_or_fall(&mut f.fighter);
    }
}

/// One line per tick of Master Hand's stage, for N64 references: the
/// battle clock, Master Hand's and the player's state, the RNG seed and
/// the wallpaper (RE-457).
#[cfg(any(feature = "headless_capture", feature = "regression_capture"))]
pub(crate) fn log_boss_state(s: &Session, tick: u64) {
    let Some(boss) = s.campaign.as_ref().and_then(|c| c.boss.as_deref()) else {
        return;
    };
    let mut line = alloc::format!(
        "boss tick={} clock={:?} gs={:?} tp={:?} seed={:08x} wp={}/{:?} fade={:.2} done={} zoom=({:.1},{:.1},{:.1})",
        tick,
        s.vs_battle.as_ref().map(|b| b.clock()),
        s.vs_battle.as_ref().map(|b| b.status),
        s.vs_battle.as_ref().map(|b| b.time_passed),
        ssb_game::rng::seed() as u32,
        boss.wallpaper.wallpaper_id,
        boss.wallpaper.current,
        boss.wallpaper.fade_step,
        u8::from(boss.wallpaper.done),
        boss.zoom.x,
        boss.zoom.y,
        boss.zoom.z,
    );
    for d in s.world.iter().flat_map(|w| w.dummies.iter()).flatten() {
        let f = &d.fighter;
        line.push_str(&alloc::format!(
            " B(st={} dmg={} djid={} pos={:.1},{:.1},{:.1})",
            f.status.status.id(),
            f.damage,
            f.damage_joint_id,
            f.pos.x,
            f.pos.y,
            f.pos.z
        ));
    }
    if let Some(pl) = s.world.as_deref().and_then(|w| w.play_state.as_ref()) {
        let f = &pl.fighter;
        line.push_str(&alloc::format!(
            " K(st={} dmg={} pos={:.1},{:.1})",
            f.status.status.id(),
            f.damage,
            f.pos.x,
            f.pos.y
        ));
    }
    let mut n = 0;
    for e in boss.wallpaper.iter() {
        n += 1;
        if n <= 6 || !matches!(e.display, spgame::boss::Display::Comet) {
            line.push_str(&alloc::format!(
                " E(w{} e{} a{} s{} {:?} p{} t{:.0},{:.0},{:.0})",
                e.wallpaper, e.effect, e.alpha, e.step, e.display, e.plays, e.translate.x, e.translate.y, e.translate.z
            ));
        }
    }
    line.push_str(&alloc::format!(" n={}", n));
    #[cfg(feature = "rng_trace")]
    ssb_game::rng::trace::take(|file, at, v| {
        let name = file.rsplit('/').next().unwrap_or(file);
        if v < 0 {
            line.push_str(&alloc::format!(" R:{}:{}", name, at));
        } else {
            line.push_str(&alloc::format!(" G{:x}@{}:{}", v, name, at));
        }
    });
    line.push('\n');
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
    }
}

/// `onepzako` per-tick state for the N64 comparison of the team's
/// replacements and clear (RE-458): the team counters, every enemy and
/// the player, then the stage-clear tally's values.
#[cfg(any(feature = "headless_capture", feature = "regression_capture"))]
pub(crate) fn log_zako_state(s: &Session, tick: u64) {
    let Some(c) = s.campaign.as_ref() else { return };
    let Some(sp) = c.frontend.session.as_ref() else { return };
    let mut line = alloc::format!(
        "zako tick={} clock={:?} gs={:?} tp={:?} end={:?} seed={:08x} score={} tr={} mask={:x}",
        tick,
        s.vs_battle.as_ref().map(|b| b.clock()),
        s.vs_battle.as_ref().map(|b| b.status),
        s.vs_battle.as_ref().map(|b| b.time_passed),
        s.vs_battle.as_ref().and_then(|b| b.end),
        ssb_game::rng::seed() as u32,
        sp.data.score,
        sp.data.time_remain,
        sp.data.bonus_get_mask[0],
    );
    if let Some(g) = sp.game.as_ref() {
        line.push_str(&alloc::format!(
            " er={} tpr={} cv={} fl={:x} icons={} var={:?}",
            g.enemy_stocks_remaining,
            g.team_players_remaining,
            g.current_variation,
            g.enemy_stock_sprite_flags,
            g.team_stock_icons().len(),
            &g.variations[..usize::from(g.enemy_player_count).min(g.variations.len())],
        ));
    }
    for d in s.world.iter().flat_map(|w| w.dummies.iter()).flatten() {
        let f = &d.fighter;
        line.push_str(&alloc::format!(
            " E(p{} k{} st{} dmg{} pos={:.1},{:.1})",
            f.port,
            f.kind as u8,
            f.status.status.id(),
            f.damage,
            f.pos.x,
            f.pos.y
        ));
    }
    if let Some(pl) = s.world.as_deref().and_then(|w| w.play_state.as_ref()) {
        let f = &pl.fighter;
        line.push_str(&alloc::format!(
            " K(st{} dmg{} pos={:.1},{:.1} tw{} hide{} cd{}) eye={:.1},{:.1},{:.1}",
            f.status.status.id(),
            f.damage,
            f.pos.x,
            f.pos.y,
            f.interface.tag_wait,
            u8::from(f.interface.tag_hide),
            u8::from(f.interface.control_disable),
            pl.camera.eye.x,
            pl.camera.eye.y,
            pl.camera.eye.z,
        ));
    }
    if let Scene1P::StageClear(sc) = &c.frontend.screen {
        line.push_str(&alloc::format!(
            " SC(t{} total{} shown{} sec{} dmg{} diff{:?} rows{:?} proceed{})",
            sc.total_tics,
            sc.total_score,
            sc.shown_score,
            sc.seconds,
            sc.damage,
            sc.difficulty,
            sc.rows.iter().flatten().map(|r| (r.bonus_id, r.points, r.reveal_tic)).collect::<alloc::vec::Vec<_>>(),
            sc.allow_proceed,
        ));
    }
    line.push('\n');
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
    }
}

/// Capture evidence for routing and the wait clock, including captures
/// made before the battle creates fighters.
#[cfg(feature = "headless_capture")]
pub(crate) fn log_capture(s: &Session, tick: u64) {
    let state = if s.players_1p.is_some() {
        "select"
    } else if let Some(c) = s.campaign.as_ref() {
        if c.blocked.is_some() {
            "blocked"
        } else {
            match c.frontend.screen {
                Scene1P::Intro(_) => "intro",
                Scene1P::Continue(_) => "continue",
                Scene1P::StageClear(_) => "stage-clear",
                Scene1P::Ending(_) => "ending",
                Scene1P::Staffroll(_) => "staffroll",
                Scene1P::Congra(_) => "congra",
                Scene1P::Challenger(_) => "challenger",
                Scene1P::Message(_) => "message",
                Scene1P::Host(Scene::Battle) => "battle",
                Scene1P::Host(Scene::BonusStage) => "bonus",
                _ => "host",
            }
        }
    } else {
        "menu"
    };
    let line = alloc::format!(
        "campaign tick={} scene={} stage={:?} clock={:?} countdown={:?} cpu={:?} targets={:?} end={:?} tasks={:?} stocks={:?} free={} max_free={}\n",
        tick,
        state,
        s.campaign
            .as_ref()
            .and_then(|c| c.frontend.session.as_ref().map(|sp| sp.data.stage)),
        s.vs_battle.as_ref().map(|b| b.clock()),
        s.world.as_deref().is_some_and(|w| w.damage_hud.countdown.is_some()),
        s.world.as_deref().and_then(|w| w.dummies[0].as_ref()).map(|d| d.computer.behavior),
        s.campaign.as_ref().and_then(|c| c.bonus.as_ref().map(|b| b.tasks_remain)),
        s.vs_battle.as_ref().and_then(|b| b.end),
        s.campaign.as_ref().and_then(|c| c.frontend.session.as_ref().map(|sp| sp.data.bonus_tasks_complete)),
        s.campaign.as_ref().and_then(|c| c.frontend.session.as_ref().map(|sp| sp.state.players[sp.data.player as usize].stock_count)),
        // The user partition's free memory, for the pack's headroom.
        unsafe { psp::sys::sceKernelTotalFreeMemSize() },
        unsafe { psp::sys::sceKernelMaxFreeMemSize() },
    );
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
        let fd = psp::sys::sceIoOpen(b"campaign_capture.txt\0".as_ptr(),
            psp::sys::IoOpenFlags::WR_ONLY | psp::sys::IoOpenFlags::CREAT | psp::sys::IoOpenFlags::TRUNC, 0o777);
        if fd.0 >= 0 {
            psp::sys::sceIoWrite(fd, line.as_ptr() as *const core::ffi::c_void, line.len());
            psp::sys::sceIoClose(fd);
        }
    }
}

/// `sc1PGameFuncUpdate`'s Go and Set checks, after the battle's frame.
/// `bgm_id` is the stage's `MPGroundData::bgm_id`, which
/// `sc1PGameSetGameStart` plays at Go on the stages that held it.
pub(crate) fn update_game(
    sp: &mut Campaign1P,
    status: ssb_game::battle::GameStatus,
    player: &ssb_game::fighter::Fighter,
    bgm_id: Option<u32>,
) {
    if let Some(game) = sp.game.as_mut() {
        let play_bgm = game.update(status, || {
            let end = match player.status.status {
                ssb_game::status::AnyStatus::Common(s) => spgame::bonus::EndStatus::from(s),
                _ => spgame::bonus::EndStatus::Other,
            };
            (end, u32::from(player.star_invincible_frames))
        });
        if let (true, Some(bgm_id)) = (play_bgm, bgm_id) {
            ssb_game::music::set_play_bgm(bgm_id);
        }
    }
}

/// The 1P Game's wait thread (`sc1PGameWaitStage*Update`) in place of the
/// VS entry focus: the countdown, each fighter's appearance or team drop,
/// and the random pattern's zooms.
#[inline(never)]
pub(crate) fn entry_frame(
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    scenes: &mut [Option<&mut play::FighterScene>; 4],
    countdown: &mut Option<ssb_game::countdown::Countdown>,
    b: &ssb_game::battle::Battle,
    wait: &spgame::wait::Wait,
) {
    let clock = b.clock();
    for action in wait.at(clock) {
        match action {
            Action::Countdown => *countdown = Some(ssb_game::countdown::Countdown::new()),
            // Its first frame plays in the fighter's priority-4 physics.
            Action::Appear(port) => {
                if let Some(f) = scenes[usize::from(port)].as_deref_mut() {
                    ssb_game::appear::appear_set_status(&mut f.fighter);
                }
            }
            Action::AppearPosition(port) => {
                if let Some(f) = scenes[usize::from(port)].as_deref_mut() {
                    ssb_game::appear::appear_set_position(
                        &mut f.fighter,
                        f32::from(stage.camera.top),
                        f32::from(stage.bounds.top),
                    );
                }
            }
            Action::Bonus3Follow => {
                if let Some(pl) = scenes[0].as_deref_mut() {
                    pl.camera.bonus_follow = true;
                }
            }
            Action::Go if stage.source_file == ssb_rom::ground_obj::BONUS3_FILE => {
                let mut c = ssb_game::countdown::Countdown::sudden_death();
                c.start_go();
                *countdown = Some(c);
            }
            // The camera follows [`entry_zoom_frame`]'s `zoom_port`.
            // Targets uses its separate bonus entry path.
            // separate bonus entry path.
            // `sc1PGameWaitStageBossUpdate` at Go: Final Destination's
            // music, unless an item's plays.
            Action::BossGo => {
                ssb_game::music::boss_go(scenes.iter().flatten().map(|f| &f.fighter))
            }
            // Before the step: [`entry_camera_frame`].
            Action::BossCameraAnim
            | Action::Zoom(_)
            | Action::CameraDefault
            // "Go" itself is the battle's (`go_tick` 601), and the entry's
            // camera mode ends there (`appear::on_go`).
            | Action::Go => {}
        }
    }
    if let Some(c) = countdown.as_mut() {
        c.tick(&game_status_sizes(p));
    }
}

/// `sc1PGameWaitStageBossUpdate`'s `gmCameraSetStatusAnim`: the wait
/// thread (priority 5) starts the intro camera from the origin, one play
/// at once (`gmCameraUpdateAnimVel`), before the frame's camera process
/// (priority 3, [`boss_after_world`]) plays it again; Master Hand's Appear
/// is the thread's in the step ([`entry_frame`]).
pub(crate) fn entry_camera_frame(
    pl: &mut play::FighterScene,
    b: &ssb_game::battle::Battle,
    wait: &spgame::wait::Wait,
    boss: Option<&BossScene>,
) {
    let Some(boss) = boss else { return };
    if wait.at(b.clock()).any(|a| a == Action::BossCameraAnim) {
        ssb_psp_runtime::scene::start_camera_anim(pl, &boss.intro_camera, ssb_engine::math::Vec3::ZERO);
    }
}

/// The wait thread's zoom events (`sc1PGameWaitStageCommonUpdate`): the
/// thread's process (priority 5) runs before the camera's (priority 3), so
/// the caller sets the zoom before the frame's camera pass reads it
/// (RE-543).
#[inline(never)]
pub(crate) fn entry_zoom_frame(
    p: &Pack<'_>,
    pl: &mut play::FighterScene,
    dummies: &Dummies,
    b: &ssb_game::battle::Battle,
    wait: &spgame::wait::Wait,
) {
    let clock = b.clock();
    // `gmCameraSetStatusPlayerZoom(fighter, 0, 0, closeup_camera_zoom,
    // 0.1, 28)` until `gmCameraSetStatusDefault`: the last zoom event so far.
    let zoom_port = wait
        .events
        .iter()
        .filter(|e| e.tick <= clock)
        .filter_map(|e| match e.action {
            Action::Zoom(port) => Some(Some(port)),
            Action::CameraDefault => Some(None),
            _ => None,
        })
        .last()
        .flatten();
    // The fighter and distance only: the camera pass reads its position
    // on the tick it runs (RE-543).
    pl.camera.entry_zoom = zoom_port.and_then(|port| {
        let f = scenes_ref(pl, dummies)[usize::from(port)]?;
        let dist = p
            .fighter(f.fighter.kind as u32)
            .map_or(1000.0, |d| d.closeup_camera_zoom);
        Some(ssb_game::camera::EntryZoom { slot: port, dist })
    });
}

/// The world's falls, collected by the campaign: callbacks, KO records and
/// the battle's stock, then `sc1PGameSetPlayerDefeatStats`. Returns
/// whether the fighter fell.
pub(crate) fn collect_fall(
    sp: &mut Campaign1P,
    battle: &mut Option<ssb_game::battle::Battle>,
    f: &mut ssb_game::fighter::Fighter,
) -> bool {
    let fell = f.dead.scored;
    core::mem::swap(&mut sp.battle, battle);
    sp.collect_fall(f);
    core::mem::swap(&mut sp.battle, battle);
    fell
}

/// `ftCommonDeadCheckRebirth`'s 1P Game branch, inside the step's
/// interrupt pass ([`ssb_game::matchtick::MatchHooks::next_member`]): an
/// enemy whose KO wait ended sleeps, or the team's next member takes its
/// seat (`sc1PGameSpawnEnemyTeamNext`), links last and runs its first
/// frame in this one (RE-458).
pub(crate) struct NextMembers<'a, 'p> {
    p: &'a Pack<'p>,
    stage: &'a ssb_rom::pack::StageDesc,
    sp: &'a mut Campaign1P,
    /// The next member's [`play::Dummy::link`].
    next_link: u32,
    /// The ports whose member was made this frame.
    made: [bool; 4],
}

impl<'a, 'p> NextMembers<'a, 'p> {
    pub(crate) fn new(
        p: &'a Pack<'p>,
        stage: &'a ssb_rom::pack::StageDesc,
        sp: &'a mut Campaign1P,
        dummies: &Dummies,
    ) -> Self {
        let next_link = dummies.iter().flatten().map(|d| d.link + 1).fold(4, u32::max);
        Self { p, stage, sp, next_link, made: [false; 4] }
    }

    #[inline(never)]
    pub(crate) fn next_member(
        &mut self,
        d: &mut play::Dummy,
        groups: &[ssb_game::map::MapGroup],
        battle: &mut ssb_game::battle::Battle,
    ) -> bool {
        let (p, stage) = (self.p, self.stage);
        let points: alloc::vec::Vec<_> = p
            .stage_points(stage)
            .filter(|point| point.kind == spgame::setup::mapobj::ENEMY_TEAM)
            .map(|point| ssb_engine::math::Vec3::new(f32::from(point.x), f32::from(point.y), 0.0))
            .collect();
        let port = d.fighter.port;
        let next = self.sp.next_enemy_in(
            battle,
            port,
            &points,
            f32::from(stage.camera.top),
            f32::from(stage.bounds.top),
        );
        let e = match next {
            spgame::setup::NextEnemy::Sleep => {
                d.fighter.dead.enemy_next_pending = false;
                ssb_game::dead::set_sleep(&mut d.fighter);
                return false;
            }
            spgame::setup::NextEnemy::Spawn(e) => e,
        };
        let team_bounds = ssb_game::status::BlastZone {
            top: f32::from(stage.team_bounds.top),
            bottom: f32::from(stage.team_bounds.bottom),
            left: f32::from(stage.team_bounds.left),
            right: f32::from(stage.team_bounds.right),
        };
        // `ftManagerMakeFighter` draws from the battle's counts, which the
        // outgoing member shares.
        let counts = d.fighter.stats.counts.clone();
        let mut n = play::Dummy::at_position(p, stage, e.fkind, e.costume, e.level, e.pos, port, &counts, groups);
        n.fighter.facing = e.facing;
        n.fighter.team = d.fighter.team;
        n.fighter.shade = e.shade;
        let detail = if e.detail_high {
            ssb_game::modelpart::Detail::High
        } else {
            ssb_game::modelpart::Detail::Low
        };
        n.fighter.model_parts.detail_curr = detail;
        n.fighter.model_parts.detail_base = detail;
        n.computer.behavior = ssb_game::computer::Behavior::Default;
        n.computer.trait_kind = e.cp_trait;
        n.is_1p_game = true;
        let setup = self.sp.game.as_ref().expect("active game").setups[usize::from(port)];
        configure(self.sp, &mut n, setup, team_bounds);
        // `is_skip_entry`: wait or fall, unlocked at once.
        ssb_game::status::set_wait_or_fall(&mut n.fighter);
        n.link = self.next_link;
        self.next_link += 1;
        *d = n;
        if let Some(m) = self.made.get_mut(usize::from(port)) {
            *m = true;
        }
        true
    }

    /// The ports whose member was made this frame, for [`members_hud`].
    pub(crate) fn made(self) -> [bool; 4] {
        self.made
    }
}

/// `sc1PGameSpawnEnemyTeamNext`'s `ifCommonPlayerDamageStopBreakAnim` for
/// each member made, in the fighter's process: before the interface
/// processes' `ifCommonPlayerDamageProcUpdate`, whose break animation would
/// otherwise draw the RNG once more in the spawn frame (RE-458).
pub(crate) fn members_stop_break_anim(made: [bool; 4], damage: &mut [ssb_game::hud::DamageDisplay; 4]) {
    for (h, _) in damage.iter_mut().zip(made).filter(|m| m.1) {
        h.stop_break_anim();
    }
}

/// After the frame's fighters: `sc1PGameTeamStockDisplayProcDisplay`'s
/// icons from this frame's remaining team (the Yoshi, Kirby and Polygon
/// teams).
pub(crate) fn members_hud(sp: &Campaign1P, hud: &mut Hud) {
    hud.team_stocks = sp.game.as_ref().map(|g| (g.stage, g.team_stock_icons()));
}

/// `sc1PBonusStageStartScene`'s practice branch: the records, then the
/// select, Luigi's challenge or the Sound Test message.
fn finish_practice(
    s: &mut Session,
    pack: Option<&Pack<'_>>,
    battle: &ssb_game::battle::Battle,
    tasks_remain: u8,
    practice: ssb_game::players_1p_bonus::BonusKind,
) {
    use ssb_game::menu::Scene as M;
    use ssb_game::players_1p_bonus::BonusKind;
    use spgame::bonus_stage::PracticeNext;
    let c = s.campaign.take().expect("campaign");
    let sp = c.frontend.session.as_ref().expect("campaign session");
    let (fkind, costume) = (sp.data.fkind, sp.data.costume);
    s.leave_battle();
    let bonus1 = practice == BonusKind::Targets;
    let next = spgame::bonus_stage::finish_practice(&mut s.backup, bonus1, fkind, tasks_remain, battle.time_passed, battle.is_reset);
    let select = if bonus1 { M::Players1PBonus1 } else { M::Players1PBonus2 };
    match next {
        PracticeNext::Select => crate::go_scene(s, pack, select, M::BonusStage, false),
        PracticeNext::LuigiChallenge => {
            s.spgame_scene.fkind = fkind;
            s.spgame_scene.costume = costume;
            s.spgame_scene.stage = Stage::Luigi as u8;
            s.menus.scene = M::OnePGame;
            s.menus.scene_prev = M::BonusStage;
            start(s, pack);
        }
        PracticeNext::SoundTestMessage => {
            s.vs_message = Some(spgame::message::Message::new(spgame::Unlock::SoundTest));
            s.vs_message_next = None;
            s.message_after = M::Startup;
            s.screen = Screen::Message;
        }
    }
}

/// The battle's end: its records go to the campaign once, then the
/// manager's next scene.
#[inline(never)]
pub(crate) fn finish_battle(s: &mut Session, pack: Option<&Pack<'_>>) {
    let Some(c) = s.campaign.as_mut() else { return };
    // `sc1PGameStartScene` / `sc1PBonusStageStartScene` after the loop.
    ssb_game::music::leave_battle();
    let sp = c.frontend.session.as_mut().expect("campaign session");
    if let Some(mut bonus) = c.bonus.take() {
        if bonus.retry_requested {
            s.vs_battle = None;
            s.leave_battle();
            on_host(s, pack, Scene::BonusStage);
            return;
        }
        let battle = s.vs_battle.take().expect("bonus battle");
        if let Some(practice) = c.practice {
            finish_practice(s, pack, &battle, bonus.tasks_remain, practice);
            return;
        }
        sp.data.is_reset = battle.is_reset;
        let tasks = bonus.tasks_remain;
        sp.finish_bonus_stage(bonus.result(&battle), tasks, &mut s.backup);
        let scene = c.frontend.sync(&s.backup);
        c.tic = 0;
        crate::leave_battle_world(&mut s.world);
        s.screen = Screen::Campaign;
        if matches!(c.frontend.screen, Scene1P::Host(_)) { on_host(s, pack, scene); }
        return;
    }
    c.boss = None;
    // Callbacks still queued on the fighters belong to this battle.
    if let Some(pl) = s.world.as_deref_mut().and_then(|w| w.play_state.as_mut().zip(Some(&mut w.dummies))) {
        let (pl, dummies) = pl;
        for f in scenes(pl, dummies).into_iter().flatten() {
            collect_fall(sp, &mut s.vs_battle, &mut f.fighter);
        }
    }
    sp.battle = s.vs_battle.take();
    sp.finish_battle(&mut s.backup);
    let scene = c.frontend.sync(&s.backup);
    c.tic = 0;
    crate::leave_battle_world(&mut s.world);
    match c.frontend.screen {
        Scene1P::Host(_) => {
            s.screen = Screen::Campaign;
            on_host(s, pack, scene);
        }
        _ => s.screen = Screen::Campaign,
    }
}

/// Authored presentation; unsupported scene requests keep an explicit marker.
#[inline(never)]
pub(crate) unsafe fn draw(
    gpu: &mut Gpu,
    pack: Option<&Pack<'_>>,
    st: &mut meshdraw::DrawState,
    s: &mut Session,
) {
    if let (Some(c), Some(p)) = (s.campaign.as_mut(), pack) {
        if let Some(sp) = c.frontend.session.as_ref() {
            c.presentation.prepare_draw(gpu, p, &c.frontend.screen, sp);
        }
    }
    gpu.set_viewport_pillarboxed();
    gpu.begin_frame(Some(BG_RESULTS));
    let Some(c) = s.campaign.as_mut() else { return };
    if let Some(blocked) = c.blocked {
        let (color, x) = match blocked {
            Blocked::Scene(scene) => (Color::rgba(160, 24, 24, 255), 40 + scene as i32 * 30),
            Blocked::Assets(stage) => (Color::rgba(200, 120, 0, 255), 40 + stage as i32 * 29),
        };
        // The port's own diagnostic, in PSP pixels.
        gpu.set_viewport_fullscreen();
        gpu.draw_rect(40, 100, 440, 172, color);
        gpu.draw_rect(x, 180, x + 24, 196, ENTRY_SELECTED);
        return;
    }
    let (Some(p), Some(sp)) = (pack, c.frontend.session.as_ref()) else {
        return;
    };
    c.presentation.draw(gpu, p, st, &mut c.frontend.screen, sp);
}

/// Deterministic drawing fixtures, not evidence of winning/losing a battle.
/// Selection still runs normally; only the requested campaign overlay is seeded.
#[cfg(any(feature = "headless_capture", feature = "capture_scene_file"))]
pub(crate) fn capture_fixture(s: &mut Session, scene: GameScene) {
    let Some(c) = s.campaign.as_mut() else { return };
    let sp = c.frontend.session.as_mut().expect("campaign");
    // A `fighters=` line's first fighter (hardware sweeps, RE-531); scenes
    // that pin their own fighter below still do.
    if let Some(kind) = crate::capture::fighter_override(0) {
        sp.data.fkind = kind;
        // The battle state's player entry, which the normal stages read.
        sp.state.players[sp.data.player as usize].fkind = kind;
    }
    match scene {
        GameScene::OnePIntro => {
            sp.data.stage = Stage::Yoshi as u8;
            sp.manager.scene = Scene::Intro;
        }
        GameScene::OnePBonus | GameScene::OnePTargetClear | GameScene::OnePTargetFall => {
            if scene != GameScene::OnePBonus { sp.data.fkind = ssb_game::fighter::FighterKind::Mario; }
            sp.data.stage = Stage::Bonus1 as u8;
            sp.manager.scene = Scene::Intro;
        }
        GameScene::OnePPlatforms | GameScene::OnePPlatformClear | GameScene::OnePPlatformFall | GameScene::OnePPlatformsRide => {
            sp.data.stage = Stage::Bonus2 as u8;
            if matches!(scene, GameScene::OnePPlatformClear | GameScene::OnePPlatformFall) { sp.data.fkind = ssb_game::fighter::FighterKind::Mario; }
            if scene == GameScene::OnePPlatformsRide { sp.data.fkind = ssb_game::fighter::FighterKind::Fox; }
            sp.manager.scene = Scene::Intro;
        }
        GameScene::OnePRace | GameScene::OnePRaceClear | GameScene::OnePRaceFall | GameScene::OnePRaceHazards => {
            sp.data.stage = Stage::Bonus3 as u8;
            sp.manager.scene = Scene::Intro;
        }
        GameScene::OnePBoss | GameScene::OnePBossDefeat | GameScene::OnePBossHit => {
            sp.data.stage = Stage::Boss as u8;
            sp.manager.scene = Scene::Intro;
        }
        GameScene::OnePMetal
        | GameScene::OnePGiant
        | GameScene::OnePZako
        | GameScene::OnePYoshi => {
            if scene == GameScene::OnePYoshi {
                // Yoshi in costume 0, so the Yoshi Team's costume-0 Yoshi
                // takes shade 1 (`sc1pgame.c:1347`).
                let kind = ssb_game::fighter::FighterKind::Yoshi;
                let costume = ssb_game::costume::costume_common_id(kind, 0);
                (sp.data.fkind, sp.data.costume) = (kind, costume);
                let player = &mut sp.state.players[sp.data.player as usize];
                (player.fkind, player.costume) = (kind, costume);
            }
            // A `onepstage=` line (hardware runs) picks any other stage.
            sp.data.stage = crate::capture::onep_stage_override().unwrap_or(match scene {
                GameScene::OnePMetal => Stage::MMario,
                GameScene::OnePGiant => Stage::Donkey,
                GameScene::OnePYoshi => Stage::Yoshi,
                _ => Stage::Zako,
            } as u8);
            // The stage's own setup (Giant Donkey Kong's random allies),
            // which the seeded jump would otherwise skip.
            sp.manager.prepare_stage(&sp.data, &mut sp.state, &s.backup);
            sp.manager.scene = Scene::Intro;
        }
        GameScene::OnePContinue | GameScene::OnePRetry => {
            sp.data.score = 123456;
            sp.manager.scene = Scene::Continue;
        }
        GameScene::OnePClear | GameScene::OnePGameClear => {
            sp.data.score = 123456;
            sp.data.time_remain = 250;
            sp.state.players[sp.data.player as usize].total_damage_given = 321;
            sp.data.bonus_get_mask = [0x0018_0005, 0, 0];
            if scene == GameScene::OnePGameClear {
                // Master Hand's stage: `GAME CLEAR`, and the difficulty's
                // clear bonus (`nSC1PGameBonusStageClear`, RE-548).
                sp.data.stage = Stage::Boss as u8;
                sp.data.bonus_get_mask[0] |= 1 << 22;
            }
            sp.manager.scene = Scene::StageClear;
        }
        // The manager steps past the last stage before the ending
        // (`sc1PManagerUpdateScene`), and back before the challengers.
        GameScene::OnePEnding | GameScene::OnePFinale => {
            sp.data.stage = Stage::Boss as u8 + 1;
            sp.manager.scene = Scene::Ending;
        }
        GameScene::OnePStaffroll => {
            sp.data.stage = Stage::Boss as u8 + 1;
            sp.manager.scene = Scene::Staffroll;
        }
        GameScene::OnePCongra => {
            sp.data.stage = Stage::Boss as u8 + 1;
            sp.data.score = 123456;
            sp.manager.scene = Scene::Congratulations;
        }
        GameScene::OnePChallenger => {
            sp.data.stage = Stage::Ness as u8;
            sp.data.challenger_fkind = ssb_game::fighter::FighterKind::Ness;
            sp.manager.scene = Scene::Challenger;
        }
        GameScene::OnePMessage => {
            sp.data.stage = Stage::Ness as u8;
            sp.data.unlock_message = Some(spgame::Unlock::Ness);
            sp.manager.message = sp.data.unlock_message;
            sp.manager.scene = Scene::Message;
        }
        _ => return,
    }
    c.frontend.sync(&s.backup);
    c.tic = 0;
    s.screen = Screen::Campaign;
}

/// Diagnostic placement fixtures use the real hit search and real blast
/// check; they do not set the task counter or the results directly.
#[cfg(any(feature = "headless_capture", feature = "capture_scene_file"))]
pub(crate) fn capture_objectives(s: &mut Session, pack: Option<&Pack<'_>>, scene: GameScene, tick: u64) {
    let Some(w) = s.world.as_deref_mut() else { return };
    // `trainingitemflash`: the walking Bob-omb's `multi` goes from 2 to
    // `ITBOMBHEI_FLASH_WAIT` after its update, as the N64 reference build
    // does, so its next update starts the critical flash (batch 58).
    if scene == GameScene::TrainingItemFlash {
        use ssb_game::item::{bombhei, ItemKind, ItemStatus};
        for slot in 0..ssb_game::item::ITEM_ALLOC_MAX as u8 {
            if let Some(it) = w.items.get_mut(slot) {
                if it.kind == ItemKind::BombHei
                    && it.status == ItemStatus::BombHei(bombhei::Status::Walk)
                    && it.multi == 2
                {
                    it.multi = bombhei::FLASH_WAIT;
                }
            }
        }
        let mut line = alloc::format!("itemflash tick={}", tick);
        if let Some(pl) = w.play_state.as_ref() {
            line += &alloc::format!(
                " P[{:?} x{:.1} y{:.1}]",
                pl.fighter.status.status, pl.fighter.pos.x, pl.fighter.pos.y
            );
        }
        for it in w.items.items() {
            line += &alloc::format!(
                " I[{:?} {:?} m{} pal{} col{} {:?} x{:.1} y{:.1}]",
                it.kind,
                it.status,
                it.multi,
                it.palette,
                it.colanim.id.0,
                it.colanim.color(),
                it.pos.x,
                it.pos.y
            );
        }
        line += "\n";
        unsafe {
            psp::sys::sceIoWrite(
                psp::sys::sceKernelStdout(),
                line.as_ptr() as *const core::ffi::c_void,
                line.len(),
            );
        }
    }
    // Master Hand's hit points run out 100 ticks after "Go": the fixture
    // writes the damage, and `ftBossCommonUpdateDamageStats` runs the real
    // defeat from there.
    // `onepbosshit`: only the damage, as the N64 reference writes
    // `percent_damage` and `stock_damage_all`; the hit that lands later
    // runs `ftBossCommonUpdateDamageStats`.
    if scene == GameScene::OnePBossHit {
        let clock = s.vs_battle.as_ref().map(|b| b.clock());
        if let Some(&(_, damage)) = ssb_capture::script::boss_hit::DAMAGE_SEEDS
            .iter()
            .find(|(c, _)| Some(*c) == clock)
        {
            for d in w.dummies.iter_mut().flatten() {
                if d.fighter.kind == ssb_game::fighter::FighterKind::Boss {
                    d.fighter.damage = damage;
                }
            }
        }
    }
    if scene == GameScene::OnePBossDefeat
        && s.vs_battle.as_ref().is_some_and(|b| b.clock() == 701)
    {
        for d in w.dummies.iter_mut().flatten() {
            if d.fighter.kind == ssb_game::fighter::FighterKind::Boss {
                d.fighter.damage = ssb_game::boss::HIT_POINTS;
                ssb_game::boss::update_damage_stats(&mut d.fighter);
            }
        }
    }
    let Some(pack) = pack else { return };
    // From battle clock 1000, every 150 ticks: each enemy placed below the
    // stage's bottom bound, so the real blast check KOs it and the
    // campaign replaces it (the Polygon and Yoshi Teams) or ends the stage.
    // The fixture writes no stocks, falls or results.
    if matches!(
        scene,
        GameScene::OnePMetal | GameScene::OnePGiant | GameScene::OnePZako | GameScene::OnePYoshi
    ) && s
        .vs_battle
        .as_ref()
        .is_some_and(|b| b.clock() >= 1000 && b.clock() % 150 == 100)
    {
        if let (Some(pl), Some(stage)) = (w.play_state.as_ref(), pack.stage(s.training_stage)) {
            let team = pl.fighter.team;
            for d in w.dummies.iter_mut().flatten() {
                // Past `nFTCommonStatusControlStart`: not dead, asleep or entering.
                if d.fighter.team != team && d.fighter.status.status.id() > ssb_game::status::Status::RebirthWait as u16 {
                    ssb_game::status::set_fall(&mut d.fighter);
                    d.fighter.floor = None;
                    d.fighter.physics.vel_air = ssb_engine::math::Vec3::ZERO;
                    d.fighter.pos.y = f32::from(stage.bounds.bottom) - 100.0;
                }
            }
        }
    }
    if scene == GameScene::OnePRaceHazards && tick == 600 {
        if let (Some(pl), ssb_game::stage::Controller::Bonus3(race)) = (w.play_state.as_mut(), &w.stage_ctl.controller) {
            ssb_game::status::set_fall(&mut pl.fighter);
            pl.fighter.floor = None;
            pl.fighter.physics.vel_air = ssb_engine::math::Vec3::ZERO;
            pl.fighter.pos = race.tarubomb_make_pos + ssb_engine::math::Vec3::new(0.0, -1000.0, 0.0);
        }
    }
    if matches!(scene, GameScene::OnePRaceClear | GameScene::OnePRaceFall) && tick == 600 {
        if let (Some(pl), Some(stage)) = (w.play_state.as_mut(), pack.stage(s.training_stage)) {
            ssb_game::status::set_fall(&mut pl.fighter);
            pl.fighter.floor = None;
            pl.fighter.physics.vel_air = ssb_engine::math::Vec3::ZERO;
            if scene == GameScene::OnePRaceFall {
                pl.fighter.pos.y = f32::from(stage.bounds.bottom) - 100.0;
            } else {
                let gate = ssb_psp_runtime::scene::MapSegments::new(pack, &stage).find(|s|
                    s.kind == ssb_game::weapon::MapSurfaceKind::Floor && s.segment.material() == ssb_game::stage::bonus3::MATERIAL_DETECT).expect("Race finish segment").segment;
                // Land through swept collision on the DETECT segment, not
                // on the earlier normal segment of the same polyline.
                pl.fighter.pos = ssb_engine::math::Vec3::new((f32::from(gate.x1) + f32::from(gate.x2)) * 0.5, (f32::from(gate.y1) + f32::from(gate.y2)) * 0.5 + 50.0, 0.0);
            }
        }
    }
    if !s.campaign.as_ref().is_some_and(|c| c.bonus.is_some()) { return; }
    if matches!(scene, GameScene::OnePTargetFall | GameScene::OnePPlatformFall) && tick == 600 {
        if let Some(pl) = w.play_state.as_mut() {
            ssb_game::status::set_fall(&mut pl.fighter);
            pl.fighter.pos.y = -20000.0;
        }
    }
    if scene == GameScene::OnePPlatformClear && (600..810).contains(&tick) && tick % 20 == 0 {
        let Some(pl) = w.play_state.as_mut() else { return };
        let Some(stage) = pack.stage(s.training_stage) else { return };
        let Some(map) = w.stage_map.as_ref() else { return };
        let Some(platform) = s.campaign.as_ref().and_then(|c| c.bonus.as_ref())
            .and_then(|b| b.platforms.iter().flatten().find(|p| !p.boarded)) else { return };
        let Some(floor) = pack.stage_lines(&stage).find(|l| l.kind == ssb_rom::pack::line_kind::FLOOR && l.yakumono == platform.group && pack.line_vertices(l).next().is_some_and(|v| v.flags & 0xff == 14)) else { return };
        let mut vertices = pack.line_vertices(&floor);
        let Some(left) = vertices.next() else { return };
        let right = vertices.last().unwrap_or(left);
        let offset = map.groups[platform.group as usize].translate;
        // Teleport above each real floor; the normal swept map collision
        // must land the fighter before the objective process can credit it.
        ssb_game::status::set_fall(&mut pl.fighter);
        pl.fighter.pos = offset + ssb_engine::math::Vec3::new((f32::from(left.x) + f32::from(right.x)) * 0.5, f32::from(left.y) + 100.0, 0.0);
        pl.fighter.floor = None;
        pl.fighter.physics.vel_air = ssb_engine::math::Vec3::ZERO;
    }
    if scene == GameScene::OnePTargetClear && (600..810).contains(&tick) {
        let Some(pl) = w.play_state.as_mut() else { return };
        // Keep the fighter over its real starting floor and offer one
        // target at a time to Mario's actual jab script; course motion
        // is stopped only for this diagnostic placement fixture.
        if tick % 20 == 0 {
            ssb_game::status::set_wait(&mut pl.fighter);
            ssb_game::status::set_attack11(&mut pl.fighter);
        }
        let next = (0..ssb_game::item::ITEM_ALLOC_MAX as u8).find(|&i| w.items.get(i).is_some_and(|i| i.kind == ssb_game::item::ItemKind::Target));
        if let Some(index) = next {
            if let Some(item) = w.items.get_mut(index) {
                use ssb_game::item::ItemAnims;
                ssb_psp_runtime::scene::ItemAnimsPort { pack, objects: &mut w.stage_objects }.stop_root(item.anim_target());
                item.pos = pl.fighter.pos + ssb_engine::math::Vec3::new(100.0 * pl.fighter.facing.sign(), 120.0, 0.0);
                item.update_attack_positions();
            }
        }
    }
}
