//! The pack-to-game bridge: a fighter standing on a real stage, on device.
//!
//! `ssb-rom` knows ROM/pack data. `ssb-game` knows portable gameplay. This
//! module joins those two systems on PSP: it walks the pack's collision
//! tables into the shape [`ssb_game::collision`] wants, converts packed
//! fighter data into `ssb-game`'s own attribute structs, ticks the fighter,
//! and keeps its skeleton/animation and battle-camera state in sync.
//!
//! ## Why the adapter is here and not in either crate
//!
//! `ssb-rom` must not know game logic and `ssb-game` must not know the pack
//! format, so [`FloorSegments`] is duplicated from `romtool`'s copy on
//! purpose. It is twenty lines, and the alternative is a shared type that
//! drags one crate into the other.
//!
//! [`FloorSegments`] allocates nothing: it reads segments straight out of the
//! mapped pack as the query asks for them. A tick's collision work is
//! therefore proportional to the stage's floor count with no per-frame setup,
//! which is what keeps this affordable inside the frame budget.
//!
//! This module owns only what connects `ssb-rom`'s `Pack` to `ssb-game`'s
//! `Fighter` to skeleton/animation to battle-camera/runtime rendering state.
//! It does not own menus, Training rules, viewer diagnostics, CPU logic,
//! stocks, or match rules -- those stay in the applications that use it.

use ssb_game::collision::Segment;
use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::ground::BodyColl;
use ssb_game::physics::PhysicsAttributes;
use ssb_game::status::AnimLengths;
use ssb_game::status::AnyStatus;
use ssb_game::status::Status;
use ssb_game::weapon::{MapSurface, MapSurfaceKind};
use ssb_rom::pack::{line_kind, FighterDesc, LineDesc, MeshDesc, Pack, StageDesc};
use ssb_rom::pack::ObjectDesc;

/// Mario Special1's direct weapon display list. Unlike fighters and stages,
/// the source descriptor names geometry directly instead of through a graph.
pub const MARIO_FIREBALL_SOURCE_FILE: u32 = 297;
pub const MARIO_FIREBALL_SOURCE_OFFSET: u32 = 0x1D8;

/// Finds the packed Mario Fireball mesh by its stable source identity.
pub fn mario_fireball_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|mesh| {
            mesh.source_file == MARIO_FIREBALL_SOURCE_FILE
                && mesh.source_offset == MARIO_FIREBALL_SOURCE_OFFSET
        })
}

/// Fox Special1's `WPAttributes.data` resolves to this direct weapon list.
pub const FOX_BLASTER_SOURCE_FILE: u32 = 316;
pub const FOX_BLASTER_SOURCE_OFFSET: u32 = 0x40;

pub fn fox_blaster_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|mesh| {
            mesh.source_file == FOX_BLASTER_SOURCE_FILE
                && mesh.source_offset == FOX_BLASTER_SOURCE_OFFSET
        })
}

/// Fox Special2's three-entry Reflector effect hierarchy.
pub fn fox_reflector_object(pack: &Pack<'_>) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| object.source_file == 346 && object.source_offset == 0x2B0)
}

/// Walks a stage's floor polylines as the `(line_id, segment)` pairs the
/// collision query consumes.
///
/// A polyline of *n* points is *n-1* segments, so this holds the previous
/// point and emits a segment each time it takes another.
pub struct FloorSegments<'a, 'p> {
    pack: &'a Pack<'p>,
    stage: &'a StageDesc,
    /// Index of the line being walked, within the stage's lines.
    line: u32,
    /// The line itself, once it turned out to be a floor.
    current: Option<LineDesc>,
    /// Index of the next point within the current line.
    point: u16,
    /// The previous point, which is the segment's start.
    prev: Option<(i16, i16, u16)>,
}

