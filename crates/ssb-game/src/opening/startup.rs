//! `mn/mncommon/mnstartup.c`: the N64 logo.
//!
//! The logo rises from y 220 to 65 over 16 tics, rests 24, then a black
//! fade of 10 tics covers it and 13 tics later the opening's room loads.
//! A, B or START after the first 8 tics loads the title instead.

use super::movie::{CameraKind, ObjectKind, World, FULL};
use super::{Exit, Kind};
use crate::menu::Piece;
use crate::spgame::congra::Fade;

/// `llN64LogoFileID` and `llN64LogoSprite`.
pub const FILE: u32 = 0xC2;
pub const LOGO: u32 = 0x73C0;

/// `mnStartupLogoThreadUpdate`'s phases: the rise, the rest, the fade's
/// wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Thread {
    /// Tics into the rise (`i`, 0 to 16).
    Rise(i32),
    Rest(i32),
    FadeWait(i32),
    Done,
}

/// `lbFadeMakeActor`'s fade: its clock and whether it fades from black
/// (`color.a == 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fader {
    fade: Fade,
    from_black: bool,
    eject: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Startup {
    /// `sMNStartupSkipAllowWait`.
    skip_wait: i32,
    /// `sMNStartupIsProceedOpening`.
    proceed: bool,
    thread: Thread,
    logo_y: f32,
    fades: [Option<Fader>; 2],
    pub world: World,
    logo: u16,
    fade_cameras: [u16; 2],
}

impl Startup {
    /// `mnStartupFuncStart`.
    pub fn new() -> Startup {
        let mut world = World::new();
        world.camera(
            100,
            0,
            [0.0, 0.0, 320.0, 240.0],
            CameraKind::Default {
                fill: Some([0, 0, 0, 0xFF]),
                zbuffer: false,
            },
        );
        world.camera(80, super::movie::link(0), FULL, CameraKind::Sprite);
        let mut piece = Piece::at(FILE, LOGO, 96.0, 220.0);
        piece.transparent = false;
        let logo = world.object(
            0,
            super::movie::Head::Zero,
            ObjectKind::Sprites(alloc::vec![piece]),
        );
        let fade = |world: &mut World| {
            world.camera(
                10,
                0,
                FULL,
                CameraKind::Fade {
                    color: [0, 0, 0],
                    alpha: 0xFF,
                },
            )
        };
        let fade_cameras = [fade(&mut world), 0];
        Startup {
            skip_wait: 8,
            proceed: false,
            thread: Thread::Rise(0),
            logo_y: 220.0,
            // `lbFadeMakeActor(..., 10, &{0, 0, 0, 0}, 16, TRUE, NULL)`.
            fades: [
                Some(Fader {
                    fade: Fade::new(16),
                    from_black: true,
                    eject: true,
                }),
                None,
            ],
            world,
            logo,
            fade_cameras,
        }
    }

    /// One frame: `mnStartupActorFuncRun`, then the logo's thread and the
    /// fades' processes. Returns the scene to load.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        if self.skip_wait != 0 {
            self.skip_wait -= 1;
        }
        if self.skip_wait == 0 && tapped {
            exit = Some(Exit::Title);
        } else if self.proceed {
            exit = Some(Exit::Next(Kind::Room));
        }
        self.run_thread();
        for i in 0..2 {
            let Some(f) = self.fades[i].as_mut() else {
                continue;
            };
            let ended = f.fade.update();
            let a = f.fade.alpha();
            let alpha = if f.from_black { 0xFF - a } else { a };
            let eject = ended && f.eject;
            if let Some(cam) = self.world.camera_mut(self.fade_cameras[i]) {
                cam.kind = CameraKind::Fade {
                    color: [0, 0, 0],
                    alpha,
                };
            }
            if eject {
                self.fades[i] = None;
                self.world.eject(self.fade_cameras[i]);
            }
        }
        if let Some(s) = self.world.sprites_mut(self.logo) {
            s[0].y = self.logo_y;
        }
        exit
    }

    /// `mnStartupLogoThreadUpdate` up to its next sleep.
    fn run_thread(&mut self) {
        self.thread = match self.thread {
            Thread::Rise(i) if i < 16 => {
                let step = (16 - i) as f32;
                self.logo_y = 65.0 - ((-(38.75 / 64.0) * step) * step);
                Thread::Rise(i + 1)
            }
            Thread::Rise(_) => {
                self.logo_y = 65.0;
                Thread::Rest(1)
            }
            Thread::Rest(i) if i < 24 => Thread::Rest(i + 1),
            Thread::Rest(_) => {
                // `lbFadeMakeActor(..., 10, &{0, 0, 0, 0xFF}, 10, FALSE, NULL)`:
                // its process (priority 0) runs this tic after the thread's.
                self.fades[1] = Some(Fader {
                    fade: Fade::new(10),
                    from_black: false,
                    eject: false,
                });
                self.fade_cameras[1] = self.world.camera(
                    10,
                    0,
                    FULL,
                    CameraKind::Fade {
                        color: [0, 0, 0],
                        alpha: 0,
                    },
                );
                Thread::FadeWait(1)
            }
            Thread::FadeWait(i) if i < 13 => Thread::FadeWait(i + 1),
            Thread::FadeWait(_) => {
                self.proceed = true;
                Thread::Done
            }
            Thread::Done => Thread::Done,
        };
    }

    /// The logo's top edge.
    pub fn logo_y(&self) -> f32 {
        self.logo_y
    }
}

impl Default for Startup {
    fn default() -> Self {
        Startup::new()
    }
}
