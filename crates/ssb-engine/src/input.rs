//! Controller abstraction and the N64 → PSP button mapping.
//!
//! The game reasons in N64 buttons. The backend reports PSP buttons. This
//! module owns the translation, so remapping later means editing one table.

/// N64 controller buttons, as bit flags matching `include/PR/controller.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct N64Buttons(pub u16);

impl N64Buttons {
    pub const A: u16 = 0x8000;
    pub const B: u16 = 0x4000;
    pub const Z: u16 = 0x2000;
    pub const START: u16 = 0x1000;
    pub const D_UP: u16 = 0x0800;
    pub const D_DOWN: u16 = 0x0400;
    pub const D_LEFT: u16 = 0x0200;
    pub const D_RIGHT: u16 = 0x0100;
    pub const L: u16 = 0x0020;
    pub const R: u16 = 0x0010;
    pub const C_UP: u16 = 0x0008;
    pub const C_DOWN: u16 = 0x0004;
    pub const C_LEFT: u16 = 0x0002;
    pub const C_RIGHT: u16 = 0x0001;

    pub fn contains(self, mask: u16) -> bool {
        self.0 & mask != 0
    }

    pub fn set(&mut self, mask: u16, on: bool) {
        if on {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }
}

/// One frame of controller state, in N64 terms.
///
/// Stick values keep the N64's `-80..=80` effective range rather than being
/// normalized, because the game's thresholds (smash-turn detection, tilt vs.
/// smash attacks) are written against those raw magnitudes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControllerState {
    pub buttons: N64Buttons,
    pub stick_x: i8,
    pub stick_y: i8,
    pub connected: bool,
}

/// Buttons that changed from held to pressed this frame.
pub fn newly_pressed(prev: N64Buttons, curr: N64Buttons) -> N64Buttons {
    N64Buttons(curr.0 & !prev.0)
}

/// Buttons released this frame.
pub fn newly_released(prev: N64Buttons, curr: N64Buttons) -> N64Buttons {
    N64Buttons(prev.0 & !curr.0)
}

/// PSP buttons, as `sceCtrl` reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PspButtons(pub u32);

impl PspButtons {
    pub const SELECT: u32 = 0x0000_0001;
    pub const START: u32 = 0x0000_0008;
    pub const UP: u32 = 0x0000_0010;
    pub const RIGHT: u32 = 0x0000_0020;
    pub const DOWN: u32 = 0x0000_0040;
    pub const LEFT: u32 = 0x0000_0080;
    pub const LTRIGGER: u32 = 0x0000_0100;
    pub const RTRIGGER: u32 = 0x0000_0200;
    pub const TRIANGLE: u32 = 0x0001_0000;
    pub const CIRCLE: u32 = 0x0002_0000;
    pub const CROSS: u32 = 0x0004_0000;
    pub const SQUARE: u32 = 0x0008_0000;

    pub fn contains(self, mask: u32) -> bool {
        self.0 & mask != 0
    }
}

/// One entry of the button mapping table.
#[derive(Debug, Clone, Copy)]
pub struct ButtonMapping {
    pub psp: u32,
    pub n64: u16,
}

/// The default mapping.
///
/// Reasoning behind the non-obvious choices:
///
/// * **Z on L-trigger, R on R-trigger.** Z is shield/grab and by far the most
///   used shoulder input in Smash 64; L and R are the same shield function, so
///   losing one is harmless. The PSP has two shoulder buttons, and Z gets the
///   more comfortable one.
/// * **C-buttons on the face buttons.** In Smash 64 the C-buttons are *only*
///   used for taunt (C-Up in some contexts) and for camera in single-player
///   modes — they are not attack inputs the way they are in Melee. So they can
///   safely take the leftover face buttons.
/// * **D-pad stays D-pad.** It is used for menu navigation and for the debug
///   menu, and the PSP D-pad maps one-to-one.
///
/// This is a starting point, not a finding: which C-button functions matter is
/// flagged as an open question in `docs/reverse-engineering.md`.
pub const DEFAULT_MAPPING: &[ButtonMapping] = &[
    ButtonMapping {
        psp: PspButtons::CROSS,
        n64: N64Buttons::A,
    },
    ButtonMapping {
        psp: PspButtons::CIRCLE,
        n64: N64Buttons::B,
    },
    ButtonMapping {
        psp: PspButtons::LTRIGGER,
        n64: N64Buttons::Z,
    },
    ButtonMapping {
        psp: PspButtons::RTRIGGER,
        n64: N64Buttons::R,
    },
    ButtonMapping {
        psp: PspButtons::START,
        n64: N64Buttons::START,
    },
    ButtonMapping {
        psp: PspButtons::UP,
        n64: N64Buttons::D_UP,
    },
    ButtonMapping {
        psp: PspButtons::DOWN,
        n64: N64Buttons::D_DOWN,
    },
    ButtonMapping {
        psp: PspButtons::LEFT,
        n64: N64Buttons::D_LEFT,
    },
    ButtonMapping {
        psp: PspButtons::RIGHT,
        n64: N64Buttons::D_RIGHT,
    },
    ButtonMapping {
        psp: PspButtons::TRIANGLE,
        n64: N64Buttons::C_UP,
    },
    ButtonMapping {
        psp: PspButtons::SQUARE,
        n64: N64Buttons::C_DOWN,
    },
];

