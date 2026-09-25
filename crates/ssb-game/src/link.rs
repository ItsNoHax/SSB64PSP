//! Link's own statuses: `ftlinkspecialn.c` (Boomerang), `ftlinkspecialhi.c`
//! (Spin Attack), `ftlinkspeciallw.c` (Bomb), the jab finisher and the
//! rapid jab (`ftcommonattack1.c`, `ftcommonattack100.c`), with the motion
//! script events from `relocData/224_LinkMainMotion.c` (US).
//!
//! Callbacks run in the source order: `proc_update` ([`update`]), then
//! `proc_physics` ([`apply_air_physics`], or ground friction), then
//! `proc_map` ([`on_ground_lost`] / [`on_landing`]).
//!
//! The Boomerang is a match-owned weapon (`crate::weapon`). The pool reports
//! back through [`crate::weapon::WeaponPool::sync_owner`], which keeps
//! [`LinkState::boomerang_out`] and the catch status in step with it.
//!
//! The Spin Attack weapon is parented to Link and only Link's physics
//! callback drives it: its size, its position history and its lifetime.
//! The port keeps it as [`LinkState::spin`] and resolves its hits with
//! [`apply_spin_attack_hits`]. It therefore takes no slot in the weapon pool.
//!
//! The Bomb is an item (`itLinkBomb`). The item system is not ported, so the
//! Bomb statuses play their pull motion and create nothing.

use ssb_engine::input::{newly_pressed, newly_released, N64Buttons};
use ssb_engine::math::{sin_cos, Vec2, Vec3};

use crate::attack::Hitbox;
use crate::fighter::{Fighter, Situation};
use crate::physics;
use crate::status::{self, AnyStatus, LinkStatus, StatusTiming};
use crate::weapon::{WeaponKind, WeaponSpawn};

/// `FTLINK_BOOMERANG_SMASH_STICK_MIN` and `..._SMASH_BUFFER`.
pub const BOOMERANG_SMASH_STICK_MIN: i32 = 56;
pub const BOOMERANG_SMASH_BUFFER: u8 = 8;

/// `FTLINK_SPINATTACK_*` (US).
pub const SPINATTACK_GRAVITY_MUL: f32 = 0.23;
pub const SPINATTACK_AIR_DRIFT_MUL: f32 = 0.5;
pub const SPINATTACK_AIR_VEL_Y: f32 = 69.0;
pub const SPINATTACK_FALLSPECIAL_DRIFT: f32 = 0.6;
pub const SPINATTACK_LANDING_LAG: f32 = 0.65;

/// `WPSPINATTACK_*` (`wpvars.h`).
pub const WPSPINATTACK_LIFETIME: u16 = 100;
pub const WPSPINATTACK_VEL: f32 = 28.0;
pub const WPSPINATTACK_VEL_CLAMP: f32 = 0.4;
pub const WPSPINATTACK_OFF_X: f32 = 40.0;
pub const WPSPINATTACK_OFF_Y: f32 = 80.0;
pub const WPSPINATTACK_ANGLE: f32 = 0.174_532_94; // 10 degrees

/// `llLinkMainSpinAttackWeaponAttributes` (the words at offset 0x0C of
/// `225_LinkMain.c`, US): angle 30, knockback 50/0/30, 5 damage. The size
/// word (400) is replaced by the motion script's flag-2 sizes.
pub const SPIN_ATTACK_HITBOX: Hitbox = Hitbox {
    damage: 5,
    offset: Vec3::ZERO,
    radius: 0.0,
    angle: 30,
    kb_scale: 50,
    kb_weight: 0,
    kb_base: 30,
};

/// Figatree lengths (`ssb_rom::anim::EXPECTED_FRAMES`).
const ATTACK100_START_LENGTH: f32 = 8.0;
const ATTACK100_END_LENGTH: f32 = 11.0;
const SPECIAL_HI_LENGTH: f32 = 60.0;
const SPECIAL_HI_END_LENGTH: f32 = 40.0;
const SPECIAL_AIR_HI_LENGTH: f32 = 100.0;
const SPECIAL_N_LENGTH: f32 = 46.0;
const SPECIAL_N_GET_LENGTH: f32 = 20.0;
const SPECIAL_LW_LENGTH: f32 = 45.0;

