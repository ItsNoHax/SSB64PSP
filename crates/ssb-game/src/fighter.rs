//! Fighter identity and per-fighter state.
//!
//! The roster ordering is `enum FTKind` from `src/ft/ftdef.h`. Preserving the
//! exact ordinals matters: extracted asset tables are indexed by fighter kind,
//! so renumbering would silently mis-associate every character's data.

use ssb_engine::input::{ControllerState, N64Buttons};
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
    /// `FTKind` from an original stage or team table.
    pub fn from_ordinal(value: u8) -> Option<Self> {
        if value < 12 {
            return Self::PLAYABLE.get(value as usize).copied();
        }
        use FighterKind::*;
        [
            Boss,
            MetalMario,
            PolyMario,
            PolyFox,
            PolyDonkey,
            PolySamus,
            PolyLuigi,
            PolyLink,
            PolyYoshi,
            PolyCaptain,
            PolyKirby,
            PolyPikachu,
            PolyPurin,
            PolyNess,
            GiantDonkey,
        ]
        .get(value as usize - 12)
        .copied()
    }
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

    /// The fighter whose special statuses this kind runs
    /// (`dFTMainSpecialStatusDescs`): Metal Mario and Polygon Mario run
    /// Mario's, Giant Donkey Kong Donkey Kong's, each Polygon its model's.
    /// The source's per-character branches of the common statuses list the
    /// same groups (`case nFTKindMario: case nFTKindMMario: case
    /// nFTKindNMario:`); where a branch leaves a variant out, the call site
    /// says so.
    pub fn character(self) -> FighterKind {
        match self {
            FighterKind::MetalMario => FighterKind::Mario,
            FighterKind::GiantDonkey => FighterKind::Donkey,
            k => k.polygon_base().unwrap_or(k),
        }
    }

    /// `FTAttributes::is_have_specialn` and its five siblings, and
    /// `is_have_catch`: every fighter but the Polygons (`2xx_N*Main.c`).
    pub fn has_specials(self) -> bool {
        !self.is_polygon()
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
    /// `desc.lr = (desc.pos.x >= 0.0F) ? -1 : +1`: a fighter starting at
    /// `x` faces the stage's centre (`sc1PTrainingModeFuncStart`; also the
    /// 1P game, bonus, demo and How to Play starts). VS faces the nearest
    /// opponent instead (`scVSBattleGetStartPlayerLR`).
    pub fn at_spawn_x(x: f32) -> Facing {
        if x >= 0.0 {
            Facing::Left
        } else {
            Facing::Right
        }
    }

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

/// `FTCOMMON_DAMAGE_SMASH_DI_*` (US).
pub const SMASH_DI_RANGE_MIN: i32 = 53;
pub const SMASH_DI_BUFFER_TICS_MAX: u8 = 4;
pub const SMASH_DI_RANGE_MUL: f32 = 2.1;

/// The percent `ftParamUpdateDamage` never lets a fighter exceed.
pub const DAMAGE_PERCENT_MAX: i32 = 999;

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
    /// `FTStruct::team` ([`crate::team`]): the port unless a team battle or
    /// Training sets it.
    pub team: u8,
    pub pos: Vec3,
    pub facing: Facing,
    pub situation: Situation,
    pub physics: PhysicsState,
    pub attributes: PhysicsAttributes,
    /// Damage percentage. The original stores this as an integer.
    pub damage: u16,
    /// `SCBattlePlayer.combo_damage_foe` / `combo_count_foe`.
    pub combo_damage_foe: u32,
    pub combo_count_foe: u32,
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
    pub interface: crate::player_interface::FighterInterface,
    /// Source shadow-specific display gate (`FTStruct::is_shadow_hide`).
    /// Common lifecycle transitions maintain it; capture/other visual systems
    /// can also set it without acquiring renderer knowledge.
    pub is_shadow_hidden: bool,
    /// The battle entry's spawn point and end facing (`crate::appear`).
    pub entry: crate::appear::Entry,
    pub input: ControllerState,
    pub prev_input: ControllerState,
    /// Collision offsets — `MPObjectColl`.
    pub coll: BodyColl,
    /// `FTAttributes.cliffcatch_coll`: facing-scaled hand reach.
    pub cliff_reach: ssb_engine::math::Vec2,
    pub map_contacts: crate::map::Contacts,
    pub map_contacts_prev: crate::map::Contacts,
    /// Each other fighter's held `(floor line, facing)`, supplied by the
    /// match before ticking: `mpCommonRunFighterSpecialCollisions` skips a
    /// cliff another fighter holds facing the same way.
    pub occupied_cliffs: [Option<(u16, Facing)>; 3],
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
    pub items: crate::item::FighterItems,
    pub item_throw: crate::item_throw::ThrowState,
    pub item_use: crate::item_use::State,
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
    /// Kirby's copy, rapid jab, Stone and Inhale state — `crate::kirby`.
    pub kirby: crate::kirby::KirbyState,
    pub pikachu: crate::pikachu::PikachuState,
    pub purin: crate::purin::PurinState,
    pub ness: crate::ness::NessState,
    /// Master Hand's passive and status variables ([`crate::boss`]).
    pub boss: crate::boss::BossState,
    /// This fighter's side of an Inhale — `crate::capture_kirby`.
    pub kirby_capture: crate::capture_kirby::CaptureKirbyState,
    pub thrown: crate::thrown::ThrownState,
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
    pub stats: crate::spgame::live::Stats,
    /// This player's `stale_info` queue ([`crate::stale`]).
    pub stale: crate::stale::StaleQueue,
    /// `FTStruct::handicap`, an index into `dFTCommonDataHandicapTable`.
    pub handicap: u8,
    /// `FTStruct::costume` ([`crate::costume`]); display only.
    pub costume: u8,
    /// One weapon creation requested by this fighter's current status. The
    /// match-owned weapon pool consumes it after fighter callbacks finish.
    pub weapon_spawn: Option<crate::weapon::WeaponSpawn>,
    /// Current posed joints, indexed as `FTStruct::joints` (four runtime
    /// joints precede the packed model nodes).
    pub joint_transforms: [Option<JointTransform>; FIGHTER_JOINTS],
    /// The running motion scripts and their flags ([`crate::motion`]).
    pub motion_script: crate::motion::MotionState,
    /// `FTStruct::attack_colls`, made by the motion scripts.
    pub attack_colls: [crate::combat::AttackColl; 4],
    /// `FTStruct::hitstatus` (`ftParamSetHitStatusAll`).
    pub hitstatus: crate::combat::HitStatus,
    /// `FTStruct::damage_colls`' live state ([`crate::hurtbox`]).
    pub damage_colls: crate::hurtbox::DamageColls,
    /// `FTStruct::intangible_tics`.
    pub intangible_frames: u16,
    /// `FTStruct::star_invincible_tics`; `star_hitstatus` is invincible
    /// while it runs (`ftParamSetStarHitStatusInvincible`).
    pub star_invincible_frames: u16,
    /// `FTStruct::damage_heal`: percent still to heal, one per frame.
    pub damage_heal: i32,
    /// `FTStruct::knockback_resist_passive`.
    pub knockback_resist_passive: f32,
    /// `FTStruct::damage_knockback_stack`: the knockback of the last damage
    /// status, which a hit during its hitlag must beat by 30 to replace it.
    pub damage_knockback_stack: f32,
    /// `FTStruct::is_knockback_paused`: in the hitlag of a damage hit.
    pub is_knockback_paused: bool,
    /// This frame's hit bookkeeping ([`crate::combat::FrameHits`]).
    pub hits: crate::combat::FrameHits,
    /// `attr->jostle_width` and `attr->jostle_x`: the body's half-width and
    /// its forward offset for [`jostle`].
    pub jostle_width: f32,
    pub jostle_x: f32,
    /// The sign of TopN's yaw, which `ftMainSetStatus` sets from `lr`
    /// (`rotate.y = lr * 90°`). A status that turns the fighter partway
    /// (a roll) keeps moving the way the model faces until the next status
    /// (`ftPhysicsApplyGroundVelTransN`'s `lr * rotate.y < 0` case).
    pub topn_lr: f32,
    /// Taps and releases gathered while in hitlag
    /// (`ftMainProcUpdateInterrupt` ORs them together until it ends).
    pub tap_carry: N64Buttons,
    pub release_carry: N64Buttons,
    /// `FTStruct::tics_since_last_z`: frames since Z was tapped (teching).
    pub tics_since_last_z: u32,
    /// `FTStruct::damage_mul`: 0.5 while knocked down.
    pub damage_mul: f32,
    /// The status an electric hit enters after `DamageE1`/`E2`
    /// (`status_vars.common.damage.status_id`).
    pub damage_e_status: Option<crate::status::AnyStatus>,
    /// Knockdown, clank and roll counters ([`crate::reaction`]).
    pub reaction: crate::reaction::ReactionState,
    /// `proc_lagupdate == ftCommonDamageCommonProcLagUpdate`: set by a
    /// damage status, cleared by the next status.
    pub is_smash_di: bool,
    /// This tick's runtime-sampled TransN motion. It is data, not a renderer
    /// handle, so host gameplay tests can provide it directly and `ssb-game`
    /// remains runtime-independent.
    pub root_motion: RootMotion,
    /// Current authored TransN translation, sampled by the animation runtime.
    pub transn: Vec3,
    pub cliff_air_mask: u32,
    /// Stage hazard timers, wind and captor state ([`crate::hazard`]).
    pub hazard: crate::hazard::HazardState,
    /// Mushroom Kingdom's pipes ([`crate::dokan`]).
    pub dokan: crate::dokan::DokanState,
    /// Blast-zone deaths and the rebirth halo ([`crate::dead`]).
    pub dead: crate::dead::DeadState,
    /// `FTStruct::colanim`: the colour the fighter is fogged towards and
    /// its light ([`crate::colanim`]).
    pub colanim: crate::colanim::ColAnim,
    /// `ifScreenFlashSetColAnimID` from a strong hit
    /// (`ftCommonDamageCheckMakeScreenFlash`), for the match's screen flash
    /// ([`crate::ko::KoEffects::observe`]).
    pub screen_flash: Option<crate::colanim::ColAnimId>,
    /// `FTStruct::damage_player`: the player whose attack hit this fighter
    /// last, credited with a KO. `None` for -1 and for
    /// `GMCOMMON_PLAYERS_MAX` (the stage, or the fighter's own weapon).
    pub damage_player: Option<u8>,
    /// Effects queued for the match to make ([`crate::fteffect`]).
    pub effects: crate::fteffect::EffectQueue,
    /// `effect_joint_array_id`: which of `effect_joint_ids` the next
    /// flame, spark or shock uses.
    pub effect_joint_array_id: u8,
    /// `FTStruct::modelpart_status` ([`crate::modelpart`], RE-425).
    pub model_parts: crate::modelpart::ModelParts,
}

