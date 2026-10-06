//! Mushroom Kingdom's pipes, `ftcommondokan.c`.
//!
//! A fighter that holds down on a pipe's floor (material `DokanL` or
//! `DokanR`) within 200 units of the pipe's map point drops in: `DokanStart`
//! slides it over the point and plays the entry, `DokanWait` carries it
//! hidden along a 30-frame cubic to the other pipe, and `DokanEnd` brings it
//! out there. One time in four the trip ends instead at the wall pipe
//! (`DokanWall`), where `DokanWalk` walks it out to fall.
//!
//! The pipes' map points come from the stage ([`crate::stage::Stage`]
//! publishes them as [`Points`]); the entry's call to
//! `grInishiePakkunSetWaitFighter` is queued as [`DokanState::plant_request`]
//! for the stage's next process, which runs before the plants'.

use ssb_engine::math::Vec3;

use crate::fighter::{Facing, Fighter, FighterKind};
use crate::status::{self, AnyStatus, Preserve, Status, StatusTiming};

/// `FTCOMMON_DOKAN_*`.
pub const STICK_RANGE_MIN: i32 = -53;
pub const BUFFER_TICS_MAX: u8 = 4;
pub const PLAYERTAG_WAIT: i32 = 20;
pub const TURN_STOP_WAIT_DEFAULT: i32 = 8;
/// `F_CLC_DTOR32(90.0F) / FTCOMMON_DOKAN_TURN_STOP_WAIT_DEFAULT`.
pub const TURN_STEP: f32 = core::f32::consts::FRAC_PI_2 / TURN_STOP_WAIT_DEFAULT as f32;
pub const POS_ADJUST: f32 = 25.0;
pub const DETECT_WIDTH: f32 = 200.0;
pub const POS_ADJUST_WAIT: i32 = 30;
pub const EXIT_WAIT: f32 = 30.0;
/// The wall-pipe exit's chance, `syUtilsRandFloat() <= 0.25F`.
pub const WALL_CHANCE: f32 = 0.25;

/// `nMPMaterialDokanL`, `nMPMaterialDokanR`.
pub const MATERIAL_DOKAN_L: u16 = 12;
pub const MATERIAL_DOKAN_R: u16 = 13;
/// `nMPMapObjKindDokanL`, `...DokanR`, `...DokanWall`.
pub const MAPOBJ_DOKAN_L: u16 = 0xA;
pub const MAPOBJ_DOKAN_R: u16 = 0xB;
pub const MAPOBJ_DOKAN_WALL: u16 = 0x14;

/// The pipes' map points: the first of each kind, as
/// `mpCollisionGetMapObjIDsKind` reports it. A stage without them has no
/// pipe floors either.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Points {
    pub left: Option<Vec3>,
    pub right: Option<Vec3>,
    pub wall: Option<Vec3>,
}

impl Points {
    pub fn get(&self, kind: u16) -> Option<Vec3> {
        match kind {
            MAPOBJ_DOKAN_L => self.left,
            MAPOBJ_DOKAN_R => self.right,
            MAPOBJ_DOKAN_WALL => self.wall,
            _ => None,
        }
    }
}

/// `ftCommonDokanStatusVars`, with the pipe points the stage publishes and
/// the flags `ftMainSetStatus` clears.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DokanState {
    pub points: Points,
    /// The material of the pipe entered.
    pub material: u16,
    /// Where the trip ends: the other pipe or the wall pipe.
    pub mapobj_kind: u16,
    pub pos_curr: Vec3,
    pub target_pos: Vec3,
    pub pos_adjust_wait: i32,
    pub playertag_wait: i32,
    pub turn_stop_wait: i32,
    /// `joints[nFTPartsJointTopN]->rotate.y` while a pipe status turns it.
    pub yaw: f32,
    /// `FTStruct::is_effect_skip`: motion and colour scripts make no
    /// effects while the fighter is in the pipe.
    pub is_effect_skip: bool,
    /// `FTStruct::is_jostle_ignore`: set by the pipes, the rolls, the down
    /// rolls and the grounded cliff phase two; [`crate::fighter::jostle`]
    /// skips the fighter.
    pub is_jostle_ignore: bool,
    /// `grInishiePakkunSetWaitFighter`, made by the entry.
    pub plant_request: bool,
    /// A setter `proc_update` called that projects against the map, run by
    /// [`run_pending`] as soon as the update returns.
    pub pending: Option<Pending>,
}

