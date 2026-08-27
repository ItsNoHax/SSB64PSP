//! Fighter identity and per-fighter state.
//!
//! The roster ordering is `enum FTKind` from `src/ft/ftdef.h`. Preserving the
//! exact ordinals matters: extracted asset tables are indexed by fighter kind,
//! so renumbering would silently mis-associate every character's data.

use ssb_engine::input::ControllerState;
use ssb_engine::math::Vec3;

use crate::collision::{self, Segment};
use crate::ground::{self, BodyColl, Standing};
use crate::physics::{PhysicsAttributes, PhysicsState};

/// Fighter identity, matching `FTKind` ordinals exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FighterKind {
    // Playable roster, ordinals 0..=11.
    Mario = 0,
    Fox = 1,
    Donkey = 2,
    Samus = 3,
    Luigi = 4,
    Link = 5,
    Yoshi = 6,
    Captain = 7,
    Kirby = 8,
    Pikachu = 9,
    /// Jigglypuff. The original uses its Japanese name, Purin.
    Purin = 10,
    Ness = 11,

    /// Master Hand.
    Boss = 12,
    /// Metal Mario.
    MetalMario = 13,

    // The Fighting Polygon Team, ordinals 14..=25.
    PolyMario = 14,
    PolyFox = 15,
    PolyDonkey = 16,
    PolySamus = 17,
    PolyLuigi = 18,
    PolyLink = 19,
    PolyYoshi = 20,
    PolyCaptain = 21,
    PolyKirby = 22,
    PolyPikachu = 23,
    PolyPurin = 24,
    PolyNess = 25,

    /// Giant Donkey Kong.
    GiantDonkey = 26,
}

impl FighterKind {
    /// The 12 selectable characters, in select-screen order.
    pub const PLAYABLE: &'static [FighterKind] = &[
        FighterKind::Mario,
        FighterKind::Fox,
        FighterKind::Donkey,
        FighterKind::Samus,
        FighterKind::Luigi,
        FighterKind::Link,
        FighterKind::Yoshi,
        FighterKind::Captain,
        FighterKind::Kirby,
        FighterKind::Pikachu,
        FighterKind::Purin,
        FighterKind::Ness,
    ];

    /// Characters locked until unlocked through 1P mode.
    pub const UNLOCKABLE: &'static [FighterKind] = &[
        FighterKind::Luigi,
        FighterKind::Captain,
        FighterKind::Ness,
        FighterKind::Purin,
    ];

    pub fn is_playable(self) -> bool {
        (self as u8) <= (FighterKind::Ness as u8)
    }

    pub fn is_polygon(self) -> bool {
        (FighterKind::PolyMario as u8..=FighterKind::PolyNess as u8).contains(&(self as u8))
    }

    /// The character a polygon fighter is modelled on, if any.
    ///
    /// The polygon team reuses the base characters' movesets, so their logic
    /// dispatches through the original.
    pub fn polygon_base(self) -> Option<FighterKind> {
        if !self.is_polygon() {
            return None;
        }
        FighterKind::PLAYABLE
            .get((self as u8 - FighterKind::PolyMario as u8) as usize)
            .copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            FighterKind::Mario => "Mario",
            FighterKind::Fox => "Fox",
            FighterKind::Donkey => "Donkey Kong",
            FighterKind::Samus => "Samus",
            FighterKind::Luigi => "Luigi",
            FighterKind::Link => "Link",
            FighterKind::Yoshi => "Yoshi",
            FighterKind::Captain => "Captain Falcon",
            FighterKind::Kirby => "Kirby",
            FighterKind::Pikachu => "Pikachu",
            FighterKind::Purin => "Jigglypuff",
            FighterKind::Ness => "Ness",
            FighterKind::Boss => "Master Hand",
            FighterKind::MetalMario => "Metal Mario",
            FighterKind::GiantDonkey => "Giant Donkey Kong",
            FighterKind::PolyMario => "Polygon Mario",
            FighterKind::PolyFox => "Polygon Fox",
            FighterKind::PolyDonkey => "Polygon Donkey Kong",
            FighterKind::PolySamus => "Polygon Samus",
            FighterKind::PolyLuigi => "Polygon Luigi",
            FighterKind::PolyLink => "Polygon Link",
            FighterKind::PolyYoshi => "Polygon Yoshi",
            FighterKind::PolyCaptain => "Polygon Captain Falcon",
            FighterKind::PolyKirby => "Polygon Kirby",
            FighterKind::PolyPikachu => "Polygon Pikachu",
            FighterKind::PolyPurin => "Polygon Jigglypuff",
            FighterKind::PolyNess => "Polygon Ness",
        }
    }
}

