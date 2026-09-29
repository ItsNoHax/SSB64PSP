use super::*;

fn stage() -> StageBounds {
    StageBounds {
        map: BlastZone {
            top: 3000.0,
            bottom: -2000.0,
            left: -4000.0,
            right: 4000.0,
        },
        camera: BlastZone {
            top: 2000.0,
            bottom: -1000.0,
            left: -3000.0,
            right: 3000.0,
        },
        rebirth: Vec2::new(100.0, 1200.0),
    }
}

fn mario() -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    f.dead.bounds = Some(stage());
    f
}

fn is(f: &Fighter, status: Status) -> bool {
    f.status.status == AnyStatus::Common(status)
}

#[test]
fn falling_below_the_bottom_bound_is_dead_down_without_a_stock_in_training() {
    let mut f = mario();
    f.pos = Vec3::new(3500.0, -2001.0, 0.0);
    assert!(check(&mut f));
    assert!(is(&f, Status::DeadDown));
    assert!(f.is_invisible && f.dead.is_ghost && f.is_shadow_hidden);
    assert_eq!(f.situation, Situation::Air);
    assert_eq!(f.dead.wait, DEAD_WAIT);
    assert_eq!(f.stocks, 3);
    assert_eq!(f.dead.falls, 1);
    // The blast is pulled back inside the camera's side bound.
    assert_eq!(
        f.dead.explode,
        Some((Vec3::new(3000.0, -2001.0, 0.0), ExplodeKind::Down))
    );
    // A ghost is not checked again.
    assert!(!check(&mut f));
}

#[test]
fn a_stock_match_takes_a_stock_and_sleeps_after_the_last() {
    let mut f = mario();
    f.dead.stock_rule = true;
    f.stocks = 0;
    f.pos.y = -2001.0;
    check(&mut f);
    assert_eq!(f.stocks, -1);
    for _ in 0..DEAD_WAIT {
        status::update(&mut f);
    }
    assert!(is(&f, Status::Sleep));
    assert!(!f.dead.rebirth_pending);
    // `ftCommonSleepSetStatus`: out of the camera and the fight.
    assert_eq!(f.dead.camera_mode, CameraMode::Ghost);
    assert!(f.dead.is_ghost);
}

#[test]
fn the_sides_are_checked_right_then_left_and_share_one_status() {
    let mut right = mario();
    right.pos = Vec3::new(4001.0, 2500.0, 0.0);
    assert!(check(&mut right));
    assert!(is(&right, Status::DeadLeftRight));
    assert_eq!(
        right.dead.explode,
        Some((Vec3::new(4001.0, 2000.0, 0.0), ExplodeKind::Right))
    );
    let mut left = mario();
    left.pos.x = -4001.0;
    assert!(check(&mut left));
    assert_eq!(left.dead.explode.map(|e| e.1), Some(ExplodeKind::Left));
    // Inside the bounds nothing happens.
    let mut f = mario();
    assert!(!check(&mut f));
    assert!(is(&f, Status::Wait));
}

#[test]
fn a_top_out_falls_toward_the_camera_one_time_in_six() {
    crate::rng::set_seed(1);
    let mut fall = 0;
    for _ in 0..1200 {
        let mut f = mario();
        f.pos.y = 3001.0;
        assert!(check(&mut f));
        match f.status.status {
            AnyStatus::Common(Status::DeadUpFall) => fall += 1,
            AnyStatus::Common(Status::DeadUpStar) => {}
            other => panic!("{other:?}"),
        }
    }
    assert!((140..260).contains(&fall), "{fall}");
}

#[test]
fn a_dead_down_wait_ends_in_a_rebirth_request() {
    let mut f = mario();
    f.pos.y = -2001.0;
    check(&mut f);
    for _ in 0..DEAD_WAIT - 1 {
        status::update(&mut f);
        assert!(!f.dead.rebirth_pending);
    }
    status::update(&mut f);
    assert!(f.dead.rebirth_pending);
}

#[test]
fn a_star_flies_off_for_180_ticks_then_waits_45() {
    let mut f = mario();
    f.pos = Vec3::new(0.0, 3001.0, 0.0);
    set_dead_up_star(&mut f);
    assert!(!f.is_invisible);
    assert_eq!(f.dead.camera_mode, CameraMode::DeadUp);
    status::update(&mut f);
    // Toward 60% of the camera's top, and away from the camera.
    assert_eq!(f.physics.vel_air.y, (2000.0 * 0.6 - 3001.0) / 180.0);
    assert_eq!(f.physics.vel_air.z, DEADUPSTAR_VEL_Z);
    tick_status(&mut f);
    assert_eq!(f.pos.z, DEADUPSTAR_VEL_Z);
    for _ in 0..DEADUP_WAIT {
        status::update(&mut f);
    }
    assert!(f.is_invisible);
    assert_eq!(f.physics.vel_air, Vec3::ZERO);
    for _ in 0..DEAD_WAIT {
        assert!(!f.dead.rebirth_pending);
        status::update(&mut f);
    }
    assert!(f.dead.rebirth_pending);
}

