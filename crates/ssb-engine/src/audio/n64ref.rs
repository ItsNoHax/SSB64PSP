//! Replays of two N64 audio captures made with the microcode run by a
//! low-level RSP (LLE) whose output the port matches sample for sample: the
//! test renders the same audio-frame sequence with the same BGM and FGM
//! calls and compares an FNV-1a hash of the PCM with the capture's.
//!
//! Captures (RE-479): US ROM (MD5 `f7c52568a31aadf26e14dc2b6416b2ed`), RMG
//! v0.9.0 flatpak (`com.github.Rosalie241.RMG`) core with
//! `mupen64plus-rsp-parallel` (LLE) and ParaLLEl-RDP, exact AI buffers from
//! `tools/n64-headless/n64_audio_dump.so`, stereo forced after the first
//! frame as `record_audio_ref.py --force-stereo` does. A second LLE core,
//! `mupen64plus-rsp-cxd4`, gives the same samples on both clips (with its
//! own frame timing). HLE (`mupen64plus-rsp-hle`) rounds the Q15 products
//! differently and is no reference.
//!
//! - `b_menu`: Start in the opening at advance 200 (stops its music),
//!   Start on the title at 355 (`nSYAudioFGMTitlePressStart`, then the
//!   Mode Select music), D-pad down at 480 and 530 and up at 580
//!   (`nSYAudioFGMMenuScroll2`), A on VS Mode at 630
//!   (`nSYAudioFGMMenuSelect`), B at 725 (no sound): the whole 14.29 s
//!   clip of 860 advances.
//! - `c_attract` (no input, first 51.94 s): the Opening music and the 23
//!   sounds and voices of the opening's fight scenes, including two pairs
//!   of sounds in one frame, a sound played with a balance
//!   (`func_80026A10`), voice steals and 190 full-scale samples. The window
//!   ends at the next sound.
//!
//! The play frames, the order of two calls in one frame and the preroll
//! were found by searching for the first differing sample: no frame ±1, no
//! swapped pair and no other preroll in 0..64 matches.

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
    /// The 368-sample frames: every tenth frame from `first` through
    /// `last` of each run (the others are 552 samples long).
    short_runs: &'static [(u32, u32)],
    calls: &'static [(u32, Call)],
    /// FNV-1a over the capture's interleaved samples.
    hash: u64,
}

/// The sequence player's tick phase at the capture's first sample, as
/// 184-sample sub-frames rendered beforehand (the N64 makes the player at
/// boot, before the capture starts).
const PREROLL: usize = 52;

fn is_short(runs: &[(u32, u32)], f: u32) -> bool {
    runs.iter()
        .any(|&(first, last)| (first..=last).contains(&f) && (f - first).is_multiple_of(10))
}

const B_MENU: Capture = Capture {
    frames: 857,
    short_runs: &[(12, 12), (23, 43), (48, 48), (61, 311), (322, 852)],
    calls: &[
        (147, Call::Bgm(33)),
        (216, Call::StopBgm),
        (356, Call::Fgm(157)),
        (363, Call::Bgm(44)),
        (481, Call::Fgm(164)),
        (531, Call::Fgm(164)),
        (581, Call::Fgm(164)),
        (631, Call::Fgm(158)),
    ],
    hash: 0xd5e2_f323_8581_c901,
};

const C_ATTRACT: Capture = Capture {
    frames: 3115,
    short_runs: &[
        (12, 12),
        (23, 43),
        (48, 48),
        (61, 1641),
        (1652, 1652),
        (1661, 1741),
        (1752, 1752),
        (1761, 1821),
        (1832, 1842),
        (1851, 1851),
        (1862, 1892),
        (1901, 1921),
        (1932, 3112),
    ],
    calls: &[
        (150, Call::Bgm(33)),
        (1684, Call::Fgm(43)),
        (1697, Call::Fgm(43)),
        (1723, Call::Fgm(429)),
        (1723, Call::Fgm(42)),
        (1789, Call::Fgm(175)),
        (1799, Call::Fgm(175)),
        (1894, Call::Fgm(401)),
        (1954, Call::Fgm(13)),
        (1974, Call::Fgm(248)),
        (2046, Call::Fgm(130)),
        (2063, Call::Fgm(115)),
        (2066, Call::Fgm(13)),
        (2081, Call::Fgm(587)),
        (2147, Call::Fgm(93)),
        (2157, Call::Fgm(195)),
        (2260, Call::Fgm(185)),
        (2482, Call::Fgm(103)),
        (2627, Call::Fgm(1)),
        (3057, Call::Fgm(127)),
        (3070, Call::Fgm(115)),
        (3075, Call::Fgm(175)),
        (3075, Call::FgmBalance(31, 77)),
        (3092, Call::Fgm(175)),
    ],
    hash: 0xf16c_3088_ce8a_83eb,
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
        let n = if is_short(c.short_runs, f) {
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn b_menu_matches_the_n64_capture() {
    if let Some(h) = render(&B_MENU) {
        assert_eq!(h, B_MENU.hash);
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn c_attract_opening_fights_match_the_n64_capture() {
    if let Some(h) = render(&C_ATTRACT) {
        assert_eq!(h, C_ATTRACT.hash);
    }
}