/// Which way a fighter faces. Stored as a float multiplier because the
/// original uses `lr` as a direct sign on X velocities and offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Facing {
    Left,
    #[default]
    Right,
}

impl Facing {
    pub fn sign(self) -> f32 {
        match self {
            Facing::Left => -1.0,
            Facing::Right => 1.0,
        }
    }

    pub fn flipped(self) -> Facing {
        match self {
            Facing::Left => Facing::Right,
            Facing::Right => Facing::Left,
        }
    }
}

/// Whether a fighter is standing on something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Situation {
    Ground,
    #[default]
    Air,
}

/// Per-fighter runtime state.
///
/// A pared-down `FTStruct`. The original is ~0x18 KB per fighter and carries
/// every subsystem's working data; this grows as subsystems are ported.
#[derive(Debug, Clone, PartialEq)]
pub struct Fighter {
    pub kind: FighterKind,
    /// Player slot, 0..=3.
    pub port: u8,
    pub pos: Vec3,
    pub facing: Facing,
    pub situation: Situation,
    pub physics: PhysicsState,
    pub attributes: PhysicsAttributes,
    /// Damage percentage. The original stores this as an integer.
    pub damage: u16,
    pub stocks: i8,
    /// Frames of hitlag remaining; the fighter is frozen while nonzero.
    pub hitlag: u16,
    /// Frames of hitstun remaining.
    pub hitstun: u16,
    pub input: ControllerState,
    pub prev_input: ControllerState,
    /// Collision offsets — `MPObjectColl`.
    pub coll: BodyColl,
    /// The floor being stood on, or `None` while airborne.
    pub floor: Option<Standing>,
    /// A drop-through platform being fallen past — `ignore_line_id`.
    pub ignore_line: Option<u16>,
}

impl Fighter {
    pub fn new(kind: FighterKind, port: u8, stocks: i8) -> Self {
        Fighter {
            kind,
            port,
            pos: Vec3::ZERO,
            facing: Facing::Right,
            situation: Situation::Air,
            physics: PhysicsState::default(),
            attributes: PhysicsAttributes::default(),
            damage: 0,
            stocks,
            hitlag: 0,
            hitstun: 0,
            input: ControllerState::default(),
            prev_input: ControllerState::default(),
            coll: BodyColl::default(),
            floor: None,
            ignore_line: None,
        }
    }

    pub fn is_grounded(&self) -> bool {
        self.situation == Situation::Ground
    }

    /// Whether the fighter is frozen by hitlag.
    ///
    /// Hitlag freezes *both* fighters in an exchange for the same number of
    /// frames, which is what gives Smash's hits their weight. A frozen fighter
    /// still reads input (for directional influence) but does not move.
    pub fn is_in_hitlag(&self) -> bool {
        self.hitlag > 0
    }

    /// Advances the per-frame timers. Returns whether hitlag ended this frame.
    pub fn tick_timers(&mut self) -> bool {
        if self.hitlag > 0 {
            self.hitlag -= 1;
            return self.hitlag == 0;
        }
        self.hitstun = self.hitstun.saturating_sub(1);
        false
    }

    /// Applies this frame's velocity to position, respecting hitlag.
    pub fn integrate(&mut self) {
        if self.is_in_hitlag() {
            return;
        }
        let v = crate::physics::total_velocity(&self.physics, self.is_grounded());
        self.pos += Vec3::new(v.x, v.y, crate::physics::clamp_z_velocity(self.pos.z, v.z));
    }