#[test]
fn a_fall_top_out_drops_from_above_the_camera() {
    let mut f = mario();
    f.dead.camera_eye = Vec3::new(50.0, 400.0, 6000.0);
    f.pos = Vec3::new(10.0, 3001.0, 0.0);
    set_dead_up_fall(&mut f);
    status::update(&mut f);
    assert_eq!(f.pos, Vec3::new(50.0, 3000.0, 3000.0));
    assert_eq!(f.physics.vel_air.y, (-1000.0 - 3001.0) / 180.0);
}

#[test]
fn a_rebirth_starts_at_the_top_and_resets_the_fighter() {
    let mut f = mario();
    f.damage = 120;
    f.costume = 2;
    f.team = 3;
    f.facing = Facing::Left;
    f.pos.y = -2001.0;
    check(&mut f);
    rebirth_down(&mut f, 1);
    assert!(is(&f, Status::RebirthDown));
    assert_eq!(f.pos, Vec3::new(100.0 - 1000.0, 3000.0, 0.0));
    assert_eq!(f.damage, 0);
    assert_eq!(f.costume, 2);
    assert_eq!(f.team, 3);
    assert_eq!(f.facing, Facing::Left);
    assert_eq!(f.situation, Situation::Ground);
    assert!(f.dead.is_ghost && f.dead.is_rebirth && !f.is_invisible);
    assert_eq!(f.dead.camera_mode, CameraMode::Ghost);
    assert_eq!(f.dead.bounds, Some(stage()));
    assert_eq!(f.dead.falls, 1);
    assert!(!f.dead.rebirth_pending);
}

#[test]
fn the_halo_lowers_the_fighter_onto_its_point_in_90_ticks() {
    let mut f = mario();
    rebirth_down(&mut f, 0);
    let mut last = f.pos.y;
    for _ in 0..HALO_LOWER_WAIT {
        status::update(&mut f);
        tick_status(&mut f);
        assert!(f.pos.y <= last);
        last = f.pos.y;
    }
    assert_eq!(f.pos.y, 1200.0);
    assert_eq!(f.pos.x, 100.0);
}

#[test]
fn the_rebirth_runs_390_ticks_and_ends_in_fall_with_invincibility() {
    let mut f = mario();
    rebirth_down(&mut f, 0);
    let mut ticks = 0;
    let mut saw_camera_back = None;
    while is(&f, Status::RebirthDown) {
        status::update(&mut f);
        ticks += 1;
        if saw_camera_back.is_none() && f.dead.camera_mode == CameraMode::Default {
            saw_camera_back = Some(ticks);
        }
    }
    assert_eq!(saw_camera_back, Some(HALO_CAMERA_WAIT));
    // The despawn wait counts down from 390: it reaches 390 - 75 after 75
    // ticks.
    assert_eq!(ticks, HALO_STAND_WAIT);
    assert!(is(&f, Status::RebirthStand));
    assert!(f.dead.is_ghost);
    while !matches!(
        f.status.status,
        AnyStatus::Common(Status::Fall | Status::FallAerial)
    ) {
        status::update(&mut f);
        ticks += 1;
        assert!(ticks <= HALO_DESPAWN_WAIT);
    }
    assert_eq!(ticks, HALO_DESPAWN_WAIT);
    assert_eq!(f.invincible_frames, REBIRTH_INVINCIBLE_FRAMES);
    assert!(!f.dead.is_ghost);
}

#[test]
fn a_second_rebirth_takes_the_next_free_halo() {
    let others = [(AnyStatus::Common(Status::RebirthWait), 0)];
    assert_eq!(halo_number(others.iter().copied()), 1);
    let others = [(AnyStatus::Common(Status::Wait), 0)];
    assert_eq!(halo_number(others.iter().copied()), 0);
    let others = [
        (AnyStatus::Common(Status::RebirthDown), 1),
        (AnyStatus::Common(Status::RebirthStand), 0),
    ];
    assert_eq!(halo_number(others.iter().copied()), 2);
}