/// The status changes the pipe updates make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// `ftCommonDokanWaitSetStatus`.
    Wait,
    /// `ftCommonDokanEndSetStatus` or `ftCommonDokanWalkSetStatus`.
    Exit,
}

/// The pipe statuses.
pub fn is_dokan(s: AnyStatus) -> bool {
    matches!(
        s,
        AnyStatus::Common(
            Status::DokanStart | Status::DokanWait | Status::DokanEnd | Status::DokanWalk
        )
    )
}

/// `ftMainSetStatus`'s resets of the flags this module owns.
pub(crate) fn on_set_status(f: &mut Fighter) {
    f.dokan.is_effect_skip = false;
    f.dokan.is_jostle_ignore = false;
}

/// Mario, Luigi and their Metal and Polygon forms turn in their own
/// animation.
fn turns_itself(kind: FighterKind) -> bool {
    matches!(
        kind,
        FighterKind::Mario
            | FighterKind::MetalMario
            | FighterKind::PolyMario
            | FighterKind::Luigi
            | FighterKind::PolyLuigi
    )
}

/// `ftMainSetStatus`'s TopN yaw: `lr * 90°`.
fn facing_yaw(f: &Fighter) -> f32 {
    f.facing.sign() * core::f32::consts::FRAC_PI_2
}

/// The TopN yaw in a pipe status, `None` outside them.
pub fn model_yaw(f: &Fighter) -> Option<f32> {
    is_dokan(f.status.status).then_some(f.dokan.yaw)
}

fn timing(f: &Fighter, s: Status) -> StatusTiming {
    match crate::motion::anim_length(f.kind, s.into()) {
        Some(len) => StatusTiming::frames(len),
        None => StatusTiming::unknown(),
    }
}

/// `ftCommonDokanStartUpdateModelYaw`: towards the camera.
fn start_update_yaw(f: &mut Fighter) {
    if f.dokan.turn_stop_wait != 0 {
        f.dokan.turn_stop_wait -= 1;
        f.dokan.yaw += -TURN_STEP * f.facing.sign();
    }
}

/// `ftCommonDokanStartCheckInterruptCommon`: a down tap on a pipe's floor
/// near enough to its point.
pub fn check(f: &mut Fighter) -> bool {
    if f.stick.y as i32 > STICK_RANGE_MIN || f.stick.tap_y >= BUFFER_TICS_MAX {
        return false;
    }
    let material = f.floor.map_or(0, |s| s.material());
    let kind = match material {
        MATERIAL_DOKAN_L => MAPOBJ_DOKAN_L,
        MATERIAL_DOKAN_R => MAPOBJ_DOKAN_R,
        _ => return false,
    };
    let Some(pos) = f.dokan.points.get(kind) else {
        return false;
    };
    if (pos.x - f.pos.x).abs() <= DETECT_WIDTH {
        set_start(f, material);
        return true;
    }
    false
}

/// `ftCommonDokanStartSetStatus`.
pub fn set_start(f: &mut Fighter, material: u16) {
    status::set_status(f, Status::DokanStart, 0.0, timing(f, Status::DokanStart));
    status::play_anim_events(f);
    f.stick.tap_y = status::STICKBUFFER_MAX;
    crate::physics::stop_all(&mut f.physics);
    f.dokan.is_jostle_ignore = true;
    f.dokan.material = material;
    let kind = if material == MATERIAL_DOKAN_L {
        MAPOBJ_DOKAN_L
    } else {
        MAPOBJ_DOKAN_R
    };
    if let Some(p) = f.dokan.points.get(kind) {
        f.dokan.pos_curr = p;
    }
    // `ftParamSetPlayerTagWait(fighter_gobj, 1)`.
    f.interface.tag_wait = 1;
    f.dokan.turn_stop_wait = if turns_itself(f.kind) {
        0
    } else {
        TURN_STOP_WAIT_DEFAULT
    };
    f.dokan.yaw = facing_yaw(f);
    start_update_yaw(f);
    f.dokan.plant_request = true;
}