/// Walks every static stage collision segment with its authored one-sided
/// kind. Fighters still deliberately consume [`FloorSegments`] alone; live
/// weapons require all four kinds for `wpMapTestAll`/rebound behavior.
pub struct MapSegments<'a, 'p> {
    pack: &'a Pack<'p>,
    stage: &'a StageDesc,
    line: u32,
    current: Option<LineDesc>,
    point: u16,
    prev: Option<(i16, i16, u16)>,
}

impl<'a, 'p> MapSegments<'a, 'p> {
    pub fn new(pack: &'a Pack<'p>, stage: &'a StageDesc) -> Self {
        Self {
            pack,
            stage,
            line: 0,
            current: None,
            point: 0,
            prev: None,
        }
    }
}

impl Iterator for MapSegments<'_, '_> {
    type Item = MapSurface;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(line) = self.current else {
                if self.line >= self.stage.line_count {
                    return None;
                }
                let candidate = self.pack.line(self.stage.first_line + self.line);
                self.line += 1;
                if let Some(candidate) = candidate.filter(|line| line.vertex_count >= 2) {
                    self.current = Some(candidate);
                    self.point = 0;
                    self.prev = None;
                }
                continue;
            };

            if self.point >= line.vertex_count {
                self.current = None;
                continue;
            }
            let vertex = self.pack.coll_vertex(line.first_vertex + self.point as u32);
            self.point += 1;
            let Some(vertex) = vertex else {
                self.current = None;
                continue;
            };
            let Some((x1, y1, flags)) = self.prev.replace((vertex.x, vertex.y, vertex.flags)) else {
                continue;
            };
            let kind = match line.kind {
                line_kind::FLOOR => MapSurfaceKind::Floor,
                line_kind::CEILING => MapSurfaceKind::Ceiling,
                line_kind::RIGHT_WALL => MapSurfaceKind::RightWall,
                line_kind::LEFT_WALL => MapSurfaceKind::LeftWall,
                _ => continue,
            };
            return Some(MapSurface {
                kind,
                segment: Segment {
                    x1,
                    y1,
                    x2: vertex.x,
                    y2: vertex.y,
                    flags,
                },
            });
        }
    }
}

impl<'a, 'p> FloorSegments<'a, 'p> {
    pub fn new(pack: &'a Pack<'p>, stage: &'a StageDesc) -> Self {
        FloorSegments {
            pack,
            stage,
            line: 0,
            current: None,
            point: 0,
            prev: None,
        }
    }
}

impl Iterator for FloorSegments<'_, '_> {
    type Item = (u16, Segment);

    fn next(&mut self) -> Option<(u16, Segment)> {
        loop {
            let Some(line) = self.current else {
                // Advance to the next floor line, skipping walls and ceilings.
                if self.line >= self.stage.line_count {
                    return None;
                }
                let l = self.pack.line(self.stage.first_line + self.line);
                self.line += 1;
                if let Some(l) = l {
                    if l.kind == line_kind::FLOOR && l.vertex_count >= 2 {
                        self.current = Some(l);
                        self.point = 0;
                        self.prev = None;
                    }
                }
                continue;
            };

            if self.point >= line.vertex_count {
                self.current = None;
                continue;
            }
            let v = self.pack.coll_vertex(line.first_vertex + self.point as u32);
            self.point += 1;
            let Some(v) = v else {
                self.current = None;
                continue;
            };

            match self.prev.replace((v.x, v.y, v.flags)) {
                None => continue,
                Some((x1, y1, flags)) => {
                    return Some((
                        line.id,
                        Segment {
                            x1,
                            y1,
                            x2: v.x,
                            y2: v.y,
                            // The original reports the flags of the segment's
                            // *first* vertex through `stand_coll_flags`.
                            flags,
                        },
                    ));
                }
            }
        }
    }
}

