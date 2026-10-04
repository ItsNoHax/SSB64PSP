//! The 1P Game (`nSCKind1PGame`) on the PSP: `spgame::Frontend`'s scenes
//! and its stages' real battles, made and scored through
//! `spgame::session::Session`.
//!
//! A scene or stage the PSP cannot run yet ends the campaign at the menu;
//! nothing stands in for it (RE-450).

use super::*;
use ssb_game::spgame::{
    frontend::{Frontend, Screen as Scene2},
    manager::Scene,
    select::Selection,
    session::Session as Game,
    setup::NextEnemy,
    Stage,
};

/// The running campaign.
pub struct Campaign {
    pub frontend: Frontend,
    /// The current overlay's scheduler clock, which `sc1PIntroFuncStart`
    /// resets.
    tic: u32,
    /// A stage's battle is loaded and running.
    in_battle: bool,
    /// The stage's `nMPMapObjKind1PGameEnemyTeam` points.
    team_points: alloc::vec::Vec<ssb_engine::math::Vec3>,
}

/// The select's START (`nSCKind1PGame`): the campaign's first scene.
pub(crate) fn start(s: &mut Session, selection: Selection) {
    match Frontend::start(selection, &s.backup) {
        Some(frontend) => {
            s.campaign = Some(alloc::boxed::Box::new(Campaign {
                frontend,
                tic: 0,
                in_battle: false,
                team_points: alloc::vec::Vec::new(),
            }));
            s.screen = Screen::Campaign;
        }
        None => s.screen = Screen::Menu,
    }
}

/// The stages whose battles need what the PSP does not have yet: Giant
/// Donkey Kong's, Metal Mario's and the Polygons' battle clips, Race to
/// the Finish's controller and Master Hand.
fn unported(stage: Stage) -> bool {
    matches!(
        stage,
        Stage::Donkey | Stage::MMario | Stage::Bonus3 | Stage::Zako | Stage::Boss
    )
}

/// Ends the campaign: `nSCKindStartup` at the title, the 1P mode menu and
/// every unported scene at the menu.
fn leave(s: &mut Session, scene: Scene) {
    s.campaign = None;
    s.play_state = None;
    s.dummies = Default::default();
    s.screen = if scene == Scene::Startup { Screen::Intro } else { Screen::Menu };
}

/// One frame of the campaign: its scene, or its stage's battle.
pub(crate) unsafe fn frame(s: &mut Session, pack: Option<&Pack<'_>>, controller: ControllerState, pressed: N64Buttons) {
    let Some(c) = s.campaign.as_mut() else {
        s.screen = Screen::Menu;
        return;
    };
    if c.in_battle {
        battle_frame(s, pack, controller, pressed);
        return;
    }
    let host = match c.frontend.screen {
        Scene2::Host(scene) => Some(scene),
        _ => None,
    };
    match host {
        Some(Scene::Battle) => return enter_battle(s, pack),
        Some(scene) => return leave(s, scene),
        None => {}
    }
    let before = core::mem::discriminant(&c.frontend.screen);
    c.tic += 1;
    // The announcements and the continue's cues have no audio to play yet.
    c.frontend.tick(c.tic, controller, pressed, &mut s.backup, |_| {});
    if core::mem::discriminant(&c.frontend.screen) != before {
        c.synced();
    }
}

impl Campaign {
    /// After a scene change: `sc1PIntroFuncStart` restarts the scheduler
    /// clock the intro reads.
    fn synced(&mut self) {
        if matches!(self.frontend.screen, Scene2::Intro(_)) {
            self.tic = 0;
        }
    }

    fn game(&mut self) -> &mut Game {
        self.frontend.session.as_deref_mut().expect("campaign session")
    }
}