/// `ftCommonDokanWaitSetStatus`: into the pipe, bound for the other one or,
/// one time in four, the wall pipe.
fn set_wait<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = crate::weapon::MapSurface>,
{
    f.become_airborne();
    status::set_any_status_preserve(
        f,
        Status::DokanWait.into(),
        0.0,
        StatusTiming::unknown(),
        Preserve::HITSTATUS,
    );
    f.is_invisible = true;
    f.dokan.is_effect_skip = true;
    f.dokan.pos_adjust_wait = 0;
    f.dead.is_menu_ignore = true;
    f.dokan.yaw = facing_yaw(f);
    f.dokan.mapobj_kind = if f.dokan.material == MATERIAL_DOKAN_L {
        MAPOBJ_DOKAN_R
    } else {
        MAPOBJ_DOKAN_L
    };
    if let Some(p) = f.dokan.points.get(f.dokan.mapobj_kind) {
        f.dokan.target_pos = p;
    }
    if crate::rng::rand_float() <= WALL_CHANCE {
        f.dokan.mapobj_kind = MAPOBJ_DOKAN_WALL;
        if let Some(p) = f.dokan.points.wall {
            f.dokan.target_pos = p;
        }
        if let Some(dist) = crate::map::project_rwall_dist(surfaces, f.dokan.target_pos) {
            f.dokan.target_pos.x += dist + f.coll.width;
        }
    }
}

/// `ftCommonDokanEndSetStatus`: out of the other pipe.
fn set_end<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = crate::weapon::MapSurface>,
{
    // `mpCommonSetFighterGround`.
    f.physics.vel_ground.x = f.physics.vel_air.x * f.facing.sign();
    f.situation = crate::fighter::Situation::Ground;
    f.physics.jumps_used = 0;
    status::set_any_status_preserve(
        f,
        Status::DokanEnd.into(),
        0.0,
        timing(f, Status::DokanEnd),
        Preserve::HITSTATUS,
    );
    f.pos = f.dokan.target_pos;
    // `mpCollisionCheckProjectFloor` from the point itself.
    f.floor = crate::map::project_floor_line(surfaces, f.pos)
        .and_then(|(line, _)| crate::map::floor_point(surfaces, line, f.pos.x))
        .map(|(_, s)| s);
    f.dokan.is_jostle_ignore = true;
    f.dokan.playertag_wait = PLAYERTAG_WAIT;
    f.dokan.yaw = facing_yaw(f);
    if turns_itself(f.kind) {
        f.dokan.turn_stop_wait = 0;
    } else {
        f.dokan.turn_stop_wait = TURN_STOP_WAIT_DEFAULT;
        f.dokan.yaw = 0.0;
    }
}

/// `ftCommonDokanWalkSetStatus`: out of the wall pipe, facing right.
fn set_walk(f: &mut Fighter) {
    f.become_airborne();
    f.facing = Facing::Right;
    status::set_any_status_preserve(
        f,
        Status::DokanWalk.into(),
        0.0,
        timing(f, Status::DokanWalk),
        Preserve::HITSTATUS,
    );
    f.pos = f.dokan.target_pos;
    f.dokan.playertag_wait = PLAYERTAG_WAIT;
    f.dokan.yaw = facing_yaw(f);
}

/// `proc_update` of the four statuses (none has a `proc_interrupt`).
/// Returns `false` outside them. The setters that project against the map
/// are left in [`DokanState::pending`] for [`run_pending`].
pub fn update(f: &mut Fighter, current: Status) -> bool {
    match current {
        // `ftCommonDokanStartProcUpdate`.
        Status::DokanStart => {
            start_update_yaw(f);
            if f.status.animation_ended() {
                f.dokan.pending = Some(Pending::Wait);
            }
        }
        // `ftCommonDokanWaitProcUpdate`.
        Status::DokanWait => {
            f.dokan.pos_adjust_wait += 1;
            if f.dokan.pos_adjust_wait == POS_ADJUST_WAIT {
                f.dokan.pending = Some(Pending::Exit);
            }
        }
        // `ftCommonDokanEndProcUpdate`, also `DokanWalk`'s.
        Status::DokanEnd | Status::DokanWalk => {
            if f.dokan.playertag_wait != 0 {
                f.dokan.playertag_wait -= 1;
                if f.dokan.playertag_wait == 0 {
                    f.interface.tag_wait = 1;
                }
            }
            // `ftCommonDokanEndUpdateModelYaw`.
            if f.status.anim_frame >= EXIT_WAIT && f.dokan.turn_stop_wait != 0 {
                f.dokan.turn_stop_wait -= 1;
                f.dokan.yaw += TURN_STEP * f.facing.sign();
            }
            // `mpCommonSetFighterWaitOrFall`.
            if f.status.animation_ended() {
                if f.is_grounded() {
                    status::anim_end_set_wait(f);
                } else {
                    status::anim_end_set_fall(f);
                }
            }
        }
        _ => return false,
    }
    true
}