/// The object a character's animations drive.
///
/// The pack stores an absolute node per animation joint, and a node belongs to
/// exactly one object, so this is the one hop the tables do not hold — a scan
/// over the object list, done once when a fighter is created.
pub fn fighter_object(pack: &Pack<'_>, kind: u32) -> Option<u32> {
    let anim = pack.fighter_anim(kind, 0)?;
    let node = (0..anim.joint_count)
        .filter_map(|i| pack.anim_joint(anim.first_joint + i))
        .map(|j| j.node)
        .find(|&n| n != ssb_rom::pack::AnimJoint::NO_NODE)?;
    (0..pack.object_count()).find(|&i| {
        pack.object(i)
            .is_some_and(|o| node >= o.first_node && node < o.first_node + o.node_count)
    })
}

/// The physics constants out of a packed [`FighterDesc`].
///
/// Duplicated from `romtool`'s copy for the same reason [`FloorSegments`] is:
/// `ssb-game` must not know the pack format, and a shared type would drag one
/// crate into the other for the sake of eighteen field copies.
pub fn physics_of(d: &FighterDesc) -> PhysicsAttributes {
    PhysicsAttributes {
        traction: d.traction,
        dash_speed: d.dash_speed,
        dash_decel: d.dash_decel,
        run_speed: d.run_speed,
        walk_speed_mul: d.walk_speed_mul,
        jump_vel_x: d.jump_vel_x,
        jump_height_mul: d.jump_height_mul,
        jump_height_base: d.jump_height_base,
        jumpaerial_vel_x: d.jumpaerial_vel_x,
        jumpaerial_height: d.jumpaerial_height,
        air_accel: d.air_accel,
        air_speed_max_x: d.air_speed_max_x,
        air_friction: d.air_friction,
        gravity: d.gravity,
        tvel_base: d.tvel_base,
        tvel_fast: d.tvel_fast,
        jumps_max: d.jumps_max,
        weight: d.weight,
        kneebend_anim_length: d.kneebend_anim_length,
        dash_to_run: d.dash_to_run,
        walkslow_anim_length: d.walkslow_anim_length,
        walkmiddle_anim_length: d.walkmiddle_anim_length,
        walkfast_anim_length: d.walkfast_anim_length,
    }
}

/// The animation lengths out of a packed [`FighterDesc`].
///
/// These come from the figatree files rather than `FTAttributes`, which is why
/// they are a separate struct rather than more fields on [`physics_of`]'s.
pub fn anim_of(d: &FighterDesc) -> AnimLengths {
    AnimLengths {
        dash: d.dash_anim_length,
        turn: d.turn_anim_length,
        run_brake: d.runbrake_anim_length,
        squat: d.squat_anim_length,
        squat_rv: d.squatrv_anim_length,
        landing: d.landing_anim_length,
        pass: d.pass_anim_length,
    }
}

/// The collision diamond out of a packed [`FighterDesc`].
pub fn body_of(d: &FighterDesc) -> BodyColl {
    BodyColl {
        top: d.coll_top,
        center: d.coll_center,
        bottom: d.coll_bottom,
        width: d.coll_width,
    }
}

/// Resolves one fighter's source floor shadow from the live fighter and the
/// stage map.  `ftShadowProcDisplay` uses the standing floor while grounded;
/// otherwise it calls `mpCollisionCheckProjectFloor` from the fighter's root
/// straight down.  It does *not* shrink or fade for altitude.
///
/// The portable fighter carries the source `is_invisible` and
/// `is_shadow_hide` gates. Common lifecycle statuses maintain the latter;
/// capture/other visual systems can set it without knowing PSP draw details.
/// Invincibility alone is deliberately not a hide condition — the original
/// keeps its shadow.
pub fn fighter_shadow(
    pack: &Pack<'_>,
    stage: &StageDesc,
    fighter: &Fighter,
    shadow_size: f32,
) -> ssb_game::shadow::ShadowGeometry {
    if !ssb_game::shadow::visible_for(
        fighter.status.status,
        fighter.is_invisible,
        fighter.is_shadow_hidden,
    ) {
        return ssb_game::shadow::ShadowGeometry::EMPTY;
    }

    let line = fighter.floor.map(|f| f.line).or_else(|| {
        ssb_game::collision::project_floor(
            FloorSegments::new(pack, stage),
            ssb_engine::math::Vec2::new(fighter.pos.x, fighter.pos.y),
        )
        .map(|f| f.line)
    });
    let Some(line) = line else {
        return ssb_game::shadow::ShadowGeometry::EMPTY;
    };
    ssb_game::shadow::build(
        fighter.pos,
        shadow_size,
        FloorSegments::new(pack, stage)
            .filter(move |(candidate, _)| *candidate == line)
            .map(|(_, segment)| segment),
    )
}

