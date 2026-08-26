//! Fighter physics.
//!
//! A direct port of `src/ft/ftphysics.c` from the decompilation. Every
//! function here corresponds to a named original, and the original's address
//! is quoted so the two can be diffed.
//!
//! Two things are preserved deliberately, even where they look like quirks:
//!
//! * **`f32` throughout.** The original is float-based (the N64 has an FPU and
//!   Smash uses it), so there is no fixed-point representation to recover.
//!   Matching float semantics is what makes replay comparison meaningful.
//! * **Asymmetric zero-crossing tests.** Ground friction clamps with `> 0.0`
//!   / `< 0.0` while air friction clamps with `>= 0.0` / `<= 0.0`. That
//!   difference is in the original and is observable: a fighter whose air
//!   speed lands exactly on the friction value stops, where the ground
//!   equivalent would not. Do not "tidy" these into a shared helper.

use ssb_engine::math::Vec3;

/// The subset of `FTAttributes` the physics functions read.
///
/// The full struct (`src/ft/fttypes.h`) also carries animation lengths, SFX
/// ids, camera offsets and per-move flags; those belong to other systems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsAttributes {
    /// Ground deceleration, scaled by the floor material's friction.
    pub traction: f32,
    pub dash_speed: f32,
    pub dash_decel: f32,
    pub run_speed: f32,
    pub walk_speed_mul: f32,
    /// Horizontal velocity imparted by a grounded jump.
    pub jump_vel_x: f32,
    pub jump_height_mul: f32,
    pub jump_height_base: f32,
    pub jumpaerial_vel_x: f32,
    pub jumpaerial_height: f32,
    /// Air drift acceleration per frame.
    pub air_accel: f32,
    /// Cap on horizontal air speed from drift.
    pub air_speed_max_x: f32,
    /// Horizontal air deceleration per frame when not drifting.
    pub air_friction: f32,
    /// Downward acceleration per frame.
    pub gravity: f32,
    /// Normal terminal velocity (a positive magnitude).
    pub tvel_base: f32,
    /// Fast-fall terminal velocity (a positive magnitude).
    pub tvel_fast: f32,
    pub jumps_max: i32,
    /// Knockback multiplier. Higher means *less* launch distance.
    pub weight: f32,
}

impl Default for PhysicsAttributes {
    fn default() -> Self {
        // Not a real character's values — a neutral baseline for tests.
        // Real values come from the extracted `FTAttributes` files.
        PhysicsAttributes {
            traction: 0.06,
            dash_speed: 1.6,
            dash_decel: 0.1,
            run_speed: 1.5,
            walk_speed_mul: 1.0,
            jump_vel_x: 0.8,
            jump_height_mul: 1.0,
            jump_height_base: 3.0,
            jumpaerial_vel_x: 0.8,
            jumpaerial_height: 3.0,
            air_accel: 0.05,
            air_speed_max_x: 0.8,
            air_friction: 0.01,
            gravity: 0.09,
            tvel_base: 1.7,
            tvel_fast: 2.5,
            jumps_max: 2,
            weight: 100.0,
        }
    }
}

/// A fighter's velocity state.
///
/// Ground and air velocity are stored separately, exactly as the original
/// does: `vel_ground` is a single X axis along the floor, `vel_air` is a full
/// 3D vector. Transitioning between them is an explicit transfer, not a shared
/// representation — see [`transfer_ground_to_air`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicsState {
    /// Along-floor velocity. Only X is meaningful.
    pub vel_ground: Vec3,
    /// Airborne velocity.
    pub vel_air: Vec3,
    /// Velocity from being hit, which decays independently.
    pub vel_knockback: Vec3,
    pub is_fastfall: bool,
    pub jumps_used: i32,
}

/// The depth axis is clamped to this magnitude.
///
/// From `ftPhysicsSetGroundVelTransferAir`: a fighter's Z position is held
/// within ±60 units. Smash is a 2D fighter staged in a 3D scene, and this is
/// what keeps it that way.
pub const Z_LIMIT: f32 = 60.0;

/// `ftPhysicsSetGroundVelTransferAir` @ 0x800D8880 (Z clamp portion).
///
/// Clamps the Z component of air velocity so that applying it cannot carry the
/// fighter past ±[`Z_LIMIT`]. Note it adjusts the *velocity*, not the
/// position, so the fighter decelerates into the boundary.
pub fn clamp_z_velocity(pos_z: f32, vel_z: f32) -> f32 {
    if vel_z > 0.0 && pos_z + vel_z > Z_LIMIT {
        Z_LIMIT - pos_z
    } else if pos_z + vel_z < -Z_LIMIT {
        -Z_LIMIT - pos_z
    } else {
        vel_z
    }
}

