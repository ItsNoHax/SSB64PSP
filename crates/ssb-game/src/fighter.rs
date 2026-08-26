//! Fighter identity and per-fighter state.
//!
//! The roster ordering is `enum FTKind` from `src/ft/ftdef.h`. Preserving the
//! exact ordinals matters: extracted asset tables are indexed by fighter kind,
//! so renumbering would silently mis-associate every character's data.

use ssb_engine::input::ControllerState;
use ssb_engine::math::Vec3;

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
}