/// Advances a fighter's skeleton pose for `status`. Shared by every fighter
/// scene that owns a skeleton: restarted on a status *change* rather than
/// every tick, since an animation carries its own clock and re-seeding it
/// each frame would freeze every fighter on frame zero. A looping animation
/// is left to loop; a finite one that has run out holds its last pose, which
/// is what the original does when a status outlives its animation.
pub fn tick_skeleton_animation(
    pack: &Pack<'_>,
    kind: u32,
    status: AnyStatus,
    skeleton: &mut ssb_rom::skeleton::Skeleton,
    started: &mut Option<AnyStatus>,
) -> Option<ssb_rom::figatree::JointPose> {
    let slot = if kind == FighterKind::Fox as u32 {
        match status {
            AnyStatus::Common(Status::Attack11) => 26,
            AnyStatus::Common(Status::Attack12) => 27,
            AnyStatus::Common(Status::AttackDash) => 31,
            AnyStatus::Common(Status::AttackS3Hi) => 32,
            AnyStatus::Common(Status::AttackS3HiS) => 33,
            AnyStatus::Common(Status::AttackS3) => 34,
            AnyStatus::Common(Status::AttackS3LwS) => 35,
            AnyStatus::Common(Status::AttackS3Lw) => 36,
            AnyStatus::Common(Status::AttackHi3) => 37,
            AnyStatus::Common(Status::AttackLw3) => 38,
            AnyStatus::Common(Status::AttackS4) => 39,
            AnyStatus::Common(Status::AttackHi4) => 40,
            AnyStatus::Common(Status::AttackLw4) => 41,
            AnyStatus::Common(Status::AttackAirN) => 42,
            AnyStatus::Common(Status::AttackAirF) => 43,
            AnyStatus::Common(Status::AttackAirB) => 44,
            AnyStatus::Common(Status::AttackAirHi) => 45,
            AnyStatus::Common(Status::AttackAirLw) => 46,
            _ => status.anim_slot(),
        }
    } else if kind == FighterKind::Donkey as u32 {
        match status {
            AnyStatus::Common(Status::Attack11) => 68,
            AnyStatus::Common(Status::Attack12) => 69,
            AnyStatus::Common(Status::AttackDash) => 70,
            AnyStatus::Common(Status::AttackS3Hi) => 71,
            AnyStatus::Common(Status::AttackS3) => 72,
            AnyStatus::Common(Status::AttackS3Lw) => 73,
            AnyStatus::Common(Status::AttackHi3) => 74,
            AnyStatus::Common(Status::AttackLw3) => 75,
            AnyStatus::Common(Status::AttackS4Hi) => 76,
            AnyStatus::Common(Status::AttackS4HiS) => 77,
            AnyStatus::Common(Status::AttackS4) => 78,
            AnyStatus::Common(Status::AttackS4LwS) => 79,
            AnyStatus::Common(Status::AttackS4Lw) => 80,
            AnyStatus::Common(Status::AttackHi4) => 81,
            AnyStatus::Common(Status::AttackLw4) => 82,
            AnyStatus::Common(Status::AttackAirN) => 83,
            AnyStatus::Common(Status::AttackAirF) => 84,
            AnyStatus::Common(Status::AttackAirB) => 85,
            AnyStatus::Common(Status::AttackAirHi) => 86,
            AnyStatus::Common(Status::AttackAirLw) => 87,
            _ => status.anim_slot(),
        }
    } else {
        status.anim_slot()
    } as u32;
    if *started != Some(status) {
        *started = Some(status);
        // A status with no motion of its own (`CatchWait`, `CaptureWait`)
        // keeps playing the previous one; its slot is that status's slot.
        if !status.keeps_motion() {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                skeleton.start(pack, &anim, 0.0, status.anim_speed());
            }
        }
    }
    // The slot is read back rather than remembered, so a status whose
    // animation the pack lacks -- Kirby has no aerial jump -- simply keeps
    // the pose it had.
    let root_before = skeleton.pose(0).copied();
    if let Some(anim) = pack.fighter_anim(kind, slot) {
        if let Some(script) = pack.anim_script(&anim) {
            let _ = skeleton.tick(script);
        }
    }
    root_before
}

