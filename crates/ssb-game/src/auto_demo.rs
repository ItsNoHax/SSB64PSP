//! `sc/sccommon/scautodemo.c`: the title's auto demo, four level-9 CPUs on
//! the next stage of a fixed order.
//!
//! Players 1 and 2 are the title's demo fighters
//! (`mnTitleSetDemoFighterKinds`), 3 and 4 two more unlocked fighters; each
//! starts at a random damage on one of the stage's eight auto demo points
//! (`nMPMapObjKindAutoDemoPlayer1..8`), already standing
//! (`desc.is_skip_entry`). The battle is a VS battle with no timer, pause or
//! entry. A focus script ([`FOCUS`]) fades the scene in, closes the camera
//! in on player 1 with its name shown and the others at level 1, then on
//! player 2, then pulls back and ends the scene on the N64 logo
//! (`nSCKindStartup`, US). A, B or START go back to the title.

use crate::backup::{Backup, CHARACTER_MASK_STARTER};
use crate::fighter::FighterKind;
use crate::menu::title::{kind_of, kinds_num, shuffled_kind, DemoData};
use crate::menu::Scene;
use crate::stage_select::gkind;

/// `dSCAutoDemoGroundOrder`.
pub const GROUND_ORDER: [u8; 8] = [
    gkind::PUPUPU,
    gkind::ZEBES,
    gkind::CASTLE,
    gkind::JUNGLE,
    gkind::SECTOR,
    gkind::YOSTER,
    gkind::YAMABUKI,
    gkind::HYRULE,
];

/// `dSCAutoDemoMapObjKindList`: `nMPMapObjKindAutoDemoPlayer1..8`.
pub const MAP_OBJ_KINDS: [u16; 8] = [0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F];

/// `dSCAutoDemoZoomEyeX` and `...Y`, in degrees.
pub const ZOOM_EYE_X: [f32; 6] = [-40.0, -28.0, -14.0, 14.0, 28.0, 40.0];
pub const ZOOM_EYE_Y: [f32; 5] = [2.0, 0.0, -6.0, -9.0, -30.0];
/// `scAutoDemoSetCameraPlayerZoom`'s pan scale and field of view; the
/// distance is the fighter's `closeup_camera_zoom`.
pub const ZOOM_PAN_SCALE: f32 = 0.3;
pub const ZOOM_FOV: f32 = 28.0;

/// `scAutoDemoInitDemo`'s CPU level, and the level the unfocused players
/// drop to.
pub const LEVEL: u8 = 9;
pub const LEVEL_UNFOCUSED: u8 = 1;
/// `scAutoDemoMakeFade`: 30 frames in from black.
pub const FADE_LENGTH: u32 = 30;
/// `scAutoDemoInitSObjs`: each name is centred on (160, 50).
pub const NAME_CENTRE: [f32; 2] = [160.0, 50.0];

/// A focus step's change (`SCAutoDemoProc.func_change`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// `scAutoDemoMakeFade`.
    Fade,
    /// `scAutoDemoSetFocusPlayer1`.
    FocusPlayer1,
    /// `scAutoDemoSetFocusPlayer2`.
    FocusPlayer2,
    /// `scAutoDemoResetFocusPlayerAll`.
    ResetFocus,
    /// `scAutoDemoSetMagnifyDisplayOn`.
    MagnifyOn,
    /// `scAutoDemoExit`.
    Exit,
}

/// `dSCAutoDemoFuncList`: frames until the next change, the change, and
/// the player whose death cuts the step short (`func_focus`).
pub const FOCUS: [(u16, Option<Change>, Option<usize>); 7] = [
    (0, None, None),
    (340, Some(Change::Fade), None),
    (340, Some(Change::FocusPlayer1), None),
    (340, Some(Change::FocusPlayer2), Some(0)),
    (400, Some(Change::ResetFocus), Some(1)),
    (60, Some(Change::MagnifyOn), None),
    (1, Some(Change::Exit), None),
];

