//! The stick-direction gates (`ftParamGetStickAngleRads` against a degree
//! constant) and the per-fighter angled-move tables (RE-503).

use crate::fighter::FighterKind;
use crate::status::{
    fsmash_status, fsmash_variants, ftilt_status, ftilt_variants, AngleVariants, Status,
    STICK_ANGLE_50,
};
use ssb_engine::math::sy_arc_tan2;

/// The slope comparisons the gates used before they compared
/// `syUtilsArcTan2`'s angle: `y > tan(d) * |x|` with `tan(d)` as a literal.
const TAN_50: f32 = 1.191_753_6;
const TAN_30: f32 = 0.577_350_26;
const TAN_21: f32 = 0.383_864_04;
const TAN_17: f32 = 0.305_730_7;
const TAN_10: f32 = 0.176_326_98;
const TAN_7: f32 = 0.122_784_56;

fn slope_ftilt(variants: AngleVariants, x: f32, y: f32) -> Status {
    let (five, three) = (
        variants == AngleVariants::Five,
        variants == AngleVariants::Three,
    );
    if five && y > TAN_30 * x.abs() {
        Status::AttackS3Hi
    } else if five && y > TAN_10 * x.abs() {
        Status::AttackS3HiS
    } else if five && y < -TAN_30 * x.abs() {
        Status::AttackS3Lw
    } else if five && y < -TAN_10 * x.abs() {
        Status::AttackS3LwS
    } else if three && y > TAN_17 * x.abs() {
        Status::AttackS3Hi
    } else if three && y < -TAN_17 * x.abs() {
        Status::AttackS3Lw
    } else {
        Status::AttackS3
    }
}

fn slope_fsmash(variants: AngleVariants, x: f32, y: f32) -> Status {
    match variants {
        AngleVariants::One => Status::AttackS4,
        AngleVariants::Three if y > TAN_17 * x.abs() => Status::AttackS4Hi,
        AngleVariants::Three if y < -TAN_17 * x.abs() => Status::AttackS4Lw,
        AngleVariants::Three => Status::AttackS4,
        AngleVariants::Five if y > TAN_21 * x.abs() => Status::AttackS4Hi,
        AngleVariants::Five if y > TAN_7 * x.abs() => Status::AttackS4HiS,
        AngleVariants::Five if y < -TAN_21 * x.abs() => Status::AttackS4Lw,
        AngleVariants::Five if y < -TAN_7 * x.abs() => Status::AttackS4LwS,
        AngleVariants::Five => Status::AttackS4,
    }
}

/// On the whole integer stick grid, the original's angle compare and the
/// slope compare the port used before pick the same move: the angle form is
/// the original's by construction, and this pins that switching to it
/// changed no input.
#[test]
fn stick_gate_angle_and_slope_agree_on_every_stick_position() {
    let variants = [
        AngleVariants::One,
        AngleVariants::Three,
        AngleVariants::Five,
    ];
    for xi in i8::MIN..=i8::MAX {
        for yi in i8::MIN..=i8::MAX {
            let (x, y) = (f32::from(xi), f32::from(yi));
            let angle = sy_arc_tan2(y, x.abs());
            let at = (xi, yi);
            assert_eq!(angle > STICK_ANGLE_50, y > TAN_50 * x.abs(), "{at:?}");
            assert_eq!(angle < -STICK_ANGLE_50, y < -TAN_50 * x.abs(), "{at:?}");
            assert_eq!(
                angle.abs() <= STICK_ANGLE_50,
                y.abs() <= TAN_50 * x.abs(),
                "{at:?}"
            );
            for v in variants {
                assert_eq!(ftilt_status(v, angle), slope_ftilt(v, x, y), "{v:?} {at:?}");
                assert_eq!(
                    fsmash_status(v, angle),
                    slope_fsmash(v, x, y),
                    "{v:?} {at:?}"
                );
            }
        }
    }
}

const KINDS: [FighterKind; 26] = [
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
    FighterKind::MetalMario,
    FighterKind::PolyMario,
    FighterKind::PolyFox,
    FighterKind::PolyDonkey,
    FighterKind::PolySamus,
    FighterKind::PolyLuigi,
    FighterKind::PolyLink,
    FighterKind::PolyYoshi,
    FighterKind::PolyCaptain,
    FighterKind::PolyKirby,
    FighterKind::PolyPikachu,
    FighterKind::PolyPurin,
    FighterKind::PolyNess,
    FighterKind::GiantDonkey,
];

/// `motion_desc[...].anim_file_id != 0` for a common status's motion.
fn has_motion_file(kind: FighterKind, status: Status) -> bool {
    crate::motion::motion_desc(kind, status.into()).is_some_and(|d| d.anim_length != 0)
}

/// The angled forward tilts and smashes the setters choose from are the
/// ones each fighter's motion table holds files for, and no fighter has an
/// `AttackHi3F` file: `ftCommonAttackHi3SetStatus`'s forward/backward
/// split never runs, so `set_utilt` always enters `AttackHi3`.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn angled_move_variants_match_the_motion_tables() {
    let table = |kind, his, hi| {
        if has_motion_file(kind, his) {
            AngleVariants::Five
        } else if has_motion_file(kind, hi) {
            AngleVariants::Three
        } else {
            AngleVariants::One
        }
    };
    for kind in KINDS {
        assert_eq!(
            ftilt_variants(kind),
            table(kind, Status::AttackS3HiS, Status::AttackS3Hi),
            "{kind:?} forward tilt"
        );
        assert_eq!(
            fsmash_variants(kind),
            table(kind, Status::AttackS4HiS, Status::AttackS4Hi),
            "{kind:?} forward smash"
        );
        assert!(has_motion_file(kind, Status::AttackHi3), "{kind:?}");
        assert!(!has_motion_file(kind, Status::AttackHi3F), "{kind:?}");
        assert!(!has_motion_file(kind, Status::AttackHi3B), "{kind:?}");
    }
}