impl Fighter {
    pub fn new(kind: FighterKind, port: u8, stocks: i8) -> Self {
        Fighter {
            kind,
            port,
            team: port,
            pos: Vec3::ZERO,
            facing: Facing::Right,
            situation: Situation::Air,
            physics: PhysicsState::default(),
            attributes: PhysicsAttributes::default(),
            damage: 0,
            combo_damage_foe: 0,
            combo_count_foe: 0,
            stocks,
            hitlag: 0,
            hitstun: 0,
            invincible_frames: 0,
            is_invisible: false,
            interface: crate::player_interface::FighterInterface::default(),
            is_shadow_hidden: false,
            entry: crate::appear::Entry::default(),
            input: ControllerState::default(),
            prev_input: ControllerState::default(),
            coll: BodyColl::default(),
            cliff_reach: ssb_engine::math::Vec2::ZERO,
            map_contacts: crate::map::Contacts::default(),
            map_contacts_prev: crate::map::Contacts::default(),
            occupied_cliffs: [None; 3],
            floor: None,
            ignore_line: None,
            anim: crate::status::AnimLengths::default(),
            status: crate::status::StatusState::default(),
            stick: crate::status::StickState::new(),
            guard: crate::status::GuardState::default(),
            items: crate::item::FighterItems::default(),
            item_throw: crate::item_throw::ThrowState::default(),
            item_use: crate::item_use::State::default(),
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
            kirby: crate::kirby::KirbyState::default(),
            pikachu: crate::pikachu::PikachuState::default(),
            purin: crate::purin::PurinState::default(),
            ness: crate::ness::NessState::default(),
            boss: crate::boss::BossState::default(),
            kirby_capture: crate::capture_kirby::CaptureKirbyState::default(),
            thrown: crate::thrown::ThrownState::default(),
            egg: crate::capture_yoshi::CaptureYoshiState::default(),
            knockback_resist: 0.0,
            is_special_interrupt: false,
            grab: crate::grab::GrabState::default(),
            motion: crate::stale::MotionId::default(),
            stats: crate::spgame::live::Stats::default(),
            stale: crate::stale::StaleQueue::default(),
            handicap: crate::stale::HANDICAP_DEFAULT,
            costume: 0,
            weapon_spawn: None,
            joint_transforms: [None; FIGHTER_JOINTS],
            root_motion: RootMotion::default(),
            transn: Vec3::ZERO,
            cliff_air_mask: match kind {
                FighterKind::Mario
                | FighterKind::Luigi
                | FighterKind::Samus
                | FighterKind::Ness => 2,
                _ => 0,
            },
            motion_script: crate::motion::MotionState::default(),
            attack_colls: [crate::combat::AttackColl::default(); 4],
            hitstatus: crate::combat::HitStatus::Normal,
            damage_colls: crate::hurtbox::DamageColls::default(),
            intangible_frames: 0,
            star_invincible_frames: 0,
            damage_heal: 0,
            // `ftManagerInitFighter`: Metal Mario resists 30 knockback
            // and Giant Donkey Kong 48, whatever their status.
            knockback_resist_passive: match kind {
                FighterKind::MetalMario => 30.0,
                FighterKind::GiantDonkey => 48.0,
                _ => 0.0,
            },
            damage_knockback_stack: 0.0,
            is_knockback_paused: false,
            hits: crate::combat::FrameHits::default(),
            jostle_width: 0.0,
            jostle_x: 0.0,
            topn_lr: 1.0,
            tap_carry: N64Buttons(0),
            release_carry: N64Buttons(0),
            tics_since_last_z: crate::status::ZTRIGLAST_TICS_MAX,
            damage_mul: 1.0,
            damage_e_status: None,
            reaction: crate::reaction::ReactionState::default(),
            is_smash_di: false,
            hazard: crate::hazard::HazardState::default(),
            dokan: crate::dokan::DokanState::default(),
            dead: crate::dead::DeadState::default(),
            colanim: crate::colanim::ColAnim::default(),
            screen_flash: None,
            damage_player: None,
            effects: crate::fteffect::EffectQueue::default(),
            effect_joint_array_id: 0,
            model_parts: crate::modelpart::ModelParts::new(kind),
        }
    }