    /// Leaves the ground, carrying momentum into the air vector.
    pub fn become_airborne(&mut self) {
        if self.situation == Situation::Air {
            return;
        }
        self.situation = Situation::Air;
        crate::physics::transfer_ground_to_air(&mut self.physics);
    }

    /// Lands, clearing air state.
    pub fn land(&mut self, floor_y: f32) {
        self.situation = Situation::Ground;
        self.pos.y = floor_y;
        self.physics.vel_ground.x = self.physics.vel_air.x;
        self.physics.vel_air = Vec3::ZERO;
        self.physics.is_fastfall = false;
        self.physics.jumps_used = 0;
    }

    /// Places the fighter on the stage beneath it, as a match start does.
    ///
    /// A spawn point sits a little above its surface (RE-030), so a fighter
    /// put there is airborne by a few units. Returns whether there was
    /// anything below to stand on.
    pub fn place_on_stage<I>(&mut self, floors: I) -> bool
    where
        I: IntoIterator<Item = (u16, Segment)>,
    {
        match ground::settle(&self.coll, self.pos, floors) {
            Some((pos, floor, _)) => {
                self.pos = pos;
                self.floor = Some(floor);
                self.situation = Situation::Ground;
                self.physics = PhysicsState::default();
                true
            }
            None => false,
        }
    }

