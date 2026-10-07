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

/// RE-474: the setters RE-473 left untraced, against N64 Training traces
/// of a data-only warp copy (Kirby against each CPU fighter for the copies).
#[test]
fn re474_setters_play_their_first_frame_and_last_the_n64s_frames() {
    type Setter = fn(&mut Fighter);
    fn copy(f: &mut Fighter, kind: FighterKind) {
        f.kirby.copy_id = kind;
        assert!(crate::kirby_copy::set_special_n(f));
    }
    // (fighter, on the ground, setter, N64 frames in the status)
    let cases: [(FighterKind, bool, Setter, u32); 13] = [
        (FighterKind::Donkey, true, status::set_donkey_special_hi, 99),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Mario),
            45,
        ),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Luigi),
            45,
        ),
        (FighterKind::Kirby, true, |f| copy(f, FighterKind::Fox), 54),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Samus),
            15,
        ),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Donkey),
            7,
        ),
        (FighterKind::Kirby, true, |f| copy(f, FighterKind::Link), 45),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Captain),
            89,
        ),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Pikachu),
            63,
        ),
        (
            FighterKind::Kirby,
            true,
            |f| copy(f, FighterKind::Purin),
            54,
        ),
        (FighterKind::Kirby, true, |f| copy(f, FighterKind::Ness), 71),
        (FighterKind::Link, true, crate::link::set_attack100_start, 7),
        (
            FighterKind::Captain,
            true,
            crate::captain::set_attack100_start,
            5,
        ),
    ];
    for (kind, ground, setter, n64) in cases {
        let mut f = fighter(kind, ground);
        setter(&mut f);
        let status: AnyStatus = f.status.status;
        assert_eq!(f.status.anim_frame, 1.0, "{status:?} first frame");
        assert_eq!(frames_in(&mut f), n64, "{status:?}");
    }
    // Captain Falcon's, Pikachu's and Ness's specials (RE-474's traces
    // without a hit; the N64's punch and kick add their hitlag).
    let specials: [(FighterKind, bool, Setter, u32); 8] = [
        (
            FighterKind::Captain,
            true,
            crate::captain::set_special_n,
            89,
        ),
        (
            FighterKind::Captain,
            true,
            crate::captain::set_special_lw,
            84,
        ),
        (
            FighterKind::Pikachu,
            true,
            crate::pikachu::set_special_n,
            63,
        ),
        (
            FighterKind::Pikachu,
            true,
            crate::pikachu::set_special_lw,
            23,
        ),
        (FighterKind::Ness, true, crate::ness::set_special_n, 71),
        (FighterKind::Ness, true, crate::ness::set_special_lw, 14),
        (FighterKind::Ness, true, crate::ness::set_special_hi, 23),
        (
            FighterKind::Kirby,
            true,
            crate::kirby::set_attack100_start,
            7,
        ),
    ];
    for (kind, ground, setter, n64) in specials {
        let mut f = fighter(kind, ground);
        setter(&mut f);
        let status: AnyStatus = f.status.status;
        assert_eq!(f.status.anim_frame, 1.0, "{status:?} first frame");
        assert_eq!(frames_in(&mut f), n64, "{status:?}");
    }
    // The aerial copies and Kirby's own rapid jab start on frame 1 too.
    let first: [(FighterKind, bool, Setter); 6] = [
        (FighterKind::Kirby, false, |f| copy(f, FighterKind::Fox)),
        (FighterKind::Kirby, false, |f| copy(f, FighterKind::Luigi)),
        (FighterKind::Kirby, false, |f| copy(f, FighterKind::Pikachu)),
        (FighterKind::Kirby, false, |f| copy(f, FighterKind::Purin)),
        (FighterKind::Kirby, true, |f| copy(f, FighterKind::Yoshi)),
        (FighterKind::Kirby, true, crate::kirby::set_attack100_start),
    ];
    for (kind, ground, setter) in first {
        let mut f = fighter(kind, ground);
        setter(&mut f);
        let status: AnyStatus = f.status.status;
        assert_eq!(f.status.anim_frame, 1.0, "{status:?} first frame");
    }
}

/// RE-474: `gobj->anim_frame` follows the figatree. A `Loop` sets it back
/// to 0 and plays on; an `End` sets it to 0 and it stands there, through
/// any status without a figatree of its own (How to Play's N64 trace:
/// Mario's `Wait` wraps at 50, `DamageFall` at 29, `DamageFlyN` ends at 29,
/// `CatchWait` keeps the catch's ended clock).
#[test]
fn the_figatree_clock_loops_and_ends_as_the_n64s() {
    let mut f = fighter(FighterKind::Mario, true);
    assert_eq!(f.status.status, AnyStatus::Common(status::Status::Wait));
    let mut seen = Vec::new();
    for _ in 0..52 {
        f.status.status = AnyStatus::Common(status::Status::Wait);
        status::play_anim(&mut f);
        seen.push(f.status.anim_frame);
    }
    assert_eq!(&seen[47..], &[48.0, 49.0, 0.0, 1.0, 2.0]);
    assert_eq!(f.status.clock, 52.0, "the status clock counts on");

    let mut f = fighter(FighterKind::Mario, false);
    status::set_status(
        &mut f,
        status::Status::DamageFlyN,
        0.0,
        status::StatusTiming::unknown(),
    );
    for _ in 0..28 {
        status::play_anim(&mut f);
    }
    assert_eq!(f.status.anim_frame, 28.0);
    status::play_anim(&mut f);
    assert_eq!(f.status.anim_frame, 0.0, "the End sets the overshoot");
    for _ in 0..10 {
        status::play_anim(&mut f);
    }
    assert_eq!(f.status.anim_frame, 0.0, "and stands there");
    assert_eq!(f.status.clock, 39.0);
    // `CatchWait` has motion -2: the stopped clock carries over.
    status::set_status(
        &mut f,
        status::Status::CatchWait,
        0.0,
        status::StatusTiming::unknown(),
    );
    status::play_anim(&mut f);
    assert_eq!(f.status.anim_frame, 0.0);
}
