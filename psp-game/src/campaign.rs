//! The 1P Game on the PSP: the portable campaign frontend
//! (`ssb_game::spgame::Frontend`, RE-448) between the 1P select and real
//! battles (`sc1PGameFuncStart`, RE-450).
//!
//! The select's START begins a campaign; its intro, continue and
//! stage-clear controllers run here, and a requested battle runs on the
//! shared Training/VS world with the campaign's `Session` collecting its
//! callbacks, falls and enemy replacements. A scene the PSP cannot run yet
//! (Board the Platforms, Race to the Finish, Master Hand, the ending, challengers, unlock
//! messages, or a battle whose fighters the pack lacks) stops the campaign
//! with an explicit blocked screen: it is never replaced by a VS battle or
//! skipped. Authored presentation is bound by campaign_screen.

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

/// The campaign the PSP is running, across its scenes.
pub(crate) struct Campaign {
    pub bonus: Option<alloc::boxed::Box<spgame::bonus_stage::BonusStage>>,
    pub frontend: alloc::boxed::Box<Frontend>,
    /// The current overlay's scheduler clock; `sc1PIntroFuncStart` and each
    /// scene start reset it.
    tic: u32,
    /// Why the campaign stopped, at a scene it cannot run yet.
    blocked: Option<Blocked>,
    presentation: presentation::Presentation,
}

/// A scene the host cannot run yet.
#[derive(Clone, Copy)]
enum Blocked {
    /// No PSP controller yet: Platforms, Race, ending, challenger, message.
    Scene(Scene),
    /// The stage or a fighter (Metal Mario, Giant Donkey Kong, the
    /// Polygons, Master Hand) is not in the pack.
    Assets(Stage),
}

impl Campaign {
    fn session(&mut self) -> &mut Campaign1P {
        self.frontend.session.as_mut().expect("campaign session")
    }
}

/// `nSCKind1PGame` from the 1P select's ready START, after the select
/// applied its scene and backup data.
pub(crate) fn start(s: &mut Session) {
    s.campaign = Some(Campaign {
        bonus: None,
        frontend: alloc::boxed::Box::new(Frontend::campaign(s.spgame_scene.clone(), &s.backup)),
        tic: 0,
        blocked: None,
        presentation: presentation::Presentation::default(),
    });
    s.screen = Screen::Campaign;
}

/// Ends the campaign for `next`: the 1P mode menu (the menu stands in for
/// it) or the title.
fn leave(s: &mut Session, next: Screen) {
    s.campaign = None;
    s.play_state = None;
    s.dummies = Default::default();
    s.vs_battle = None;
    s.screen = next;
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
        s.screen = Screen::Menu;
        return;
    };
    if c.blocked.is_some() {
        if pressed.contains(N64Buttons::START) || pressed.contains(N64Buttons::B) {
            leave(s, Screen::Menu);
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
        Scene::ModeMenu => leave(s, Screen::Menu),
        Scene::Startup => leave(s, Screen::Intro),
        other => {
            if let Some(c) = s.campaign.as_mut() {
                c.blocked = Some(Blocked::Scene(other));
            }
        }
    }
}

