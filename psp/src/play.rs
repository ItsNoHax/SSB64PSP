//! The gameplay slice: a fighter standing on a real stage, on device.
//!
//! Everything the simulation needs is in `ssb-game`, which knows nothing about
//! the PSP or the pack format. This module is the join: it walks the pack's
//! collision tables into the shape [`ssb_game::collision`] wants, ticks the
//! fighter, and draws where it ended up.
//!
//! ## Why the adapter is here and not in either crate
//!
//! `ssb-rom` must not know game logic and `ssb-game` must not know the pack
//! format, so [`FloorSegments`] is duplicated from `romtool`'s copy on purpose.
//! It is twenty lines, and the alternative is a shared type that drags one
//! crate into the other.
//!
//! [`FloorSegments`] allocates nothing: it reads segments straight out of the
//! mapped pack as the query asks for them. A tick's collision work is
//! therefore proportional to the stage's floor count with no per-frame setup,
//! which is what keeps this affordable inside the frame budget.

use ssb_game::collision::Segment;
use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::ground::BodyColl;
use ssb_game::physics::PhysicsAttributes;
use ssb_game::status::Status;
use ssb_game::status::AnimLengths;
use ssb_rom::pack::{line_kind, FighterDesc, LineDesc, Pack, StageDesc};

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
            let v = self
                .pack
                .coll_vertex(line.first_vertex + self.point as u32);
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
fn fighter_object(pack: &Pack<'_>, kind: u32) -> Option<u32> {
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
fn physics_of(d: &FighterDesc) -> PhysicsAttributes {
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
fn anim_of(d: &FighterDesc) -> AnimLengths {
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
fn body_of(d: &FighterDesc) -> BodyColl {
    BodyColl {
        top: d.coll_top,
        center: d.coll_center,
        bottom: d.coll_bottom,
        width: d.coll_width,
    }
}

/// The on-device gameplay slice.
///
/// One fighter, no opponent, no match rules — the point is that the ported
/// physics and the ported collision run together against real stage data at
/// 60 Hz, which is the thing neither host tests nor a static render can show.
pub struct Play {
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
    started: Option<Status>,
}

impl Play {
    /// Puts a fighter at a stage's first player spawn.
    ///
    /// Deliberately *not* settled onto the surface: the spawn sits a few units
    /// up (RE-030) and letting it fall that distance is the first thing worth
    /// watching. Returns a `Play` even when the stage has no spawn, so the
    /// overlay can say so rather than the view going blank.
    pub fn at_spawn(pack: &Pack<'_>, stage: &StageDesc) -> Play {
        let kind = FighterKind::Mario;
        let mut fighter = Fighter::new(kind, 0, 3);

        // Real constants if the pack has them: gravity 2.4 and terminal
        // velocity 44 rather than the 0.09 and 1.7 the first port guessed.
        let desc = pack.fighter(kind as u32);
        let from_pack = desc.is_some();
        if let Some(d) = desc {
            fighter.attributes = physics_of(&d);
            fighter.coll = body_of(&d);
            fighter.anim = anim_of(&d);
        }

        let mut placed = false;
        if let Some(spawn) = pack.spawn(stage, 0) {
            fighter.pos = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0);
            placed = ssb_game::collision::project_floor(
                FloorSegments::new(pack, stage),
                ssb_engine::math::Vec2::new(fighter.pos.x, fighter.pos.y),
            )
            .is_some();
        }
        Play {
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
            started: None,
        }
    }

    /// Advances one tick against the stage.
    ///
    /// `input` is the mapped N64 pad; `jump` is the jump button's state this
    /// frame, from which the tap and release edges are derived. The status
    /// machine wants edges rather than levels because a short hop is defined
    /// by the button coming back *up* inside the jumpsquat.
    pub fn tick(
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
        self.fighter.tick(|| FloorSegments::new(pack, stage));
        if self.fighter.is_grounded() {
            self.airborne_ticks = 0;
        } else {
            self.airborne_ticks = self.airborne_ticks.saturating_add(1);
        }
        self.tick_animation(pack);
    }

    /// Starts the animation the current status calls for, then advances it.
    ///
    /// Restarted on a status *change* rather than every tick: an animation
    /// carries its own clock, and re-seeding it each frame would freeze every
    /// fighter on frame zero. A looping one is left to loop; a finite one that
    /// has run out holds its last pose, which is what the original does when a
    /// status outlives its animation.
    fn tick_animation(&mut self, pack: &Pack<'_>) {
        if self.object == u32::MAX {
            return;
        }
        let status = self.fighter.status.status;
        let slot = status.anim_slot() as u32;
        if self.started != Some(status) {
            self.started = Some(status);
            if let Some(anim) = pack.fighter_anim(self.fighter.kind as u32, slot) {
                self.skeleton
                    .start(pack, &anim, 0.0, status.anim_speed());
            }
        }
        // The slot is read back rather than remembered, so a status whose
        // animation the pack lacks -- Kirby has no aerial jump -- simply keeps
        // the pose it had.
        if let Some(anim) = pack.fighter_anim(self.fighter.kind as u32, slot) {
            if let Some(script) = pack.anim_script(&anim) {
                let _ = self.skeleton.tick(script);
            }
        }
    }

    /// The status the fighter is in, as a fixed-width label for the overlay.
    pub fn status_name(&self) -> &'static str {
        use ssb_game::status::Status::*;
        match self.fighter.status.status {
            Wait => "wait    ",
            WalkSlow => "walk-slw",
            WalkMiddle => "walk-mid",
            WalkFast => "walk-fst",
            Dash => "dash    ",
            Run => "run     ",
            RunBrake => "brake   ",
            Turn => "turn    ",
            KneeBend => "jumpsqt ",
            JumpF => "jump-f  ",
            JumpB => "jump-b  ",
            JumpAerialF => "djump-f ",
            JumpAerialB => "djump-b ",
            Fall => "fall    ",
            FallAerial => "fall-a  ",
            Squat => "squat   ",
            SquatWait => "squat-w ",
            LandingLight => "land    ",
            LandingHeavy => "land-hvy",
            Pass => "pass    ",
        }
    }

    /// The floor's material, or `None` while airborne.
    pub fn material(&self) -> Option<u16> {
        self.fighter.floor.map(|f| f.material())
    }
}
