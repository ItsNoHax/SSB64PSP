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

#[cfg(feature = "headless_capture")]
use psp::sys;

use ssb_engine::input::{newly_pressed, Input, N64Buttons};
use ssb_engine::renderer::Color;

use gu::Gpu;
use input::PspInput;

/// Loop-iteration count since boot. `psp-game` has no fixed-timestep sim
/// accumulator yet (unlike `psp/`'s `Clock`/`FixedClock`), so this is simply
/// the draw-loop tick -- deterministic regardless of host wall-clock speed,
/// which is what `regression_capture`/`headless_capture` need. Only
/// consulted by `deterministic_capture_frozen`/`scripted_buttons`; harmless
/// to maintain unconditionally (`psp/main.rs`'s own `sim_frame_index`
/// comment).
const DETERMINISTIC_CAPTURE_TICKS: u64 = 16;

/// `true` once `regression_capture`'s scripted input has run past its fixed
/// script and reached its capture tick; always `false` otherwise, so callers
/// need one guard, not a cfg per call site (mirrors `psp/main.rs`'s function
/// of the same name).
#[inline]
fn deterministic_capture_frozen(sim_frame_index: u64) -> bool {
    sim_frame_index >= DETERMINISTIC_CAPTURE_TICKS && cfg!(feature = "regression_capture")
}

/// A fixed, tick-indexed button script standing in for real `sceCtrl` input
/// under `regression_capture`. `psp/` has no precedent for this (its own
/// deterministic-capture features only ever freeze *output* -- physics,
/// animation, camera -- never override input, because its viewer has no
/// input-driven state machine to script); `psp-game`'s Intro -> Menu ->
/// Training navigation does, so this is what actually lets
/// PPSSPPHeadless drive and pixel-confirm the Menu -> Training confirm
/// transition deterministically. That transition was RE-289's one open item:
/// synthetic X11 key injection into a windowed PPSSPP instance (tried with
/// both the project's Xlib fallback and `xdotool`) raced this desktop's
/// Wayland/XWayland compositor focus arbitration and could not reliably
/// deliver the key, independent of which injection tool sent it -- and it
/// also takes over real keyboard focus on the developer's desktop while it
/// runs. Headless capture has neither problem.
///
/// Tick 4 confirms past the Intro screen. Tick 8 confirms Training: the menu
/// cursor starts on `TRAINING_ENTRY` (`cursor: usize = 0` below), so no
/// D-pad navigation is needed first. Only consulted when
/// `deterministic_capture_frozen` reads `regression_capture` as enabled;
/// harmless to keep unconditionally.
fn scripted_buttons(tick: u64) -> N64Buttons {
    match tick {
        4 | 8 => N64Buttons(N64Buttons::A),
        _ => N64Buttons(0),
    }
}

/// Ask PPSSPPHeadless to save the current display framebuffer. Real PSPs do
/// not implement the emulator-only devctl, so the same build remains safe to
/// load on hardware (where the call simply returns an error). Verbatim copy
/// of `psp/main.rs`'s function of the same name/behaviour.
#[cfg(feature = "headless_capture")]
fn emit_headless_screenshot() {
    const EMULATOR_DEVCTL_EMIT_SCREENSHOT: u32 = 0x20;

    unsafe {
        sys::sceIoDevctl(
            b"emulator:\0".as_ptr(),
            EMULATOR_DEVCTL_EMIT_SCREENSHOT,
            core::ptr::null_mut(),
            0,
            core::ptr::null_mut(),
            0,
        );
    }
}

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
    let mut sim_frame_index: u64 = 0;
    #[cfg(feature = "headless_capture")]
    let mut headless_capture_sent = false;

    loop {
        sim_frame_index = sim_frame_index.saturating_add(1);
        pad.poll();
        let (prev, curr) = if cfg!(feature = "regression_capture") {
            (
                scripted_buttons(sim_frame_index.saturating_sub(1)),
                scripted_buttons(sim_frame_index),
            )
        } else {
            (pad.previous(0).buttons, pad.state(0).buttons)
        };
        let pressed = newly_pressed(prev, curr);

        if !deterministic_capture_frozen(sim_frame_index) {
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

        #[cfg(feature = "headless_capture")]
        if !headless_capture_sent && deterministic_capture_frozen(sim_frame_index) {
            emit_headless_screenshot();
            headless_capture_sent = true;
        }
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