/// `gcGetInterpValueCubic(1/30, wait, base, target, 0, 0)`.
pub fn interp_cubic(length: f32, base: f32, target: f32) -> f32 {
    let length_invert = 1.0 / POS_ADJUST_WAIT as f32;
    let t2 = length * length;
    let i2 = length_invert * length_invert;
    let f16 = t2 * length * i2;
    let f10 = 2.0 * f16 * length_invert;
    let s14 = 3.0 * t2 * i2;
    (f10 - s14 + 1.0) * base + target * (s14 - f10)
}

/// The rest of the `proc_update` that left a [`Pending`] setter: the end of
/// the entry, or of the 30-frame trip.
pub fn run_pending<I, F>(f: &mut Fighter, surfaces: &F)
where
    F: Fn() -> I,
    I: IntoIterator<Item = crate::weapon::MapSurface>,
{
    match f.dokan.pending.take() {
        Some(Pending::Wait) => set_wait(f, surfaces),
        Some(Pending::Exit) if f.dokan.mapobj_kind == MAPOBJ_DOKAN_WALL => set_walk(f),
        Some(Pending::Exit) => set_end(f, surfaces),
        None => {}
    }
}

/// The physics and map steps the pipe statuses replace. `DokanStart`'s and
/// `DokanEnd`'s map step is the common ground one, so they return `false`
/// and leave their physics to [`apply_ground_physics`]; `DokanWait` and
/// `DokanWalk` place the fighter themselves and skip the map.
pub fn tick_status(f: &mut Fighter) -> bool {
    match f.status.status {
        // `ftCommonDokanWaitProcMap`.
        AnyStatus::Common(Status::DokanWait) => {
            let wait = f.dokan.pos_adjust_wait as f32;
            let d = f.dokan;
            f.pos.x = interp_cubic(wait, d.pos_curr.x, d.target_pos.x);
            f.pos.y = interp_cubic(wait, d.pos_curr.y, d.target_pos.y);
            true
        }
        // No physics; `mpCommonUpdateFighterProjectFloor` only finds the
        // floor under the exit for the shadow.
        AnyStatus::Common(Status::DokanWalk) => true,
        _ => false,
    }
}