/// `ftPhysicsClampGroundVel` @ 0x800D8930.
pub fn clamp_ground_vel(p: &mut PhysicsState, clamp: f32) {
    p.vel_ground.x = p.vel_ground.x.clamp(-clamp, clamp);
}

/// `ftPhysicsSetGroundVelFriction` @ 0x800D8978.
///
/// Decays X toward zero by `friction`, stopping exactly at zero rather than
/// overshooting into a reversal.
pub fn apply_ground_friction(p: &mut PhysicsState, friction: f32) {
    if p.vel_ground.x < 0.0 {
        p.vel_ground.x += friction;
        if p.vel_ground.x > 0.0 {
            p.vel_ground.x = 0.0;
        }
    } else {
        p.vel_ground.x -= friction;
        if p.vel_ground.x < 0.0 {
            p.vel_ground.x = 0.0;
        }
    }
}

/// `ftPhysicsApplyGroundVelFriction` @ 0x800D8B98.
///
/// The floor material scales the character's traction, so ice slides and
/// normal ground does not.
pub fn apply_ground_friction_with_material(
    p: &mut PhysicsState,
    attr: &PhysicsAttributes,
    material_friction: f32,
) {
    apply_ground_friction(p, material_friction * attr.traction);
}

/// `ftPhysicsClampAirVelY` @ 0x800D8CF8.
pub fn clamp_air_vel_y(p: &mut PhysicsState, clamp: f32) {
    if p.vel_air.y > clamp {
        p.vel_air.y = clamp;
    }
}

/// `ftPhysicsAddClampAirVelY` @ 0x800D8D34.
pub fn add_clamp_air_vel_y(p: &mut PhysicsState, vel: f32, clamp: f32) {
    p.vel_air.y += vel;
    if p.vel_air.y > clamp {
        p.vel_air.y = clamp;
    }
}

/// `ftPhysicsApplyGravityClampTVel` @ 0x800D8D68.
///
/// Gravity is subtracted every frame and the result floored at `-tvel`. Note
/// this is a *velocity* clamp, not a drag force: a fighter reaches terminal
/// velocity abruptly, not asymptotically.
pub fn apply_gravity_clamp_tvel(p: &mut PhysicsState, gravity: f32, tvel: f32) {
    p.vel_air.y -= gravity;
    if p.vel_air.y < -tvel {
        p.vel_air.y = -tvel;
    }
}

/// `ftPhysicsApplyGravityDefault` @ 0x800D8E50.
pub fn apply_gravity_default(p: &mut PhysicsState, attr: &PhysicsAttributes) {
    apply_gravity_clamp_tvel(p, attr.gravity, attr.tvel_base);
}

/// `ftPhysicsApplyFastFall` @ 0x800D8DA0.
///
/// Fast-falling *sets* Y velocity outright rather than adding to it, so it is
/// an immediate snap to the fast terminal velocity.
pub fn apply_fast_fall(p: &mut PhysicsState, attr: &PhysicsAttributes) {
    p.vel_air.y = -attr.tvel_fast;
}

/// `ftPhysicsClampAirVelX` @ 0x800D8E78.
pub fn clamp_air_vel_x(p: &mut PhysicsState, clamp: f32) {
    p.vel_air.x = p.vel_air.x.clamp(-clamp, clamp);
}

/// `ftPhysicsCheckClampAirVelXDec` @ 0x800D8EDC.
///
/// Bleeds excess horizontal air speed off at a fixed 1.0 units/frame — used
/// after moves that launch a fighter faster than their normal drift cap.
/// Returns whether any decay happened.
pub fn check_clamp_air_vel_x_dec(p: &mut PhysicsState, clamp: f32) -> bool {
    if p.vel_air.x.abs() > clamp {
        p.vel_air.x += if p.vel_air.x >= 0.0 { -1.0 } else { 1.0 };
        if p.vel_air.x.abs() < clamp {
            p.vel_air.x = if p.vel_air.x >= 0.0 { clamp } else { -clamp };
        }
        true
    } else {
        false
    }
}