    /// Advances one tick against a stage.
    ///
    /// The order is the original's: timers, then the frame's physics, then the
    /// move is handed to [`ground`] to be tested against the stage. Velocity is
    /// never applied to position directly here — [`ground::move_air`] owns that,
    /// because it is the only thing that can subdivide the movement.
    ///
    /// `floors` is called more than once per tick and must yield every floor
    /// segment worth testing. Passing a closure rather than an iterator is what
    /// keeps this allocation-free on the PSP.
    pub fn tick<I, F>(&mut self, floors: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = (u16, Segment)>,
    {
        self.tick_timers();
        if self.is_in_hitlag() {
            return;
        }

        match self.situation {
            Situation::Ground => self.tick_ground(floors),
            Situation::Air => self.tick_air(floors),
        }
    }

    fn tick_ground<I, F>(&mut self, floors: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = (u16, Segment)>,
    {
        let Some(standing) = self.floor else {
            // Grounded with no floor recorded is not a state the original can
            // reach; treat it as airborne rather than guessing a surface.
            self.become_airborne();
            return;
        };

        // The surface's material scales traction, which is the only thing a
        // floor's material does — `ftPhysicsApplyGroundVelFriction`.
        let friction = collision::material_friction(standing.flags);
        crate::physics::apply_ground_friction_with_material(
            &mut self.physics,
            &self.attributes,
            friction,
        );

        let want = Vec3::new(
            self.pos.x + self.physics.vel_ground.x + self.physics.vel_knockback.x,
            self.pos.y,
            self.pos.z,
        );
        let moved = ground::move_ground(&self.coll, want, standing.line, floors);
        self.pos = moved.pos;

        match moved.floor {
            Some(f) => self.floor = Some(f),
            // Walked off the end of the line.
            None => {
                self.floor = None;
                self.become_airborne();
            }
        }
    }

    fn tick_air<I, F>(&mut self, floors: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = (u16, Segment)>,
    {
        if self.physics.is_fastfall {
            crate::physics::apply_fast_fall(&mut self.physics, &self.attributes);
        } else {
            crate::physics::apply_gravity_default(&mut self.physics, &self.attributes);
        }
        crate::physics::apply_air_drift(&mut self.physics, &self.attributes, self.input.stick_x);

        let v = crate::physics::total_velocity(&self.physics, false);
        let want = Vec3::new(
            self.pos.x + v.x,
            self.pos.y + v.y,
            self.pos.z + crate::physics::clamp_z_velocity(self.pos.z, v.z),
        );

        let moved = ground::move_air(&self.coll, self.pos, want, self.ignore_line, floors);
        self.pos.x = moved.pos.x;
        self.pos.z = moved.pos.z;

        match moved.floor {
            Some(f) => {
                self.floor = Some(f);
                self.ignore_line = None;
                self.land(moved.pos.y);
            }
            None => {
                self.pos.y = moved.pos.y;
                self.floor = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_ordinals_match_the_original_enum() {
        assert_eq!(FighterKind::Mario as u8, 0);
        assert_eq!(FighterKind::Ness as u8, 11);
        assert_eq!(FighterKind::Boss as u8, 12);
        assert_eq!(FighterKind::PolyMario as u8, 14);
        assert_eq!(FighterKind::GiantDonkey as u8, 26);
    }

    #[test]
    fn twelve_playable_characters() {
        assert_eq!(FighterKind::PLAYABLE.len(), 12);
        for f in FighterKind::PLAYABLE {
            assert!(f.is_playable(), "{}", f.name());
            assert!(!f.is_polygon());
        }
    }

    #[test]
    fn polygon_fighters_map_back_to_their_base_character() {
        assert_eq!(
            FighterKind::PolyMario.polygon_base(),
            Some(FighterKind::Mario)
        );
        assert_eq!(
            FighterKind::PolyNess.polygon_base(),
            Some(FighterKind::Ness)
        );
        assert_eq!(FighterKind::Mario.polygon_base(), None);
        assert_eq!(FighterKind::Boss.polygon_base(), None);
    }

    #[test]
    fn every_fighter_has_a_distinct_name() {
        let all = FighterKind::PLAYABLE;
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.name(), b.name());
            }
        }
    }

    #[test]
    fn facing_sign_flips() {
        assert_eq!(Facing::Right.sign(), 1.0);
        assert_eq!(Facing::Left.sign(), -1.0);
        assert_eq!(Facing::Right.flipped(), Facing::Left);
    }

    #[test]
    fn hitlag_freezes_movement_entirely() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.physics.vel_air = Vec3::new(5.0, -5.0, 0.0);
        f.hitlag = 3;

        let before = f.pos;
        f.integrate();
        assert_eq!(f.pos, before, "hitlag must freeze position");

        // Tick it out, then movement resumes.
        for _ in 0..3 {
            f.tick_timers();
        }
        assert!(!f.is_in_hitlag());
        f.integrate();
        assert_eq!(f.pos, Vec3::new(5.0, -5.0, 0.0));
    }

    #[test]
    fn tick_timers_reports_the_frame_hitlag_ends() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.hitlag = 2;
        assert!(!f.tick_timers());
        assert!(f.tick_timers(), "should report the ending frame");
        assert!(!f.tick_timers());
    }

    #[test]
    fn hitstun_only_counts_down_outside_hitlag() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.hitlag = 2;
        f.hitstun = 10;
        f.tick_timers();
        assert_eq!(f.hitstun, 10, "hitstun is paused during hitlag");
        f.tick_timers();
        f.tick_timers();
        assert_eq!(f.hitstun, 9);
    }

    #[test]
    fn landing_transfers_air_momentum_and_resets_jumps() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.physics.vel_air = Vec3::new(1.2, -3.0, 0.0);
        f.physics.jumps_used = 2;
        f.physics.is_fastfall = true;

        f.land(10.0);

