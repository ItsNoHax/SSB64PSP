//! Replays of two N64 audio captures (mupen64plus + rsp-hle, exact AI
//! buffers, US ROM) whose output the port matches sample for sample: the
//! test renders the same audio-frame sequence with the same BGM and FGM
//! calls and compares an FNV-1a hash of the PCM with the capture's.
//!
//! - `b_menu`: the opening skipped with Start, the title's Start sound
//!   (`nSYAudioFGMTitlePressStart`), the Mode Select music, three cursor
//!   moves (`nSYAudioFGMMenuScroll2`) and A on VS Mode
//!   (`nSYAudioFGMMenuSelect`): the whole 15.57 s clip.
//! - `c_attract` (first 50.77 s): the Opening music and the 25 sounds and
//!   voices of the opening's fight scenes up to the fourth Boss slam,
//!   including two sounds in one frame, a sound played with a balance
//!   (`func_80026A10`) and voice steals. The window ends before the first
//!   full-scale sample where the capture's saturation order differs.
//!
//! The captures' frame lengths follow the N64's 552/368 rhythm; the play
//! frames were found by searching for the first differing sample.

extern crate std;

use super::testdata;
use super::{AudioSystem, FRAME_SAMPLES_MAX, FRAME_SAMPLES_MIN};

/// One call made before rendering frame `at`.
#[derive(Clone, Copy)]
enum Call {
    Bgm(u32),
    StopBgm,
    Fgm(u16),
    FgmBalance(u16, u8),
}

struct Capture {
    /// Audio frames rendered.
    frames: u32,
    /// Whether frame `f` is 368 samples long (otherwise 552).
    short: fn(u32) -> bool,
    calls: &'static [(u32, Call)],
    /// FNV-1a over the capture's interleaved samples.
    hash: u64,
}

/// The sequence player's tick phase at the capture's first sample, as
/// 184-sample sub-frames rendered beforehand (the N64 makes the player at
/// boot, before the capture starts).
const PREROLL: usize = 11;

fn every_tenth_from_13(f: u32) -> bool {
    f >= 13 && (f - 13).is_multiple_of(10)
}

const B_MENU: Capture = Capture {
    frames: 934,
    short: every_tenth_from_13,
    calls: &[
        (116, Call::Bgm(33)),
        (271, Call::StopBgm),
        (429, Call::Fgm(157)),
        (433, Call::Bgm(44)),
        (557, Call::Fgm(164)),
        (608, Call::Fgm(164)),
        (659, Call::Fgm(164)),
        (710, Call::Fgm(158)),
    ],
    hash: 0x09ba_456e_ea71_8acd,
};

const C_ATTRACT: Capture = Capture {
    frames: 3045,
    // One 11-frame gap between two short frames.
    short: |f| {
        (f <= 1723 && every_tenth_from_13(f)) || (f >= 1734 && (f - 1734).is_multiple_of(10))
    },
    calls: &[
        (116, Call::Bgm(33)),
        (1648, Call::Fgm(43)),
        (1661, Call::Fgm(43)),
        (1687, Call::Fgm(42)),
        (1687, Call::Fgm(429)),
        (1755, Call::Fgm(175)),
        (1765, Call::Fgm(175)),
        (1857, Call::Fgm(401)),
        (1917, Call::Fgm(13)),
        (1937, Call::Fgm(248)),
        (2007, Call::Fgm(130)),
        (2024, Call::Fgm(115)),
        (2027, Call::Fgm(13)),
        (2042, Call::Fgm(587)),
        (2097, Call::Fgm(93)),
        (2107, Call::Fgm(195)),
        (2212, Call::Fgm(185)),
        (2412, Call::Fgm(103)),
        (2557, Call::Fgm(1)),
        (2998, Call::Fgm(127)),
        (3011, Call::Fgm(115)),
        (3016, Call::FgmBalance(31, 77)),
        (3016, Call::Fgm(175)),
        (3033, Call::Fgm(175)),
    ],
    hash: 0x98c7_e4a3_bb54_743b,
};

/// Renders `c` and returns the FNV-1a hash of its samples, or `None`
/// without a ROM.
fn render(c: &Capture) -> Option<u64> {
    let mut sys = AudioSystem::new(testdata::section()?).expect("audio system");
    let mut buf = [0i16; FRAME_SAMPLES_MAX * 2];
    for _ in 0..PREROLL {
        sys.render_frame(&mut buf, 184);
    }
    let mut hash = 0xCBF2_9CE4_8422_2325u64;
    for f in 0..c.frames {
        for &(at, call) in c.calls {
            if at == f {
                match call {
                    Call::Bgm(id) => {
                        sys.play_bgm(0, id);
                    }
                    Call::StopBgm => sys.stop_bgm_all(),
                    Call::Fgm(id) => {
                        assert!(sys.play_fgm(id).is_some(), "fgm {id} at {f}");
                    }
                    Call::FgmBalance(id, bal) => {
                        assert!(sys.play_fgm_balance(id, bal).is_some(), "fgm {id} at {f}");
                    }
                }
            }
        }
        let n = if (c.short)(f) {
            FRAME_SAMPLES_MIN
        } else {
            FRAME_SAMPLES_MAX
        };
        sys.render_frame(&mut buf, n);
        for s in &buf[..n * 2] {
            hash = (hash ^ (*s as u16 as u64)).wrapping_mul(0x0100_0000_01B3);
        }
    }
    Some(hash)
}

#[test]
fn b_menu_matches_the_n64_capture() {
    if let Some(h) = render(&B_MENU) {
        assert_eq!(h, B_MENU.hash);
    }
}

#[test]
fn c_attract_opening_fights_match_the_n64_capture() {
    if let Some(h) = render(&C_ATTRACT) {
        assert_eq!(h, C_ATTRACT.hash);
    }
}
