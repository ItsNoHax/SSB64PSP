//! The setters that play their first frame (`ftMainPlayAnimEventsAll`),
//! checked against N64 Training traces of the unmodified US ROM's code
//! (RE-473): each status shows `anim_frame` 1 on the frame it starts and
//! lasts the frames the N64 holds it before the status its end sets.

use crate::fighter::{Fighter, FighterKind, Situation};
use crate::status::{self, AnyStatus};

fn fighter(kind: FighterKind, ground: bool) -> Fighter {
    let mut f = Fighter::new(kind, 0, 3);
    if ground {
        status::set_wait(&mut f);
    } else {
        f.physics.jumps_used = 1;
        status::set_fall(&mut f);
    }
    f
}

/// Frames in the setter's status, counting the frame it starts on, until
/// its end sets another.
fn frames_in(f: &mut Fighter) -> u32 {
    let start = f.status.status;
    let mut frames = 1;
    loop {
        status::update(f);
        if f.status.status != start {
            return frames;
        }
        frames += 1;
        assert!(frames < 400, "{start:?} never ends");
    }
}

#[test]
fn traced_setters_play_their_first_frame_and_last_the_n64s_frames() {
    type Setter = fn(&mut Fighter);
    // (fighter, on the ground, setter, N64 frames in the status)
    let cases: [(FighterKind, bool, Setter, u32); 17] = [
        (FighterKind::Link, true, crate::link::set_special_n, 45),
        (FighterKind::Link, true, crate::link::set_special_hi, 59),
        (FighterKind::Link, true, crate::link::set_special_lw, 44),
        (FighterKind::Fox, true, status::set_fox_special_hi_start, 7),
        (FighterKind::Fox, false, status::set_fox_special_hi_start, 7),
        (FighterKind::Fox, true, status::set_fox_special_lw_start, 3),
        (FighterKind::Fox, false, status::set_fox_special_lw_start, 3),
        (FighterKind::Samus, true, crate::samus::set_special_hi, 49),
        (
            FighterKind::Samus,
            false,
            crate::samus::set_special_air_hi,
            47,
        ),
        (FighterKind::Yoshi, true, crate::yoshi::set_special_hi, 71),
        (FighterKind::Purin, true, crate::purin::set_special_n, 54),
        (FighterKind::Purin, true, crate::purin::set_special_hi, 179),
        (FighterKind::Purin, true, crate::purin::set_special_lw, 249),
        (FighterKind::Kirby, true, crate::kirby::set_special_n, 19),
        (FighterKind::Kirby, true, crate::kirby::set_special_hi, 59),
        (FighterKind::Kirby, true, crate::kirby::set_special_lw, 5),
        (FighterKind::Kirby, false, crate::kirby::set_special_lw, 22),
    ];
    for (kind, ground, setter, n64) in cases {
        let mut f = fighter(kind, ground);
        assert_eq!(f.situation == Situation::Ground, ground);
        setter(&mut f);
        let status: AnyStatus = f.status.status;
        assert_eq!(f.status.anim_frame, 1.0, "{status:?} first frame");
        assert_eq!(frames_in(&mut f), n64, "{status:?}");
    }
}
