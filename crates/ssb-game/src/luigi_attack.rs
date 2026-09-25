//! Luigi motion attacks from `relocData/220_LuigiMainMotion.c` (US).
//! Windows are half open and follow the script's Wait/WaitAsync cursor.
//! Lengths are the figatree lengths from `ssb_rom::anim::EXPECTED_FRAMES`;
//! most of them are Mario's files, which `dFTLuigiMotionDescs` names.

use crate::attack::{ActiveHitbox, Hitbox, MoveData};
use ssb_engine::math::Vec3;

#[allow(clippy::too_many_arguments)]
const fn hit(
    damage: i32,
    size: f32,
    x: f32,
    y: f32,
    z: f32,
    angle: i32,
    growth: i32,
    weight: i32,
    base: i32,
) -> Hitbox {
    Hitbox {
        damage,
        radius: size / 2.0,
        offset: Vec3::new(x, y, z),
        angle,
        kb_scale: growth,
        kb_weight: weight,
        kb_base: base,
    }
}

macro_rules! h {
    ($d:expr,$sz:expr,$x:expr,$y:expr,$z:expr,$a:expr,$g:expr,$w:expr,$b:expr,$s:expr,$e:expr) => {
        ActiveHitbox::new(hit($d, $sz, $x, $y, $z, $a, $g, $w, $b), $s, $e)
    };
}
macro_rules! mv {
    ($name:ident,$len:expr,$lag:expr,[$($hb:expr),* $(,)?]) => {
        pub static $name: MoveData = MoveData { hitboxes: &[$($hb),*], length_frames: $len, landing_lag_percent: $lag };
    };
}

mv!(
    JAB1,
    18.0,
    None,
    [
        h!(2, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 2.0, 4.0),
        h!(2, 160.0, 0.0, 0.0, 0.0, 361, 50, 0, 8, 2.0, 4.0),
    ]
);
mv!(
    JAB2,
    20.0,
    None,
    [
        h!(2, 180.0, 16.0, 0.0, 0.0, 70, 50, 0, 8, 3.0, 6.0),
        h!(2, 180.0, 0.0, 0.0, 0.0, 70, 50, 0, 8, 3.0, 6.0),
    ]
);
// `SetAttackCollSize(0, 180)` at frame 5 grows the first box in place; the
// hit record is kept because nothing is cleared.
mv!(
    JAB3,
    26.0,
    None,
    [
        h!(4, 150.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 3.0, 5.0),
        h!(4, 280.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 3.0, 9.0),
        h!(4, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 10, 5.0, 9.0),
    ]
);

/// Six alternating two-damage boxes, each cleared before the next, so each
/// is its own hit record. The frame-44 finisher is created and cleared in
/// the same frame (its `WaitAsync(4)` has already passed) and never exists
/// during a collision check.
const fn dash_boxes() -> [ActiveHitbox; 6] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 6];
    let windows: [(f32, f32); 6] = [
        (2.0, 6.0),
        (6.0, 10.0),
        (10.0, 17.0),
        (17.0, 24.0),
        (24.0, 31.0),
        (31.0, 38.0),
    ];
    let mut i = 0;
    while i < 6 {
        out[i] = ActiveHitbox::new(
            hit(2, 180.0, 0.0, 0.0, 0.0, 30, 100, 40, 0),
            windows[i].0,
            windows[i].1,
        )
        .with_hit_generation(i as u8);
        i += 1;
    }
    out
}
const DASH_BOXES: [ActiveHitbox; 6] = dash_boxes();
pub static DASH: MoveData = MoveData {
    hitboxes: &DASH_BOXES,
    length_frames: 60.0,
    landing_lag_percent: None,
};

// `FTiltHigh`, `FTilt` and `FTiltLow` carry identical boxes.
macro_rules! ftilt {
    () => {
        MoveData {
            hitboxes: &[
                h!(10, 180.0, 20.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 13.0),
                h!(10, 220.0, 80.0, 0.0, 0.0, 361, 100, 0, 10, 8.0, 13.0),
            ],
            length_frames: 43.0,
            landing_lag_percent: None,
        }
    };
}
pub static FTILT_HI: MoveData = ftilt!();
pub static FTILT: MoveData = ftilt!();
pub static FTILT_LW: MoveData = ftilt!();
mv!(
    UTILT,
    42.0,
    None,
    [
        h!(10, 180.0, 0.0, 0.0, 0.0, 80, 150, 0, 0, 5.0, 17.0),
        h!(10, 290.0, 60.0, 0.0, 0.0, 80, 150, 0, 0, 5.0, 17.0),
    ]
);
mv!(
    DTILT,
    18.0,
    None,
    [
        h!(7, 180.0, 20.0, 0.0, 0.0, 361, 100, 0, 0, 3.0, 7.0),
        h!(7, 260.0, 140.0, 0.0, 0.0, 361, 100, 0, 0, 3.0, 7.0),
    ]
);