/// What the battle shows the focus script, and what the script does to it.
pub trait World {
    /// `scAutoDemoCheckStopFocusPlayer`: the player is in a dead status.
    fn is_dead(&self, player: usize) -> bool;
    /// `ftGetStruct(...)->level`.
    fn set_level(&mut self, player: usize, level: u8);
    /// `scAutoDemoSetCameraPlayerZoom`: the camera's player zoom on
    /// `player` with these eye angles in radians.
    fn zoom(&mut self, player: usize, eye: (f32, f32));
    /// `gmCameraSetStatusDefault`.
    fn reset_camera(&mut self);
    /// `ftParamSetModelPartDetailAll` and `detail_base`: high or low.
    fn set_detail_high(&mut self, player: usize, high: bool);
    /// `lbFadeMakeActor`.
    fn fade(&mut self);
    /// `syUtilsRandIntRange`.
    fn rand(&mut self, range: i32) -> i32;
}

/// `scAutoDemoInitDemo`'s battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    pub gkind: u8,
    pub fkinds: [FighterKind; 4],
    /// `players[].stock_damage_all`, each fighter's starting damage.
    pub damage: [u16; 4],
}

/// The stage the next [`init`] picks: `GROUND_ORDER` in turn, without
/// the random number generator, so a host can read its files ahead
/// (RE-476).
pub fn next_gkind(demo: &DemoData) -> u8 {
    let order = usize::from(demo.demo_gkind_order).min(GROUND_ORDER.len() - 1);
    GROUND_ORDER[order]
}

/// Every fighter the next [`init`] can field: the title's two, and the
/// unlocked kinds players 3 and 4 are drawn from (RE-476). Without the
/// random number generator.
pub fn candidate_kinds(demo: &DemoData, backup: &Backup) -> impl Iterator<Item = FighterKind> {
    let unlocked = backup.fighter_mask
        | CHARACTER_MASK_STARTER
        | (1 << demo.demo_fkind[0] as u16)
        | (1 << demo.demo_fkind[1] as u16);
    (0..16usize)
        .filter(move |&k| unlocked & (1 << k) != 0)
        .map(kind_of)
}

/// `scAutoDemoInitDemo`: the stage after the last, then each player's
/// fighter and damage in turn.
pub fn init(demo: &mut DemoData, backup: &Backup, rand: &mut impl FnMut(i32) -> i32) -> Setup {
    let gkind = next_gkind(demo);
    demo.demo_gkind_order += 1;
    if usize::from(demo.demo_gkind_order) >= GROUND_ORDER.len() {
        demo.demo_gkind_order = 0;
    }
    let mut mask = (1u16 << demo.demo_fkind[0] as u16) | (1u16 << demo.demo_fkind[1] as u16);
    let mut fkinds = [FighterKind::Mario; 4];
    let mut damage = [0; 4];
    for player in 0..4 {
        // `scAutoDemoGetFighterKind`.
        fkinds[player] = if player < 2 {
            demo.demo_fkind[player]
        } else {
            let unlocked = backup.fighter_mask | CHARACTER_MASK_STARTER;
            let fresh = kinds_num(unlocked) - kinds_num(mask);
            let k = shuffled_kind(unlocked, mask, rand(fresh));
            mask |= 1 << k;
            kind_of(k)
        };
        // `scAutoDemoGetPlayerDamage`.
        damage[player] = if player < 2 {
            rand(30) as u16
        } else {
            (rand(60) + 40) as u16
        };
    }
    Setup {
        gkind,
        fkinds,
        damage,
    }
}

/// `scAutoDemoGetPlayerStartPosition` for players 1 to 4 in turn: a random
/// point of the eight not yet taken. Returns each player's map object kind.
pub fn start_points(rand: &mut impl FnMut(i32) -> i32) -> [u16; 4] {
    let mut taken = [false; 8];
    core::array::from_fn(|player| {
        let random = rand(MAP_OBJ_KINDS.len() as i32 - player as i32);
        let mut select = MAP_OBJ_KINDS[0];
        let mut j = 0;
        for (i, &kind) in MAP_OBJ_KINDS.iter().enumerate() {
            select = kind;
            if !taken[i] {
                if random == j {
                    taken[i] = true;
                    break;
                }
                j += 1;
            }
        }
        select
    })
}

