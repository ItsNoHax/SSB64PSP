//! The surface-angle gates compare `syVectorAngleDiff3D`'s angle with a
//! degree constant; the port's earlier cosine-literal forms disagreed with it
//! within a few ULPs of the threshold (RE-503). Each witness below was found
//! by searching around the threshold, and each test checks the witness
//! still separates the two forms.

use ssb_engine::math::{div_nonzero, sin_cos, Vec2, Vec3};

const UP: Vec2 = Vec2::new(0.0, 1.0);

fn v(x: u32, y: u32) -> Vec3 {
    Vec3::new(f32::from_bits(x), f32::from_bits(y), 0.0)
}

fn dot(n: Vec2, v: Vec3) -> f32 {
    n.x * v.x + n.y * v.y
}

/// `mpCommonCheckFighterDamageCollide` etc.: a damage surface strikes past
/// `F_CLC_DTOR32(110.0F)` from its normal.
#[test]
fn damage_surface_compares_the_angle_at_110_degrees() {
    let diff = v(0xbf70_8fb2, 0xbeaf_1d42);
    let old = div_nonzero(dot(UP, diff), diff.length()) < -0.342_020_15;
    assert!(!old);
    assert!(crate::map::faces(diff, UP));
}

/// `ftPikachuSpecialAirHiProcMap`: Quick Attack halts past
/// `F_CLC_DTOR32(135.0F)`.
#[test]
fn quick_attack_halt_compares_the_angle_at_135_degrees() {
    let vel = v(0xc007_c3b6, 0xc007_c3b6);
    let old = dot(UP, vel) < vel.length() * -core::f32::consts::FRAC_1_SQRT_2;
    assert!(!old);
    assert!(crate::pikachu::halts(UP, vel));
}

/// `ftNessSpecialAirHiJibakuProcMap`: PK Thunder blast halts past
/// `F_CLC_DTOR32(155.0F)`.
#[test]
fn pk_thunder_blast_halt_compares_the_angle_at_155_degrees() {
    let vel = v(0x41a9_0c17, 0xc235_42f7);
    let old = dot(UP, vel) < vel.length() * sin_cos(155.0 * core::f32::consts::PI / 180.0).1;
    assert!(old);
    assert!(!crate::ness::jibaku_halts(UP, vel));
}