    pub fn is_grounded(&self) -> bool {
        self.situation == Situation::Ground
    }

    /// `ftParamUpdateDamage` @ 0x800EA248: adds to the percent, which the
    /// original caps at 999.
    pub fn add_damage(&mut self, damage: i32) {
        self.stats
            .emit(crate::spgame::live::Event::Damage(damage.max(0) as u32));
        let total = i32::from(self.damage) + damage.max(0);
        self.damage = total.min(DAMAGE_PERCENT_MAX) as u16;
    }

    /// `ftParamUpdatePlayerBattleStats`: every accepted hit from another
    /// player counts, including a zero-damage hit. World/self hits do not.
    pub fn record_combo_damage(&mut self, player: Option<u8>, damage: i32) {
        if player.is_some_and(|p| p < 4 && p != self.port) {
            self.stats.emit(crate::spgame::live::Event::Credit {
                player: player.unwrap(),
                damage: damage.max(0) as u32,
            });
            self.combo_damage_foe += damage.max(0) as u32;
            self.combo_count_foe += 1;
        }
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
    /// Hitstun only counts down here; the damage velocity is independent of
    /// it and decays in [`crate::physics::update_damage_velocity`], so a
    /// strong hit keeps carrying the fighter after hitstun ends, as in
    /// `ftMainProcPhysicsMap`.
    pub fn tick_timers(&mut self) -> bool {
        let mut lag_ended = false;
        if self.hitlag > 0 {
            self.hitlag -= 1;
            if self.hitlag == 0 {
                self.is_knockback_paused = false;
                lag_ended = true;
            }
        }
        // `intangible_tics`/`invincible_tics` run down in hitlag too. When
        // the last one runs out, `ftMainProcUpdateInterrupt` ends a running
        // `NoDamage` flicker after the frame's colour animation update
        // ([`crate::colanim::run_update_interrupt`]).
        if self.intangible_frames > 0 {
            self.intangible_frames -= 1;
            if self.intangible_frames == 0 {
                self.colanim.is_nodamage_expired = true;
            }
        }
        if self.invincible_frames > 0 {
            self.invincible_frames -= 1;
            if self.invincible_frames == 0 && self.intangible_frames == 0 {
                self.colanim.is_nodamage_expired = true;
            }
        }
        // The Star's timer and the heal follow. Their colour animations
        // (`colanim_id` 0x4A and 9) end in the same deferred check. The
        // Star's warning frame only restores the stage music.
        if self.star_invincible_frames > 0 {
            self.star_invincible_frames -= 1;
            if self.star_invincible_frames == 0 {
                self.colanim.is_star_expired = true;
            }
        }
        if self.damage_heal != 0 {
            self.damage_heal -= 1;
            if self.damage != 0 {
                self.damage -= 1;
            }
            if self.damage == 0 {
                self.damage_heal = 0;
            }
            if self.damage_heal == 0 {
                self.colanim.is_heal_expired = true;
            }
        }
        if self.hitlag > 0 {
            return false;
        }
        if self.hitstun > 0 {
            self.hitstun -= 1;
        }
        if self.cliffcatch_wait > 0 {
            self.cliffcatch_wait -= 1;
        }
        lag_ended
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
        // `mpCommonSetFighterAir`: `vel_air` already holds the last ground
        // step (`ftPhysicsSetGroundVelTransferAir`); only its depth stops.
        self.physics.vel_ground = Vec3::ZERO;
        self.physics.vel_air.z = 0.0;
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
        // `mpCommonSetFighterLandingParams`'s Jigglypuff case.
        self.purin.pound_count = 0;
    }

    /// `ftManagerInitFighter`'s floor projection: a fighter made over a
    /// floor less than 300 units below stands on it from the start
    /// (`mpCollisionCheckProjectFloor`); otherwise it starts airborne with
    /// one jump spent. Master Hand always starts airborne. Returns whether a
    /// floor was found.
    pub fn init_floor<I>(&mut self, floors: I) -> bool
    where
        I: IntoIterator<Item = (u16, Segment)>,
    {
        let below =
            collision::project_floor(floors, ssb_engine::math::Vec2::new(self.pos.x, self.pos.y));
        match below {
            Some(b) if b.dist > -300.0 && self.kind != FighterKind::Boss => {
                self.situation = Situation::Ground;
                self.pos.y += b.dist;
                self.floor = Some(Standing {
                    line: b.line,
                    flags: b.flags,
                    normal: b.normal,
                });
            }
            _ => {
                self.situation = Situation::Air;
                self.physics.jumps_used = 1;
            }
        }
        below.is_some()
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
        // `pl->stick_range` is clamped to ±`I_CONTROLLER_RANGE_MAX`; every
        // status reads the clamped value (a jumpsquat's force too).
        let max = crate::status::STICK_MAX;
        input.stick_x = (i32::from(input.stick_x)).clamp(-max, max) as i8;
        input.stick_y = (i32::from(input.stick_y)).clamp(-max, max) as i8;
        if input.buttons.contains(ssb_engine::input::N64Buttons::R) {
            input.buttons.0 |= ssb_engine::input::N64Buttons::A | ssb_engine::input::N64Buttons::Z;
        }
        let (carry_tap, carry_release) = if self.hitlag != 0 {
            (self.button_tap(), self.button_release())
        } else {
            (N64Buttons(0), N64Buttons(0))
        };
        self.tap_carry = carry_tap;
        self.release_carry = carry_release;
        self.prev_input = self.input;
        self.input = input;
        self.stick
            .step(input.stick_x, input.stick_y, jump_tapped, jump_released);
        // `tics_since_last_z`.
        if self.tics_since_last_z < crate::status::ZTRIGLAST_TICS_MAX {
            self.tics_since_last_z += 1;
        }
        if self.button_tap().contains(N64Buttons::Z) {
            self.tics_since_last_z = 0;
        }
    }

    /// `input.pl.button_tap`: buttons pressed this frame, plus any pressed
    /// during the hitlag that ends this frame.
    pub fn button_tap(&self) -> N64Buttons {
        N64Buttons(self.tap_carry.0 | (self.input.buttons.0 & !self.prev_input.buttons.0))
    }

    /// `input.pl.button_release`, with the same hitlag buffering.
    pub fn button_release(&self) -> N64Buttons {
        N64Buttons(self.release_carry.0 | (self.prev_input.buttons.0 & !self.input.buttons.0))
    }

    /// `button_tap = button_release = 0` (`ftMainProcParams` on a hit).
    pub fn clear_taps(&mut self) {
        self.tap_carry = N64Buttons(0);
        self.release_carry = N64Buttons(0);
        self.prev_input.buttons = self.input.buttons;
    }

    /// Supplies the TransN displacement sampled by the outer animation
    /// runtime for the frame about to run. A caller that has no skeleton (host
    /// tests, for example) may leave this at its default zero motion.
    pub fn set_root_motion(&mut self, motion: RootMotion) {
        self.root_motion = motion;
    }

    /// World position of a motion collision offset. Host-only callers with
    /// no skeleton use TopN's turn/facing transform for joint 0 and the previous
    /// root-offset fallback for non-root joints.
    pub fn joint_world(&self, joint: u8, offset: Vec3) -> Vec3 {
        if let Some(transform) = self.joint_transforms.get(joint as usize).copied().flatten() {
            transform.point(offset)
        } else if joint == 0 {
            let axes = crate::item_throw::model_axes(self);
            self.pos + axes[0] * offset.x + axes[1] * offset.y + axes[2] * offset.z
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
        self.tick_map(|| floors().into_iter().map(crate::map::floor_surface));
    }

    /// Full map input, shared by host matches and the PSP runtime.
    pub fn tick_map<I, F>(&mut self, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        self.tick_interrupt(&surfaces);
        self.tick_physics_map(&surfaces);
    }

    /// `ftMainProcUpdateInterrupt` (process priority 5). A match runs this
    /// for every fighter, then the stage processes, then
    /// [`Self::tick_physics_map`] for every fighter (priority 4): stage
    /// controllers read fighters between the two.
    pub fn tick_interrupt<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        self.tick_timers();
        self.resolve_cliff_release(surfaces);
        // `proc_passive`: an electric hit's `DamageE` hands over to the
        // real damage status once hitlag is over.
        crate::attack::update_damage_e(self);
        crate::reaction::check_set_invincible(self);
        // The previous frame's push is spent; the stage sets a new one.
        self.hazard.vel_push = Vec3::ZERO;
        // `is_events_forward = TRUE`, before `ftMainPlayAnimEventsAll`: the
        // status scripts' effects wait for the end of the physics pass.
        self.motion_script.is_events_forward = true;
        if self.is_in_hitlag() {
            // `ftMainRunUpdateColAnim` runs in hitlag too.
            crate::colanim::run_update_interrupt(self);
            return;
        }

        if self.interface.tag_wait > 1 && !self.interface.control_disable {
            self.interface.tag_wait -= 1;
        }
        // `proc_update` + `proc_interrupt` can move between ground and air
        // (a jumpsquat ending, a platform drop), so the
        // situation is re-read afterwards rather than captured before.
        crate::status::update(self);
        // Master Hand's `proc_update` and `proc_interrupt` read the map.
        crate::boss::update(self, surfaces);
        crate::dokan::run_pending(self, surfaces);
        self.resolve_cliff_release(surfaces);
        // `ftMainProcUpdateInterrupt`, after proc_update/proc_interrupt,
        // outside hitlag. Hit searches later in the frame start a new combo.
        if self.hitstun == 0 {
            self.combo_damage_foe = 0;
            self.combo_count_foe = 0;
        }
    }

    /// `ftMainProcPhysicsMap` (process priority 4), after the stage, ending
    /// with the motion scripts' effects
    /// (`ftMainUpdateMotionEventsForwardEffect`, [`crate::motion::end_physics`]).
    pub fn tick_physics_map<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        self.tick_physics_map_before_accessory(surfaces);
        crate::item_use::accessory(self);
    }

