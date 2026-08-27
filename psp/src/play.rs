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
    /// Whether the pack supplied this character's real constants. When false
    /// the fighter falls under [`PhysicsAttributes::MARIO`], and the overlay
    /// says so rather than letting a stale pack look like a physics bug.
    pub from_pack: bool,
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
            from_pack,
        }
    }

    /// Advances one tick against the stage.
    pub fn tick(&mut self, pack: &Pack<'_>, stage: &StageDesc) {
        self.fighter.tick(|| FloorSegments::new(pack, stage));
        if self.fighter.is_grounded() {
            self.airborne_ticks = 0;
        } else {
            self.airborne_ticks = self.airborne_ticks.saturating_add(1);
        }
    }

    /// The floor's material, or `None` while airborne.
    pub fn material(&self) -> Option<u16> {
        self.fighter.floor.map(|f| f.material())
    }
}
