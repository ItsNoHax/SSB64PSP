//! `mv/mvopening/mvopeningnewcomers.c`: the four hidden fighters sliding
//! in on white, as silhouettes until the save has unlocked them, then a
//! fade to black and the title.

use alloc::vec;
use alloc::vec::Vec;

use super::movie::{link, CameraKind, Fill, Head, Joints, Model, ObjectKind, Persp, World, FULL};
use super::Exit;
use crate::fighter::FighterKind;
use ssb_engine::math::Vec3;

/// `llMVOpeningNewcomers1FileID` and `...2FileID`.
pub const FILE1: u32 = 0x3D;
pub const FILE2: u32 = 0x3E;

/// One newcomer: its file, shown and hidden display lists and `AnimJoint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Newcomer {
    pub kind: FighterKind,
    pub file: u32,
    pub show: u32,
    pub hidden: u32,
    pub anim: u32,
}

/// `mvOpeningNewcomersMakeAll`'s order: Jigglypuff, Captain Falcon,
/// Luigi, Ness.
pub const NEWCOMERS: [Newcomer; 4] = [
    Newcomer {
        kind: FighterKind::Purin,
        file: FILE1,
        show: 0x5C28,
        hidden: 0x203A8,
        anim: 0x5E44,
    },
    Newcomer {
        kind: FighterKind::Captain,
        file: FILE2,
        show: 0x1C238,
        hidden: 0x355C0,
        anim: 0x1C9D4,
    },
    Newcomer {
        kind: FighterKind::Luigi,
        file: FILE1,
        show: 0x1C838,
        hidden: 0x28C28,
        anim: 0x1CE94,
    },
    Newcomer {
        kind: FighterKind::Ness,
        file: FILE2,
        show: 0x2A448,
        hidden: 0x3BAF8,
        anim: 0x2A864,
    },
];

/// `mvOpeningNewcomersCheckLocked`: the newcomer's bit is clear in the
/// backup's fighter mask (`LBBACKUP_MASK_FIGHTER`).
pub fn locked(fighter_mask: u16, kind: FighterKind) -> bool {
    match kind {
        FighterKind::Captain | FighterKind::Ness | FighterKind::Purin | FighterKind::Luigi => {
            fighter_mask & (1 << kind as u16) == 0
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Newcomers {
    tics: i32,
    pub world: World,
    models: [u16; 4],
    /// `mvOpeningNewcomersMakeHide`'s display and
    /// `sMVOpeningNewcomersOverlayAlpha`.
    hide: Option<(u16, i32)>,
}

impl Newcomers {
    /// `mvOpeningNewcomersFuncStart`, with `gSCManagerBackupData.fighter_mask`.
    pub fn new(fighter_mask: u16) -> Newcomers {
        let mut world = World::new();
        world.camera(
            100,
            0,
            FULL,
            CameraKind::Default {
                fill: Some([0xFF, 0xFF, 0xFF, 0xFF]),
                zbuffer: true,
            },
        );
        // `mvOpeningNewcomersMakeNewcomersCamera`: a fixed, narrow view.
        world.camera(
            40,
            link(27),
            FULL,
            CameraKind::Persp {
                persp: Persp {
                    eye: Vec3::new(45.36104, 19.91594, 15494.226),
                    at: Vec3::new(-109.73612, 257.7266, -14.981689),
                    up_x: 0.0,
                    fovy: 2.864789,
                    near: 128.0,
                    far: 16384.0,
                    ..Persp::DEFAULT
                },
                zbuffer: false,
            },
        );
        world.camera(20, link(26), FULL, CameraKind::Sprite);
        // Each newcomer one `DObj` on its shown or hidden list
        // (`gcDrawDObjDLHead1`), its joint script on the root.
        let mut models = [0; 4];
        for (i, n) in NEWCOMERS.iter().enumerate() {
            let dl = if locked(fighter_mask, n.kind) {
                n.hidden
            } else {
                n.show
            };
            models[i] = world.object(
                27,
                Head::One,
                ObjectKind::Model(
                    Model::new(n.file, dl, Head::One).joints(Joints::root(n.file, n.anim)),
                ),
            );
        }
        world.light = [0.0, 0.0];
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMOpeningNewcomersClash);
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceAnnounceTitleWait);
        Newcomers {
            tics: 0,
            world,
            models,
            hide: None,
        }
    }

    /// One frame: `mvOpeningNewcomersFuncRun`, then the newcomers'
    /// processes; then the hide display's fade.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 30 {
                // `mvOpeningNewcomersMakeHide`: link 26, head 1.
                let id = self
                    .world
                    .object(26, Head::One, ObjectKind::Fills(Vec::new()));
                self.hide = Some((id, 0));
            }
            if self.tics == 40 {
                exit = Some(Exit::Title);
            }
        }
        for id in self.models {
            if let Some(m) = self.world.model_mut(id) {
                m.play();
            }
        }
        if let Some((id, alpha)) = self.hide.as_mut() {
            // `mvOpeningNewcomersHideProcDisplay` raises the alpha once a
            // drawn frame, then fills the viewport black.
            if *alpha < 0xFF {
                *alpha += 0x28;
                if *alpha > 0xFF {
                    *alpha = 0xFF;
                }
            }
            let a = *alpha as u8;
            let id = *id;
            if let Some(f) = self.world.fills_mut(id) {
                *f = vec![Fill {
                    rect: FULL,
                    rgba: [0, 0, 0, a],
                }];
            }
        }
        exit
    }

    /// The black overlay's alpha this frame (0 before it is made).
    pub fn hide_alpha(&self) -> u8 {
        self.hide.map_or(0, |(_, a)| a as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graphs(n: &mut Newcomers) -> Vec<u32> {
        let ids = n.models;
        ids.iter()
            .map(|&id| n.world.model_mut(id).unwrap().graph)
            .collect()
    }

    #[test]
    fn a_new_save_shows_silhouettes_and_unlocked_fighters_show_themselves() {
        // `LBBACKUP_MASK_FIGHTER` of the eight starters.
        let starters = 0x036F;
        let mut n = Newcomers::new(starters);
        assert_eq!(graphs(&mut n), [0x203A8, 0x355C0, 0x28C28, 0x3BAF8]);
        let mut n = Newcomers::new(starters | (1 << 4) | (1 << 11));
        assert_eq!(graphs(&mut n), [0x203A8, 0x355C0, 0x1C838, 0x2A448]);
    }

    #[test]
    fn the_fade_starts_at_30_and_the_title_loads_at_40() {
        let mut n = Newcomers::new(0);
        for _ in 0..29 {
            assert_eq!(n.tick(false), None);
        }
        assert_eq!(n.hide_alpha(), 0);
        n.tick(false);
        assert_eq!(n.hide_alpha(), 0x28);
        for _ in 30..39 {
            assert_eq!(n.tick(false), None);
        }
        assert_eq!(n.tick(false), Some(Exit::Title));
        assert_eq!(n.hide_alpha(), 0xFF);
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut n = Newcomers::new(0);
        for _ in 0..9 {
            assert_eq!(n.tick(true), None);
        }
        assert_eq!(n.tick(true), Some(Exit::Title));
    }
}
