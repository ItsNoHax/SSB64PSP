//! `mv/mvopening/mvopeningportraits.c`: the eight portraits in two sets
//! of four rows, each row uncovered by a black cover sliding out of the way
//! and covered again by it sliding back.

use alloc::vec::Vec;

use super::movie::{link, CameraKind, Fill, Head, ObjectKind, World, FULL};
use super::{Exit, Kind};
use crate::menu::Piece;

/// `llMVOpeningPortraitsSet1FileID` and `...Set2FileID`.
pub const FILE_SET1: u32 = 0x35;
pub const FILE_SET2: u32 = 0x36;
/// Each set's four rows, top to bottom (Samus, Mario, Fox, Pikachu; Link,
/// Kirby, Donkey Kong, Yoshi), and set 1's cover.
pub const ROWS: [u32; 4] = [0x9960, 0x13310, 0x1CCC0, 0x26670];
pub const COVER: u32 = 0x2B2D0;

/// The cover's display (`mvOpeningPortraitsCoverProcDisplay`), drawn by
/// the host: [`Portraits::cover_fills`], then [`Portraits::cover_sprite`].
pub const HOST_COVER: u16 = 1;

/// The cover's slide per tic and its rest beyond either edge.
const SLIDE: f32 = 93.0;
const EDGE: f32 = 656.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Portraits {
    tics: i32,
    /// `sMVOpeningPortraitsRow`.
    row: i32,
    cover_x: f32,
    cover_y: f32,
    pub world: World,
    portraits: u16,
    cover: u16,
    /// The cover display's rectangles this frame.
    fills: Vec<Fill>,
}

fn set(file: u32) -> Vec<Piece> {
    ROWS.iter()
        .enumerate()
        .map(|(i, &at)| {
            let mut p = Piece::at(file, at, 10.0, 10.0 + 55.0 * i as f32);
            p.transparent = false;
            p
        })
        .collect()
}

impl Portraits {
    /// `mvOpeningPortraitsFuncStart`.
    pub fn new() -> Portraits {
        let mut world = World::new();
        world.camera(
            100,
            0,
            FULL,
            CameraKind::Default {
                fill: Some([0, 0, 0, 0xFF]),
                zbuffer: true,
            },
        );
        world.camera(80, link(27), FULL, CameraKind::Sprite);
        world.camera(60, link(28), FULL, CameraKind::Sprite);
        let portraits = world.object(27, Head::Zero, ObjectKind::Sprites(set(FILE_SET1)));
        let cover = world.object(28, Head::Zero, ObjectKind::Host(HOST_COVER));
        let mut p = Portraits {
            tics: 0,
            row: 0,
            cover_x: EDGE,
            cover_y: 10.0,
            world,
            portraits,
            cover,
            fills: Vec::new(),
        };
        p.update_cover_display();
        p
    }

    /// One frame: `mvOpeningPortraitsFuncRun`, then the cover's process.
    pub fn tick(&mut self, tapped: bool) -> Option<Exit> {
        let mut exit = None;
        self.tics += 1;
        if self.tics >= 10 {
            if tapped {
                exit = Some(Exit::Title);
            }
            if self.tics == 75 {
                self.world.eject(self.portraits);
                self.portraits =
                    self.world
                        .object(27, Head::Zero, ObjectKind::Sprites(set(FILE_SET2)));
            }
            if self.tics == 150 {
                exit = Some(Exit::Next(Kind::Mario));
            }
        }
        self.update_cover();
        self.update_cover_display();
        exit
    }

    /// `mvOpeningPortraitsCoverProcUpdate`.
    fn update_cover(&mut self) {
        let t = self.tics;
        if t == 75 {
            self.cover_x = -EDGE;
        }
        if t < 75 {
            if self.cover_x < EDGE {
                self.cover_x = (self.cover_x + SLIDE).min(EDGE);
            }
        } else if self.cover_x > -EDGE {
            self.cover_x = (self.cover_x - SLIDE).max(-EDGE);
        }
        let (x, y, row) = match t {
            15 => (-EDGE, 10.0, 0),
            45 => (-EDGE, 65.0, 1),
            30 => (-EDGE, 120.0, 2),
            60 => (-EDGE, 175.0, 3),
            105 => (EDGE, 10.0, 0),
            135 => (EDGE, 65.0, 1),
            90 => (EDGE, 120.0, 2),
            120 => (EDGE, 175.0, 3),
            _ => return,
        };
        self.cover_x = x;
        self.cover_y = y;
        self.row = row;
    }

    /// `mvOpeningPortraitsCoverProcDisplay`: black over the other three
    /// rows and over the row's part the cover sprite leaves bare, then the
    /// cover sprite in black.
    fn update_cover_display(&mut self) {
        let row = self.row;
        let mut fills = Vec::new();
        let black = [0, 0, 0, 0xFF];
        for r in 0..4 {
            if r != row {
                let uly = 10.0 + 55.0 * r as f32;
                fills.push(Fill {
                    rect: [10.0, uly, 310.0, uly + 55.0],
                    rgba: black,
                });
            }
        }
        // `mvOpeningPortraitsBlockPartialRow`, with the cover's `s32` x.
        let pos_x = self.cover_x as i32;
        let uly = (10 + row * 55) as f32;
        let lry = (65 + row * 55) as f32;
        if pos_x > 0 {
            fills.push(Fill {
                rect: [0.0, uly, pos_x as f32, lry],
                rgba: black,
            });
        }
        if pos_x + 656 < 0 {
            fills.push(Fill {
                rect: [0.0, uly, 320.0, lry],
                rgba: black,
            });
        }
        if pos_x + 656 < 320 {
            fills.push(Fill {
                rect: [(pos_x + 656) as f32, uly, 320.0, lry],
                rgba: black,
            });
        }
        self.fills = fills;
    }

    /// The cover display's black rectangles, drawn before its sprite.
    pub fn cover_fills(&self) -> &[Fill] {
        &self.fills
    }

    /// The cover sprite (`llMVOpeningPortraitsSet1CoverSprite`), black, at
    /// the cover's place: drawn after the fills by the host.
    pub fn cover_sprite(&self) -> Piece {
        Piece::clear(FILE_SET1, COVER, self.cover_x, self.cover_y).prim([0, 0, 0])
    }

    /// The cover's display object.
    pub fn cover_id(&self) -> u16 {
        self.cover
    }
}

impl Default for Portraits {
    fn default() -> Self {
        Portraits::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn rows_uncover_in_the_source_order_and_the_scene_ends_at_150() {
        let mut p = Portraits::new();
        let mut rows = vec![];
        let mut exit = None;
        for _ in 0..150 {
            exit = p.tick(false);
            if p.tics % 15 == 0 && p.tics < 150 {
                rows.push(p.row);
            }
        }
        assert_eq!(rows, [0, 2, 1, 3, 3, 2, 0, 3, 1]);
        assert_eq!(exit, Some(Exit::Next(Kind::Mario)));
    }

    #[test]
    fn a_tap_before_tic_10_does_nothing() {
        let mut p = Portraits::new();
        for _ in 0..9 {
            assert_eq!(p.tick(true), None);
        }
        assert_eq!(p.tick(true), Some(Exit::Title));
    }
}