/// `MissingBoomerang` (`0x1D2C`): `WaitAsync(26)` then `SetFlag0(1)`.
const BOOMERANG_SPAWN_FRAME: f32 = 26.0;
/// `Bomb`: `WaitAsync(29)` then `SetFlag0(1)`.
const BOMB_PULL_FRAME: f32 = 29.0;
/// `UpSpecial`: `SetFlag1(1)` at frame 12 restores full gravity.
const SPIN_FLAG1_FRAME: f32 = 12.0;
/// `UpSpecial`'s `SetFlag2` events: frame and weapon radius. `None` turns
/// the weapon's attack off (flag 2 = 13).
const SPIN_FLAG2_EVENTS: [(f32, Option<f32>); 4] = [
    (12.0, Some(120.0)),
    (20.0, Some(100.0)),
    (26.0, Some(80.0)),
    (30.0, None),
];

/// The Spin Attack weapon (`wpLinkSpinAttack`) and the fighter-side vars
/// that drive it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpinAttack {
    /// The weapon's `translate`.
    pub position: Vec3,
    /// `physics.vel_air`, set by its map callback for the next frame.
    pub vel: Vec2,
    /// `weapon_vars.spin_attack.pos_x/pos_y`: Link's `translate` for the
    /// last four physics frames, stored as `s16`.
    pub pos_x: [i16; 4],
    pub pos_y: [i16; 4],
    pub pos_index: usize,
    /// `weapon_vars.spin_attack.vel`: how far the two boxes spread.
    pub spread: Vec2,
    /// `attack_coll.offsets[0..2]`.
    pub offsets: [Vec2; 2],
    /// `attack_coll.size`, or `None` while the attack is off.
    pub radius: Option<f32>,
    pub lifetime: u16,
    /// Ports in the attack record. `can_rehit_fighter` is clear and the
    /// record is never cleared, so each target is hit at most once.
    pub hit_ports: u8,
}

impl SpinAttack {
    /// `wpLinkSpinAttackMakeWeapon`: TopN plus 80 up, boxes 40 either side.
    fn new(f: &Fighter) -> Self {
        let (sin, cos) = sin_cos(WPSPINATTACK_ANGLE);
        let x = f.pos.x as i16;
        let y = f.pos.y as i16;
        SpinAttack {
            position: f.pos + Vec3::new(0.0, WPSPINATTACK_OFF_Y, 0.0),
            vel: Vec2::new(0.0, 0.0),
            pos_x: [x; 4],
            pos_y: [y; 4],
            pos_index: 0,
            spread: Vec2::new(cos * WPSPINATTACK_VEL, sin * WPSPINATTACK_VEL),
            offsets: [
                Vec2::new(WPSPINATTACK_OFF_X, 0.0),
                Vec2::new(-WPSPINATTACK_OFF_X, 0.0),
            ],
            radius: None,
            lifetime: WPSPINATTACK_LIFETIME,
            hit_ports: 0,
        }
    }

    /// `wpLinkSpinAttackProcUpdate`, the manager's move, then
    /// `wpLinkSpinAttackProcMap`.
    fn step(&mut self) {
        let speed =
            ssb_engine::math::sqrt(self.spread.x * self.spread.x + self.spread.y * self.spread.y);
        if speed > 0.0 {
            let slowed = if speed < WPSPINATTACK_VEL_CLAMP {
                0.0
            } else {
                speed - WPSPINATTACK_VEL_CLAMP
            };
            self.spread.x = self.spread.x * slowed / speed;
            self.spread.y = self.spread.y * slowed / speed;
            self.offsets[0].x += self.spread.x;
            self.offsets[0].y += self.spread.y;
            self.offsets[1].x -= self.spread.x;
            self.offsets[1].y += self.spread.y;
        }
        self.position.x += self.vel.x;
        self.position.y += self.vel.y;
        let oldest = (self.pos_index + 1) % 4;
        self.vel.x = f32::from(self.pos_x[oldest]) - self.position.x;
        self.vel.y = f32::from(self.pos_y[oldest]) + WPSPINATTACK_OFF_Y - self.position.y;
    }

    /// World centres of the two boxes while the attack is on.
    pub fn hit_positions(&self) -> Option<[Vec3; 2]> {
        self.radius?;
        Some(
            self.offsets
                .map(|o| self.position + Vec3::new(o.x, o.y, 0.0)),
        )
    }
}

/// `FTLinkPassiveVars`, Link's status vars and the Attack100 vars.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinkState {
    /// `passive_vars.link.boomerang_gobj != NULL`.
    pub boomerang_out: bool,
    /// `status_vars.link.specialn.is_smash`.
    pub is_smash: bool,
    /// Motion flag 0 of the Boomerang, Bomb or Spin Attack script was
    /// consumed.
    pub flag0_done: bool,
    /// `status_vars.link.specialhi.spin_attack_gobj`.
    pub spin: Option<SpinAttack>,
    /// `status_vars.common.attack100.is_anim_end` and `.is_goto_loop`.
    pub rapid_is_anim_end: bool,
    pub rapid_is_goto_loop: bool,
    /// `status_vars.common.attackair.rehit_timer`.
    pub dair_rehit_timer: u8,
    /// The down air's boxes were cleared by a hit and not yet refreshed.
    pub dair_cleared: bool,
}

