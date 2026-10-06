//! Luigi's statuses are Mario's (`ftluigistatus.h` points at the Mario
//! callbacks); his motion scripts are his own ([`crate::motion`]). These
//! tests cover the places the two differ.

#[cfg(test)]
mod tests {
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
    fn super_jump_launches_on_frame_seven() {
        let mut f = luigi(Situation::Ground);
        press_b(&mut f, 0, 80);
        assert!(status::check_special_hi(&mut f));
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialHi));
        f.input.stick_x = -80;
        f.stick.step(-80, 0, false, false);
        let mut launched = None;
        for _ in 0..12 {
            status::update(&mut f);
            if launched.is_none() && f.mario_special_hi.launch_started {
                launched = Some(f.status.anim_frame);
            }
        }
        assert_eq!(launched, Some(status::LUIGI_SUPERJUMP_LAUNCH_FRAME));
        assert_eq!(f.facing, Facing::Left);
    }

    #[test]
    fn neutral_b_queues_a_luigi_fireball_on_frame_sixteen() {
        let mut f = luigi(Situation::Ground);
        press_b(&mut f, 0, 0);
        assert!(status::check_special_n(&mut f));
        // The setter played frame 1 (RE-468).
        for _ in 0..14 {
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
        pool.tick(core::iter::empty, None);
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
        for _ in 0..12 {
            status::update(&mut f);
        }
        f.physics.vel_air.y = -30.0;
        let speed = f.motion_script.flags[1] as f32 * 0.01;
        assert!(speed > 0.0);
        status::set_landing_or_landing_air(&mut f);
        assert_eq!(f.status.status, Status::LandingAirNull);
        assert_eq!(f.status.timing.anim_speed, speed);
    }
}