macro_rules! fsmash {
    ($damage:expr, $x:expr) => {
        MoveData {
            hitboxes: &[
                h!($damage, 180.0, 0.0, 0.0, 0.0, 361, 100, 0, 30, 16.0, 21.0),
                h!($damage, 240.0, $x, 0.0, 0.0, 361, 100, 0, 30, 16.0, 21.0),
            ],
            length_frames: 42.0,
            landing_lag_percent: None,
        }
    };
}
pub static FSMASH_HI: MoveData = fsmash!(16, 60.0);
pub static FSMASH_HI_S: MoveData = fsmash!(16, 50.0);
pub static FSMASH: MoveData = fsmash!(15, 50.0);
pub static FSMASH_LW_S: MoveData = fsmash!(14, 50.0);
pub static FSMASH_LW: MoveData = fsmash!(14, 50.0);
// The head's `SetHitStatusPartID(12, 3)` intangibility waits on the
// hit-status system.
mv!(
    USMASH,
    60.0,
    None,
    [h!(19, 400.0, 0.0, 100.0, 0.0, 85, 120, 0, 26, 7.0, 16.0)]
);
mv!(
    DSMASH,
    45.0,
    None,
    [
        h!(17, 170.0, 0.0, 0.0, 20.0, 361, 100, 0, 20, 8.0, 30.0),
        h!(17, 210.0, 120.0, 0.0, 50.0, 361, 100, 0, 20, 8.0, 30.0),
        h!(17, 170.0, 0.0, 0.0, 20.0, 361, 100, 0, 20, 8.0, 30.0),
        h!(17, 210.0, 120.0, 0.0, 50.0, 361, 100, 0, 20, 8.0, 30.0),
    ]
);

// Every aerial replaces its strong boxes with weak ones without a clear,
// so one hit record covers both phases.
mv!(
    AIR_N,
    50.0,
    Some(50),
    [
        h!(14, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 3.0, 11.0),
        h!(14, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 15, 3.0, 11.0),
        h!(14, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 15, 3.0, 11.0),
        h!(11, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 11.0, 37.0),
        h!(11, 240.0, 10.0, 0.0, 0.0, 361, 100, 0, 0, 11.0, 37.0),
        h!(11, 260.0, 0.0, 0.0, 0.0, 361, 100, 0, 0, 11.0, 37.0),
    ]
);
mv!(
    AIR_F,
    40.0,
    None,
    [
        h!(16, 220.0, -30.0, 45.0, 0.0, 361, 100, 0, 10, 11.0, 15.0),
        h!(16, 270.0, 80.0, 30.0, 0.0, 361, 100, 0, 10, 11.0, 15.0),
        h!(10, 220.0, -30.0, 45.0, 0.0, 361, 100, 0, 0, 15.0, 27.0),
        h!(10, 270.0, 80.0, 30.0, 0.0, 361, 100, 0, 0, 15.0, 27.0),
    ]
);
mv!(
    AIR_B,
    40.0,
    None,
    [
        h!(16, 240.0, -30.0, 45.0, 0.0, 361, 100, 0, 10, 10.0, 14.0),
        h!(16, 290.0, 80.0, 30.0, 0.0, 361, 100, 0, 10, 10.0, 14.0),
        h!(10, 220.0, -30.0, 45.0, 0.0, 361, 100, 0, 0, 14.0, 20.0),
        h!(10, 270.0, 80.0, 30.0, 0.0, 361, 100, 0, 0, 14.0, 20.0),
    ]
);
mv!(
    AIR_HI,
    40.0,
    None,
    [
        h!(12, 220.0, 0.0, 0.0, 0.0, 80, 120, 0, 0, 2.0, 5.0),
        h!(12, 250.0, 0.0, 0.0, 0.0, 80, 120, 0, 0, 2.0, 5.0),
        h!(9, 220.0, 0.0, 0.0, 0.0, 70, 120, 0, 0, 5.0, 12.0),
        h!(9, 250.0, 0.0, 0.0, 0.0, 70, 120, 0, 0, 5.0, 12.0),
    ]
);