/// `FTCOMMON_ATTACKAIRLW_LINK_REHIT_*`.
pub const DAIR_REHIT_TIMER: u8 = 30;
pub const DAIR_REHIT_FRAME_BEGIN: f32 = 35.0;
pub const DAIR_REHIT_FRAME_END: f32 = 65.0;
pub const DAIR_REHIT_BOUNCE_VEL_Y: f32 = 40.0;

/// `ftCommonAttackAirLwProcHit`'s Link branch: the down air bounces off
/// what it hits and clears its boxes. Past frame 35 the status restarts at
/// frame 35, whose fast-forward skips the frame-5 `MakeAttackColl`.
pub fn on_attack_hit(f: &mut Fighter) {
    if f.kind != crate::fighter::FighterKind::Link || f.status.status != status::Status::AttackAirLw
    {
        return;
    }
    f.link.dair_cleared = true;
    f.physics.is_fastfall = false;
    f.physics.vel_air.y = DAIR_REHIT_BOUNCE_VEL_Y;
    if f.status.anim_frame > DAIR_REHIT_FRAME_BEGIN {
        let timing = f.status.timing;
        status::set_status(
            f,
            status::Status::AttackAirLw,
            DAIR_REHIT_FRAME_BEGIN,
            timing,
        );
    }
    f.link.dair_rehit_timer = DAIR_REHIT_TIMER;
}

/// `ftCommonAttackAirLwProcUpdate`'s Link branch: 30 frames after a hit,
/// the boxes come back with a fresh record if the swing is still before
/// frame 65. A hit past frame 35 therefore never refreshes.
pub fn update_attack_air_lw(f: &mut Fighter) {
    if f.link.dair_rehit_timer != 0 {
        f.link.dair_rehit_timer -= 1;
        if f.link.dair_rehit_timer == 0 && f.status.anim_frame < DAIR_REHIT_FRAME_END {
            f.link.dair_cleared = false;
        }
    }
    if f.status.animation_ended() {
        status::set_fall(f);
    }
}

/// Whether the attacker's current boxes were removed by its own
/// `proc_hit`.
pub fn attack_colls_cleared(f: &Fighter) -> bool {
    f.link.dair_cleared && f.status.status == status::Status::AttackAirLw
}

fn set(f: &mut Fighter, status: LinkStatus, frame: f32, length: f32) {
    status::set_any_status(
        f,
        AnyStatus::Link(status),
        frame,
        StatusTiming::frames(length),
    );
}

fn crossed(f: &Fighter, at: f32) -> bool {
    let frame = f.status.anim_frame;
    frame >= at && frame - f.status.timing.anim_speed < at
}

/// `ftLinkSpecialNProcStatus`.
fn special_n_proc_status(f: &mut Fighter) {
    f.link.flag0_done = false;
    f.link.is_smash = i32::from(f.input.stick_x).abs() >= BOOMERANG_SMASH_STICK_MIN
        && f.stick.hold_x < BOOMERANG_SMASH_BUFFER;
}

/// `ftLinkSpecialNSetStatus`.
pub fn set_special_n(f: &mut Fighter) {
    special_n_proc_status(f);
    if f.link.boomerang_out {
        set(f, LinkStatus::SpecialNEmpty, 0.0, SPECIAL_N_LENGTH);
        f.is_special_interrupt = true;
    } else {
        set(f, LinkStatus::SpecialN, 0.0, SPECIAL_N_LENGTH);
    }
}

/// `ftLinkSpecialAirNSetStatus`.
pub fn set_special_air_n(f: &mut Fighter) {
    special_n_proc_status(f);
    if f.link.boomerang_out {
        set(f, LinkStatus::SpecialAirNEmpty, 0.0, SPECIAL_N_LENGTH);
        f.is_special_interrupt = true;
    } else {
        set(f, LinkStatus::SpecialAirN, 0.0, SPECIAL_N_LENGTH);
    }
}

/// `ftLinkSpecialNGetSetStatus`, called by the weapon pool when a returning
/// Boomerang reaches a fighter whose `is_special_interrupt` is set.
pub fn set_special_n_get(f: &mut Fighter) {
    if f.situation == Situation::Air {
        set(f, LinkStatus::SpecialAirNReturn, 0.0, SPECIAL_N_GET_LENGTH);
    } else {
        set(f, LinkStatus::SpecialNGet, 0.0, SPECIAL_N_GET_LENGTH);
    }
}