/// The PSP nub reports 0..=255 centred near 128; the N64 stick reports roughly
/// -80..=80. This is the outward scale factor.
pub const NUB_TO_N64_SCALE: f32 = 80.0 / 128.0;

/// Deadzone applied to the nub before scaling, in nub units. The PSP nub
/// drifts noticeably, and Smash treats any nonzero stick tilt as walk input.
pub const NUB_DEADZONE: i32 = 20;

/// Converts a raw PSP nub axis (0..=255) to the N64's signed range.
pub fn nub_axis_to_n64(raw: u8) -> i8 {
    let centered = raw as i32 - 128;
    if centered.abs() <= NUB_DEADZONE {
        return 0;
    }
    // Rescale so the axis still reaches full deflection after the deadzone is
    // subtracted, rather than capping short of it. Integer math throughout:
    // `f32::round` lives in `std`, and this crate builds `no_std` for the PSP.
    let sign = centered.signum();
    let magnitude = centered.abs() - NUB_DEADZONE;
    let range = 127 - NUB_DEADZONE;
    let scaled = (magnitude * 80 + range / 2) / range;
    (sign * scaled.min(80)) as i8
}

/// Translates a frame of PSP input into N64 controller state.
///
/// `nub_y` is inverted: `sceCtrl` reports Y increasing downward, the N64 stick
/// reports Y increasing upward.
pub fn map_psp_to_n64(
    buttons: PspButtons,
    nub_x: u8,
    nub_y: u8,
    mapping: &[ButtonMapping],
) -> ControllerState {
    let mut out = ControllerState {
        connected: true,
        stick_x: nub_axis_to_n64(nub_x),
        stick_y: -nub_axis_to_n64(nub_y),
        ..Default::default()
    };
    for m in mapping {
        if buttons.contains(m.psp) {
            out.buttons.set(m.n64, true);
        }
    }
    out
}

/// What the game needs from an input backend.
pub trait Input {
    /// Samples all ports. Called once per simulation tick.
    fn poll(&mut self);

    /// State for `port` (0..4) this tick.
    fn state(&self, port: usize) -> ControllerState;

    /// State for `port` on the previous tick, for edge detection.
    fn previous(&self, port: usize) -> ControllerState;

    /// Starts rumble on `port`. A no-op on backends without a motor.
    fn set_rumble(&mut self, port: usize, on: bool);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nub_center_is_dead() {
        assert_eq!(nub_axis_to_n64(128), 0);
        assert_eq!(nub_axis_to_n64(128 + NUB_DEADZONE as u8), 0);
    }

    #[test]
    fn nub_extremes_reach_full_deflection() {
        assert_eq!(nub_axis_to_n64(255), 80);
        assert_eq!(nub_axis_to_n64(0), -80);
    }

    #[test]
    fn nub_is_monotonic_across_its_range() {
        let mut prev = nub_axis_to_n64(0);
        for raw in 1..=255u8 {
            let v = nub_axis_to_n64(raw);
            assert!(v >= prev, "raw {raw}: {v} < {prev}");
            prev = v;
        }
    }

    #[test]
    fn maps_cross_to_a_and_circle_to_b() {
        let s = map_psp_to_n64(
            PspButtons(PspButtons::CROSS | PspButtons::CIRCLE),
            128,
            128,
            DEFAULT_MAPPING,
        );
        assert!(s.buttons.contains(N64Buttons::A));
        assert!(s.buttons.contains(N64Buttons::B));
        assert!(!s.buttons.contains(N64Buttons::Z));
    }

    #[test]
    fn stick_y_is_inverted_relative_to_the_nub() {
        // Nub pushed "down" (raw 255) should read as N64 stick down (negative).
        let s = map_psp_to_n64(PspButtons(0), 128, 255, DEFAULT_MAPPING);
        assert_eq!(s.stick_y, -80);
        let s = map_psp_to_n64(PspButtons(0), 128, 0, DEFAULT_MAPPING);
        assert_eq!(s.stick_y, 80);
    }

    #[test]
    fn edge_detection_reports_only_transitions() {
        let prev = N64Buttons(N64Buttons::A);
        let curr = N64Buttons(N64Buttons::A | N64Buttons::B);
        assert_eq!(newly_pressed(prev, curr).0, N64Buttons::B);
        assert_eq!(newly_released(prev, curr).0, 0);
        assert_eq!(newly_released(curr, prev).0, N64Buttons::B);
    }

    #[test]
    fn every_psp_button_maps_to_a_distinct_n64_button() {
        for (i, a) in DEFAULT_MAPPING.iter().enumerate() {
            for b in &DEFAULT_MAPPING[i + 1..] {
                assert_ne!(a.psp, b.psp, "psp button mapped twice");
                assert_ne!(a.n64, b.n64, "n64 button mapped twice");
            }
        }
    }
}
