//! Fighter identity and per-fighter state.
//!
//! The roster ordering is `enum FTKind` from `src/ft/ftdef.h`. Preserving the
//! exact ordinals matters: extracted asset tables are indexed by fighter kind,
//! so renumbering would silently mis-associate every character's data.

use ssb_engine::input::ControllerState;
use ssb_engine::math::Vec3;

use crate::collision::{self, Segment};
use crate::ground::{self, BodyColl, Standing};
use crate::physics::{PhysicsAttributes, PhysicsState, RootMotion};

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

/// A sampled fighter joint in match coordinates. The three columns carry the
/// joint's rotation and scale; `origin` is its world position. The runtime
/// builds this from the current skeleton, while gameplay remains pack-free.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointTransform {
    pub axes: [Vec3; 3],
    pub origin: Vec3,
}

impl JointTransform {
    pub fn point(self, offset: Vec3) -> Vec3 {
        self.origin + self.axes[0] * offset.x + self.axes[1] * offset.y + self.axes[2] * offset.z
    }
}

pub const FIGHTER_JOINTS: usize = 40;

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
    /// Frames of hit-invincibility remaining — `ftParamSetTimedHitStatusInvincible`,
    /// set after a rebirth. A hit landing while this is nonzero is a no-op
    /// (`crate::attack`'s `apply_hit_from`/`apply_shield_hit` both check it).
    pub invincible_frames: u16,
    /// Source display gate (`FTStruct::is_invisible`). This is intentionally
    /// distinct from invincibility: `ftShadowProcDisplay` hides only the
    /// former.
    pub is_invisible: bool,
    /// Source shadow-specific display gate (`FTStruct::is_shadow_hide`).
    /// Common lifecycle transitions maintain it; capture/other visual systems
    /// can also set it without acquiring renderer knowledge.
    pub is_shadow_hidden: bool,
    pub input: ControllerState,
    pub prev_input: ControllerState,
    /// Collision offsets — `MPObjectColl`.
    pub coll: BodyColl,
    /// The floor being stood on, or `None` while airborne.
    pub floor: Option<Standing>,
    /// A drop-through platform being fallen past — `ignore_line_id`.
    pub ignore_line: Option<u16>,
    /// How long the statuses that end on their own animation last. Read from
    /// the animation files rather than `FTAttributes`, so it travels beside
    /// `attributes` rather than inside it.
    pub anim: crate::status::AnimLengths,
    /// Which status the fighter is in, and its working state.
    pub status: crate::status::StatusState,
    /// The derived stick state the status machine reads — tap counters and
    /// all. Kept beside `input` rather than inside it because `ControllerState`
    /// is the raw pad and this is what `ftMainProcUpdateInterrupt` makes of it.
    pub stick: crate::status::StickState,
    /// Shield health/decay/release-lag state — `crate::status::GuardState`.
    pub guard: crate::status::GuardState,
    /// Ledge-hang working state — `crate::status::CliffState`.
    pub cliff: crate::status::CliffState,
    /// Frames before this fighter can grab a ledge again —
    /// `FTStruct::cliffcatch_wait`, set after letting go or falling from one.
    pub cliffcatch_wait: u16,
    /// Jab-combo follow-up window — `crate::status::Attack1State`.
    pub attack1: crate::status::Attack1State,
    /// Shared "helpless fall" working state, used by `Status::FallSpecial`
    /// — `crate::status::FallSpecialState`.
    pub fall_special: crate::status::FallSpecialState,
    /// Per-use flags from Mario's Super Jump Punch motion script.
    pub mario_special_hi: crate::status::MarioSpecialHiState,
    /// Per-use flags from Mario's Tornado motion script.
    pub mario_special_lw: crate::status::MarioSpecialLwState,
    /// Per-use flags from Mario's Fireball motion script.
    pub mario_special_n: crate::status::MarioSpecialNState,
    /// One-shot Blaster motion-event state.
    pub fox_special_n: crate::status::FoxSpecialNState,
    /// Fire Fox startup, charge, and travel counters.
    pub fox_special_hi: crate::status::FoxSpecialHiState,
    pub fox_special_lw: crate::status::FoxSpecialLwState,
    pub donkey_special_n: crate::status::DonkeySpecialNState,
    pub donkey_special_lw: crate::status::DonkeySpecialLwState,
    /// Charge level, recoil and special-move flags — `crate::samus`.
    pub samus: crate::samus::SamusState,
    /// Boomerang ownership, Spin Attack weapon and rapid-jab state —
    /// `crate::link`.
    pub link: crate::link::LinkState,
    /// Egg Throw and aerial-jump state — `crate::yoshi`.
    pub yoshi: crate::yoshi::YoshiState,
    /// Falcon Punch, Kick and Dive state.
    pub captain: crate::captain::CaptainState,
    /// This fighter's side of an Egg Lay — `crate::capture_yoshi`.
    pub egg: crate::capture_yoshi::CaptureYoshiState,
    /// `FTStruct::knockback_resist_status`: knockback a hit loses before it
    /// applies. `set_any_status` clears it; Yoshi's aerial jump sets it.
    pub knockback_resist: f32,
    /// `FTStruct::is_special_interrupt`: whether a returning Boomerang may
    /// put this fighter into its catch status. `set_any_status` clears it;
    /// Wait, the slow walks, Squat, KneeBend, the jumps, the falls and Link's
    /// empty-handed Boomerang statuses set it again.
    pub is_special_interrupt: bool,
    /// Grab, capture and throw link to another fighter — `crate::grab`.
    pub grab: crate::grab::GrabState,
    /// `motion_attack_id` / `motion_count` ([`crate::stale`]).
    pub motion: crate::stale::MotionId,
    /// This player's `stale_info` queue ([`crate::stale`]).
    pub stale: crate::stale::StaleQueue,
    /// `FTStruct::handicap`, an index into `dFTCommonDataHandicapTable`.
    pub handicap: u8,
    /// `FTStruct::costume` ([`crate::costume`]); display only.
    pub costume: u8,
    /// One weapon creation requested by this fighter's current status. The
    /// match-owned weapon pool consumes it after fighter callbacks finish.
    pub weapon_spawn: Option<crate::weapon::WeaponSpawn>,
    /// This frame's runtime-sampled world position for a fighter motion's
    /// weapon attachment joint. `ssb-game` deliberately stores plain data,
    /// never a skeleton or pack handle; callers without a renderer fall back
    /// to the fighter root when it is absent.
    pub weapon_spawn_anchor: Option<Vec3>,
    /// Current posed joints, indexed as `FTStruct::joints` (four runtime
    /// joints precede the packed model nodes).
    pub joint_transforms: [Option<JointTransform>; FIGHTER_JOINTS],
    /// This tick's runtime-sampled TransN motion. It is data, not a renderer
    /// handle, so host gameplay tests can provide it directly and `ssb-game`
    /// remains runtime-independent.
    pub root_motion: RootMotion,
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
            invincible_frames: 0,
            is_invisible: false,
            is_shadow_hidden: false,
            input: ControllerState::default(),
            prev_input: ControllerState::default(),
            coll: BodyColl::default(),
            floor: None,
            ignore_line: None,
            anim: crate::status::AnimLengths::default(),
            status: crate::status::StatusState::default(),
            stick: crate::status::StickState::new(),
            guard: crate::status::GuardState::default(),
            cliff: crate::status::CliffState::default(),
            cliffcatch_wait: 0,
            attack1: crate::status::Attack1State::default(),
            fall_special: crate::status::FallSpecialState::default(),
            mario_special_hi: crate::status::MarioSpecialHiState::default(),
            mario_special_lw: crate::status::MarioSpecialLwState::default(),
            mario_special_n: crate::status::MarioSpecialNState::default(),
            fox_special_n: crate::status::FoxSpecialNState::default(),
            fox_special_hi: crate::status::FoxSpecialHiState::default(),
            fox_special_lw: crate::status::FoxSpecialLwState::default(),
            donkey_special_n: crate::status::DonkeySpecialNState::default(),
            donkey_special_lw: crate::status::DonkeySpecialLwState::default(),
            samus: crate::samus::SamusState::default(),
            link: crate::link::LinkState::default(),
            yoshi: crate::yoshi::YoshiState::default(),
            captain: crate::captain::CaptainState::default(),
            egg: crate::capture_yoshi::CaptureYoshiState::default(),
            knockback_resist: 0.0,
            is_special_interrupt: false,
            grab: crate::grab::GrabState::default(),
            motion: crate::stale::MotionId::default(),
            stale: crate::stale::StaleQueue::default(),
            handicap: crate::stale::HANDICAP_DEFAULT,
            costume: 0,
            weapon_spawn: None,
            weapon_spawn_anchor: None,
            joint_transforms: [None; FIGHTER_JOINTS],
            root_motion: RootMotion::default(),
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
    ///
    /// The original bleeds `vel_knockback` off by friction every frame of
    /// hitstun (`ftPhysicsSetGroundVelFriction`-shaped decay). No such curve
    /// exists yet (`crate::attack`'s module docs), so knockback here is held
    /// constant through hitstun and snapped to zero the instant it ends,
    /// rather than decaying gradually — a fighter still slides at the hit's
    /// full speed on the last hitstun frame and stops dead on the next.
    pub fn tick_timers(&mut self) -> bool {
        if self.hitlag > 0 {
            self.hitlag -= 1;
            return self.hitlag == 0;
        }
        if self.hitstun > 0 {
            self.hitstun -= 1;
            if self.hitstun == 0 {
                self.physics.vel_knockback = Vec3::ZERO;
            }
        }
        if self.invincible_frames > 0 {
            self.invincible_frames -= 1;
        }
        if self.cliffcatch_wait > 0 {
            self.cliffcatch_wait -= 1;
        }
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
    ///
    /// The air-to-ground velocity transfer is guarded on actually being
    /// airborne, mirroring [`Fighter::become_airborne`]. Without the guard a
    /// caller that has already entered a grounded status — which does the
    /// transfer itself — would run it a second time against an already-zeroed
    /// `vel_air` and silently delete the fighter's horizontal momentum. That
    /// is not a crash; it is a fighter that lands from a running jump and
    /// stops dead, which reads as a physics problem rather than an ordering
    /// one.
    pub fn land(&mut self, floor_y: f32) {
        if self.situation == Situation::Air {
            self.physics.vel_ground.x = self.physics.vel_air.x;
            self.physics.vel_air = Vec3::ZERO;
            self.physics.is_fastfall = false;
        }
        self.situation = Situation::Ground;
        self.pos.y = floor_y;
        self.physics.jumps_used = 0;
        // `mpCommonSetFighterGround`'s Samus case.
        self.samus.charge_recoil = 0;
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

    /// Feeds one frame of controller input in — `ftMainProcUpdateInterrupt`.
    ///
    /// Call before [`Fighter::tick`]. This is separate because the derived
    /// stick state has to be advanced exactly once per frame whether or not
    /// the fighter is frozen: the original updates it before the hitlag check,
    /// which is how a frozen fighter can still buffer a direction.
    ///
    /// A human player's R trigger is also A + Z (`ftMainProcUpdateInterrupt`,
    /// `ftmain.c`: `if (button_hold & R_TRIG) button_hold |= (A_BUTTON |
    /// Z_TRIG)`), which is how R shields and grabs. The expanded hold is what
    /// taps and releases are derived from, so `prev_input` carries it too.
    pub fn set_input(&mut self, input: ControllerState, jump_tapped: bool, jump_released: bool) {
        let mut input = input;
        if input.buttons.contains(ssb_engine::input::N64Buttons::R) {
            input.buttons.0 |= ssb_engine::input::N64Buttons::A | ssb_engine::input::N64Buttons::Z;
        }
        self.prev_input = self.input;
        self.input = input;
        self.stick
            .step(input.stick_x, input.stick_y, jump_tapped, jump_released);
    }

    /// Supplies the TransN displacement sampled by the outer animation
    /// runtime for the frame about to run. A caller that has no skeleton (host
    /// tests, for example) may leave this at its default zero motion.
    pub fn set_root_motion(&mut self, motion: RootMotion) {
        self.root_motion = motion;
    }

    /// Supplies a motion attachment sampled by the outer animation runtime
    /// for the frame about to run. This is consumed only by a status event
    /// that needs it, then cleared with the other per-frame runtime input.
    pub fn set_weapon_spawn_anchor(&mut self, anchor: Vec3) {
        self.weapon_spawn_anchor = Some(anchor);
    }

    /// World position of a motion collision offset. Host-only callers with
    /// no skeleton use TopN's facing transform for joint 0 and the previous
    /// root-offset fallback for non-root joints.
    pub fn joint_world(&self, joint: u8, offset: Vec3) -> Vec3 {
        if let Some(transform) = self.joint_transforms.get(joint as usize).copied().flatten() {
            transform.point(offset)
        } else if joint == 0 {
            self.pos
                + Vec3::new(
                    offset.z * self.facing.sign(),
                    offset.y,
                    -offset.x * self.facing.sign(),
                )
        } else {
            self.pos + Vec3::new(offset.x * self.facing.sign(), offset.y, offset.z)
        }
    }

    /// Takes the one weapon creation emitted by this fighter's motion script.
    /// The request is one-shot until a later animation event queues another.
    pub fn take_weapon_spawn(&mut self) -> Option<crate::weapon::WeaponSpawn> {
        self.weapon_spawn.take()
    }

    /// Advances one tick against a stage.
    ///
    /// The order is the original's, and it is the order the four per-status
    /// callbacks run in: timers, then the status's own update and interrupt
    /// (which may replace the status outright), then the physics of whatever
    /// status is now current, then the move is handed to [`ground`] to be
    /// tested against the stage. Velocity is never applied to position
    /// directly here — [`ground::move_air`] owns that, because it is the only
    /// thing that can subdivide the movement.
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

        // `proc_update` + `proc_interrupt`. This can move the fighter between
        // ground and air (a jumpsquat ending, a platform drop), so the
        // situation is re-read afterwards rather than captured before.
        crate::status::update(self);

        // `ftCommonYoshiEggProcPhysics`'s own half, ahead of the common
        // physics it ends with.
        if self.status.status == crate::status::Status::YoshiEgg {
            crate::capture_yoshi::physics(self);
        }

        // A held fighter's position is its catcher's hand, not the result
        // of its own velocity (`ftCommonCapturePulledProcPhysics`).
        if crate::grab::tick_held(self, &floors) {
            self.root_motion = RootMotion::default();
            self.weapon_spawn_anchor = None;
            return;
        }
        match self.situation {
            Situation::Ground => self.tick_ground(floors),
            Situation::Air => self.tick_air(floors),
        }
        // Root motion is an input sample, not persistent fighter state. This
        // prevents a missed runtime sample from replaying an old displacement.
        self.root_motion = RootMotion::default();
        self.weapon_spawn_anchor = None;
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

        // `proc_physics`: each grounded status drives the along-floor velocity
        // its own way — a walk is set from the stick, a dash decelerates only
        // after frame 7, a run holds. Friction is the default, and the floor's
        // material scales it (`ftPhysicsApplyGroundVelFriction`).
        let friction = collision::material_friction(standing.flags);
        // `apply_status_physics` only special-cases a handful of common
        // statuses and falls back to plain friction for everything else —
        // an extended status (`crate::status::AnyStatus::Mario`-style) has
        // no special-cased ground physics of its own either, so any common
        // status stands in for "use the default friction branch".
        let physics_status = match self.status.status {
            crate::status::AnyStatus::Common(s) => s,
            crate::status::AnyStatus::Mario(_) => crate::status::Status::Wait,
            crate::status::AnyStatus::Fox(_) => crate::status::Status::Wait,
            crate::status::AnyStatus::Donkey(s) => {
                crate::grab::ground_physics_status(crate::status::AnyStatus::Donkey(s))
                    .unwrap_or(crate::status::Status::Wait)
            }
            crate::status::AnyStatus::Samus(_) => crate::status::Status::Wait,
            crate::status::AnyStatus::Link(_) => crate::status::Status::Wait,
            crate::status::AnyStatus::Yoshi(_) => crate::status::Status::Wait,
            crate::status::AnyStatus::Captain(_) => crate::status::Status::Wait,
        };
        if crate::captain::apply_ground_physics(self) {
        } else if self.status.status
            == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialHi)
        {
            crate::physics::apply_ground_vel_transn(
                &mut self.physics,
                self.root_motion,
                self.facing.sign(),
            );
        } else if self.status.status
            == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialLw)
        {
            if crate::status::apply_mario_special_lw_ground_physics(self) {
                self.floor = None;
                return;
            }
        } else if matches!(
            self.status.status,
            crate::status::AnyStatus::Fox(
                crate::status::FoxStatus::SpecialHi | crate::status::FoxStatus::SpecialHiEnd
            )
        ) {
            crate::status::apply_fox_special_hi_ground_physics(self);
        } else if self.status.status
            == crate::status::AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialHi)
        {
            crate::status::apply_donkey_special_hi_ground_physics(self);
        } else {
            crate::status::apply_status_physics(
                &mut self.physics,
                &self.attributes,
                physics_status,
                self.status.anim_frame,
                self.input.stick_x,
                friction,
            );
        }

        // A walk's speed is a magnitude; the facing decides its sign.
        if self.status.status.is_walk() {
            self.physics.vel_ground.x = self.physics.vel_ground.x.abs() * self.facing.sign();
        }

        let want = Vec3::new(
            self.pos.x + self.physics.vel_ground.x + self.physics.vel_knockback.x,
            self.pos.y,
            self.pos.z,
        );
        let moved = ground::move_ground(&self.coll, want, standing.line, floors);
        self.pos = moved.pos;

        match moved.floor {
            Some(f) => self.floor = Some(f),
            // Walked off the end of the line. `mpCommonSetFighterFallOnGroundBreak`
            // drops the fighter into a fall rather than leaving it in a walk
            // with no ground under it.
            None => {
                self.floor = None;
                if self.status.status
                    == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialN)
                {
                    crate::status::switch_mario_fireball_air(self);
                } else if self.status.status
                    == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialLw)
                {
                    crate::status::switch_mario_tornado_air(self);
                } else if matches!(
                    self.status.status,
                    crate::status::AnyStatus::Fox(
                        crate::status::FoxStatus::SpecialHiStart
                            | crate::status::FoxStatus::SpecialHiHold
                            | crate::status::FoxStatus::SpecialHi
                            | crate::status::FoxStatus::SpecialHiEnd
                    )
                ) {
                    crate::status::switch_fox_special_hi_air(self);
                } else if matches!(
                    self.status.status,
                    crate::status::AnyStatus::Fox(
                        crate::status::FoxStatus::SpecialLwStart
                            | crate::status::FoxStatus::SpecialLwLoop
                            | crate::status::FoxStatus::SpecialLwHit
                            | crate::status::FoxStatus::SpecialLwEnd
                            | crate::status::FoxStatus::SpecialLwTurn
                    )
                ) {
                    crate::status::switch_fox_special_lw_air(self);
                } else if matches!(
                    self.status.status,
                    crate::status::AnyStatus::Donkey(
                        crate::status::DonkeyStatus::SpecialNStart
                            | crate::status::DonkeyStatus::SpecialNLoop
                            | crate::status::DonkeyStatus::SpecialNEnd
                            | crate::status::DonkeyStatus::SpecialNFull
                            | crate::status::DonkeyStatus::SpecialHi
                    )
                ) {
                    crate::status::switch_donkey_special_air(self);
                } else if !crate::samus::on_ground_lost(self)
                    && !crate::link::on_ground_lost(self)
                    && !crate::yoshi::on_ground_lost(self)
                    && !crate::captain::on_ground_lost(self)
                    && !crate::capture_yoshi::on_ground_lost(self)
                    && !crate::grab::on_ground_lost(self)
                {
                    self.become_airborne();
                    crate::status::set_fall(self);
                }
            }
        }
    }

    fn tick_air<I, F>(&mut self, floors: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = (u16, Segment)>,
    {
        // `ftPhysicsCheckSetFastFall` runs from every airborne status's own
        // `proc_physics` in the original; here it is the one thing every
        // airborne tick does regardless of status.
        let special_air_hi = self.status.status
            == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialAirHi);
        let special_air_lw = self.status.status
            == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialAirLw);
        let fox_special_hi = matches!(
            self.status.status,
            crate::status::AnyStatus::Fox(
                crate::status::FoxStatus::SpecialAirHiStart
                    | crate::status::FoxStatus::SpecialAirHiHold
                    | crate::status::FoxStatus::SpecialAirHi
                    | crate::status::FoxStatus::SpecialAirHiEnd
                    | crate::status::FoxStatus::SpecialAirHiBound
            )
        );
        let fox_special_lw = matches!(
            self.status.status,
            crate::status::AnyStatus::Fox(
                crate::status::FoxStatus::SpecialAirLwStart
                    | crate::status::FoxStatus::SpecialAirLwHit
                    | crate::status::FoxStatus::SpecialAirLwEnd
                    | crate::status::FoxStatus::SpecialAirLwLoop
                    | crate::status::FoxStatus::SpecialAirLwTurn
            )
        );
        let donkey_special_hi = self.status.status
            == crate::status::AnyStatus::Donkey(crate::status::DonkeyStatus::SpecialAirHi);
        if !special_air_hi
            && !special_air_lw
            && !fox_special_hi
            && !fox_special_lw
            && !donkey_special_hi
            && !crate::samus::skips_fast_fall(self.status.status)
            && !crate::link::skips_fast_fall(self.status.status)
            && !crate::yoshi::skips_fast_fall(self)
            && !crate::captain::skips_fast_fall(self.status.status)
        {
            crate::status::check_set_fast_fall(self);
        }
        if special_air_hi {
            crate::status::apply_mario_special_air_hi_physics(self);
        } else if special_air_lw {
            crate::status::apply_mario_special_lw_air_physics(self);
        } else if fox_special_hi {
            crate::status::apply_fox_special_hi_air_physics(self);
        } else if fox_special_lw {
            crate::status::apply_fox_special_lw_air_physics(self);
        } else if donkey_special_hi {
            crate::status::apply_donkey_special_hi_air_physics(self);
        } else if crate::samus::apply_air_physics(self)
            || crate::link::apply_air_physics(self)
            || crate::yoshi::apply_air_physics(self)
            || crate::captain::apply_air_physics(self)
        {
        } else if self.status.status == crate::status::Status::FallSpecial {
            // `ftCommonFallSpecialProcPhysics` @ `ftcommonfallspecial.c:15`:
            // its own fall-speed rule and its own drift clamp, instead of
            // the generic ones below.
            if self.physics.is_fastfall {
                crate::physics::apply_fast_fall(&mut self.physics, &self.attributes);
            } else if self.fall_special.is_fall_accelerate {
                crate::physics::apply_gravity_default(&mut self.physics, &self.attributes);
            } else {
                crate::physics::apply_gravity_clamp_tvel(
                    &mut self.physics,
                    self.attributes.gravity,
                    self.attributes.tvel_fast,
                );
            }
            let drift = self.fall_special.drift;
            if !crate::physics::check_clamp_air_vel_x_dec(&mut self.physics, drift) {
                crate::physics::clamp_air_vel_x_stick_range(
                    &mut self.physics,
                    self.input.stick_x,
                    crate::physics::AIRDRIFT_STICK_MIN,
                    self.attributes.air_accel,
                    drift,
                );
                crate::physics::apply_air_friction(&mut self.physics, &self.attributes);
            }
        } else {
            if self.physics.is_fastfall {
                crate::physics::apply_fast_fall(&mut self.physics, &self.attributes);
            } else {
                crate::physics::apply_gravity_default(&mut self.physics, &self.attributes);
            }
            crate::physics::apply_air_drift(
                &mut self.physics,
                &self.attributes,
                self.input.stick_x,
            );
        }

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
                if crate::status::fox_fire_fox_floor_contact(self, f.normal, moved.pos.y) {
                    return;
                }
                if matches!(
                    self.status.status,
                    crate::status::AnyStatus::Donkey(
                        crate::status::DonkeyStatus::SpecialAirNStart
                            | crate::status::DonkeyStatus::SpecialAirNLoop
                            | crate::status::DonkeyStatus::SpecialAirNEnd
                            | crate::status::DonkeyStatus::SpecialAirNFull
                            | crate::status::DonkeyStatus::SpecialAirHi
                    )
                ) {
                    self.land(moved.pos.y);
                    crate::status::switch_donkey_special_ground(self);
                    return;
                }
                if crate::samus::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::link::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::yoshi::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::captain::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::capture_yoshi::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::grab::on_landing(self, moved.pos.y) {
                    return;
                }
                // The landing status is chosen from the velocity *before*
                // `land` clears it — a fastfall still at terminal velocity on
                // contact is what earns the heavy landing. Landing mid-aerial
                // takes its own landing-lag status instead of the plain one —
                // `crate::status::set_landing_or_landing_air`.
                crate::status::set_landing_or_landing_air(self);
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
    fn joint_offset_uses_rotated_and_scaled_axes() {
        let mut fighter = Fighter::new(FighterKind::Mario, 0, 3);
        fighter.joint_transforms[28] = Some(JointTransform {
            axes: [
                Vec3::new(0.0, 0.0, -2.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
            ],
            origin: Vec3::new(100.0, 200.0, 30.0),
        });
        assert_eq!(
            fighter.joint_world(28, Vec3::new(10.0, 20.0, 30.0)),
            Vec3::new(160.0, 220.0, 10.0)
        );
    }

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
    fn knockback_holds_through_hitstun_then_snaps_to_zero() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.hitstun = 2;
        f.physics.vel_knockback = Vec3::new(17.0, 0.0, 0.0);
        f.tick_timers();
        assert_eq!(f.hitstun, 1);
        assert_eq!(f.physics.vel_knockback, Vec3::new(17.0, 0.0, 0.0));
        f.tick_timers();
        assert_eq!(f.hitstun, 0);
        assert_eq!(f.physics.vel_knockback, Vec3::ZERO);
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

    /// Holds a controller input for one frame and ticks against Dream Land.
    fn step(f: &mut Fighter, x: i8, y: i8) {
        let input = ControllerState {
            stick_x: x,
            stick_y: y,
            ..Default::default()
        };
        f.set_input(input, false, false);
        f.tick(dream_land);
    }

    #[test]
    fn a_fighter_walks_along_the_stage_and_stops_when_the_stick_is_released() {
        use crate::status::Status;

        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(0.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        crate::status::set_wait(&mut f);

        // Ease the stick out so this is a walk and not a dash.
        for _ in 0..8 {
            step(&mut f, 30, 0);
        }
        assert!(f.status.status.is_walk(), "{:?}", f.status.status);
        assert!(f.pos.x > 0.0, "should have moved forward");
        assert_eq!(f.pos.y, 0.0, "and stayed on the floor");
        assert_eq!(f.facing, Facing::Right);

        // Release: the fighter waits and slides to a stop under traction.
        let walked_to = f.pos.x;
        for _ in 0..60 {
            step(&mut f, 0, 0);
        }
        assert_eq!(f.status.status, Status::Wait);
        assert_eq!(f.physics.vel_ground.x, 0.0);
        assert!(f.pos.x > walked_to, "momentum carries a little further");
        assert_eq!(f.pos.y, 0.0);
    }

    #[test]
    fn a_harder_push_walks_faster() {
        let mut slow = Fighter::new(FighterKind::Mario, 0, 3);
        let mut fast = Fighter::new(FighterKind::Mario, 0, 3);
        for f in [&mut slow, &mut fast] {
            f.pos = Vec3::new(0.0, 4.0, 0.0);
            assert!(f.place_on_stage(dream_land()));
            crate::status::set_wait(f);
        }
        for _ in 0..20 {
            step(&mut slow, 30, 0);
            step(&mut fast, 80, 0);
        }
        assert!(
            fast.pos.x > slow.pos.x,
            "fast {} vs slow {}",
            fast.pos.x,
            slow.pos.x
        );
    }

    #[test]
    fn a_fighter_jumps_off_the_stage_and_lands_back_on_it() {
        use crate::status::Status;

        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(0.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        crate::status::set_wait(&mut f);

        // Flick up. Entering jumpsquat takes the frame the input arrives on,
        // then Mario's `kneebend_anim_length` of 3 elapse before takeoff.
        step(&mut f, 0, 80);
        assert_eq!(f.status.status, Status::KneeBend);

        let mut squat_frames = 0;
        while f.is_grounded() && squat_frames < 10 {
            step(&mut f, 0, 80);
            squat_frames += 1;
        }
        assert_eq!(squat_frames, 3, "Mario's jumpsquat is 3 frames");
        assert!(f.pos.y > 0.0);

        let mut peak = f.pos.y;
        let mut landed = None;
        for tick in 0..200 {
            step(&mut f, 0, 0);
            peak = peak.max(f.pos.y);
            if f.is_grounded() {
                landed = Some(tick);
                break;
            }
        }
        let airtime = landed.expect("never came back down") + 1;
        // Mario is 320 units tall (his collision diamond's `top`), so a full
        // hop clearing ~1360 is a little over four times his own height, in
        // the air for about 1.2 seconds. That is the floaty jump Smash 64
        // actually has, and it falls out of the extracted numbers alone:
        // `80 * 0.7 + 26 = 82` initial velocity against gravity 2.4.
        assert!(
            (1300.0..1450.0).contains(&peak),
            "full hop peaked at {peak}, expected ~1360"
        );
        assert!(
            peak > 4.0 * f.coll.top.max(320.0),
            "should clear his own height several times over"
        );
        assert!(
            (65..85).contains(&airtime),
            "airtime {airtime} frames, expected ~74"
        );
        assert_eq!(f.pos.y, 0.0, "landed back exactly on the floor");
        assert_eq!(f.status.status, Status::LandingLight);
    }

    #[test]
    fn a_double_jump_goes_higher_than_a_single_one() {
        let apex = |double: bool| {
            let mut f = Fighter::new(FighterKind::Mario, 0, 3);
            f.pos = Vec3::new(0.0, 4.0, 0.0);
            f.place_on_stage(dream_land());
            crate::status::set_wait(&mut f);
            // Entry frame plus Mario's three frames of jumpsquat.
            for _ in 0..4 {
                step(&mut f, 0, 80);
            }
            let mut peak = f.pos.y;
            let mut jumped_again = false;
            for _ in 0..200 {
                // Release, then flick up again at the top of the first jump.
                let up = double && !jumped_again && f.physics.vel_air.y < 0.0;
                if up {
                    step(&mut f, 0, 0);
                    step(&mut f, 0, 80);
                    jumped_again = true;
                } else {
                    step(&mut f, 0, 0);
                }
                peak = peak.max(f.pos.y);
                if f.is_grounded() {
                    break;
                }
            }
            peak
        };
        let single = apex(false);
        let double = apex(true);
        assert!(double > single, "double {double} vs single {single}");
    }

    #[test]
    fn walking_off_the_edge_falls_rather_than_staying_in_a_walk() {
        use crate::status::Status;

        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        // Dream Land's main platform ends at x = 2318.
        f.pos = Vec3::new(2300.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        crate::status::set_wait(&mut f);

        for _ in 0..40 {
            step(&mut f, 30, 0);
            if !f.is_grounded() {
                break;
            }
        }
        assert!(!f.is_grounded(), "should have walked off the end");
        assert!(f.pos.x > 2318.0);
        assert_eq!(f.status.status, Status::Fall);
    }

    #[test]
    fn landing_keeps_the_horizontal_momentum_a_jump_carried() {
        // Entering the landing status transfers air velocity to ground
        // velocity, and so does `land`. Doing both would zero it, and the
        // symptom would be a fighter that lands from a moving jump and stops
        // dead -- which looks like a physics bug and is an ordering one.
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.pos = Vec3::new(0.0, 4.0, 0.0);
        assert!(f.place_on_stage(dream_land()));
        crate::status::set_wait(&mut f);

        // Jump forward, holding the stick so the jump carries speed.
        for _ in 0..4 {
            step(&mut f, 60, 80);
        }
        assert!(!f.is_grounded());
        assert!(f.physics.vel_air.x > 0.0, "the jump should carry forward");

        let mut airborne = 0;
        while !f.is_grounded() && airborne < 200 {
            step(&mut f, 60, 0);
            airborne += 1;
        }
        assert!(f.is_grounded(), "never landed");
        assert!(
            f.physics.vel_ground.x > 0.0,
            "landed with {} ground velocity; momentum was lost",
            f.physics.vel_ground.x
        );
    }

    /// `ftMainProcUpdateInterrupt`: a held R reads as A + Z to the status machine,
    /// so pressing R alone taps both.
    #[test]
    fn r_trigger_holds_a_and_z() {
        use ssb_engine::input::{newly_pressed, N64Buttons};
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.set_input(ControllerState::default(), false, false);
        f.set_input(
            ControllerState {
                buttons: N64Buttons(N64Buttons::R),
                ..Default::default()
            },
            false,
            false,
        );
        let taps = newly_pressed(f.prev_input.buttons, f.input.buttons);
        assert!(taps.contains(N64Buttons::A) && taps.contains(N64Buttons::Z));
        assert!(taps.contains(N64Buttons::R));
    }
}