/// `ftLinkSpecialNMakeBoomerang`: TopN, with the stick read at flag 0.
fn make_boomerang(f: &mut Fighter) {
    if f.link.flag0_done || f.status.anim_frame < BOOMERANG_SPAWN_FRAME {
        return;
    }
    f.link.flag0_done = true;
    f.weapon_spawn = Some(WeaponSpawn {
        kind: WeaponKind::LinkBoomerang {
            is_smash: f.link.is_smash,
            stick_x: f.input.stick_x,
            stick_y: f.input.stick_y,
        },
        owner_port: f.port,
        position: f.joint_world(0, Vec3::ZERO),
        facing: f.facing.sign(),
    });
    f.link.boomerang_out = true;
}

/// `ftLinkSpecialHiProcStatus` and `ftLinkSpecialHiSetStatus`. Flag 0 is
/// set at frame 0, so the weapon is made on this frame's physics.
pub fn set_special_hi(f: &mut Fighter) {
    destroy_spin(f);
    set(f, LinkStatus::SpecialHi, 0.0, SPECIAL_HI_LENGTH);
    let mut spin = SpinAttack::new(f);
    spin.step();
    f.link.spin = Some(spin);
    f.link.flag0_done = true;
}

/// `ftLinkSpecialAirHiSetStatus`. The aerial Spin Attack makes no weapon
/// (`ftLinkSpecialHiMakeWeapon(fighter_gobj, TRUE)`).
pub fn set_special_air_hi(f: &mut Fighter) {
    destroy_spin(f);
    set(f, LinkStatus::SpecialAirHi, 0.0, SPECIAL_AIR_HI_LENGTH);
    f.link.flag0_done = true;
    f.physics.vel_air.y = SPINATTACK_AIR_VEL_Y;
    f.physics.jumps_used = f.attributes.jumps_max;
}

/// `ftLinkSpecialHiEndSetStatus`.
fn set_special_hi_end(f: &mut Fighter) {
    set(f, LinkStatus::SpecialHiEnd, 0.0, SPECIAL_HI_END_LENGTH);
}

/// `ftLinkSpecialHiDestroyWeapon`.
fn destroy_spin(f: &mut Fighter) {
    f.link.spin = None;
}

/// `ftLinkSpecialHiProcDamage`, which all three Spin Attack statuses
/// install.
pub fn on_damage(f: &mut Fighter) {
    if is_spin_status(f.status.status) {
        destroy_spin(f);
    }
}

fn is_spin_status(status: AnyStatus) -> bool {
    matches!(
        status,
        AnyStatus::Link(
            LinkStatus::SpecialHi | LinkStatus::SpecialHiEnd | LinkStatus::SpecialAirHi
        )
    )
}

/// `ftLinkSpecialHiUpdateWeaponVars`: flag-2 size, position history and
/// lifetime, from `ftLinkSpecialHiProcPhysics` / `...AirHiProcPhysics`.
fn update_spin_vars(f: &mut Fighter) {
    let pos = f.pos;
    let events: [bool; 4] = SPIN_FLAG2_EVENTS.map(|(at, _)| crossed(f, at));
    let Some(spin) = f.link.spin.as_mut() else {
        return;
    };
    for (event, (_, radius)) in events.iter().zip(SPIN_FLAG2_EVENTS) {
        if *event {
            spin.radius = radius;
        }
    }
    spin.pos_index = (spin.pos_index + 1) % 4;
    spin.pos_x[spin.pos_index] = pos.x as i16;
    spin.pos_y[spin.pos_index] = pos.y as i16;
    spin.lifetime -= 1;
    if spin.lifetime == 0 {
        f.link.spin = None;
    }
}

/// `ftLinkSpecialLwSetStatus`. Holding an item would throw it instead; no
/// item can be held yet.
pub fn set_special_lw(f: &mut Fighter) {
    f.link.flag0_done = false;
    set(f, LinkStatus::SpecialLw, 0.0, SPECIAL_LW_LENGTH);
}

/// `ftLinkSpecialAirLwSetStatus`.
pub fn set_special_air_lw(f: &mut Fighter) {
    f.link.flag0_done = false;
    set(f, LinkStatus::SpecialAirLw, 0.0, SPECIAL_LW_LENGTH);
}

/// `ftLinkSpecialLwMakeBomb`. `itLinkBombMakeItem` needs the item system,
/// so the flag is consumed with no item.
fn make_bomb(f: &mut Fighter) {
    if !f.link.flag0_done && f.status.anim_frame >= BOMB_PULL_FRAME {
        f.link.flag0_done = true;
    }
}