/// `sc1PGameFuncStart`: the manager's stage setup and the fighters it
/// describes, on the shared battle world.
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
        if p.fighter(b.fkind as u32).is_none() || p.spawn(&desc, setup.mapobj_kind).is_none() {
            return Err(Blocked::Assets(stage));
        }
        roster[i] = Some(Entrant {
            kind: b.fkind,
            costume: b.costume,
            level: b.level,
            handicap: b.handicap,
            spawn: setup.mapobj_kind,
            team: b.team,
            color: b.color,
            human: b.pkind == spgame::PlayerKind::Man,
        });
    }
    let setups = game.setups;
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
    };
    s.enter(pack, gkind, roster, Some(rules));
    s.scene_gkind = gkind;
    // `itManagerInitItems` with the 1P stage's switches.
    s.items.normal_switches = switches;
    s.items.normal_drops =
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
    let Some(pl) = s.play_state.as_mut() else {
        return Err(Blocked::Assets(stage));
    };
    for f in scenes(pl, &mut s.dummies).into_iter().flatten() {
        let port = usize::from(f.fighter.port);
        configure(sp, f, setups[port], team_bounds);
    }
    for d in s.dummies.iter_mut().flatten() {
        let setup = setups[usize::from(d.fighter.port)];
        d.is_1p_game = true;
        d.computer.trait_kind = setup.cp_trait;
    }
    s.damage_hud.single_stock = Some(single);
    for (port, d) in s.damage_hud.damage.iter_mut().enumerate() {
        if positions[port] != 0 {
            *d = ssb_game::hud::DamageDisplay::at(port, d.damage, positions[port]);
        }
    }
    c.tic = 0;
    Ok(())
}

/// `sc1PBonusStageFuncStart`: a separate bonus world, one fighter and ten
/// targets. Campaign stocks and its accumulated battle records stay put.
fn enter_bonus(s: &mut Session, pack: Option<&Pack<'_>>) -> Result<(), Blocked> {
    let c = s.campaign.as_mut().expect("campaign");
    let data = &c.frontend.session.as_ref().expect("campaign session").data;
    if data.stage() != Some(Stage::Bonus1) { return Err(Blocked::Scene(Scene::BonusStage)); }
    if data.player != 0 { return Err(Blocked::Assets(Stage::Bonus1)); }
    let bonus = spgame::bonus_stage::BonusStage::targets(data);
    let p = pack.ok_or(Blocked::Assets(Stage::Bonus1))?;
    let index = ssb_psp_runtime::scene::common_stage_index(p, bonus.state.gkind).ok_or(Blocked::Assets(Stage::Bonus1))?;
    let stage = p.stage(index).ok_or(Blocked::Assets(Stage::Bonus1))?;
    let kind = data.fkind as usize;
    let placement = (0..p.object_count()).filter_map(|i| p.object(i)).find(|o|
        o.source_file == 124 + kind as u32 && o.source_offset == ssb_rom::bonus1::COURSES[kind].placements)
        .ok_or(Blocked::Assets(Stage::Bonus1))?;
    if placement.node_count != 11 || p.fighter(data.fkind as u32).is_none()
        || p.spawn(&stage, spgame::setup::mapobj::PLAYER).is_none() {
        return Err(Blocked::Assets(Stage::Bonus1));
    }
    let positions: alloc::vec::Vec<_> = (1..11).filter_map(|i| p.node(placement.first_node + i))
        .map(|n| ssb_engine::math::Vec3::new(n.rest_translate[0], n.rest_translate[1], n.rest_translate[2])).collect();
    let mut roster: Roster = [None; 4];
    roster[0] = Some(Entrant { kind: data.fkind, costume: data.costume, level: 1,
        handicap: ssb_game::stale::HANDICAP_DEFAULT, spawn: spgame::setup::mapobj::PLAYER,
        team: 0, color: 0, human: true });
    let gkind = bonus.state.gkind;
    s.enter(pack, gkind, roster, Some(VsRules { rule: ssb_game::battle::Rule::Time,
        time_limit: bonus.state.time_limit, stocks: 0, team_rules: ssb_game::team::TeamRules::FREE_FOR_ALL }));
    s.scene_gkind = gkind;
    s.vs_battle = Some(bonus.battle());
    s.items.appear = None;
    bonus.make_targets(&positions, &mut s.items);
    let pl = s.play_state.as_mut().ok_or(Blocked::Assets(Stage::Bonus1))?;
    ssb_game::status::set_wait_or_fall(&mut pl.fighter);
    pl.fighter.facing = if pl.fighter.pos.x >= 0.0 { ssb_game::fighter::Facing::Left } else { ssb_game::fighter::Facing::Right };
    pl.fighter.stats.enable();
    pl.fighter.dead.stock_rule = false;
    pl.fighter.dead.spgame_rule = false;
    pl.fighter.dead.bonus_rule = true;
    pl.bonus_follow = true;
    s.damage_hud.damage[0] = ssb_game::hud::DamageDisplay::at(0, 0, 55);
    s.damage_hud.bonus_tasks = Some(10);
    s.campaign.as_mut().expect("campaign").bonus = Some(alloc::boxed::Box::new(bonus));
    Ok(())
}