/// Eight two-frame drill pulses, three frames apart. `RefreshAttackCollID`
/// after each clear starts a new hit record. Luigi has no `LandingAirLw`
/// motion, so the script's `SetFlag1(20)` selects `LandingAirNull`.
const fn air_lw_boxes() -> [ActiveHitbox; 16] {
    let empty = ActiveHitbox::new(hit(0, 0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0), 0.0, 0.0);
    let mut out = [empty; 16];
    let mut pulse = 0;
    while pulse < 8 {
        let start = 10.0 + pulse as f32 * 3.0;
        out[pulse * 2] = ActiveHitbox::new(
            hit(3, 350.0, -30.0, 45.0, 0.0, -70, 100, 30, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        out[pulse * 2 + 1] = ActiveHitbox::new(
            hit(3, 350.0, 50.0, 30.0, 0.0, -70, 100, 30, 0),
            start,
            start + 2.0,
        )
        .with_hit_generation(pulse as u8);
        pulse += 1;
    }
    out
}
const AIR_LW_BOXES: [ActiveHitbox; 16] = air_lw_boxes();
pub static AIR_LW: MoveData = MoveData {
    hitboxes: &AIR_LW_BOXES,
    length_frames: 39.0,
    landing_lag_percent: Some(20),
};

/// `SuperJumpPunchGround` falls through into `0x17FC` without an `End`.
/// The frame-2 sweet spot is replaced at frame 3 without a clear, so a
/// target it hit is not hit again by the one-damage boxes. The
/// `SetHitStatusAll(3)` intangible frame waits on the hit-status system.
macro_rules! superjump {
    ($angle:expr, $base:expr) => {
        MoveData {
            hitboxes: &[
                h!(25, 280.0, 0.0, 0.0, 0.0, $angle, 80, 0, $base, 2.0, 3.0),
                h!(25, 120.0, 160.0, 0.0, 0.0, $angle, 80, 0, $base, 2.0, 3.0),
                h!(1, 310.0, 0.0, 0.0, 60.0, 361, 100, 10, 0, 3.0, 25.0),
                h!(1, 260.0, 150.0, 0.0, 60.0, 361, 100, 10, 0, 3.0, 25.0),
            ],
            length_frames: crate::status::MARIO_SUPERJUMP_LENGTH_FRAMES,
            landing_lag_percent: None,
        }
    };
}
pub static SUPERJUMP_GROUND: MoveData = superjump!(90, 90);
pub static SUPERJUMP_AIR: MoveData = superjump!(80, 80);

// Luigi Cyclone: the opening boxes stay live, never cleared, through the
// thirteen three-frame loop; the frame-43 finishers replace them. One hit
// record covers the whole move, unlike Mario's pulsed Tornado. The grounded
// script replaces only slots 0 and 1, so its centre box stays until the
// clear at frame 45.
mv!(
    CYCLONE_GROUND,
    crate::status::MARIO_TORNADO_GROUND_LENGTH_FRAMES,
    None,
    [
        h!(16, 140.0, 0.0, 300.0, 150.0, 65, 90, 0, 65, 0.0, 43.0),
        h!(16, 140.0, 0.0, 300.0, -150.0, 65, 90, 0, 65, 0.0, 43.0),
        h!(16, 180.0, 0.0, 0.0, 0.0, 90, 90, 0, 65, 0.0, 45.0),
        h!(18, 200.0, 0.0, 300.0, 150.0, 70, 100, 0, 80, 43.0, 45.0),
        h!(18, 200.0, 0.0, 300.0, -150.0, 70, 100, 0, 80, 43.0, 45.0),
    ]
);
mv!(
    CYCLONE_AIR,
    crate::status::MARIO_TORNADO_AIR_LENGTH_FRAMES,
    None,
    [
        h!(15, 140.0, 0.0, 300.0, 150.0, 65, 90, 0, 65, 0.0, 43.0),
        h!(15, 140.0, 0.0, 300.0, -150.0, 65, 90, 0, 65, 0.0, 43.0),
        h!(15, 180.0, 0.0, 120.0, 0.0, 90, 90, 0, 65, 0.0, 43.0),
        h!(18, 200.0, 0.0, 300.0, 150.0, 70, 100, 0, 80, 43.0, 47.0),
        h!(18, 200.0, 0.0, 300.0, -150.0, 70, 100, 0, 80, 43.0, 47.0),
        h!(18, 220.0, 0.0, 120.0, 0.0, 70, 100, 0, 80, 43.0, 47.0),
        h!(18, 90.0, 0.0, 420.0, 0.0, 70, 100, 0, 80, 43.0, 47.0),
    ]
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::{Facing, Fighter, FighterKind, Situation};
    use crate::status::{self, AnyStatus, MarioStatus, Status};
    use ssb_engine::input::N64Buttons;

    fn luigi(situation: Situation) -> Fighter {
        let mut f = Fighter::new(FighterKind::Luigi, 0, 3);
        f.situation = situation;
        f.status.status = if situation == Situation::Ground {
            Status::Wait.into()
        } else {
            Status::Fall.into()
        };
        f
    }

    fn press_b(f: &mut Fighter, x: i8, y: i8) {
        f.input.stick_x = x;
        f.input.stick_y = y;
        f.stick.step(x, y, false, false);
        f.prev_input.buttons = N64Buttons::default();
        f.input.buttons = N64Buttons(N64Buttons::B);
    }

    #[test]
    fn dash_attack_finisher_never_exists_during_a_check() {
        assert!(DASH.hitboxes.iter().all(|h| !h.is_active(44.0)));
        let generations: [u8; 6] = core::array::from_fn(|i| DASH.hitboxes[i].hit_generation);
        assert_eq!(generations, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn cyclone_is_one_hit_record_with_a_lingering_ground_centre() {
        assert!(CYCLONE_GROUND
            .hitboxes
            .iter()
            .all(|h| h.hit_generation == 0));
        assert!(CYCLONE_AIR.hitboxes.iter().all(|h| h.hit_generation == 0));
        let active =
            |m: &MoveData, frame: f32| m.hitboxes.iter().filter(|h| h.is_active(frame)).count();
        assert_eq!(active(&CYCLONE_GROUND, 42.0), 3);
        assert_eq!(active(&CYCLONE_GROUND, 44.0), 3);
        assert_eq!(active(&CYCLONE_GROUND, 45.0), 0);
        assert_eq!(active(&CYCLONE_AIR, 46.0), 4);
        assert_eq!(active(&CYCLONE_AIR, 47.0), 0);
    }

    #[test]
    fn super_jump_sweet_spot_is_one_frame_then_one_damage() {
        for m in [&SUPERJUMP_GROUND, &SUPERJUMP_AIR] {
            let at = |frame: f32| m.hitboxes.iter().filter(move |h| h.is_active(frame));
            assert!(at(2.0).all(|h| h.hitbox.damage == 25));
            assert!(at(3.0).all(|h| h.hitbox.damage == 1));
            assert!(at(24.0).count() == 2 && at(25.0).count() == 0);
        }
        assert_eq!(SUPERJUMP_GROUND.hitboxes[0].hitbox.angle, 90);
        assert_eq!(SUPERJUMP_AIR.hitboxes[0].hitbox.angle, 80);
    }

    #[test]
    fn super_jump_launches_on_frame_seven() {
        let mut f = luigi(Situation::Ground);
        press_b(&mut f, 0, 80);
        assert!(status::check_special_hi(&mut f));
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialHi));
        // `update` advances the frame before the interrupt runs.
        f.status.anim_frame = 5.0;
        f.input.stick_x = -80;
        f.stick.step(-80, 0, false, false);
        status::update(&mut f);
        assert!(!f.mario_special_hi.launch_started);
        status::update(&mut f);
        assert_eq!(f.status.anim_frame, status::LUIGI_SUPERJUMP_LAUNCH_FRAME);
        assert!(f.mario_special_hi.launch_started);
        assert_eq!(f.facing, Facing::Left);
    }

    #[test]
    fn neutral_b_queues_a_luigi_fireball_on_frame_sixteen() {
        let mut f = luigi(Situation::Ground);
        press_b(&mut f, 0, 0);
        assert!(status::check_special_n(&mut f));
        for _ in 0..15 {
            status::update(&mut f);
            assert_eq!(f.take_weapon_spawn(), None);
        }
        status::update(&mut f);
        let spawn = f.take_weapon_spawn().expect("frame 16");
        assert_eq!(spawn.kind, crate::weapon::WeaponKind::LuigiFireball);

        let mut pool = crate::weapon::WeaponPool::default();
        assert!(pool.spawn(spawn));
        let shot = pool.first_fireball().unwrap();
        assert_eq!(shot.lifetime, 80);
        assert_eq!(shot.damage, 6);
        assert_eq!(shot.velocity.x, 36.0);
        pool.tick(core::iter::empty);
        assert_eq!(pool.first_fireball().unwrap().velocity.y, 0.0);
    }

    #[test]
    fn jab_combo_reaches_luigis_finisher_and_dair_lands_in_null() {
        assert_eq!(
            status::attack13_status(FighterKind::Luigi),
            Some(AnyStatus::Mario(MarioStatus::Attack13))
        );
        let mut f = luigi(Situation::Air);
        f.anim.landing = 10.0;
        status::set_air_attack(&mut f, Status::AttackAirLw);
        status::set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::LandingAirNull);
        assert_eq!(f.status.timing.anim_length, Some(2.0));
    }
}
