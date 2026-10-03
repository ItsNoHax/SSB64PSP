//! `sc1PTrainingModeMake*`: stat sprites on display link 23 while closed,
//! the open menu's panel/underline on link 22 and its sprites on link 23.
//! Positions belonging to ROM tables are supplied by the asset adapter.

use crate::training::{MainOption, TrainingMenu};

/// Pointer-slot identities in `llSC1PTrainingModeFileID` (254).
pub const DISPLAY_LABEL: u32 = 0;
pub const DISPLAY_OPTION: u32 = 0x20;
pub const MENU_LABEL: u32 = 0xBC;
pub const MENU_OPTION: u32 = 0x13C;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteInfo {
    pub size: [u16; 2],
    pub color: [u8; 4],
    pub pos: [i16; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    Fill {
        rect: [f32; 4],
        color: [u8; 4],
    },
    Sprite {
        slot: u32,
        pos: [f32; 2],
        prim: [u8; 4],
        env: [u8; 3],
    },
}

/// `Damage/ComboDisplayProcDisplay` holds the last sprites for 90 process
/// ticks after the source stat clears. A new hit replaces them immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counter {
    last: u32,
    pub shown: u32,
    pub reset_wait: u8,
}

impl Counter {
    pub fn tick(&mut self) {
        self.reset_wait = self.reset_wait.saturating_sub(1);
    }