/// `scAutoDemoMakeFocusInterface`'s GObj and the names' `SObj`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoDemo {
    /// `sSCAutoDemoFunc`: the next step.
    step: usize,
    /// `sSCAutoDemoFocusChangeWait`.
    wait: u16,
    /// Players 1 and 2's names shown (`sSCAutoDemoFighterNameGObj`).
    pub names: [bool; 2],
    /// `gIFCommonPlayerInterface.is_magnify_display`.
    pub magnify_display: bool,
}

impl AutoDemo {
    /// `scAutoDemoMakeFocusInterface`, whose first update runs at once:
    /// the fade starts with the scene.
    pub fn new(world: &mut dyn World) -> (AutoDemo, Option<Scene>) {
        let mut d = AutoDemo {
            step: 0,
            wait: 0,
            names: [false; 2],
            magnify_display: true,
        };
        let load = d.update_focus(world);
        (d, load)
    }

    /// `scAutoDemoFuncRun`: `tapped` is A, B or START on any controller.
    pub fn tick(&mut self, tapped: bool, world: &mut dyn World) -> Option<Scene> {
        let exit = tapped.then_some(Scene::Title);
        let load = self.update_focus(world);
        exit.or(load)
    }

    /// `scAutoDemoUpdateFocus`.
    fn update_focus(&mut self, world: &mut dyn World) -> Option<Scene> {
        let mut load = None;
        if let Some(&(_, _, Some(player))) = FOCUS.get(self.step) {
            if world.is_dead(player) {
                self.wait = 0;
            }
        }
        while self.wait == 0 {
            // `scAutoDemoChangeFocus`.
            let Some(&(wait, change, _)) = FOCUS.get(self.step) else {
                break;
            };
            self.wait = wait;
            if let Some(c) = change {
                if let Some(scene) = self.change(c, world) {
                    load = Some(scene);
                }
            }
            self.step += 1;
        }
        self.wait = self.wait.saturating_sub(1);
        load
    }

    fn change(&mut self, c: Change, world: &mut dyn World) -> Option<Scene> {
        match c {
            Change::Fade => world.fade(),
            Change::FocusPlayer1 => {
                if world.is_dead(0) {
                    self.wait = 0;
                } else {
                    for p in 1..4 {
                        world.set_level(p, LEVEL_UNFOCUSED);
                    }
                    self.zoom(0, world);
                    world.set_detail_high(0, true);
                    self.names[0] = true;
                    self.magnify_display = false;
                }
            }
            Change::FocusPlayer2 => {
                self.names[0] = false;
                world.set_detail_high(0, false);
                if world.is_dead(1) {
                    self.wait = 0;
                } else {
                    world.set_level(1, LEVEL);
                    for p in [0, 2, 3] {
                        world.set_level(p, LEVEL_UNFOCUSED);
                    }
                    self.zoom(1, world);
                    world.set_detail_high(1, true);
                    self.names[1] = true;
                }
            }
            Change::ResetFocus => {
                world.reset_camera();
                for p in 0..4 {
                    world.set_level(p, LEVEL);
                }
                world.set_detail_high(1, false);
                self.names[1] = false;
            }
            Change::MagnifyOn => self.magnify_display = true,
            // `scAutoDemoExit` (US).
            Change::Exit => return Some(Scene::Startup),
        }
        None
    }