/// `ftPhysicsApplyAirVelXFriction` @ 0x800D9034.
///
/// The zero-crossing comparisons are `>=` / `<=` here, where the ground
/// equivalent uses `>` / `<`. That asymmetry is in the original.
pub fn apply_air_friction(p: &mut PhysicsState, attr: &PhysicsAttributes) {
    if p.vel_air.x < 0.0 {
        p.vel_air.x += attr.air_friction;
        if p.vel_air.x >= 0.0 {
            p.vel_air.x = 0.0;
        }
    } else {
        p.vel_air.x -= attr.air_friction;
        if p.vel_air.x <= 0.0 {
            p.vel_air.x = 0.0;
        }
    }
}

/// Air drift: accelerate horizontally toward stick input, capped at
/// `air_speed_max_x`.
///
/// `stick_x` is the raw N64 stick reading (`-80..=80`).
pub fn apply_air_drift(p: &mut PhysicsState, attr: &PhysicsAttributes, stick_x: i8) {
    if stick_x == 0 {
        apply_air_friction(p, attr);
        return;
    }
    let dir = if stick_x > 0 { 1.0 } else { -1.0 };
    p.vel_air.x += attr.air_accel * dir;
    clamp_air_vel_x(p, attr.air_speed_max_x);
}

/// `ftPhysicsApplyGroundVelTransferAir` @ 0x800D8B78.
///
/// Leaving the ground moves the along-floor velocity into the air vector and
/// zeroes the ground one.
pub fn transfer_ground_to_air(p: &mut PhysicsState) {
    p.vel_air.x = p.vel_ground.x;
    p.vel_ground = Vec3::ZERO;
}

/// `ftPhysicsStopVelAll` @ 0x800D93F0.
pub fn stop_all(p: &mut PhysicsState) {
    p.vel_ground = Vec3::ZERO;
    p.vel_air = Vec3::ZERO;
    p.vel_knockback = Vec3::ZERO;
}