/// The on-device gameplay slice: one fighter, connected end to end from
/// `ssb-rom` `Pack` data through `ssb-game` physics/collision to skeleton
/// animation and the battle camera.
///
/// No opponent, no match rules — the point is that the ported physics and
/// the ported collision run together against real stage data at 60 Hz, which
/// is the thing neither host tests nor a static render can show. Owns only
/// what connects those systems; it does not own menus, Training rules,
/// viewer diagnostics, CPU logic, stocks, or match rules.
pub struct FighterScene {
    pub fighter: Fighter,
    /// Whether the fighter found a floor when it was placed.
    pub placed: bool,
    /// Ticks since the fighter last touched the ground, for the overlay.
    pub airborne_ticks: u32,
    /// Jump button last frame, so this frame's tap and release can be derived.
    pub jump_was_held: bool,
    /// Whether the pack supplied this character's real constants. When false
    /// the fighter falls under [`PhysicsAttributes::MARIO`], and the overlay
    /// says so rather than letting a stale pack look like a physics bug.
    pub from_pack: bool,
    /// The fighter's animation, and the status it was started for.
    ///
    /// Kept here rather than in `ssb-game` because starting one needs the
    /// pack, which Layer A must not know about — the same split the physics
    /// constants use.
    pub skeleton: ssb_rom::skeleton::Skeleton,
    /// Object whose nodes the skeleton drives, or `u32::MAX` when the pack has
    /// no model for this character.
    pub object: u32,
    /// The real battle camera (RE-131), ticked alongside the fighter each
    /// frame in [`FighterScene::tick`].
    pub camera: ssb_game::camera::Camera,
    /// `FTAttributes.cam_offset_y` (`refs/ssb-decomp-re/src/ft/fttypes.h`) --
    /// deliberately *not* part of `PhysicsAttributes` (that struct's own doc
    /// comment already carves camera offsets out as belonging to "other
    /// systems"), so it lives here beside the camera that actually uses it,
    /// added to the fighter's own `y` before every [`ssb_game::camera::Camera::tick`]
    /// call, matching `gmCameraUpdateInterests`' own `target_pos.y +=
    /// fp->attr->cam_offset_y`.
    pub cam_offset_y: f32,
    /// Initial `FTStruct.camera_zoom_frame`, copied from the fighter's real
    /// `FTAttributes.camera_zoom` value.
    pub camera_zoom_frame: f32,
    /// `FTAttributes::shadow_size`, preserved separately from physics because
    /// it feeds `ftShadowProcDisplay`, not a gameplay calculation.
    pub shadow_size: f32,
    started: Option<AnyStatus>,
    /// TransN pose immediately before the last animation parser advance. On
    /// the next fighter tick it pairs with the current hidden pose to recover
    /// the exact `transn - anim_vel` delta the original physics reads.
    root_motion_before_tick: Option<ssb_rom::figatree::JointPose>,
}