/// `sc1PGameFuncStart`: the stage's battle state and setups
/// (`Session::start_battle`), the stage, then each fighter as
/// `ftManagerMakeFighter` makes it from its `FTDesc`.
fn enter_battle(s: &mut Session, pack: Option<&Pack<'_>>) {
    let Some(c) = s.campaign.as_mut() else { return };
    let game = c.game();
    let stage = game.data.stage();
    // The host has one controller, which drives port 0.
    if pack.is_none() || game.data.player != 0 || stage.is_none_or(unported) {
        return leave(s, Scene::Battle);
    }
    game.start_battle(&s.backup);
    let entrants = game.entrants();
    let gkind = game.state.gkind;
    let roster: Roster = entrants.map(|e| {
        e.map(|e| Entrant {
            kind: e.kind,
            costume: e.costume,
            level: e.level,
            handicap: e.handicap,
            spawn: e.setup.mapobj_kind,
            team: e.team,
            color: e.color,
            human: e.human,
        })
    });
    let switches = ssb_game::item::normal::Switches {
        appearance: game.state.item_appearance,
        toggles: game.state.item_toggles,
    };
    s.vs_battle = None;
    s.roster = roster;
    s.play_state = None;
    s.training_stage = enter_training(
        pack,
        gkind,
        roster,
        None,
        switches,
        &mut None,
        &mut TrainingWorld {
            play_state: &mut s.play_state,
            dummies: &mut s.dummies,
            weapons: &mut s.weapons,
            items: &mut s.items,
            stage_objects: &mut s.stage_objects,
            stage_map: &mut s.stage_map,
            stage_ctl: &mut s.stage_ctl,
            damage_hud: &mut s.damage_hud,
        },
    );
    let (Some(p), Some(pl)) = (pack, s.play_state.as_mut()) else {
        return leave(s, Scene::Battle);
    };
    let Some(stage) = p.stage(s.training_stage) else {
        return leave(s, Scene::Battle);
    };
    let Some(c) = s.campaign.as_mut() else { return };
    let game = c.game();
    let team_bounds = zone(&stage.team_bounds);
    // `sc1PGameGetEnemyStartLR`, from each slot's start point.
    let start_x = |i: usize| {
        entrants[i]
            .and_then(|e| p.spawn(&stage, e.setup.mapobj_kind))
            .map_or(0.0, |point| f32::from(point.x))
    };
    let facing: [Option<ssb_game::fighter::Facing>; 4] = core::array::from_fn(|port| {
        let g = game.game.as_ref()?;
        entrants[port].map(|_| g.start_facing(&game.state, port, start_x))
    });
    for (port, f) in scenes(pl, &mut s.dummies).into_iter().enumerate() {
        let (Some(f), Some(e)) = (f, entrants[port]) else { continue };
        let fighter = &mut f.fighter;
        if let Some(facing) = facing[port] {
            fighter.facing = facing;
        }
        if fighter.kind == ssb_game::fighter::FighterKind::Kirby {
            ssb_game::kirby::init_copy(fighter, e.setup.copy_kind);
        }
        game.configure_fighter(fighter, team_bounds);
        fighter.interface.magnify_ignore = e.setup.is_magnify_ignore;
        if e.setup.is_skip_entry {
            ssb_game::status::set_wait_or_fall(fighter);
        } else {
            ssb_game::appear::entry_set_status(fighter);
        }
        f.camera_zoom_frame *= e.setup.camera_frame_mul;
    }
    for (d, e) in s.dummies.iter_mut().zip(&entrants[1..]) {
        let (Some(d), Some(e)) = (d.as_deref_mut(), e) else { continue };
        d.computer.behavior = ssb_game::computer::Behavior::Default;
        d.computer.trait_kind = e.setup.cp_trait;
        d.is_1p_game = true;
    }
    let rules = game.battle.as_ref().map(ssb_game::battle::Battle::team_rules);
    if let Some(rules) = rules {
        s.weapons.team_rules = rules;
        s.items.team_rules = rules;
    }
    c.team_points = p
        .stage_points(&stage)
        .filter(|point| point.kind == ssb_game::spgame::setup::mapobj::ENEMY_TEAM)
        .map(|point| ssb_engine::math::Vec3::new(f32::from(point.x), f32::from(point.y), 0.0))
        .collect();
    c.in_battle = true;
    s.make_wallpaper(pack, gkind, false);
    s.training_menu = None;
    s.training_paused = false;
}