pub(crate) fn bonus_frame(p: &Pack<'_>, hud: &mut Hud, b: &ssb_game::battle::Battle, tasks: u8) {
    hud.bonus_tasks = Some(tasks);
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
                Scene1P::Host(Scene::Battle) => "battle",
                Scene1P::Host(Scene::BonusStage) => "bonus",
                _ => "host",
            }
        }
    } else {
        "menu"
    };
    let line = alloc::format!(
        "campaign tick={} scene={} stage={:?} clock={:?} countdown={:?} cpu={:?} targets={:?} end={:?} tasks={:?} stocks={:?}\n",
        tick,
        state,
        s.campaign
            .as_ref()
            .and_then(|c| c.frontend.session.as_ref().map(|sp| sp.data.stage)),
        s.vs_battle.as_ref().map(|b| b.clock()),
        s.damage_hud.countdown.is_some(),
        s.dummies[0].as_ref().map(|d| d.computer.behavior),
        s.campaign.as_ref().and_then(|c| c.bonus.as_ref().map(|b| b.tasks_remain)),
        s.vs_battle.as_ref().and_then(|b| b.end),
        s.campaign.as_ref().and_then(|c| c.frontend.session.as_ref().map(|sp| sp.data.bonus_tasks_complete)),
        s.campaign.as_ref().and_then(|c| c.frontend.session.as_ref().map(|sp| sp.state.players[sp.data.player as usize].stock_count)),
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
pub(crate) fn update_game(
    sp: &mut Campaign1P,
    status: ssb_game::battle::GameStatus,
    player: &ssb_game::fighter::Fighter,
) {
    if let Some(game) = sp.game.as_mut() {
        game.update(status, || {
            let end = match player.status.status {
                ssb_game::status::AnyStatus::Common(s) => spgame::bonus::EndStatus::from(s),
                _ => spgame::bonus::EndStatus::Other,
            };
            (end, u32::from(player.star_invincible_frames))
        });
    }
}

/// The 1P Game's wait thread (`sc1PGameWaitStage*Update`) in place of the
/// VS entry focus: the countdown, each fighter's appearance or team drop,
/// and the random pattern's zooms.
#[inline(never)]
pub(crate) fn entry_frame(
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    hud: &mut Hud,
    b: &ssb_game::battle::Battle,
    wait: &spgame::wait::Wait,
) {
    let clock = b.clock();
    for action in wait.at(clock) {
        match action {
            Action::Countdown => hud.countdown = Some(ssb_game::countdown::Countdown::new()),
            Action::Appear(port) => {
                if let Some(f) = scenes(pl, dummies)[usize::from(port)].as_deref_mut() {
                    ssb_game::appear::appear_set_status(&mut f.fighter);
                }
            }
            Action::AppearPosition(port) => {
                if let Some(f) = scenes(pl, dummies)[usize::from(port)].as_deref_mut() {
                    ssb_game::appear::appear_set_position(
                        &mut f.fighter,
                        f32::from(stage.camera.top),
                        f32::from(stage.bounds.top),
                    );
                }
            }
            // The camera follows `zoom_port` below; Race and Master Hand
            // are not entered. Targets uses its separate bonus entry path.
            Action::Zoom(_)
            | Action::CameraDefault
            | Action::Go
            | Action::BossGo
            | Action::Bonus3Follow
            | Action::BossCameraAnim => {}
        }
    }
    if let Some(c) = hud.countdown.as_mut() {
        c.tick(&game_status_sizes(p));
    }
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
    pl.entry_zoom = zoom_port.and_then(|port| {
        let f = scenes_ref(pl, dummies)[usize::from(port)]?;
        let mut pos = f.fighter.pos;
        pos.y += f.cam_offset_y;
        let dist = p
            .fighter(f.fighter.kind as u32)
            .map_or(1000.0, |d| d.closeup_camera_zoom);
        Some((pos, dist))
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

/// `ftCommonDeadCheckRebirth`'s 1P Game branch: an enemy whose KO wait
/// ended sleeps, or is replaced in its slot by the team's next member
/// (`sc1PGameSpawnEnemyTeamNext`).
#[inline(never)]
pub(crate) fn replace_enemies(
    p: &Pack<'_>,
    stage: &ssb_rom::pack::StageDesc,
    dummies: &mut Dummies,
    sp: &mut Campaign1P,
    battle: &mut Option<ssb_game::battle::Battle>,
    hud: &mut Hud,
) {
    let points: alloc::vec::Vec<_> = p
        .stage_points(stage)
        .filter(|point| point.kind == spgame::setup::mapobj::ENEMY_TEAM)
        .map(|point| ssb_engine::math::Vec3::new(f32::from(point.x), f32::from(point.y), 0.0))
        .collect();
    let team_bounds = ssb_game::status::BlastZone {
        top: f32::from(stage.team_bounds.top),
        bottom: f32::from(stage.team_bounds.bottom),
        left: f32::from(stage.team_bounds.left),
        right: f32::from(stage.team_bounds.right),
    };
    for slot in dummies.iter_mut() {
        let Some(d) = slot.as_deref_mut() else {
            continue;
        };
        if !d.fighter.dead.enemy_next_pending {
            continue;
        }
        let port = d.fighter.port;
        core::mem::swap(&mut sp.battle, battle);
        let next = sp.next_enemy(
            port,
            &points,
            f32::from(stage.camera.top),
            f32::from(stage.bounds.top),
        );
        core::mem::swap(&mut sp.battle, battle);
        match next {
            spgame::setup::NextEnemy::Sleep => {
                d.fighter.dead.enemy_next_pending = false;
                ssb_game::dead::set_sleep(&mut d.fighter);
            }
            spgame::setup::NextEnemy::Spawn(e) => {
                let mut n =
                    play::Dummy::at_position(p, stage, e.fkind, e.costume, e.level, e.pos, port);
                n.fighter.facing = e.facing;
                n.fighter.team = d.fighter.team;
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
                let setup = sp.game.as_ref().expect("active game").setups[usize::from(port)];
                configure(sp, &mut n, setup, team_bounds);
                // `is_skip_entry`: wait or fall, unlocked at once.
                ssb_game::status::set_wait_or_fall(&mut n.fighter);
                // `ifCommonPlayerDamageStopBreakAnim`.
                if let Some(h) = hud.damage.get_mut(usize::from(port)) {
                    h.stop_break_anim();
                }
                *slot = Some(alloc::boxed::Box::new(n));
            }
        }
    }
}

/// The battle's end: its records go to the campaign once, then the
/// manager's next scene.
#[inline(never)]
pub(crate) fn finish_battle(s: &mut Session, pack: Option<&Pack<'_>>) {
    let Some(c) = s.campaign.as_mut() else { return };
    let sp = c.frontend.session.as_mut().expect("campaign session");
    if let Some(mut bonus) = c.bonus.take() {
        let battle = s.vs_battle.take().expect("bonus battle");
        sp.data.is_reset = battle.is_reset;
        let tasks = bonus.tasks_remain;
        sp.finish_bonus_stage(bonus.result(&battle), tasks, &mut s.backup);
        let scene = c.frontend.sync(&s.backup);
        c.tic = 0;
        s.play_state = None;
        s.dummies = Default::default();
        s.screen = Screen::Campaign;
        if matches!(c.frontend.screen, Scene1P::Host(_)) { on_host(s, pack, scene); }
        return;
    }
    // Callbacks still queued on the fighters belong to this battle.
    if let Some(pl) = s.play_state.as_mut() {
        for f in scenes(pl, &mut s.dummies).into_iter().flatten() {
            collect_fall(sp, &mut s.vs_battle, &mut f.fighter);
        }
    }
    sp.battle = s.vs_battle.take();
    sp.finish_battle(&mut s.backup);
    let scene = c.frontend.sync(&s.backup);
    c.tic = 0;
    s.play_state = None;
    s.dummies = Default::default();
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
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(BG_RESULTS));
    let Some(c) = s.campaign.as_mut() else { return };
    if let Some(blocked) = c.blocked {
        let (color, x) = match blocked {
            Blocked::Scene(scene) => (Color::rgba(160, 24, 24, 255), 40 + scene as i32 * 30),
            Blocked::Assets(stage) => (Color::rgba(200, 120, 0, 255), 40 + stage as i32 * 29),
        };
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
#[cfg(feature = "headless_capture")]
pub(crate) fn capture_fixture(s: &mut Session, scene: GameScene) {
    let Some(c) = s.campaign.as_mut() else { return };
    let sp = c.frontend.session.as_mut().expect("campaign");
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
        GameScene::OnePContinue | GameScene::OnePRetry => {
            sp.data.score = 123456;
            sp.manager.scene = Scene::Continue;
        }
        GameScene::OnePClear => {
            sp.data.score = 123456;
            sp.data.time_remain = 250;
            sp.state.players[sp.data.player as usize].total_damage_given = 321;
            sp.data.bonus_get_mask = [0x0018_0005, 0, 0];
            sp.manager.scene = Scene::StageClear;
        }
        _ => return,
    }
    c.frontend.sync(&s.backup);
    c.tic = 0;
    s.screen = Screen::Campaign;
}

/// Diagnostic placement fixtures use the real hit search and real blast
/// check; they do not set the task counter or the results directly.
#[cfg(feature = "headless_capture")]
pub(crate) fn capture_objectives(s: &mut Session, pack: Option<&Pack<'_>>, scene: GameScene, tick: u64) {
    let Some(pack) = pack else { return };
    if !s.campaign.as_ref().is_some_and(|c| c.bonus.is_some()) { return; }
    if scene == GameScene::OnePTargetFall && tick == 600 {
        if let Some(pl) = s.play_state.as_mut() {
            ssb_game::status::set_fall(&mut pl.fighter);
            pl.fighter.pos.y = -20000.0;
        }
    }
    if scene == GameScene::OnePTargetClear && (600..810).contains(&tick) {
        let Some(pl) = s.play_state.as_mut() else { return };
        // Keep the fighter over its real starting floor and offer one
        // target at a time to Mario's actual jab script; course motion
        // is stopped only for this diagnostic placement fixture.
        if tick % 20 == 0 {
            ssb_game::status::set_wait(&mut pl.fighter);
            ssb_game::status::set_attack11(&mut pl.fighter);
        }
        let next = (0..ssb_game::item::ITEM_ALLOC_MAX as u8).find(|&i| s.items.get(i).is_some_and(|i| i.kind == ssb_game::item::ItemKind::Target));
        if let Some(index) = next {
            if let Some(item) = s.items.get_mut(index) {
                use ssb_game::item::ItemAnims;
                ssb_psp_runtime::scene::ItemAnimsPort { pack, objects: &mut s.stage_objects }.stop_root(item.anim_target());
                item.pos = pl.fighter.pos + ssb_engine::math::Vec3::new(100.0 * pl.fighter.facing.sign(), 120.0, 0.0);
                item.update_attack_positions();
            }
        }
    }
}
