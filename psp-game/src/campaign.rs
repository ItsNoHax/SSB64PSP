//! The 1P Game on the PSP: the portable campaign frontend
//! (`ssb_game::spgame::Frontend`, RE-448) between the 1P select and real
//! battles (`sc1PGameFuncStart`, RE-450).
//!
//! The select's START begins a campaign; its intro, continue and
//! stage-clear controllers run here, and a requested battle runs on the
//! shared Training/VS world with the campaign's `Session` collecting its
//! callbacks, falls and enemy replacements. A scene the PSP cannot run yet
//! (the bonus stages, Master Hand, the ending, challengers, unlock
//! messages, or a battle whose fighters the pack lacks) stops the campaign
//! with an explicit blocked screen: it is never replaced by a VS battle or
//! skipped. The presentation screens draw interim placeholders until their
//! authored sprites are packed.

use super::*;
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
    pub frontend: alloc::boxed::Box<Frontend>,
    /// The current overlay's scheduler clock; `sc1PIntroFuncStart` and each
    /// scene start reset it.
    tic: u32,
    /// Why the campaign stopped, at a scene it cannot run yet.
    blocked: Option<Blocked>,
}

/// A scene the host cannot run yet.
#[derive(Clone, Copy)]
enum Blocked {
    /// No PSP controller yet: bonus stages, ending, challenger, message.
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
        frontend: alloc::boxed::Box::new(Frontend::campaign(s.spgame_scene.clone(), &s.backup)),
        tic: 0,
        blocked: None,
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
pub(crate) fn frame(s: &mut Session, pack: Option<&Pack<'_>>, controller: ControllerState, pressed: N64Buttons) {
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
    c.frontend.tick(c.tic, controller, pressed, &mut s.backup, |e| {
        if let Event::Host(scene) = e {
            host = Some(scene);
        }
    });
    if core::mem::discriminant(&c.frontend.screen) != before {
        c.tic = 0;
    }
    if let Some(scene) = host {
        on_host(s, pack, scene);
    }
}

/// A scene request from the frontend.
fn on_host(s: &mut Session, pack: Option<&Pack<'_>>, scene: Scene) {
    match scene {
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
    let index = ssb_psp_runtime::scene::common_stage_index(p, gkind).ok_or(Blocked::Assets(stage))?;
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
    s.items.normal_drops = ssb_game::item::normal::DropWeights::new(switches, desc.item_weights.as_ref());
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

/// `ftManagerMakeFighter`'s 1P Game fields and `Session`'s KO machinery.
fn configure(
    sp: &mut Campaign1P,
    f: &mut play::FighterScene,
    setup: spgame::setup::PlayerSetup,
    team_bounds: ssb_game::status::BlastZone,
) {
    sp.configure_fighter(&mut f.fighter, team_bounds);
    if ssb_game::kirby::is_kirby(f.fighter.kind) {
        f.fighter.kirby.copy_id = setup.copy_kind;
    }
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
                _ => "host",
            }
        }
    } else {
        "menu"
    };
    let line = alloc::format!(
        "campaign tick={} scene={} stage={:?} clock={:?} countdown={:?} cpu={:?}\n",
        tick,
        state,
        s.campaign.as_ref().and_then(|c| c.frontend.session.as_ref().map(|sp| sp.data.stage)),
        s.vs_battle.as_ref().map(|b| b.clock()),
        s.damage_hud.countdown.is_some(),
        s.dummies[0].as_ref().map(|d| d.computer.behavior),
    );
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
    }
}

/// `sc1PGameFuncUpdate`'s Go and Set checks, after the battle's frame.
pub(crate) fn update_game(sp: &mut Campaign1P, status: ssb_game::battle::GameStatus, player: &ssb_game::fighter::Fighter) {
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
            // The camera follows `zoom_port` below; Race, Master Hand and
            // the bonus stages are not entered.
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
        let dist = p.fighter(f.fighter.kind as u32).map_or(1000.0, |d| d.closeup_camera_zoom);
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
        let Some(d) = slot.as_deref_mut() else { continue };
        if !d.fighter.dead.enemy_next_pending {
            continue;
        }
        let port = d.fighter.port;
        core::mem::swap(&mut sp.battle, battle);
        let next = sp.next_enemy(port, &points, f32::from(stage.camera.top), f32::from(stage.bounds.top));
        core::mem::swap(&mut sp.battle, battle);
        match next {
            spgame::setup::NextEnemy::Sleep => {
                d.fighter.dead.enemy_next_pending = false;
                ssb_game::dead::set_sleep(&mut d.fighter);
            }
            spgame::setup::NextEnemy::Spawn(e) => {
                let mut n = play::Dummy::at_position(p, stage, e.fkind, e.costume, e.level, e.pos, port);
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

/// Interim presentation of the campaign's own scenes: the authored intro,
/// continue and stage-clear sprites are not packed yet. Each scene shows
/// its state with plain rectangles; a blocked scene shows red.
#[inline(never)]
pub(crate) fn draw(gpu: &mut Gpu, s: &Session) {
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(BG_RESULTS));
    let Some(c) = s.campaign.as_ref() else { return };
    let stage = c.frontend.session.as_ref().map_or(0, |sp| sp.data.stage);
    match c.blocked {
        // The scene with no controller yet, marked under its slot in the
        // manager's scene list.
        Some(Blocked::Scene(scene)) => {
            gpu.draw_rect(40, 100, 440, 172, Color::rgba(160, 24, 24, 255));
            let x = 40 + scene as i32 * 30;
            gpu.draw_rect(x, 180, x + 24, 196, ENTRY_SELECTED);
        }
        // The stage whose fighters or ground the pack lacks.
        Some(Blocked::Assets(stage)) => {
            gpu.draw_rect(40, 100, 440, 172, Color::rgba(200, 120, 0, 255));
            let x = 40 + stage as i32 * 29;
            gpu.draw_rect(x, 180, x + 24, 196, ENTRY_SELECTED);
        }
        None => {}
    }
    // The campaign's fourteen stages, the current one lit.
    for i in 0..14u8 {
        let x = 40 + i32::from(i) * 29;
        let color = if i == stage {
            ENTRY_SELECTED
        } else if i < stage {
            ENTRY_ENABLED
        } else {
            ENTRY_DISABLED
        };
        gpu.draw_rect(x, 40, x + 24, 56, color);
    }
    match &c.frontend.screen {
        Scene1P::Continue(scene) if scene.options_shown => {
            let (yes, no) = if scene.yes {
                (ENTRY_SELECTED, ENTRY_ENABLED)
            } else {
                (ENTRY_ENABLED, ENTRY_SELECTED)
            };
            gpu.draw_rect(120, 180, 220, 210, yes);
            gpu.draw_rect(260, 180, 360, 210, no);
        }
        Scene1P::StageClear(scene) => {
            let rows = scene
                .rows
                .iter()
                .flatten()
                .filter(|r| r.reveal_tic < scene.total_tics)
                .count();
            for row in 0..rows {
                let y = 80 + row as i32 * 16;
                gpu.draw_rect(120, y, 360, y + 10, ENTRY_ENABLED);
            }
        }
        _ => {}
    }
}