/// `ftCommonDokanStartProcPhysics`: 25 units a frame onto the pipe's
/// point. `DokanEnd` has no `proc_physics`. Returns `false` outside them.
pub fn apply_ground_physics(f: &mut Fighter) -> bool {
    match f.status.status {
        AnyStatus::Common(Status::DokanStart) => {
            let kind = if f.dokan.material == MATERIAL_DOKAN_L {
                MAPOBJ_DOKAN_L
            } else {
                MAPOBJ_DOKAN_R
            };
            if let Some(p) = f.dokan.points.get(kind) {
                if f.pos.x > p.x {
                    f.pos.x -= POS_ADJUST;
                    if f.pos.x <= p.x {
                        f.pos.x = p.x;
                    }
                } else if f.pos.x < p.x {
                    f.pos.x += POS_ADJUST;
                    if f.pos.x >= p.x {
                        f.pos.x = p.x;
                    }
                }
            }
            true
        }
        AnyStatus::Common(Status::DokanEnd) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::Segment;
    use crate::fighter::Situation;
    use crate::ground::Standing;
    use crate::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
    use ssb_engine::math::Vec2;

    fn surface(
        kind: MapSurfaceKind,
        line: u16,
        a: (i16, i16),
        b: (i16, i16),
        flags: u16,
    ) -> MapSurface {
        MapSurface {
            motion: None,
            kind,
            segment: Segment {
                x1: a.0,
                y1: a.1,
                x2: b.0,
                y2: b.1,
                flags,
            },
            topology: Some(SurfaceTopology {
                line,
                point: 0,
                segments: 1,
                vertex1: line * 2,
                vertex2: line * 2 + 1,
            }),
        }
    }

    /// Two pipe tops at Y 0 (left at X -1250, right at 1250), the ground
    /// between them at -400, and a wall pipe at X -2500 against a wall at
    /// -2600.
    fn course() -> Vec<MapSurface> {
        vec![
            surface(
                MapSurfaceKind::Floor,
                0,
                (-1500, 0),
                (-1000, 0),
                MATERIAL_DOKAN_L,
            ),
            surface(
                MapSurfaceKind::Floor,
                1,
                (1000, 0),
                (1500, 0),
                MATERIAL_DOKAN_R,
            ),
            surface(MapSurfaceKind::Floor, 2, (-3000, -400), (3000, -400), 0),
            surface(MapSurfaceKind::RightWall, 3, (-2600, 0), (-2600, -1000), 0),
        ]
    }

    const POINTS: Points = Points {
        left: Some(Vec3::new(-1250.0, 0.0, 0.0)),
        right: Some(Vec3::new(1250.0, 0.0, 0.0)),
        wall: Some(Vec3::new(-2500.0, -300.0, 0.0)),
    };

    fn on_left_pipe(kind: FighterKind, x: f32) -> Fighter {
        let mut f = Fighter::new(kind, 0, 3);
        f.situation = Situation::Ground;
        f.status.status = Status::SquatWait.into();
        f.pos = Vec3::new(x, 0.0, 0.0);
        f.floor = Some(Standing {
            line: 0,
            flags: MATERIAL_DOKAN_L,
            normal: Vec2::new(0.0, 1.0),
        });
        f.dokan.points = POINTS;
        f.coll.width = 120.0;
        f
    }

    fn press_down(f: &mut Fighter) {
        f.stick.y = -80;
        f.stick.tap_y = 0;
    }

    fn frame(f: &mut Fighter, s: &[MapSurface]) {
        let surfaces = || s.iter().copied();
        f.tick_interrupt(&surfaces);
        f.tick_physics_map(&surfaces);
    }

    /// A seed whose next `syUtilsRandFloat` picks (or skips) the wall pipe.
    fn seed_for(wall: bool) -> i32 {
        (1..)
            .find(|&s| {
                crate::rng::set_seed(s);
                (crate::rng::rand_float() <= WALL_CHANCE) == wall
            })
            .unwrap()
    }

    #[test]
    fn a_down_tap_near_the_pipe_point_enters_it() {
        let s = course();
        // Too far: 201 units from the point.
        let mut f = on_left_pipe(FighterKind::Fox, -1250.0 + 201.0);
        press_down(&mut f);
        assert!(!check(&mut f));
        // A held stick, not a tap.
        let mut f = on_left_pipe(FighterKind::Fox, -1100.0);
        f.stick.y = -80;
        f.stick.tap_y = BUFFER_TICS_MAX;
        assert!(!check(&mut f));
        // Not enough down.
        let mut f = on_left_pipe(FighterKind::Fox, -1100.0);
        press_down(&mut f);
        f.stick.y = (STICK_RANGE_MIN + 1) as i8;
        assert!(!check(&mut f));

        let mut f = on_left_pipe(FighterKind::Fox, -1100.0);
        f.physics.vel_ground.x = 12.0;
        press_down(&mut f);
        frame(&mut f, &s);
        assert_eq!(f.status.status, Status::DokanStart);
        assert!(f.dokan.plant_request);
        assert_eq!(f.dokan.material, MATERIAL_DOKAN_L);
        assert_eq!(f.dokan.pos_curr, POINTS.left.unwrap());
        assert_eq!(f.stick.tap_y, status::STICKBUFFER_MAX);
        assert_eq!(f.physics.vel_ground.x, 0.0);
        // 25 units a frame onto the point, from the physics of that frame.
        assert_eq!(f.pos.x, -1125.0);
        frame(&mut f, &s);
        assert_eq!(f.pos.x, -1150.0);
        // The turn to the camera: one step at entry and one a frame.
        assert_eq!(f.dokan.turn_stop_wait, TURN_STOP_WAIT_DEFAULT - 2);
        // Facing right, the yaw falls from 90°.
        let yaw = model_yaw(&f).unwrap();
        assert!((yaw - (core::f32::consts::FRAC_PI_2 - 2.0 * TURN_STEP)).abs() < 1e-6);
    }

    /// Mario and Luigi turn in their own animation.
    #[test]
    fn mario_and_luigi_keep_their_facing() {
        for kind in [FighterKind::Mario, FighterKind::PolyLuigi] {
            let mut f = on_left_pipe(kind, -1250.0);
            set_start(&mut f, MATERIAL_DOKAN_L);
            assert_eq!(f.dokan.turn_stop_wait, 0);
            assert_eq!(model_yaw(&f), Some(core::f32::consts::FRAC_PI_2));
        }
    }

    fn enter_and_wait(kind: FighterKind, s: &[MapSurface], wall: bool) -> Fighter {
        let mut f = on_left_pipe(kind, -1250.0);
        press_down(&mut f);
        frame(&mut f, s);
        assert_eq!(f.status.status, Status::DokanStart);
        f.stick.y = 0;
        for _ in 0..200 {
            if f.status.status != Status::DokanStart {
                break;
            }
            crate::rng::set_seed(seed_for(wall));
            frame(&mut f, s);
        }
        assert_eq!(f.status.status, Status::DokanWait);
        f
    }

    /// The trip is 30 frames of cubic easing, hidden, to the other pipe,
    /// where the fighter comes out on the ground.
    #[test]
    fn the_pipe_carries_the_fighter_to_the_other_pipe() {
        let s = course();
        let mut f = enter_and_wait(FighterKind::Fox, &s, false);
        assert!(f.is_invisible);
        assert!(f.dokan.is_effect_skip);
        assert!(f.dead.is_menu_ignore);
        assert!(f.interface.tag_hide);
        assert!(!f.is_grounded());
        assert_eq!(f.dokan.mapobj_kind, MAPOBJ_DOKAN_R);
        assert_eq!(f.dokan.target_pos, POINTS.right.unwrap());
        // The frame it enters, the map places it at wait 0: the entry point.
        assert_eq!(f.pos.x, -1250.0);
        let mut xs = Vec::new();
        for _ in 0..POS_ADJUST_WAIT - 1 {
            frame(&mut f, &s);
            assert_eq!(f.status.status, Status::DokanWait);
            xs.push(f.pos.x);
        }
        // Halfway along at frame 15, symmetric about it.
        assert!((xs[14] - 0.0).abs() < 1e-3);
        assert!(xs.windows(2).all(|w| w[1] > w[0]));
        frame(&mut f, &s);
        assert_eq!(f.status.status, Status::DokanEnd);
        assert!(f.is_grounded());
        assert!(!f.is_invisible);
        assert!(!f.dokan.is_effect_skip);
        assert_eq!(f.pos, POINTS.right.unwrap());
        assert_eq!(f.floor.map(|s| s.line), Some(1));
        assert_eq!(model_yaw(&f), Some(0.0));
        assert_eq!(f.dokan.playertag_wait, PLAYERTAG_WAIT);
        for _ in 0..400 {
            if f.status.status != Status::DokanEnd {
                break;
            }
            frame(&mut f, &s);
        }
        assert_eq!(f.status.status, Status::Wait);
        assert_eq!(f.pos.x, 1250.0);
    }

    /// One time in four the trip ends at the wall pipe, just right of the
    /// wall, and the fighter walks out facing right and falls.
    #[test]
    fn the_wall_pipe_walks_the_fighter_out() {
        let s = course();
        let mut f = enter_and_wait(FighterKind::Fox, &s, true);
        assert_eq!(f.dokan.mapobj_kind, MAPOBJ_DOKAN_WALL);
        assert_eq!(f.dokan.target_pos.x, -2600.0 + 120.0);
        assert_eq!(f.dokan.target_pos.y, -300.0);
        for _ in 0..POS_ADJUST_WAIT {
            frame(&mut f, &s);
        }
        assert_eq!(f.status.status, Status::DokanWalk);
        assert_eq!(f.facing, Facing::Right);
        assert!(!f.is_grounded());
        let at = f.pos;
        for _ in 0..400 {
            if f.status.status != Status::DokanWalk {
                break;
            }
            frame(&mut f, &s);
            if f.status.status == Status::DokanWalk {
                assert_eq!(f.pos, at, "the walk leaves the body where it is");
            }
        }
        assert_eq!(f.status.status, Status::Fall);
    }

    #[test]
    fn the_cubic_eases_in_and_out() {
        assert_eq!(interp_cubic(0.0, 10.0, 70.0), 10.0);
        assert!((interp_cubic(30.0, 10.0, 70.0) - 70.0).abs() < 1e-4);
        assert!((interp_cubic(15.0, 10.0, 70.0) - 40.0).abs() < 1e-4);
        assert!(interp_cubic(3.0, 0.0, 1.0) < 0.1 * 0.5);
    }
}

#[cfg(test)]
mod motion_tests {
    use super::*;

    /// Every fighter's entry, exit and wall walk end on their own
    /// (`ftAnimEndCheckSetStatus`), so each needs its motion's length.
    #[test]
    fn every_fighter_has_its_pipe_motions() {
        for &kind in FighterKind::PLAYABLE {
            for s in [Status::DokanStart, Status::DokanEnd, Status::DokanWalk] {
                let len = crate::motion::anim_length(kind, s.into());
                assert!(len.is_some_and(|l| l > 0.0), "{kind:?} {s:?}");
            }
        }
    }
}