fn zone(e: &ssb_rom::pack::Extent) -> ssb_game::status::BlastZone {
    ssb_game::status::BlastZone {
        top: f32::from(e.top),
        bottom: f32::from(e.bottom),
        left: f32::from(e.left),
        right: f32::from(e.right),
    }
}

/// One frame of the stage's battle, its team replacements, then on its
/// end `sc1PGameFuncLights`'s handoff: the result into the session and
/// on to the manager's next scene.
unsafe fn battle_frame(s: &mut Session, pack: Option<&Pack<'_>>, controller: ControllerState, pressed: N64Buttons) {
    let (Some(p), Some(pl), Some(c)) = (pack, s.play_state.as_mut(), s.campaign.as_mut()) else {
        return;
    };
    let game = c.frontend.session.as_deref_mut().expect("campaign session");
    let done = training_frame(
        p,
        s.training_stage,
        pl,
        &mut s.dummies,
        &mut s.weapons,
        &mut s.items,
        &mut s.material_anim,
        &mut s.stage_objects,
        &mut s.stage_map,
        &mut s.stage_ctl,
        controller,
        pressed,
        Some(Rules::OneP(&mut *game)),
        &mut s.damage_hud,
        false,
    );
    let magnify_display = game.battle.as_ref().is_none_or(|b| {
        matches!(
            b.status,
            ssb_game::battle::GameStatus::Wait | ssb_game::battle::GameStatus::Go
        )
    });
    s.damage_hud.players.tick(magnify_display);
    next_enemies(p, s.training_stage, &mut s.dummies, game, &c.team_points, &mut s.damage_hud);
    if done {
        game.finish_battle(&mut s.backup);
        c.frontend.sync(&s.backup);
        c.in_battle = false;
        c.synced();
        s.play_state = None;
        s.dummies = Default::default();
    }
}

/// `sc1PGameSpawnEnemyTeamNext` for each enemy whose KO wait ended this
/// frame: it sleeps, or the next team member drops into its slot. The
/// original replaces it inside the fallen fighter's update; here the
/// newcomer is made after the frame and first runs on the next (RE-450).
fn next_enemies(
    p: &Pack<'_>,
    stage_index: u32,
    dummies: &mut Dummies,
    game: &mut Game,
    points: &[ssb_engine::math::Vec3],
    hud: &mut Hud,
) {
    let Some(stage) = p.stage(stage_index) else { return };
    for (i, slot) in dummies.iter_mut().enumerate() {
        let Some(d) = slot.as_deref_mut().filter(|d| d.fighter.dead.enemy_next_pending) else {
            continue;
        };
        d.fighter.dead.enemy_next_pending = false;
        let port = i as u8 + 1;
        match game.next_enemy(port, points, f32::from(stage.camera.top), f32::from(stage.bounds.top)) {
            NextEnemy::Sleep => ssb_game::dead::set_sleep(&mut d.fighter),
            NextEnemy::Spawn(e) => {
                let mut d = play::Dummy::at_point(p, &stage, e.fkind, e.costume, e.level, e.pos, port);
                let f = &mut d.fighter;
                f.facing = e.facing;
                let detail = if e.detail_high {
                    ssb_game::modelpart::Detail::High
                } else {
                    ssb_game::modelpart::Detail::Low
                };
                f.model_parts.detail_curr = detail;
                f.model_parts.detail_base = detail;
                if f.kind == ssb_game::fighter::FighterKind::Kirby {
                    ssb_game::kirby::init_copy(f, e.copy_kind);
                }
                game.configure_fighter(f, zone(&stage.team_bounds));
                f.interface.magnify_ignore = e.is_magnify_ignore;
                // `is_skip_entry`.
                ssb_game::status::set_wait_or_fall(f);
                d.camera_zoom_frame *= e.camera_frame_mul;
                d.computer.behavior = ssb_game::computer::Behavior::Default;
                d.computer.trait_kind = e.cp_trait;
                d.is_1p_game = true;
                if let Some(display) = hud.damage.get_mut(usize::from(port)) {
                    display.stop_break_anim();
                }
                *slot = Some(alloc::boxed::Box::new(d));
            }
        }
    }
}

