//! Layer C: the PSP front-end/Training-Mode executable (F1).
//!
//! A second, independent PSP application alongside the existing debug asset
//! viewer in `psp/` -- its own crate, its own EBOOT, sharing only the
//! portable `crates/ssb-engine`/`ssb-rom`/`ssb-game` libraries (`AGENTS.md`'s
//! F1 carve-out, `plans/gameplay/F1.md`). `psp/`'s own build and EBOOT are
//! unmodified by this crate's existence.
//!
//! Current increment: intro screen -> main menu -> a Training-Mode
//! placeholder, all drawn as flat coloured rectangles (`gu.rs`). No fighter,
//! stage, hitbox, or damage code runs yet -- that is later F1 work, tracked
//! in `plans/gameplay/F1.md`'s remaining acceptance criteria and `TODO.md`.
//! What exists here proves criterion 1 (a real, independently booting
//! second EBOOT) and the intro/menu navigation shape of criteria 2-3.

#![no_std]
#![no_main]

mod gu;
mod input;

use ssb_engine::input::{newly_pressed, Input, N64Buttons};
use ssb_engine::renderer::Color;

use gu::Gpu;
use input::PspInput;

psp::module!("ssb64_psp_game", 1, 0);

fn psp_main() {
    psp::enable_home_button();
    unsafe { run() }
}

/// Which screen is active. Deliberately just these three: nothing here may
/// grow stocks, a match timer, CPU AI, or items without widening `AGENTS.md`'s
/// F1 carve-out first.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Intro,
    Menu,
    /// The Training-Mode placeholder. Not yet a real scene: no stage, no
    /// fighter, no combat -- see `plans/gameplay/F1.md` acceptance criteria
    /// 4-7 for what still has to land here.
    Training,
}

/// Main-menu entries. Only `Training` is selectable; the others are visible,
/// inert placeholders (`plans/gameplay/F1.md` allows this explicitly).
const MENU_ENTRIES: usize = 3;
const TRAINING_ENTRY: usize = 0;

const BG_INTRO: Color = Color::rgba(24, 32, 64, 255);
const BG_MENU: Color = Color::rgba(16, 16, 24, 255);
const BG_TRAINING: Color = Color::rgba(20, 48, 24, 255);
const ENTRY_SELECTED: Color = Color::rgba(255, 200, 40, 255);
const ENTRY_ENABLED: Color = Color::rgba(200, 200, 200, 255);
const ENTRY_DISABLED: Color = Color::rgba(70, 70, 70, 255);

unsafe fn run() -> ! {
    let mut gpu = Gpu::init();
    let mut pad = PspInput::init();

    let mut screen = Screen::Intro;
    let mut cursor: usize = 0;

    loop {
        pad.poll();
        let curr = pad.state(0).buttons;
        let prev = pad.previous(0).buttons;
        let pressed = newly_pressed(prev, curr);

        match screen {
            Screen::Intro => {
                if pressed.contains(N64Buttons::A) || pressed.contains(N64Buttons::START) {
                    screen = Screen::Menu;
                }
            }
            Screen::Menu => {
                if pressed.contains(N64Buttons::D_DOWN) {
                    cursor = (cursor + 1) % MENU_ENTRIES;
                } else if pressed.contains(N64Buttons::D_UP) {
                    cursor = (cursor + MENU_ENTRIES - 1) % MENU_ENTRIES;
                } else if pressed.contains(N64Buttons::A) && cursor == TRAINING_ENTRY {
                    screen = Screen::Training;
                }
            }
            Screen::Training => {
                // No combat yet -- B returns to the menu so the placeholder
                // is at least navigable end to end.
                if pressed.contains(N64Buttons::B) {
                    screen = Screen::Menu;
                }
            }
        }

        match screen {
            Screen::Intro => {
                gpu.begin_frame(BG_INTRO);
            }
            Screen::Menu => {
                gpu.begin_frame(BG_MENU);
                draw_menu(&mut gpu, cursor);
            }
            Screen::Training => {
                gpu.begin_frame(BG_TRAINING);
            }
        }
        gpu.end_frame();
    }
}

/// Draws the menu entries as a vertical stack of rectangles: one bar per
/// entry, the selected one in `ENTRY_SELECTED`, `Training` in
/// `ENTRY_ENABLED` when not selected, and the stubbed entries dimmed. No text
/// yet (`gu.rs`'s module doc explains why), so entries are distinguished by
/// screen position and enabled/disabled colour rather than a label.
fn draw_menu(gpu: &mut Gpu, cursor: usize) {
    const ENTRY_HEIGHT: i32 = 32;
    const ENTRY_GAP: i32 = 16;
    const ENTRY_WIDTH: i32 = 200;
    const LEFT: i32 = 40;
    const TOP: i32 = 60;

    for i in 0..MENU_ENTRIES {
        let y0 = TOP + i as i32 * (ENTRY_HEIGHT + ENTRY_GAP);
        let color = if i == cursor {
            ENTRY_SELECTED
        } else if i == TRAINING_ENTRY {
            ENTRY_ENABLED
        } else {
            ENTRY_DISABLED
        };
        gpu.draw_rect(LEFT, y0, LEFT + ENTRY_WIDTH, y0 + ENTRY_HEIGHT, color);
    }
}