    /// `scAutoDemoSetCameraPlayerZoom`: a random eye angle on each axis,
    /// x first.
    fn zoom(&self, player: usize, world: &mut dyn World) {
        let x = ZOOM_EYE_X[world.rand(ZOOM_EYE_X.len() as i32) as usize];
        let y = ZOOM_EYE_Y[world.rand(ZOOM_EYE_Y.len() as i32) as usize];
        world.zoom(player, (x.to_radians(), y.to_radians()));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn candidate_kinds_hold_every_fighter_init_fields() {
        let backup = crate::backup::Backup::default();
        for seed in 0..64i32 {
            let mut demo = crate::menu::title::DemoData::default();
            let mut n = seed;
            let mut rand = |range: i32| {
                n = n.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (n >> 8).rem_euclid(range.max(1))
            };
            demo.set_demo_fighter_kinds(&backup, &mut rand);
            let candidates: alloc::vec::Vec<_> = super::candidate_kinds(&demo, &backup).collect();
            let gkind = super::next_gkind(&demo);
            let setup = super::init(&mut demo, &backup, &mut rand);
            assert_eq!(setup.gkind, gkind);
            for k in setup.fkinds {
                assert!(candidates.contains(&k), "{k:?} not in {candidates:?}");
            }
        }
    }

    use super::*;

    #[derive(Default)]
    struct Log {
        dead: [bool; 4],
        levels: [u8; 4],
        zoom: Option<usize>,
        fades: u32,
        high: [bool; 4],
    }

    impl World for Log {
        fn is_dead(&self, player: usize) -> bool {
            self.dead[player]
        }
        fn set_level(&mut self, player: usize, level: u8) {
            self.levels[player] = level;
        }
        fn zoom(&mut self, player: usize, _eye: (f32, f32)) {
            self.zoom = Some(player);
        }
        fn reset_camera(&mut self) {
            self.zoom = None;
        }
        fn set_detail_high(&mut self, player: usize, high: bool) {
            self.high[player] = high;
        }
        fn fade(&mut self) {
            self.fades += 1;
        }
        fn rand(&mut self, _range: i32) -> i32 {
            0
        }
    }

    #[test]
    fn the_focus_script_runs_its_course() {
        let mut w = Log::default();
        let (mut d, load) = AutoDemo::new(&mut w);
        assert_eq!((load, w.fades), (None, 1));
        let mut frame = 0;
        let exit = loop {
            frame += 1;
            if let Some(scene) = d.tick(false, &mut w) {
                break scene;
            }
            match frame {
                339 => assert_eq!(w.zoom, None),
                340 => {
                    assert_eq!(w.zoom, Some(0));
                    assert_eq!(w.levels, [0, 1, 1, 1]);
                    assert!(d.names[0] && !d.magnify_display);
                }
                680 => {
                    assert_eq!(w.zoom, Some(1));
                    assert_eq!(w.levels, [1, 9, 1, 1]);
                    assert_eq!(d.names, [false, true]);
                }
                1020 => {
                    assert_eq!(w.zoom, None);
                    assert_eq!(w.levels, [9; 4]);
                    assert!(!d.magnify_display);
                }
                1419 => assert!(!d.magnify_display),
                1420 => assert!(d.magnify_display),
                _ => {}
            }
        };
        assert_eq!((exit, frame), (Scene::Startup, 1480));
    }

    #[test]
    fn a_dead_focus_moves_on_at_once() {
        let mut w = Log::default();
        let (mut d, _) = AutoDemo::new(&mut w);
        for _ in 0..340 {
            d.tick(false, &mut w);
        }
        assert_eq!(w.zoom, Some(0));
        w.dead[0] = true;
        d.tick(false, &mut w);
        assert_eq!(w.zoom, Some(1));
    }

    #[test]
    fn start_points_take_eight_distinct_points() {
        let mut picks = [3, 3, 0, 4].into_iter();
        let points = start_points(&mut |_| picks.next().unwrap());
        assert_eq!(points, [0x1B, 0x1C, 0x18, 0x1F]);
    }

    #[test]
    fn init_walks_the_ground_order() {
        let mut demo = DemoData {
            demo_fkind: [FighterKind::Fox, FighterKind::Ness],
            demo_gkind_order: 7,
            ..DemoData::default()
        };
        let backup = Backup::default();
        let s = init(&mut demo, &backup, &mut |_| 0);
        assert_eq!(s.gkind, gkind::HYRULE);
        assert_eq!(demo.demo_gkind_order, 0);
        // Mario and Donkey Kong: the first starters Fox and Ness leave.
        assert_eq!(
            s.fkinds,
            [
                FighterKind::Fox,
                FighterKind::Ness,
                FighterKind::Mario,
                FighterKind::Donkey
            ]
        );
        assert_eq!(s.damage, [0, 0, 40, 40]);
    }
}