    pub fn observe(&mut self, value: u32, max: u32) {
        let value = value.min(max);
        if value == 0 {
            if self.last != 0 {
                self.reset_wait = 90;
                self.last = 0;
            }
            if self.reset_wait == 0 {
                self.last = 1;
            }
        }
        if value != self.last {
            self.shown = value;
            self.last = value;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    pub damage: Counter,
    pub combo: Counter,
}

impl Stats {
    pub fn tick(&mut self) {
        self.damage.tick();
        self.combo.tick();
    }

    pub fn observe(&mut self, damage: u32, combo: u32) {
        self.damage.observe(damage, 999);
        self.combo.observe(combo, 99);
    }
}

const GREEN: [u8; 3] = [0x6C, 0xFF, 0x6C];
const LABEL: [u8; 3] = [0xAF, 0xAE, 0xDD];
const ORANGE: [u8; 3] = [0xF3, 0xA7, 0x6A];
const RED: [u8; 3] = [0xF3, 0x10, 0x0E];

fn piece(slot: u32, pos: [f32; 2], info: SpriteInfo, rgb: Option<[u8; 3]>, env: [u8; 3]) -> Draw {
    let [r, g, b] = rgb.unwrap_or([info.color[0], info.color[1], info.color[2]]);
    Draw::Sprite {
        slot,
        pos,
        prim: [r, g, b, info.color[3]],
        env,
    }
}

fn row(option: MainOption) -> usize {
    match option {
        MainOption::Cp => 0,
        MainOption::Item => 1,
        MainOption::Speed => 2,
        MainOption::View => 3,
        MainOption::Reset => 4,
        MainOption::Exit => 5,
    }
}

/// Visit the US Training interface in camera/display order. `sprite` reads
/// sizes, colours and signed label positions from the original ROM tables.
pub fn visit(
    menu: &TrainingMenu,
    stats: &Stats,
    open: bool,
    held_item: u8,
    sprite: impl Fn(u32) -> Option<SpriteInfo>,
    mut draw: impl FnMut(Draw),
) {
    // Link 22 is behind link 23, including the always-visible stat sprites.
    let selected = row(menu.main_option);
    if open {
        // 1-cycle gDPFillRectangle excludes the lower/right endpoints.
        draw(Draw::Fill {
            rect: [68.0, 47.0, 253.0, 198.0],
            color: [0, 0x64, 0xFF, 0x64],
        });
        if let Some(label) = sprite(MENU_LABEL + selected as u32 * 8 + 4) {
            let right = if selected < 4 {
                sprite(MENU_OPTION + 29 * 4).map(|s| 237.0 + f32::from(s.size[0]) - 2.0)
            } else {
                Some(f32::from(label.pos[0]) + f32::from(label.size[0]) + 2.0)
            };
            if let Some(right) = right {
                let y = f32::from(label.pos[1]) + f32::from(label.size[1]) - 1.0;
                // Fill cycle includes both lower/right endpoints.
                draw(Draw::Fill {
                    rect: [f32::from(label.pos[0]) - 13.0, y, right + 1.0, y + 2.0],
                    color: [255, 0, 0, 255],
                });
            }
        }
    }
    if !open {
        for i in 0..4 {
            let slot = DISPLAY_LABEL + i * 8 + 4;
            if let Some(s) = sprite(slot) {
                draw(piece(slot, s.pos.map(f32::from), s, Some(LABEL), [0; 3]));
            }
        }
        for (value, centers, y) in [
            (stats.damage.shown, &[75, 85, 95][..], 20.0),
            (stats.combo.shown, &[69, 79][..], 36.0),
        ] {
            let mut unit = if centers.len() == 3 { 100 } else { 10 };
            for &center in centers {
                let slot = DISPLAY_OPTION + (value / unit % 10) * 4;
                if let Some(s) = sprite(slot) {
                    let x = (center as f32 - f32::from(s.size[0]) * 0.5) as i32 as f32;
                    draw(piece(slot, [x, y], s, Some(GREEN), [0; 3]));
                }
                unit /= 10;
            }
        }
        for (index, pos) in [
            (38, [100.0, 20.0]),
            (27 + u32::from(menu.speed_option), [276.0, 20.0]),
            (31 + u32::from(menu.cp_option), [191.0, 20.0]),
            (37, [292.0, 36.0]),
        ] {
            let slot = DISPLAY_OPTION + index * 4;
            if let Some(s) = sprite(slot) {
                draw(piece(slot, pos, s, Some(GREEN), [0; 3]));
            }
        }
        let slot = DISPLAY_OPTION + (10 + u32::from(held_item)) * 4;
        if let Some(s) = sprite(slot) {
            let x = 292.0 - f32::from(s.size[0]);
            draw(piece(slot, [x, 36.0], s, Some(GREEN), [0; 3]));
            let slot = DISPLAY_OPTION + 36 * 4;
            if let Some(s) = sprite(slot) {
                draw(piece(
                    slot,
                    [x - f32::from(s.size[0]), 36.0],
                    s,
                    Some(GREEN),
                    [0; 3],
                ));
            }
        }
        return;
    }
    for i in 0..10 {
        let slot = MENU_LABEL + i * 8 + 4;
        if let Some(s) = sprite(slot) {
            draw(piece(
                slot,
                s.pos.map(f32::from),
                s,
                (i < 6).then_some(ORANGE),
                [0; 3],
            ));
        }
    }
    for (index, y, tint) in [
        (21 + u32::from(menu.cp_option), 65.0, Some([255; 3])),
        (
            u32::from(menu.item_option),
            if menu.item_option == 11 { 83.0 } else { 85.0 },
            Some([255; 3]),
        ),
        (17 + u32::from(menu.speed_option), 105.0, Some([255; 3])),
        // Preserve `view_menu_option + ViewStart` as the callback does.
        (26 + u32::from(menu.view_option), 125.0, None),
    ] {
        let slot = MENU_OPTION + index * 4;
        if let Some(s) = sprite(slot) {
            let x = 191.0 - f32::from(s.size[0] / 2);
            draw(piece(slot, [x, y], s, tint, [0x4A, 0x2E, 0x60]));
        }
    }
    if selected < 4 {
        // Motion-Sensor Bomb's option is two pixels higher but the arrows
        // compensate by five rather than three: all rows land at y+3.
        let y = 68.0 + selected as f32 * 20.0;
        for (index, x) in [(28, 137.0), (29, 237.0)] {
            let slot = MENU_OPTION + index * 4;
            if let Some(s) = sprite(slot) {
                draw(piece(slot, [x, y], s, Some(RED), [0; 3]));
            }
        }
    }
    let slot = MENU_OPTION + 30 * 4;
    if let (Some(s), Some(label)) = (sprite(slot), sprite(MENU_LABEL + selected as u32 * 8 + 4)) {
        draw(piece(
            slot,
            [71.0, f32::from(label.pos[1]) - 1.0],
            s,
            None,
            [0; 3],
        ));
    }
}

#[cfg(test)]
#[path = "training_layer_tests.rs"]
mod tests;