impl FighterScene {
    /// Compose a fighter model, including the extra runtime joint in the
    /// held pose. `CapturePulled`'s first script drives TransN; its model
    /// root script is relative to that transform. The pack stores TransN
    /// as `NO_NODE`, so a plain `Skeleton::compose` leaves the model below
    /// its gameplay root even though floor collision is correct (RE-331).
    pub fn compose_model(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [ssb_rom::scene::Mat4],
    ) -> usize {
        let count = self.skeleton.compose(pack, object, out);
        if matches!(
            self.fighter.status.status,
            AnyStatus::Common(Status::CapturePulled | Status::CaptureWait)
        ) && self.skeleton.joint_node(0).is_none()
        {
            if let Some(pose) = self.skeleton.pose(0) {
                let scale = ssb_rom::pack::MODEL_SCALE;
                let runtime = ssb_rom::scene::Mat4::from_trs(
                    [
                        pose.translate[0] / scale,
                        pose.translate[1] / scale,
                        pose.translate[2] / scale,
                    ],
                    pose.rotate,
                    pose.scale,
                );
                for matrix in &mut out[..count] {
                    *matrix = runtime.mul(matrix);
                }
            }
        }
        count
    }

    /// Puts a fighter of `kind` at a stage's `spawn_index`'th spawn point.
    ///
    /// Deliberately *not* settled onto the surface: a spawn sits a few units
    /// up (RE-030) and letting it fall that distance is the first thing worth
    /// watching. Returns a `FighterScene` even when the stage has no such
    /// spawn, so a caller can say so (`placed`) rather than the view going
    /// blank; a caller that needs to distinguish "no spawn point" from
    /// "spawn point, but nothing to stand on" should check
    /// `pack.spawn(stage, spawn_index)` itself before calling this.
    pub fn at_spawn(
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        spawn_index: u16,
    ) -> FighterScene {
        // `Fighter::new`'s `port` and `pack.spawn`'s `player` are different
        // concepts that happen to share the same value by this project's own
        // convention (port N spawns at spawn N) -- the same convention the
        // pre-generalization `Play`/`Dummy` split already baked in (ports 0
        // and 1 for spawns 0 and 1).
        let mut fighter = Fighter::new(kind, spawn_index as u8, 3);

        // Real constants if the pack has them: gravity 2.4 and terminal
        // velocity 44 rather than the 0.09 and 1.7 the first port guessed.
        let desc = pack.fighter(kind as u32);
        let from_pack = desc.is_some();
        let mut cam_offset_y = 0.0;
        let mut camera_zoom_frame = 1.0;
        let mut shadow_size = 200.0;
        if let Some(d) = desc {
            fighter.attributes = physics_of(&d);
            fighter.coll = body_of(&d);
            fighter.anim = anim_of(&d);
            cam_offset_y = d.cam_offset_y;
            camera_zoom_frame = d.camera_zoom;
            shadow_size = d.shadow_size;
        }

        let mut placed = false;
        if let Some(spawn) = pack.spawn(stage, spawn_index) {
            fighter.pos = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0);
            placed = ssb_game::collision::project_floor(
                FloorSegments::new(pack, stage),
                ssb_engine::math::Vec2::new(fighter.pos.x, fighter.pos.y),
            )
            .is_some();
        }
        FighterScene {
            fighter,
            placed,
            airborne_ticks: 0,
            jump_was_held: false,
            from_pack,
            skeleton: ssb_rom::skeleton::Skeleton::new(),
            // Which object a character's animations drive is stored per joint,
            // so any one of them names it; a scan over the objects finds which
            // one owns that node. Done once, here, rather than per tick.
            object: fighter_object(pack, kind as u32).unwrap_or(u32::MAX),
            // Matches real hardware's own `dGMCameraCObjVecDefault` rest
            // state (RE-131) rather than starting already converged on the
            // fighter -- the pan-in from that rest state as the match
            // begins is real, observable behaviour, not a startup glitch to
            // hide.
            camera: ssb_game::camera::Camera::default(),
            cam_offset_y,
            camera_zoom_frame,
            shadow_size,
            started: None,
            root_motion_before_tick: None,
        }
    }

    /// Advances fighter physics/collision and skeleton animation one tick,
    /// without touching the battle camera.
    ///
    /// `input` is the mapped N64 pad; `jump_held` is the jump button's state
    /// this frame, from which the tap and release edges are derived. The
    /// status machine wants edges rather than levels because a short hop is
    /// defined by the button coming back *up* inside the jumpsquat.
    ///
    /// Used directly by scenes with no camera of their own (e.g. a
    /// stationary training target); [`FighterScene::tick`] calls this then
    /// also advances the camera, for scenes that own one.
    pub fn tick_fighter(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        input: ssb_engine::input::ControllerState,
        jump_held: bool,
    ) {
        let tapped = jump_held && !self.jump_was_held;
        let released = !jump_held && self.jump_was_held;
        self.jump_was_held = jump_held;

        self.fighter.set_input(input, tapped, released);
        if matches!(
            self.fighter.status.status,
            AnyStatus::Mario(
                ssb_game::status::MarioStatus::SpecialN
                    | ssb_game::status::MarioStatus::SpecialAirN
            )
        ) {
            if let Some(anchor) = self.weapon_anchor(pack, 16, 0.0) {
                self.fighter.set_weapon_spawn_anchor(anchor);
            }
        }
        if matches!(
            self.fighter.status.status,
            AnyStatus::Fox(
                ssb_game::status::FoxStatus::SpecialN
                    | ssb_game::status::FoxStatus::SpecialAirN
            )
        ) {
            if let Some(anchor) = self.weapon_anchor(pack, 17, 60.0) {
                self.fighter.set_weapon_spawn_anchor(anchor);
            }
        }
        if matches!(
            self.fighter.status.status,
            AnyStatus::Mario(
                ssb_game::status::MarioStatus::SpecialHi
                    | ssb_game::status::MarioStatus::SpecialAirHi
            )
        ) {
            if let (Some(before), Some(current)) =
                (self.root_motion_before_tick, self.skeleton.pose(0))
            {
                self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                    delta: ssb_engine::math::Vec3::new(
                        current.translate[0] - before.translate[0],
                        current.translate[1] - before.translate[1],
                        current.translate[2] - before.translate[2],
                    ),
                    rotate_z: current.rotate[2],
                });
            }
        }
        self.fighter.tick(|| FloorSegments::new(pack, stage));
        if self.fighter.is_grounded() {
            self.airborne_ticks = 0;
        } else {
            self.airborne_ticks = self.airborne_ticks.saturating_add(1);
        }
        self.tick_animation(pack);
        // A held fighter hangs from this fighter's `joint_itemheavy_id`; the
        // match loop hands the sampled position over in `grab::exchange`.
        self.fighter.grab.anchor = if self.fighter.grab.catch.is_some() {
            ssb_game::grab::itemheavy_joint(self.fighter.kind)
                .and_then(|joint| self.fighter_part_anchor(pack, joint))
        } else {
            None
        };
    }

    /// Advances the battle camera one tick from the fighter's current
    /// position. `additional_camera_interest` is a second interest point
    /// (e.g. a training dummy) the camera should also try to frame.
    pub fn tick_camera(
        &mut self,
        stage: &StageDesc,
        additional_camera_interest: Option<ssb_game::camera::Interest>,
    ) {
        let (_, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        let bounds = ssb_game::camera::Bounds {
            top: stage.camera.top as f32,
            bottom: stage.camera.bottom as f32,
            left: stage.camera.left as f32,
            right: stage.camera.right as f32,
        };
        let mut target = self.fighter.pos;
        target.y += self.cam_offset_y;
        let primary = ssb_game::camera::Interest {
            target_pos: target,
            facing_left: matches!(self.fighter.facing, ssb_game::fighter::Facing::Left),
            zoom_frame: self.camera_zoom_frame,
            zoom_range: 1.0,
            idle_zoomed_out: self.fighter.status.status == Status::Wait
                && self.fighter.status.anim_frame >= 120.0,
        };
        let interests = [primary, additional_camera_interest.unwrap_or(primary)];
        let interest_count = if additional_camera_interest.is_some() {
            2
        } else {
            1
        };
        self.camera.tick_interests(
            &interests[..interest_count],
            bounds,
            stage.camera_light_angle_z,
            vw as f32 / vh as f32,
        );
    }

    /// Advances one tick against the stage: fighter physics/collision and
    /// skeleton animation ([`FighterScene::tick_fighter`]), then the battle
    /// camera ([`FighterScene::tick_camera`]). Convenience wrapper for
    /// scenes that own a camera; a scene that does not (e.g. a stationary
    /// training target) calls `tick_fighter` directly instead.
    pub fn tick(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        input: ssb_engine::input::ControllerState,
        jump_held: bool,
        additional_camera_interest: Option<ssb_game::camera::Interest>,
    ) {
        self.tick_fighter(pack, stage, input, jump_held);
        self.tick_camera(stage, additional_camera_interest);
    }

    /// Starts the animation the current status calls for, then advances it.
    fn tick_animation(&mut self, pack: &Pack<'_>) {
        if self.object == u32::MAX {
            return;
        }
        self.root_motion_before_tick = tick_skeleton_animation(
            pack,
            self.fighter.kind as u32,
            self.fighter.status.status,
            &mut self.skeleton,
            &mut self.started,
        );
    }

    /// Maps an authored weapon attachment joint and forward offset into
    /// match coordinates before the fighter's motion event consumes it.
    /// Mario's Fireball uses joint 16; Fox's Blaster uses joint 17 plus 60.
    fn weapon_anchor(
        &self,
        pack: &Pack<'_>,
        joint: usize,
        offset_x: f32,
    ) -> Option<ssb_engine::math::Vec3> {
        let node = self.skeleton.joint_node(joint)?;
        self.node_anchor(pack, node, offset_x)
    }

    /// `FTAttributes::joint_itemheavy_id` indexes `FTStruct::joints`, where
    /// TopN, TransN, XRotN and YRotN precede the model's DObjDesc array
    /// (`nFTPartsJointCommonStart == 4`, `ftmanager.c`). The packed object
    /// contains only that array. A part need not have an animation script, so
    /// look up its packed node directly instead of using `Skeleton::joint_node`.
    fn fighter_part_anchor(
        &self,
        pack: &Pack<'_>,
        part_id: usize,
    ) -> Option<ssb_engine::math::Vec3> {
        let object = pack.object(self.object)?;
        let local_index = part_id.checked_sub(4)? as u32;
        if local_index >= object.node_count {
            return None;
        }
        self.node_anchor(pack, object.first_node + local_index, 0.0)
    }

    fn node_anchor(
        &self,
        pack: &Pack<'_>,
        node: u32,
        offset_x: f32,
    ) -> Option<ssb_engine::math::Vec3> {
        let object = pack.object(self.object)?;
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = self.compose_model(pack, &object, &mut posed);
        let local_index = node.checked_sub(object.first_node)? as usize;
        if local_index >= count {
            return None;
        }
        let local = posed[local_index].translation();
        let scale = ssb_rom::pack::MODEL_SCALE;
        let facing = self.fighter.facing.sign();
        Some(ssb_engine::math::Vec3::new(
            self.fighter.pos.x + (local[2] * scale + offset_x) * facing,
            self.fighter.pos.y + local[1] * scale,
            self.fighter.pos.z - local[0] * scale * facing,
        ))
    }
}

/// Which way to turn a model so it faces the way the fighter does.
///
/// Fighter models are authored facing `+Z` — shoulders spanning X — while a
/// match runs along X, so every one of them is a quarter turn off (RE-038).
/// Feeds `Gpu::model_transform`'s `rot_radians` argument.
pub fn facing_turn(facing: ssb_game::fighter::Facing) -> f32 {
    match facing {
        ssb_game::fighter::Facing::Right => core::f32::consts::FRAC_PI_2,
        ssb_game::fighter::Facing::Left => -core::f32::consts::FRAC_PI_2,
    }
}