/// The 1P Game's wait thread (`sc1PGameWaitThreadUpdate`) on the battle's
/// clock: the countdown, each fighter's entry or drop-in and the entry
/// zoom; then the countdown's tick.
pub(crate) fn wait_frame(
    p: &Pack<'_>,
    stage_index: u32,
    pl: &mut play::FighterScene,
    dummies: &mut Dummies,
    hud: &mut Hud,
    game: &mut Game,
) {
    use ssb_game::spgame::wait::Action;
    let (Some(b), Some(wait), Some(stage)) = (game.battle.as_ref(), game.wait.as_ref(), p.stage(stage_index)) else {
        return;
    };
    for action in wait.at(b.clock()) {
        let mut fighters = scenes(pl, dummies);
        match action {
            Action::Countdown => hud.countdown = Some(ssb_game::countdown::Countdown::new()),
            Action::Appear(port) => {
                if let Some(f) = fighters.get_mut(usize::from(port)).and_then(Option::as_deref_mut) {
                    ssb_game::appear::appear_set_status(&mut f.fighter);
                }
            }
            Action::AppearPosition(port) => {
                if let Some(f) = fighters.get_mut(usize::from(port)).and_then(Option::as_deref_mut) {
                    ssb_game::appear::appear_set_position(
                        &mut f.fighter,
                        f32::from(stage.camera.top),
                        f32::from(stage.bounds.top),
                    );
                }
            }
            Action::Zoom(port) => hud.campaign_zoom = Some(port),
            Action::CameraDefault => hud.campaign_zoom = None,
            // Race to the Finish's and Master Hand's, whose stages end the
            // campaign before their battles.
            Action::Bonus3Follow | Action::BossCameraAnim | Action::Go | Action::BossGo => {}
        }
    }
    if let Some(countdown) = hud.countdown.as_mut() {
        countdown.tick(&game_status_sizes(p));
    }
    // `gmCameraSetStatusPlayerZoom(fighter, 0, 0, closeup_camera_zoom,
    // 0.1, 28)`, the same zoom as the VS entry focus.
    let target = hud.campaign_zoom.and_then(|port| {
        let scene = scenes_ref(pl, dummies).get(usize::from(port)).copied().flatten()?;
        let mut pos = scene.fighter.pos;
        pos.y += scene.cam_offset_y;
        let dist = p.fighter(scene.fighter.kind as u32).map_or(1000.0, |d| d.closeup_camera_zoom);
        Some((pos, dist))
    });
    pl.entry_zoom = target;
}

/// The campaign's frame: its stage's battle, or its scene. The intro,
/// continue and stage-clear scenes' sprites, cameras and fighters are not
/// drawn yet; they show their clear colour.
pub(crate) unsafe fn draw(
    gpu: &mut Gpu,
    s: &mut Session,
    pack: &Option<Pack<'_>>,
    draw_state: &mut meshdraw::DrawState,
    effect_visuals: &mut EffectVisuals,
    draw_assets: &DrawAssets,
    no_pack_color: Color,
) {
    if s.campaign.as_ref().is_some_and(|c| c.in_battle) {
        let c = s.campaign.take();
        let battle = c
            .as_ref()
            .and_then(|c| c.frontend.session.as_ref())
            .and_then(|game| game.battle.as_ref());
        draw_world(gpu, s, pack, draw_state, effect_visuals, draw_assets, no_pack_color, battle);
        s.campaign = c;
        return;
    }
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(BG_RESULTS));
}