/// `ftCommonAttack100StartSetStatus`'s Link case.
pub fn set_attack100_start(f: &mut Fighter) {
    set(f, LinkStatus::Attack100Start, 0.0, ATTACK100_START_LENGTH);
    f.link.rapid_is_anim_end = false;
    f.link.rapid_is_goto_loop = false;
}

/// `ftCommonAttack100LoopSetStatus`. The vars carry over.
fn set_attack100_loop(f: &mut Fighter) {
    set(
        f,
        LinkStatus::Attack100Loop,
        0.0,
        crate::link_attack::RAPID_LOOP_LENGTH,
    );
}

/// Link's `proc_update` and `proc_interrupt` callbacks, then the physics
/// half that drives the Spin Attack weapon.
pub fn update(f: &mut Fighter) {
    let AnyStatus::Link(current) = f.status.status else {
        return;
    };
    match current {
        // `ftCommonAttack13ProcUpdate`: the `Attack100` branch is Captain's.
        LinkStatus::Attack13 | LinkStatus::Attack100End | LinkStatus::SpecialNGet => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        LinkStatus::Attack100Start => {
            if f.status.animation_ended() {
                set_attack100_loop(f);
            }
        }
        LinkStatus::Attack100Loop => update_attack100_loop(f),
        LinkStatus::SpecialN => {
            make_boomerang(f);
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        LinkStatus::SpecialAirN => {
            make_boomerang(f);
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        LinkStatus::SpecialNEmpty => {
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        LinkStatus::SpecialAirNReturn | LinkStatus::SpecialAirNEmpty => {
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        LinkStatus::SpecialLw => {
            make_bomb(f);
            if f.status.animation_ended() {
                status::set_wait(f);
            }
        }
        LinkStatus::SpecialAirLw => {
            make_bomb(f);
            if f.status.animation_ended() {
                status::set_fall(f);
            }
        }
        LinkStatus::SpecialHi => {
            if f.status.animation_ended() {
                set_special_hi_end(f);
            } else {
                update_spin_vars(f);
            }
        }
        LinkStatus::SpecialHiEnd => {
            if f.status.animation_ended() {
                destroy_spin(f);
                status::set_wait(f);
            }
        }
        LinkStatus::SpecialAirHi => {
            if f.status.animation_ended() {
                destroy_spin(f);
                status::set_fall_special(
                    f,
                    SPINATTACK_FALLSPECIAL_DRIFT,
                    true,
                    true,
                    SPINATTACK_LANDING_LAG,
                    false,
                );
            } else {
                update_spin_vars(f);
            }
        }
    }
    step_spin(f);
}

/// The weapon manager's pass over the Spin Attack weapon. A weapon whose
/// owner has left the Spin Attack statuses is removed: nothing would
/// update or destroy it any more.
fn step_spin(f: &mut Fighter) {
    if !is_spin_status(f.status.status) {
        f.link.spin = None;
    }
    if let Some(spin) = f.link.spin.as_mut() {
        spin.step();
    }
}

/// `ftCommonAttack100LoopProcUpdate` and `...ProcInterrupt`. The loop
/// figatree wraps every 35 frames; the paused script's fifth flag 1 fires on
/// the wrap, which is also the frame `is_anim_end` becomes set.
fn update_attack100_loop(f: &mut Fighter) {
    let wrapped = f.status.animation_ended();
    if wrapped {
        f.link.rapid_is_anim_end = true;
    }
    let flag1 = crate::link_attack::RAPID_FLAG1_FRAMES
        .iter()
        .any(|&at| crossed(f, at));
    if flag1 {
        if f.link.rapid_is_anim_end && !f.link.rapid_is_goto_loop {
            set(f, LinkStatus::Attack100End, 0.0, ATTACK100_END_LENGTH);
            return;
        }
        f.link.rapid_is_goto_loop = false;
    }
    if wrapped {
        set_attack100_loop(f);
    }
    if newly_pressed(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A)
        || newly_released(f.prev_input.buttons, f.input.buttons).contains(N64Buttons::A)
    {
        f.link.rapid_is_goto_loop = true;
    }
}

/// Aerial `proc_physics` for Link's airborne statuses. Returns `false` for
/// statuses this module does not own.
pub fn apply_air_physics(f: &mut Fighter) -> bool {
    let AnyStatus::Link(current) = f.status.status else {
        return false;
    };
    let attr = f.attributes;
    match current {
        // `ftPhysicsApplyAirVelFriction`: no drift input.
        LinkStatus::SpecialAirN
        | LinkStatus::SpecialAirNReturn
        | LinkStatus::SpecialAirNEmpty
        | LinkStatus::SpecialAirLw => {
            if f.physics.is_fastfall {
                physics::apply_fast_fall(&mut f.physics, &attr);
            } else {
                physics::apply_gravity_default(&mut f.physics, &attr);
            }
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        // `ftLinkSpecialAirHiProcPhysics`.
        LinkStatus::SpecialAirHi => {
            let gravity = if f.status.anim_frame >= SPIN_FLAG1_FRAME {
                attr.gravity
            } else {
                attr.gravity * SPINATTACK_GRAVITY_MUL
            };
            physics::apply_gravity_clamp_tvel(&mut f.physics, gravity, attr.tvel_base);
            if !physics::check_clamp_air_vel_x_dec(&mut f.physics, attr.air_speed_max_x) {
                physics::clamp_air_vel_x_stick_range(
                    &mut f.physics,
                    f.input.stick_x,
                    physics::AIRDRIFT_STICK_MIN,
                    attr.air_accel * SPINATTACK_AIR_DRIFT_MUL,
                    attr.air_speed_max_x,
                );
                physics::apply_air_friction(&mut f.physics, &attr);
            }
        }
        _ => return false,
    }
    true
}

/// Grounded `proc_map` callbacks after the floor ran out. Returns `false`
/// when the common fall applies (`mpCommonSetFighterFallOnEdgeBreak`,
/// `...OnGroundBreak`).
pub fn on_ground_lost(f: &mut Fighter) -> bool {
    let AnyStatus::Link(current) = f.status.status else {
        return false;
    };
    let frame = f.status.anim_frame;
    match current {
        // `ftLinkSpecialNSwitchStatusAir`.
        LinkStatus::SpecialN => {
            f.become_airborne();
            set(f, LinkStatus::SpecialAirN, frame, SPECIAL_N_LENGTH);
        }
        // `ftLinkSpecialNEmptySwitchStatusAir`.
        LinkStatus::SpecialNEmpty => {
            f.become_airborne();
            set(f, LinkStatus::SpecialAirNEmpty, frame, SPECIAL_N_LENGTH);
            f.is_special_interrupt = true;
        }
        // `ftLinkSpecialLwProcMap`.
        LinkStatus::SpecialLw => {
            f.become_airborne();
            set(f, LinkStatus::SpecialAirLw, frame, SPECIAL_LW_LENGTH);
        }
        // `ftLinkSpecialHiProcMap`: the attack and the weapon carry over.
        LinkStatus::SpecialHi => {
            f.become_airborne();
            set(f, LinkStatus::SpecialAirHi, frame, SPECIAL_AIR_HI_LENGTH);
            f.physics.jumps_used = f.attributes.jumps_max;
        }
        // `ftLinkSpecialHiEndProcMap`.
        LinkStatus::SpecialHiEnd => {
            destroy_spin(f);
            return false;
        }
        _ => return false,
    }
    true
}

/// Aerial `proc_map` callbacks on floor contact. The caller has already
/// recorded the floor. Returns `false` when the common landing applies.
pub fn on_landing(f: &mut Fighter, floor_y: f32) -> bool {
    let AnyStatus::Link(current) = f.status.status else {
        return false;
    };
    let frame = f.status.anim_frame;
    match current {
        // `ftLinkSpecialAirNSwitchStatusGround`.
        LinkStatus::SpecialAirN => {
            f.land(floor_y);
            set(f, LinkStatus::SpecialN, frame, SPECIAL_N_LENGTH);
        }
        // `ftLinkSpecialAirNEmptySwitchStatusGround`.
        LinkStatus::SpecialAirNEmpty => {
            f.land(floor_y);
            set(f, LinkStatus::SpecialNEmpty, frame, SPECIAL_N_LENGTH);
            f.is_special_interrupt = true;
        }
        // `ftLinkSpecialAirLwProcMap`.
        LinkStatus::SpecialAirLw => {
            f.land(floor_y);
            set(f, LinkStatus::SpecialLw, frame, SPECIAL_LW_LENGTH);
        }
        // `ftLinkSpecialAirHiProcMap`. Its ledge catch waits on ledge
        // detection.
        LinkStatus::SpecialAirHi => {
            destroy_spin(f);
            f.land(floor_y);
            set_special_hi_end(f);
        }
        _ => return false,
    }
    true
}

/// No Link status calls `ftPhysicsCheckSetFastFall`.
pub fn skips_fast_fall(status: AnyStatus) -> bool {
    matches!(status, AnyStatus::Link(_))
}

/// The Spin Attack weapon's collision phase against one fighter.
pub fn apply_spin_attack_hits(attacker: &mut Fighter, defender: &mut Fighter) {
    if attacker.port == defender.port {
        return;
    }
    let Some(spin) = attacker.link.spin.as_mut() else {
        return;
    };
    let Some(radius) = spin.radius else {
        return;
    };
    let bit = 1u8 << (defender.port & 7);
    if spin.hit_ports & bit != 0 {
        return;
    }
    let hitbox = Hitbox {
        radius,
        ..SPIN_ATTACK_HITBOX
    };
    for position in spin.hit_positions().into_iter().flatten() {
        if crate::attack::apply_hitbox_at(&hitbox, position, defender) {
            spin.hit_ports |= bit;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind;
    use crate::status::Status;
    use ssb_engine::input::ControllerState;

    fn link(ground: bool) -> Fighter {
        let mut f = Fighter::new(FighterKind::Link, 0, 3);
        f.attributes.jumps_max = 2;
        f.attributes.gravity = 2.0;
        f.attributes.tvel_base = 60.0;
        f.attributes.air_speed_max_x = 30.0;
        if ground {
            f.situation = Situation::Ground;
        }
        f
    }

    fn press(f: &mut Fighter, buttons: u16) {
        let input = ControllerState {
            buttons: N64Buttons(buttons),
            ..Default::default()
        };
        f.set_input(input, false, false);
    }

    #[test]
    fn boomerang_leaves_on_frame_26_and_an_outstanding_one_empties_the_throw() {
        let mut f = link(true);
        set_special_n(&mut f);
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::SpecialN));
        for _ in 0..25 {
            status::update(&mut f);
            assert!(f.weapon_spawn.is_none());
        }
        status::update(&mut f);
        let spawn = f.take_weapon_spawn().expect("flag 0 at frame 26");
        assert!(matches!(
            spawn.kind,
            WeaponKind::LinkBoomerang {
                is_smash: false,
                ..
            }
        ));
        assert!(f.link.boomerang_out);
        while f.status.status != Status::Wait {
            status::update(&mut f);
        }
        assert!(f.is_special_interrupt);
        set_special_n(&mut f);
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::SpecialNEmpty));
        assert!(f.is_special_interrupt);
        for _ in 0..30 {
            status::update(&mut f);
        }
        assert!(f.weapon_spawn.is_none());
    }

    #[test]
    fn a_fresh_full_stick_press_makes_a_smash_boomerang() {
        let mut f = link(true);
        f.set_input(
            ControllerState {
                stick_x: 80,
                ..Default::default()
            },
            false,
            false,
        );
        assert_eq!(f.stick.hold_x, 1);
        set_special_n(&mut f);
        assert!(f.link.is_smash);
        let mut held = link(true);
        for _ in 0..8 {
            held.set_input(
                ControllerState {
                    stick_x: 80,
                    ..Default::default()
                },
                false,
                false,
            );
        }
        set_special_n(&mut held);
        assert!(!held.link.is_smash);
    }

    #[test]
    fn ground_spin_attack_grows_its_weapon_on_the_flag2_frames() {
        let mut f = link(true);
        set_special_hi(&mut f);
        let spin = f.link.spin.expect("flag 0 at frame 0 makes the weapon");
        assert_eq!(spin.radius, None);
        for frame in 1..=40 {
            status::update(&mut f);
            let radius = f.link.spin.and_then(|s| s.radius);
            let expected = match frame {
                12..=19 => Some(120.0),
                20..=25 => Some(100.0),
                26..=29 => Some(80.0),
                _ => None,
            };
            assert_eq!(radius, expected, "frame {frame}");
        }
        // The boxes spread outward from ±40 as the spread speed decays.
        let spin = f.link.spin.unwrap();
        assert!(spin.offsets[0].x > 500.0);
        assert_eq!(spin.offsets[1].x, -spin.offsets[0].x);
        while f.status.status == AnyStatus::Link(LinkStatus::SpecialHi) {
            status::update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::SpecialHiEnd));
        while f.status.status == AnyStatus::Link(LinkStatus::SpecialHiEnd) {
            status::update(&mut f);
        }
        assert_eq!(f.status.status, Status::Wait);
        assert!(f.link.spin.is_none());
    }

    #[test]
    fn spin_attack_weapon_hits_each_fighter_once() {
        let mut f = link(true);
        set_special_hi(&mut f);
        for _ in 0..12 {
            status::update(&mut f);
        }
        let spin = f.link.spin.unwrap();
        let [right, _] = spin.hit_positions().unwrap();
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.situation = Situation::Ground;
        target.pos = right;
        apply_spin_attack_hits(&mut f, &mut target);
        assert_eq!(target.damage, 5);
        target.hitlag = 0;
        apply_spin_attack_hits(&mut f, &mut target);
        assert_eq!(target.damage, 5);
    }

    #[test]
    fn aerial_spin_attack_launches_floats_then_falls_helplessly() {
        let mut f = link(false);
        set_special_air_hi(&mut f);
        assert_eq!(f.physics.vel_air.y, SPINATTACK_AIR_VEL_Y);
        assert!(f.link.spin.is_none());
        apply_air_physics(&mut f);
        assert_eq!(
            f.physics.vel_air.y,
            SPINATTACK_AIR_VEL_Y - 2.0 * SPINATTACK_GRAVITY_MUL
        );
        f.status.anim_frame = 12.0;
        let before = f.physics.vel_air.y;
        apply_air_physics(&mut f);
        assert_eq!(f.physics.vel_air.y, before - 2.0);
        f.status.anim_frame = 99.0;
        status::update(&mut f);
        assert_eq!(f.status.status, Status::FallSpecial);
        assert!(f.fall_special.is_goto_landing);
    }

    #[test]
    fn aerial_spin_attack_lands_into_spin_end() {
        let mut f = link(false);
        set_special_air_hi(&mut f);
        assert!(on_landing(&mut f, 0.0));
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::SpecialHiEnd));
        assert_eq!(f.status.anim_frame, 0.0);
        assert!(f.is_grounded());
    }

    #[test]
    fn rapid_jab_ends_only_after_a_full_loop_without_input() {
        let mut f = link(true);
        set_attack100_start(&mut f);
        for _ in 0..8 {
            press(&mut f, 0);
            status::update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::Attack100Loop));
        // No input: the first cycle still plays through to the wrap.
        for _ in 0..34 {
            press(&mut f, 0);
            status::update(&mut f);
            assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::Attack100Loop));
        }
        press(&mut f, 0);
        status::update(&mut f);
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::Attack100End));

        // Mashing keeps it looping past the wrap.
        set_attack100_start(&mut f);
        let mut held = false;
        for _ in 0..(8 + 35 + 20) {
            held = !held;
            press(&mut f, if held { N64Buttons::A } else { 0 });
            status::update(&mut f);
        }
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::Attack100Loop));
    }

    #[test]
    fn down_air_bounces_and_rehits_only_when_hit_early() {
        let mut f = link(false);
        let mut target = Fighter::new(FighterKind::Mario, 1, 3);
        target.situation = Situation::Ground;
        let mut record = crate::attack::HitRecord::default();
        status::set_air_attack(&mut f, Status::AttackAirLw);
        for _ in 0..10 {
            status::update(&mut f);
        }
        target.pos = f.joint_world(11, Vec3::new(0.0, 0.0, 100.0));
        assert!(crate::attack::apply_hit_from(&f, &mut target, &mut record));
        on_attack_hit(&mut f);
        assert_eq!(f.physics.vel_air.y, DAIR_REHIT_BOUNCE_VEL_Y);
        assert_eq!(f.status.anim_frame, 10.0);
        target.hitlag = 0;
        for _ in 0..29 {
            status::update(&mut f);
            assert!(!crate::attack::apply_hit_from(&f, &mut target, &mut record));
        }
        status::update(&mut f);
        assert_eq!(f.status.anim_frame, 40.0);
        assert!(crate::attack::apply_hit_from(&f, &mut target, &mut record));

        // A hit past frame 35 restarts the swing at 35 and never refreshes.
        let mut late = link(false);
        status::set_air_attack(&mut late, Status::AttackAirLw);
        late.status.anim_frame = 50.0;
        on_attack_hit(&mut late);
        assert_eq!(late.status.anim_frame, DAIR_REHIT_FRAME_BEGIN);
        for _ in 0..30 {
            status::update(&mut late);
        }
        assert!(attack_colls_cleared(&late));
    }

    #[test]
    fn bomb_pull_plays_through_and_switches_situation_with_its_frame() {
        let mut f = link(true);
        set_special_lw(&mut f);
        for _ in 0..10 {
            status::update(&mut f);
        }
        assert!(on_ground_lost(&mut f));
        assert_eq!(f.status.status, AnyStatus::Link(LinkStatus::SpecialAirLw));
        assert_eq!(f.status.anim_frame, 10.0);
        while f.status.status == AnyStatus::Link(LinkStatus::SpecialAirLw) {
            status::update(&mut f);
        }
        assert!(f.link.flag0_done);
        assert!(matches!(
            f.status.status,
            AnyStatus::Common(Status::Fall | Status::FallAerial)
        ));
    }
}