    /// A runtime that samples animated joints after the map pass calls this,
    /// refreshes those joints, then runs `item_use::accessory`. Host callers
    /// without a skeleton use `tick_physics_map` for the complete pass.
    pub fn tick_physics_map_before_accessory<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        self.tick_physics_map_procs(surfaces);
        crate::motion::end_physics(self);
    }

    fn tick_physics_map_procs<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        crate::appear::tick_effect_clock(self);
        // `ftCommonThrownReleaseFighterLoseGrip`'s
        // `mpCommonRunFighterCollisionDefault`: one collision pass from the
        // catcher's position with its diamond to the dropped TopN.
        if let Some((from, previous)) = self.grab.release_sweep.take() {
            self.pos =
                crate::map::run_default_collision(&self.coll, &previous, from, self.pos, || {
                    surfaces()
                });
        }
        if self.is_in_hitlag() {
            // `proc_lagupdate`: Smash DI nudges a fighter frozen by a hit.
            self.smash_di(surfaces);
            crate::dead::check(self);
            return;
        }
        let surfaces = || surfaces();
        // The dead and rebirth statuses have no `proc_physics`, and their
        // `proc_map` (if any) replaces the map step.
        if crate::dead::tick_status(self) {
            self.root_motion = RootMotion::default();
            return;
        }
        // Master Hand's statuses own their physics and map callbacks.
        if crate::boss::tick_status(self, &surfaces) {
            self.root_motion = RootMotion::default();
            return;
        }
        // The battle entry places the fighter itself and skips the map.
        if crate::appear::tick_status(self) {
            self.root_motion = RootMotion::default();
            return;
        }

        // `ftCommonYoshiEggProcPhysics`'s own half, ahead of the common
        // physics it ends with.
        if self.status.status == crate::status::Status::YoshiEgg {
            crate::capture_yoshi::physics(self);
        }

        // A held fighter's position is its catcher's hand, not the result
        // of its own velocity (`ftCommonCapturePulledProcPhysics`).
        if crate::grab::tick_held(self, || crate::map::floors(surfaces())) {
            self.root_motion = RootMotion::default();
            crate::dead::check(self);
            return;
        }
        self.map_contacts_prev = self.map_contacts;
        self.map_contacts = crate::map::Contacts::default();
        if crate::dokan::tick_status(self) {
            self.root_motion = RootMotion::default();
            crate::dead::check(self);
            return;
        }
        if crate::hazard::tick_status(self, &surfaces) {
            crate::fteffect::kirby_map_star(self);
            self.root_motion = RootMotion::default();
            crate::dead::check(self);
            return;
        }
        if !self.tick_cliff(&surfaces) {
            match self.situation {
                Situation::Ground => self.tick_ground(surfaces),
                Situation::Air => self.tick_air(surfaces),
            }
        }
        // `ftParamKirbyTryMakeMapStarEffect`, after `proc_map`.
        crate::fteffect::kirby_map_star(self);
        // `ftCommonDeadCheckInterruptCommon` runs between the position step
        // and `proc_map` in `ftMainProcPhysicsMap`; here the ground and air
        // ticks do both, so it runs after the map step.
        crate::dead::check(self);
        // Root motion is an input sample, not persistent fighter state. This
        // prevents a missed runtime sample from replaying an old displacement.
        self.root_motion = RootMotion::default();
    }

    /// Finish a cliff damage callback in the match's hit-resolution pass.
    pub fn resolve_cliff_release<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        if let Some(previous) = self.cliff.release_previous.take() {
            let result = crate::map::move_air(
                &self.coll,
                previous,
                self.pos,
                crate::map::AirOptions::default(),
                surfaces,
            );
            self.pos = result.moved.pos;
        }
    }

    fn tick_cliff<I, F>(&mut self, surfaces: &F) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        use crate::map;
        if !map::is_cliff_hold(self.status.status) && !map::is_cliff_phase2(self.status.status) {
            return false;
        }
        let Some(corner) = map::cliff_corner(surfaces, self.cliff.line, self.facing.sign()) else {
            self.floor = None;
            self.cliffcatch_wait = crate::status::CLIFF_CATCH_WAIT;
            crate::status::set_fall(self);
            return false;
        };
        self.cliff.corner = corner;
        if map::is_cliff_hold(self.status.status) {
            self.physics = PhysicsState::default();
            self.pos.x = corner.x + self.transn.z * self.facing.sign() * self.attributes.size;
            self.pos.y = corner.y + self.transn.y * self.attributes.size;
            return true;
        }
        if self.cliff.place_phase2 {
            self.pos.x = corner.x + 5.0 * self.facing.sign();
            if let Some((y, floor)) = map::floor_point(surfaces, self.cliff.line, self.pos.x) {
                self.pos.y = y;
                self.floor = Some(floor);
            }
            self.cliff.place_phase2 = false;
        }
        let mut motion = self.root_motion;
        motion.delta *= self.attributes.size;
        if self.is_grounded() {
            // `motion` is already scaled by `attr->size` above; the step
            // follows TopN's yaw (`lr * rotate.y < 0` flips it).
            crate::physics::apply_ground_vel_transn(&mut self.physics, motion, self.topn_lr, 1.0);
            let want =
                self.pos + Vec3::new(self.physics.vel_ground.x, 0.0, self.physics.vel_ground.z);
            let stop = !matches!(
                self.status.status,
                crate::status::AnyStatus::Common(
                    crate::status::Status::CliffClimbQuick2
                        | crate::status::Status::CliffClimbSlow2
                )
            );
            let (moved, contacts) =
                map::move_ground(&self.coll, self.pos, want, self.cliff.line, stop, surfaces);
            self.pos = moved.pos;
            self.floor = moved.floor;
            self.map_contacts = contacts;
            if self.floor.is_none() {
                crate::status::set_fall(self);
            }
        } else {
            // `motion` is already scaled by `attr->size` above.
            crate::physics::apply_air_vel_transn_all(
                &mut self.physics,
                motion,
                self.facing.sign(),
                1.0,
            );
            let mut want = self.pos + self.physics.vel_air;
            let speed = map::line_speed(surfaces, self.cliff.line);
            if let Some((y, _)) = map::floor_point(surfaces, self.cliff.line, want.x + speed.x) {
                want.x += speed.x;
                want.y = y + self.transn.y;
                self.physics.vel_air = want - self.pos;
            }
            let result = map::move_air(
                &self.coll,
                self.pos,
                want,
                map::AirOptions::default(),
                surfaces,
            );
            self.pos = result.moved.pos;
            self.map_contacts = result.contacts;
            if let Some(floor) = result.moved.floor {
                self.floor = Some(floor);
                self.land(self.pos.y);
            }
        }
        true
    }

    /// `ftCommonDamageCommonProcLagUpdate`: during a hit's hitlag, a fresh
    /// hard stick tilt moves the fighter `2.1` units per stick unit (US),
    /// once per tilt. The move then goes through the map like any other
    /// (`proc_map` still runs in hitlag); an airborne fighter that reaches
    /// a floor stops on it and lands when hitlag ends.
    fn smash_di<I, F>(&mut self, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        let damage_air = matches!(
            self.status.status,
            crate::status::AnyStatus::Common(s) if crate::reaction::is_damage_air(s)
        );
        if damage_air {
            // `proc_map` runs in hitlag too: without a nudge the sweep has
            // no movement, so all it does is roll the damage masks.
            self.reaction.coll_mask_prev = self.reaction.coll_mask_curr;
            self.reaction.coll_mask_curr = 0;
        }
        let (x, y) = (self.stick.x as i32, self.stick.y as i32);
        let nudge = self.is_smash_di
            && self.hitlag != 0
            && x * x + y * y >= SMASH_DI_RANGE_MIN * SMASH_DI_RANGE_MIN
            && (self.stick.tap_x < SMASH_DI_BUFFER_TICS_MAX
                || self.stick.tap_y < SMASH_DI_BUFFER_TICS_MAX);
        let want = if nudge {
            self.stick.tap_x = crate::status::STICKBUFFER_MAX;
            self.stick.tap_y = crate::status::STICKBUFFER_MAX;
            Vec3::new(
                self.pos.x + x as f32 * SMASH_DI_RANGE_MUL,
                self.pos.y + y as f32 * SMASH_DI_RANGE_MUL,
                self.pos.z,
            )
        } else {
            self.pos
        };
        match (self.situation, self.floor) {
            (Situation::Ground, Some(standing)) => {
                let (moved, _) = crate::map::move_ground(
                    &self.coll,
                    self.pos,
                    want,
                    standing.line,
                    false,
                    surfaces,
                );
                self.pos = moved.pos;
                if let Some(f) = moved.floor {
                    self.floor = Some(f);
                }
            }
            _ if damage_air => {
                self.reaction.coll_mask_curr = self.reaction.coll_mask_prev;
                self.tick_damage_map(self.pos, want, surfaces);
            }
            _ => {
                let result = crate::map::move_air(
                    &self.coll,
                    self.pos,
                    want,
                    crate::map::AirOptions {
                        ignore_line: self.ignore_line,
                        skip_pass: false,
                        cliff: None,
                        ceil_heavy: false,
                    },
                    surfaces,
                );
                self.pos = result.moved.pos;
                // `mpCommonUpdateFighterKinetics`, `proc_map` in hitlag too:
                // a hit reaction launched from where it stood and nudged into
                // the floor lands there, and slides once the hitlag is over
                // (RE-466).
                if let Some(floor) = result.moved.floor.filter(|_| {
                    matches!(
                        self.status.status,
                        crate::status::AnyStatus::Common(s) if s.keeps_situation()
                    )
                }) {
                    self.map_contacts = result.contacts;
                    self.floor = Some(floor);
                    self.ignore_line = None;
                    self.land(result.moved.pos.y);
                }
            }
        }
    }

    /// `ftCommonDamageAirCommonProcMap`: the damage sweep
    /// ([`crate::map::move_damage`]), then the surface reaction.
    fn tick_damage_map<I, F>(&mut self, from: Vec3, want: Vec3, surfaces: &F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        // `mpCommonCheckFighterDamageCollision` rolls the masks first.
        self.reaction.coll_mask_prev = self.reaction.coll_mask_curr;
        let result = crate::map::move_damage_pushed(
            &self.coll,
            from,
            want,
            self.hazard.vel_push,
            self.reaction.coll_mask_prev,
            self.hitlag > 0,
            self.ignore_line,
            surfaces,
        );
        self.reaction.coll_mask_curr = result.mask_curr;
        self.map_contacts = result.contacts;
        self.pos = result.moved.pos;
        if let Some(floor) = result.moved.floor {
            self.floor = Some(floor);
            self.ignore_line = None;
        }
        crate::reaction::damage_air_proc_map(self, &result);
        if self.situation == Situation::Air {
            self.floor = None;
        }
    }

    /// The ground statuses' `proc_map` when `mpProcessUpdateMain` finds no
    /// floor under the fighter (`mpCommonSetFighterFallOnGroundBreak` and
    /// the specials' own air switches): the status falls or switches to its
    /// aerial counterpart.
    fn on_ground_break(&mut self) {
        crate::item_throw::on_floor_lost(self);
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
        } else if matches!(
            self.status.status,
            crate::status::AnyStatus::Common(s) if s.keeps_situation()
        ) {
            // `mpCommonUpdateFighterKinetics`: the hit reaction
            // carries on in the air.
            self.become_airborne();
            self.physics.jumps_used = 1;
        } else if !crate::item_use::on_ground_lost(self)
            && !crate::samus::on_ground_lost(self)
            && !crate::link::on_ground_lost(self)
            && !crate::yoshi::on_ground_lost(self)
            && !crate::captain::on_ground_lost(self)
            && !crate::kirby::on_ground_lost(self)
            && !crate::pikachu::on_ground_lost(self)
            && !crate::purin::on_ground_lost(self)
            && !crate::ness::on_ground_lost(self)
            && !crate::capture_yoshi::on_ground_lost(self)
            && !crate::grab::on_ground_lost(self)
        {
            self.become_airborne();
            crate::status::set_fall(self);
        }
    }

    fn tick_ground<I, F>(&mut self, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        let Some(standing) = self.floor else {
            // Grounded on no line: the rebirth halo's fake floor (`-2`,
            // `ftCommonRebirthDownSetStatus`). The first ground status after
            // the rebirth statuses finds no floor in its `proc_map` and falls
            // (RE-468: Luigi's down tap on the halo squats and falls on the
            // same frame).
            self.on_ground_break();
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
            crate::status::AnyStatus::Kirby(_)
            | crate::status::AnyStatus::Pikachu(_)
            | crate::status::AnyStatus::Purin(_)
            | crate::status::AnyStatus::Ness(_)
            | crate::status::AnyStatus::Boss(_) => crate::status::Status::Wait,
        };
        if crate::dokan::apply_ground_physics(self)
            || crate::item_use::apply_ground_physics(self)
            || crate::captain::apply_ground_physics(self)
            || crate::kirby::apply_ground_physics(self)
            || crate::pikachu::apply_ground_physics(self)
            || crate::purin::apply_ground_physics(self)
            || crate::ness::apply_ground_physics(self)
            || crate::reaction::apply_ground_physics(self)
        {
        } else if crate::status::uses_ground_friction_or_transn(self.status.status)
            && crate::motion::uses_transn(self.kind, self.status.status)
        {
            // `ftPhysicsApplyGroundFrictionOrTransN`: a clip with a TransN
            // joint moves the fighter by its step, scaled by TopN's
            // `attr->size` (`ftPhysicsApplyGroundVelTransN`).
            crate::physics::apply_ground_vel_transn(
                &mut self.physics,
                self.root_motion,
                self.topn_lr,
                self.attributes.size,
            );
        } else if self.status.status == crate::status::Status::LightThrowDash
            // `ftCommonAttackDash`'s `proc_physics` too (RE-468: How to
            // Play's dash attack kept the dash's speed).
            || self.status.status == crate::status::Status::AttackDash
            // `ftCommonTurnRunProcPhysics` is `ftPhysicsApplyGroundVelTransN`.
            || self.status.status == crate::status::Status::TurnRun
            || self.status.status
                == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialHi)
        {
            crate::physics::apply_ground_vel_transn(
                &mut self.physics,
                self.root_motion,
                self.topn_lr,
                self.attributes.size,
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
            // `ftPhysicsSetGroundVelAbsStickRange` works on the
            // facing-relative velocity the original keeps; this port's is
            // the world one.
            let relative = self.status.status.is_walk();
            if relative {
                self.physics.vel_ground.x *= self.facing.sign();
            }
            crate::status::apply_status_physics(
                &mut self.physics,
                &self.attributes,
                physics_status,
                self.status.anim_frame,
                self.input.stick_x,
                friction,
            );
            if relative {
                self.physics.vel_ground.x *= self.facing.sign();
            }
        }

        crate::thrown::damage_physics(self);
        crate::physics::update_damage_velocity(
            &mut self.physics,
            true,
            standing.normal,
            friction,
            self.attributes.traction,
        );
        // `ftPhysicsSetGroundVelTransferAir`: the ground speed runs along
        // the floor, `lr * floor_angle.y * vel_ground.x` across (RE-468: on
        // the jungle's slope Samus's 56 is 54.97).
        let ground_x = if self.status.status
            == crate::status::AnyStatus::Pikachu(crate::status::PikachuStatus::SpecialHi)
        {
            self.physics.vel_air.x
        } else {
            self.physics.vel_ground.x * standing.normal.y
        };
        // `ftPhysicsSetGroundVelTransferAir` keeps this frame's step in
        // `vel_air`, jostle included; leaving the ground
        // (`mpCommonSetFighterAir`) carries it on (RE-468: Mario's Tornado
        // out of a jostle keeps its -6.75).
        if self.status.status
            != crate::status::AnyStatus::Pikachu(crate::status::PikachuStatus::SpecialHi)
        {
            self.physics.vel_air = Vec3::new(
                ground_x + self.physics.vel_jostle_x,
                -standing.normal.x * self.physics.vel_ground.x,
                ground_vel_z(self.pos.z, self.physics.vel_jostle_z, 0.0),
            );
        }
        // `ftPhysicsSetGroundVelTransferAir`: the jostle adds to the step.
        let want = Vec3::new(
            self.pos.x + ground_x + self.physics.vel_jostle_x + self.physics.vel_knockback.x,
            self.pos.y,
            self.pos.z
                + ground_vel_z(
                    self.pos.z,
                    self.physics.vel_jostle_z,
                    self.physics.vel_ground.z,
                ),
        );
        let stop_edge = crate::map::stops_at_edge(self.status.status);
        if self.status.status == crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialHi)
        {
            self.fox_special_hi.pass_timer = self.fox_special_hi.pass_timer.saturating_add(1);
        }
        let (moved, contacts) = crate::map::move_ground_pushed(
            &self.coll,
            self.pos,
            want,
            self.hazard.vel_push,
            standing.line,
            stop_edge,
            &surfaces,
        );
        self.map_contacts = contacts;
        self.pos = moved.pos;

        if crate::map::ground_callback(self, moved.floor) {
            return;
        }

        match moved.floor {
            Some(f) => self.floor = Some(f),
            // Walked off the end of the line. `mpCommonSetFighterFallOnGroundBreak`
            // drops the fighter into a fall rather than leaving it in a walk
            // with no ground under it.
            None => {
                self.floor = None;
                self.on_ground_break();
            }
        }
    }

    fn tick_air<I, F>(&mut self, surfaces: F)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = crate::weapon::MapSurface>,
    {
        if crate::map::is_cliff_hold(self.status.status) {
            self.physics = crate::physics::PhysicsState::default();
            return;
        }
        // `ftPhysicsCheckSetFastFall` runs from every airborne status's own
        // `proc_physics` in the original; here it is the one thing every
        // airborne tick does regardless of status.
        let special_air_hi = self.status.status
            == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialAirHi);
        // The grounded Super Jump once `SetAirJumpMax` lifts it off
        // (`ftMarioSpecialHiProcPhysics`, `is_air_bool == FALSE`).
        let special_hi_lifted = self.status.status
            == crate::status::AnyStatus::Mario(crate::status::MarioStatus::SpecialHi);
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
        // `StopCeil` has no `proc_physics`: the bonk holds its velocity.
        let stop_ceil = self.status.status == crate::status::Status::StopCeil;
        // `ftCommonDamageCommonProcPhysics`: until hitstun runs out a hit
        // reaction takes gravity and air friction but no drift and no
        // fast-fall input (`ftPhysicsApplyAirVelFriction`).
        let damage_hitstun = self.hitstun > 0
            && matches!(
                self.status.status,
                crate::status::AnyStatus::Common(s) if s.keeps_situation()
                    || matches!(
                        s,
                        crate::status::Status::DamageE1
                            | crate::status::Status::DamageE2
                            | crate::status::Status::DamageFlyHi
                            | crate::status::Status::DamageFlyN
                            | crate::status::Status::DamageFlyLw
                            | crate::status::Status::DamageFlyTop
                            | crate::status::Status::DamageFlyRoll
                            | crate::status::Status::WallDamage
                    )
            );
        if !special_air_hi
            && !special_hi_lifted
            && !special_air_lw
            && !fox_special_hi
            && !fox_special_lw
            && !donkey_special_hi
            && !crate::item_use::skips_fast_fall(self.status.status)
            && !crate::samus::skips_fast_fall(self.status.status)
            && !crate::link::skips_fast_fall(self.status.status)
            && !crate::yoshi::skips_fast_fall(self)
            && !crate::captain::skips_fast_fall(self.status.status)
            && !crate::kirby::skips_fast_fall(self.status.status)
            && !crate::purin::skips_fast_fall(self.status.status)
            && !crate::ness::skips_fast_fall(self)
            && !crate::capture_kirby::is_star(self.status.status)
            && !damage_hitstun
            && !stop_ceil
        {
            crate::status::check_set_fast_fall(self);
        }
        if stop_ceil {
        } else if special_air_hi {
            crate::status::apply_mario_special_air_hi_physics(self);
        } else if special_hi_lifted {
            // `ftPhysicsApplyAirVelTransNAll`, undamped.
            crate::physics::apply_air_vel_transn_all(
                &mut self.physics,
                self.root_motion,
                self.facing.sign(),
                self.attributes.size,
            );
        } else if special_air_lw {
            crate::status::apply_mario_special_lw_air_physics(self);
        } else if fox_special_hi {
            crate::status::apply_fox_special_hi_air_physics(self);
        } else if fox_special_lw {
            crate::status::apply_fox_special_lw_air_physics(self);
        } else if donkey_special_hi {
            crate::status::apply_donkey_special_hi_air_physics(self);
        } else if crate::reaction::apply_air_physics(self)
            || crate::samus::apply_air_physics(self)
            || crate::link::apply_air_physics(self)
            || crate::yoshi::apply_air_physics(self)
            || crate::captain::apply_air_physics(self)
            || crate::kirby::apply_air_physics(self)
            || crate::pikachu::apply_air_physics(self)
            || crate::purin::apply_air_physics(self)
            || crate::ness::apply_air_physics(self)
            || crate::capture_kirby::apply_air_physics(self)
        {
        } else if damage_hitstun {
            if self.physics.is_fastfall {
                crate::physics::apply_fast_fall(&mut self.physics, &self.attributes);
            } else {
                crate::physics::apply_gravity_default(&mut self.physics, &self.attributes);
            }
            if !crate::physics::check_clamp_air_vel_x_dec(
                &mut self.physics,
                self.attributes.air_speed_max_x,
            ) {
                crate::physics::apply_air_friction(&mut self.physics, &self.attributes);
            }
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

        crate::thrown::damage_physics(self);
        crate::physics::update_damage_velocity(
            &mut self.physics,
            false,
            ssb_engine::math::Vec2::new(0.0, 1.0),
            0.0,
            self.attributes.traction,
        );
        let v = crate::physics::total_velocity(&self.physics, false);
        let want = Vec3::new(
            self.pos.x + v.x,
            self.pos.y + v.y,
            self.pos.z + crate::physics::clamp_z_velocity(self.pos.z, v.z),
        );

        let skip_pass = self.status.status
            == crate::status::AnyStatus::Pikachu(crate::status::PikachuStatus::SpecialAirHi)
            && self.pikachu.pass_timer < 2
            || crate::ness::skip_pass(self)
            || self.status.status == crate::status::Status::FallSpecial
                && self.fall_special.is_allow_pass
                && self.input.stick_y < -44;
        let skip_pass = if self.status.status
            == crate::status::AnyStatus::Fox(crate::status::FoxStatus::SpecialAirHi)
        {
            self.fox_special_hi.pass_timer = self.fox_special_hi.pass_timer.saturating_add(1);
            skip_pass || self.fox_special_hi.pass_timer < 15
        } else {
            skip_pass
        };
        let from = self.pos;
        if matches!(
            self.status.status,
            crate::status::AnyStatus::Common(s) if crate::reaction::is_damage_air(s)
        ) {
            self.tick_damage_map(from, want, &surfaces);
            return;
        }
        let cliff = crate::map::allows_cliff(self).then_some(crate::map::CliffQuery {
            facing: self.facing.sign(),
            wait: self.cliffcatch_wait,
            reach: self.cliff_reach,
            occupied: self.occupied_cliffs.map(|c| c.map(|(l, f)| (l, f.sign()))),
        });
        let result = crate::map::move_air_pushed(
            &self.coll,
            from,
            want,
            self.hazard.vel_push,
            crate::map::AirOptions {
                ignore_line: self.ignore_line,
                skip_pass,
                cliff,
                ceil_heavy: crate::map::is_ceil_heavy_status(self.status.status)
                    && self.physics.vel_air.y >= crate::map::CEILHEAVY_VEL_Y_MIN,
            },
            &surfaces,
        );
        let moved = result.moved;
        self.map_contacts = result.contacts;
        self.pos.x = moved.pos.x;
        self.pos.z = moved.pos.z;

        if moved.floor.is_none() {
            self.pos.y = moved.pos.y;
            if let Some((line, corner)) = result.cliff {
                crate::status::set_cliff_catch(self, line, corner);
                return;
            }
            // `mpCommonProcFighterCliffFloorCeil`: no ledge and no floor,
            // but a heavy ceiling bonk.
            if result.ceil_stop {
                crate::reaction::set_stop_ceil(self);
                self.floor = None;
                return;
            }
            crate::map::air_callback(self);
        }

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
                if crate::item_use::on_landing(self) {
                    self.pos.y = moved.pos.y;
                    return;
                }
                if crate::status::mario_special_hi_on_landing(self, moved.pos.y) {
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
                if crate::kirby::on_landing(self, moved.pos.y)
                    || crate::pikachu::on_landing(self, moved.pos.y, f.normal)
                    || crate::purin::on_landing(self, moved.pos.y)
                    || crate::ness::on_landing(self, moved.pos.y, f.normal)
                {
                    return;
                }
                if crate::capture_kirby::on_landing(self, moved.pos.y, f.normal) {
                    self.floor = None;
                    return;
                }
                if crate::capture_yoshi::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::grab::on_landing(self, moved.pos.y) {
                    return;
                }
                if crate::reaction::on_landing(self, moved.pos.y) {
                    return;
                }
                if matches!(
                    self.status.status,
                    crate::status::AnyStatus::Common(s) if s.keeps_situation()
                ) {
                    // `mpCommonUpdateFighterKinetics` lands the fighter
                    // without leaving the hit reaction
                    // (`mpCommonSetFighterGround`).
                    self.land(moved.pos.y);
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

/// What [`jostle`] reads of another fighter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JostleBody {
    pub pos: Vec3,
    pub lr: f32,
    pub jostle_x: f32,
    pub jostle_width: f32,
    /// The floor line, when grounded.
    pub line: Option<u16>,
    /// `capture_gobj != NULL`.
    pub is_captured: bool,
}

impl JostleBody {
    pub fn of(f: &Fighter) -> Self {
        JostleBody {
            pos: f.pos,
            lr: f.facing.sign(),
            jostle_x: f.jostle_x,
            jostle_width: f.jostle_width,
            line: f.floor.map(|s| s.line).filter(|_| f.is_grounded()),
            is_captured: f.grab.capture.is_some(),
        }
    }
}

/// `ftMainProcUpdateInterrupt`'s jostle, after the status's interrupt and
/// outside hitlag: a grounded fighter overlapping another grounded fighter
/// on its floor line (each `jostle_width` either side of its position plus
/// `jostle_x` forward) is pushed 6.75 a frame apart, and 3 in depth.
/// `others` are the other fighters in link order, each with whether it
/// comes after this one (`is_check_self`).
pub fn jostle(f: &mut Fighter, others: &[(JostleBody, bool)]) {
    f.physics.vel_jostle_x = 0.0;
    f.physics.vel_jostle_z = 0.0;
    let Some(line) = f.floor.map(|s| s.line).filter(|_| f.is_grounded()) else {
        return;
    };
    if f.dokan.is_jostle_ignore {
        return;
    }
    let mut is_jostle = false;
    // A captured fighter takes the loop's `else` branch, as this one does.
    let mut passed_captured = false;
    for &(o, after_self) in others {
        if o.is_captured {
            passed_captured = true;
            continue;
        }
        let is_check_self = after_self || passed_captured;
        if o.line != Some(line) {
            continue;
        }
        let dist_x = (f.pos.x + f.jostle_x * f.facing.sign()) - (o.pos.x + o.jostle_x * o.lr);
        if dist_x.abs() >= f.jostle_width + o.jostle_width {
            continue;
        }
        is_jostle = true;
        let self_sign = if is_check_self { -1.0 } else { 1.0 };
        f.physics.vel_jostle_x += 6.75
            * if dist_x == 0.0 {
                self_sign
            } else if dist_x < 0.0 {
                -1.0
            } else {
                1.0
            };
        let dist_z = f.pos.z - o.pos.z;
        f.physics.vel_jostle_z += 3.0
            * if dist_z != 0.0 {
                if dist_z < 0.0 {
                    -1.0
                } else {
                    1.0
                }
            } else if dist_x == 0.0 {
                self_sign
            } else if dist_x < 0.0 {
                1.0
            } else {
                -1.0
            };
    }
    if !is_jostle && f.pos.z != 0.0 {
        f.physics.vel_jostle_z = if f.pos.z < 0.0 { 3.0 } else { -3.0 };
    }
}

/// `ftPhysicsSetGroundVelTransferAir`'s depth: the jostle's push, kept
/// from crossing the plane in one step and within ±60.
fn ground_vel_z(z: f32, jostle_z: f32, ground_z: f32) -> f32 {
    let mut v = jostle_z;
    if (jostle_z > 0.0 && z < 0.0 && z + v > 0.0) || (v < 0.0 && z > 0.0 && z + v < 0.0) {
        v = -z;
    }
    if v > 0.0 && z + v > 60.0 {
        v = 60.0 - z;
    } else if v < 0.0 && z + v < -60.0 {
        v = -60.0 - z;
    }
    v + ground_z
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spawn_at_or_right_of_centre_faces_left() {
        assert_eq!(Facing::at_spawn_x(0.0), Facing::Left);
        assert_eq!(Facing::at_spawn_x(1110.0), Facing::Left);
        assert_eq!(Facing::at_spawn_x(-1397.0), Facing::Right);
    }

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
        // The frame hitlag ends runs `proc_update`, which counts hitstun.
        f.tick_timers();
        assert_eq!(f.hitstun, 9);
    }

    #[test]
    fn hitstun_ending_leaves_the_damage_velocity_alone() {
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.hitstun = 1;
        f.physics.vel_knockback = Vec3::new(17.0, 0.0, 0.0);
        f.tick_timers();
        assert_eq!(f.hitstun, 0);
        assert_eq!(f.physics.vel_knockback, Vec3::new(17.0, 0.0, 0.0));
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
        use ssb_engine::input::N64Buttons;
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
        let taps = f.button_tap();
        assert!(taps.contains(N64Buttons::A) && taps.contains(N64Buttons::Z));
        assert!(taps.contains(N64Buttons::R));
    }
}