        assert!(f.is_grounded());
        assert_eq!(f.pos.y, 10.0);
        assert_eq!(f.physics.vel_ground.x, 1.2);
        assert_eq!(f.physics.vel_air, Vec3::ZERO);
        assert_eq!(f.physics.jumps_used, 0);
        assert!(!f.physics.is_fastfall);
    }

    #[test]
    fn depth_axis_stays_within_bounds_under_sustained_push() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.physics.vel_air.z = 20.0;
        for _ in 0..20 {
            f.integrate();
        }
        assert_eq!(f.pos.z, crate::physics::Z_LIMIT);
    }

    /// Dream Land's main platform, as it really is in the pack: level at
    /// y 0 from x -2318 to 2318, ledge-grabbable and solid.
    fn dream_land() -> [(u16, Segment); 1] {
        [(
            0,
            Segment {
                x1: -2318,
                y1: 0,
                x2: 2318,
                y2: 0,
                flags: collision::flags::CLIFF,
            },
        )]
    }

    #[test]
    fn a_spawn_is_placed_on_the_stage_below_it() {
        // Spawns sit a few units above their surface (RE-030).
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(-1000.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        assert_eq!(f.pos.y, 0.0);
        assert!(f.is_grounded());
        assert!(f.floor.expect("a floor").grabbable());
    }

    #[test]
    fn a_spawn_over_nothing_is_reported_rather_than_guessed() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(9000.0, 4.0, 0.0);
        assert!(!f.place_on_stage(dream_land()));
        assert!(!f.is_grounded());
    }

    #[test]
    fn a_fighter_dropped_onto_a_stage_lands_and_stays_put() {
        // The property the whole vertical slice rests on. Falling for a second
        // and then standing for a second must end exactly on the surface, with
        // no drift and no re-landing.
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(0.0, 100.0, 0.0);

        let mut landed_on = None;
        for tick in 0..120 {
            f.tick(dream_land);
            if f.is_grounded() && landed_on.is_none() {
                landed_on = Some(tick);
            }
        }
        assert!(landed_on.is_some(), "never reached the stage");
        assert_eq!(f.pos.y, 0.0, "settled exactly on the surface");
        assert_eq!(f.physics.vel_air, Vec3::ZERO);
        assert!(f.is_grounded(), "did not bounce back off");
    }

    #[test]
    fn a_grounded_fighter_does_not_fall_through_its_own_floor() {
        // Gravity must not be applied while grounded: this is the failure that
        // would look like a fighter slowly sinking into the stage.
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(500.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        for _ in 0..600 {
            f.tick(dream_land);
        }
        assert_eq!(f.pos, Vec3::new(500.0, 0.0, 0.0));
    }

    #[test]
    fn sliding_off_the_edge_leaves_the_ground() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(2300.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        // A push toward the ledge, faster than friction can stop.
        f.physics.vel_ground.x = 30.0;
        for _ in 0..30 {
            f.tick(dream_land);
        }
        assert!(!f.is_grounded(), "walked off Dream Land's right ledge");
        assert!(f.floor.is_none());
        assert!(f.pos.y < 0.0, "and started falling");
    }

    #[test]
    fn hitlag_freezes_a_fighter_in_mid_air() {
        // A fighter frozen by a hit must not fall during the freeze; that is
        // what gives a hit its weight.
        //
        // `ftMain` decrements the counter and *then* gates physics on it
        // reaching zero, so the tick that ends hitlag already moves: ten
        // frames of hitlag freeze nine.
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(0.0, 500.0, 0.0);
        f.hitlag = 10;
        let held = f.pos;
        for _ in 0..9 {
            f.tick(dream_land);
        }
        assert_eq!(f.pos, held);
        f.tick(dream_land);
        assert!(f.pos.y < held.y, "and resumes falling on the tick it ends");
    }

    #[test]
    fn ice_lets_a_fighter_slide_further_than_common_ground() {
        // The only thing a floor's material does is scale traction.
        let slide = |material: u16| {
            let seg = [(
                0,
                Segment {
                    x1: -2318,
                    y1: 0,
                    x2: 2318,
                    y2: 0,
                    flags: material,
                },
            )];
            let mut f = Fighter::new(FighterKind::Mario, 0, 3);
            f.pos = Vec3::new(0.0, 4.0, 0.0);
            f.place_on_stage(seg);
            f.physics.vel_ground.x = 10.0;
            for _ in 0..60 {
                f.tick(|| seg);
            }
            f.pos.x
        };
        // Material 3 has a quarter of the common material's friction.
        assert!(
            slide(3) > slide(0),
            "material 3 is the slippery one in dMPCollisionMaterialFrictions"
        );
    }
}