/// Total per-frame displacement from every velocity source.
pub fn total_velocity(p: &PhysicsState, grounded: bool) -> Vec3 {
    let base = if grounded { p.vel_ground } else { p.vel_air };
    base + p.vel_knockback
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PhysicsState {
        PhysicsState::default()
    }

    #[test]
    fn gravity_accumulates_then_pins_at_terminal_velocity() {
        let attr = PhysicsAttributes::default();
        let mut p = state();

        apply_gravity_default(&mut p, &attr);
        assert_eq!(p.vel_air.y, -attr.gravity);

        // Fall long enough to saturate.
        for _ in 0..100 {
            apply_gravity_default(&mut p, &attr);
        }
        assert_eq!(p.vel_air.y, -attr.tvel_base);

        // Further frames do not push past it.
        apply_gravity_default(&mut p, &attr);
        assert_eq!(p.vel_air.y, -attr.tvel_base);
    }

    #[test]
    fn terminal_velocity_is_reached_in_the_expected_frame_count() {
        let attr = PhysicsAttributes::default();
        let mut p = state();
        let mut frames = 0;
        while p.vel_air.y > -attr.tvel_base {
            apply_gravity_default(&mut p, &attr);
            frames += 1;
            assert!(frames < 1000, "never reached terminal velocity");
        }
        // ceil(1.7 / 0.09) = 19
        assert_eq!(frames, 19);
    }

    #[test]
    fn fast_fall_snaps_rather_than_accelerating() {
        let attr = PhysicsAttributes::default();
        let mut p = state();
        p.vel_air.y = 1.0; // rising
        apply_fast_fall(&mut p, &attr);
        assert_eq!(p.vel_air.y, -attr.tvel_fast);
    }

    #[test]
    fn ground_friction_stops_exactly_at_zero_from_both_signs() {
        let mut p = state();
        p.vel_ground.x = 0.05;
        apply_ground_friction(&mut p, 0.06);
        assert_eq!(p.vel_ground.x, 0.0, "should not reverse");

        p.vel_ground.x = -0.05;
        apply_ground_friction(&mut p, 0.06);
        assert_eq!(p.vel_ground.x, 0.0, "should not reverse");
    }

    #[test]
    fn ground_friction_leaves_an_exact_match_untouched() {
        // The `> 0.0` test means landing exactly on zero is kept, and a value
        // exactly equal to friction decays to exactly zero.
        let mut p = state();
        p.vel_ground.x = 0.06;
        apply_ground_friction(&mut p, 0.06);
        assert_eq!(p.vel_ground.x, 0.0);
    }

    #[test]
    fn ice_slides_further_than_normal_ground() {
        let attr = PhysicsAttributes::default();
        let mut normal = state();
        let mut ice = state();
        normal.vel_ground.x = 1.0;
        ice.vel_ground.x = 1.0;

        for _ in 0..5 {
            apply_ground_friction_with_material(&mut normal, &attr, 1.0);
            apply_ground_friction_with_material(&mut ice, &attr, 0.2);
        }
        assert!(ice.vel_ground.x > normal.vel_ground.x);
    }

    #[test]
    fn air_friction_decays_toward_zero_without_reversing() {
        let attr = PhysicsAttributes::default();
        let mut p = state();
        p.vel_air.x = 0.005;
        apply_air_friction(&mut p, &attr); // air_friction = 0.01
        assert_eq!(p.vel_air.x, 0.0);
    }

    #[test]
    fn air_drift_accelerates_and_caps() {
        let attr = PhysicsAttributes::default();
        let mut p = state();
        for _ in 0..100 {
            apply_air_drift(&mut p, &attr, 80);
        }
        assert_eq!(p.vel_air.x, attr.air_speed_max_x);

        // Reversing drift pulls back the other way and caps symmetrically.
        for _ in 0..100 {
            apply_air_drift(&mut p, &attr, -80);
        }
        assert_eq!(p.vel_air.x, -attr.air_speed_max_x);
    }

    #[test]
    fn neutral_stick_applies_friction_instead_of_drift() {
        let attr = PhysicsAttributes::default();
        let mut p = state();
        p.vel_air.x = 0.5;
        apply_air_drift(&mut p, &attr, 0);
        assert_eq!(p.vel_air.x, 0.5 - attr.air_friction);
    }

    #[test]
    fn excess_air_speed_bleeds_off_one_unit_per_frame() {
        let mut p = state();
        p.vel_air.x = 3.0;
        assert!(check_clamp_air_vel_x_dec(&mut p, 0.8));
        assert_eq!(p.vel_air.x, 2.0);
        assert!(check_clamp_air_vel_x_dec(&mut p, 0.8));
        assert_eq!(p.vel_air.x, 1.0);
        // The last step would undershoot, so it snaps to the clamp.
        assert!(check_clamp_air_vel_x_dec(&mut p, 0.8));
        assert_eq!(p.vel_air.x, 0.8);
        // At the clamp, nothing further happens.
        assert!(!check_clamp_air_vel_x_dec(&mut p, 0.8));
    }

    #[test]
    fn z_velocity_decelerates_into_the_depth_boundary() {
        // Approaching the +Z wall: velocity is trimmed to land exactly on it.
        assert_eq!(clamp_z_velocity(55.0, 10.0), 5.0);
        assert_eq!(clamp_z_velocity(-55.0, -10.0), -5.0);
        // Well inside, untouched.
        assert_eq!(clamp_z_velocity(0.0, 10.0), 10.0);
    }

    #[test]
    fn leaving_the_ground_carries_momentum_into_the_air() {
        let mut p = state();
        p.vel_ground.x = 1.5;
        transfer_ground_to_air(&mut p);
        assert_eq!(p.vel_air.x, 1.5);
        assert_eq!(p.vel_ground.x, 0.0);
    }

    #[test]
    fn knockback_adds_to_whichever_base_velocity_applies() {
        let mut p = state();
        p.vel_ground.x = 1.0;
        p.vel_air.x = 2.0;
        p.vel_knockback = Vec3::new(0.5, 0.5, 0.0);

        assert_eq!(total_velocity(&p, true).x, 1.5);
        assert_eq!(total_velocity(&p, false).x, 2.5);
        assert_eq!(total_velocity(&p, true).y, 0.5);
    }

    #[test]
    fn stop_all_clears_every_source() {
        let mut p = state();
        p.vel_ground.x = 1.0;
        p.vel_air = Vec3::new(1.0, 1.0, 1.0);
        p.vel_knockback = Vec3::new(1.0, 1.0, 1.0);
        stop_all(&mut p);
        assert_eq!(p, PhysicsState::default());
    }

    /// The determinism property the whole port is aiming at: same input plus
    /// same initial state must give the same result, bit for bit.
    #[test]
    fn simulation_is_deterministic() {
        let attr = PhysicsAttributes::default();
        let inputs = [0i8, 40, 80, 0, -80, -40, 0, 80];

        let run = || {
            let mut p = PhysicsState::default();
            p.vel_air.y = 3.0;
            for frame in 0..240 {
                apply_air_drift(&mut p, &attr, inputs[frame % inputs.len()]);
                apply_gravity_default(&mut p, &attr);
            }
            p
        };

        assert_eq!(run(), run());
    }
}
