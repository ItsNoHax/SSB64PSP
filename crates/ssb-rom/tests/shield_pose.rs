//! The packed shield poses (RE-367), read back from the built pack.

use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::StageJoint;
use ssb_rom::pack::{AnimDesc, AnimJoint, Pack};
use ssb_rom::skeleton::{
    apply_shield_pose, shield_lookup, translation_scale, ShieldJoints, Skeleton,
};

const FOX: u32 = 1;
const PLAYABLE: u32 = 12;
/// `anim::SLOT_GUARD_ON`.
const SLOT_GUARD_ON: u32 = 600;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

fn bound_nodes(pack: &Pack<'_>, anim: &AnimDesc) -> Vec<u32> {
    let mut nodes: Vec<u32> = (0..anim.joint_count)
        .filter_map(|j| pack.anim_joint(anim.first_joint + j))
        .map(|j| j.node)
        .filter(|&n| n != AnimJoint::NO_NODE)
        .collect();
    nodes.sort_unstable();
    nodes
}

#[test]
fn every_fighter_has_eight_sectors_over_its_own_joints() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for kind in 0..PLAYABLE {
        let guard_on = pack.fighter_anim(kind, SLOT_GUARD_ON).unwrap();
        let joints = bound_nodes(&pack, &guard_on);
        for sector in 0..AnimDesc::SHIELD_SECTORS {
            let anim = pack.shield_pose(kind, sector).unwrap();
            assert_eq!(
                anim.joint_count as usize,
                joints.len() + 2,
                "{kind} {sector}"
            );
            assert_eq!(bound_nodes(&pack, &anim), joints, "{kind} {sector}");
            // `XRotN` is scripted only to stay at rest (Link), which lets
            // the runtime skip it.
            let xrotn = pack.anim_joint(anim.first_joint).unwrap();
            if xrotn.script != AnimJoint::NO_SCRIPT {
                let script = pack.anim_script(&anim).unwrap();
                for frame in [0.0, 22.5, 44.0] {
                    let mut pose = JointPose::default();
                    StageJoint::start_changed(xrotn.script, frame)
                        .tick(script, 1.0, &mut pose)
                        .unwrap();
                    assert_eq!(pose, JointPose::default(), "{kind} {sector} {frame}");
                }
            }
            // `YRotN`, the shield's joint, always is.
            let yrotn = pack
                .anim_joint(anim.first_joint + anim.joint_count - 1)
                .unwrap();
            assert_ne!(yrotn.script, AnimJoint::NO_SCRIPT, "{kind} {sector}");
        }
    }
    assert!(pack.shield_pose(0, AnimDesc::SHIELD_SECTORS).is_none());
}

/// Fox's sector 0, entry 3 (`dFoxShieldPose_script2_1`) starts at rotate
/// (0.4712, -0.7330, -0.4712); its `dobj_lookup` entry is (0, -0.5236, 0).
/// The range blends between them.
#[test]
fn the_stick_range_blends_foxs_sector_toward_the_neutral_pose() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let anim = pack.shield_pose(FOX, 0).unwrap();
    let script = pack.anim_script(&anim).unwrap();
    let (translate, rotate) = shield_lookup(script, &anim, 3).unwrap();
    assert_eq!(translate, [0.0, 0.0, 0.0]);
    assert!(
        (rotate[1] + core::f32::consts::FRAC_PI_6).abs() < 1e-5,
        "{rotate:?}"
    );
    let node = pack.anim_joint(anim.first_joint + 3).unwrap().node;

    let rotate_at = |range: f32| {
        let guard_on = pack.fighter_anim(FOX, SLOT_GUARD_ON).unwrap();
        let mut skeleton = Skeleton::new();
        skeleton.start(&pack, &guard_on, 0.0, 1.0);
        let mut yrotn = JointPose::default();
        apply_shield_pose(
            &pack,
            &anim,
            0.0,
            range,
            ShieldJoints::All,
            &mut skeleton,
            &mut yrotn,
        );
        let joint = (0..skeleton.joint_count())
            .find(|&i| skeleton.joint_node(i) == Some(node))
            .unwrap();
        (skeleton.pose(joint).unwrap().rotate, yrotn)
    };

    let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
    let (full, yrotn) = rotate_at(1.0);
    assert!(close(full, [0.471_239, -0.733_038, -0.471_239]), "{full:?}");
    let (neutral, _) = rotate_at(0.0);
    assert!(close(neutral, rotate), "{neutral:?}");
    let (half, _) = rotate_at(0.5);
    let mid = core::array::from_fn(|k| (full[k] + rotate[k]) / 2.0);
    assert!(close(half, mid), "{half:?}");
    // The shield sits above the fighter, not at its feet.
    assert!(yrotn.translate[1] > 100.0, "{yrotn:?}");
}

/// `GuardOn`/`GuardOff` move only `YRotN`.
#[test]
fn the_shield_joint_pose_leaves_the_body_alone() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let anim = pack.shield_pose(FOX, 2).unwrap();
    let guard_on = pack.fighter_anim(FOX, SLOT_GUARD_ON).unwrap();
    let mut skeleton = Skeleton::new();
    skeleton.start(&pack, &guard_on, 0.0, 1.0);
    let before: Vec<JointPose> = (0..skeleton.joint_count())
        .map(|i| *skeleton.pose(i).unwrap())
        .collect();
    let mut yrotn = JointPose::default();
    apply_shield_pose(
        &pack,
        &anim,
        10.0,
        1.0,
        ShieldJoints::ShieldJoint,
        &mut skeleton,
        &mut yrotn,
    );
    for (i, pose) in before.iter().enumerate() {
        assert_eq!(skeleton.pose(i), Some(pose));
    }
    assert_ne!(yrotn, JointPose::default());
}

#[test]
fn luigi_scales_come_from_the_pack_and_scale_the_shield_neutral() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let scales = pack.fighter_translate_scales(4).expect("Luigi scales");
    assert_eq!(scales.len(), 29 * 12);
    assert_eq!(translation_scale(scales, 0), [1.0; 3]);
    assert!((translation_scale(scales, 3)[1] - 1.1379).abs() < 0.0001);
    assert!((translation_scale(scales, 12)[1] - 2.886).abs() < 0.0001);
    for joint in 0..29 {
        let vector = translation_scale(scales, joint);
        assert_eq!([vector[0], vector[2]], [1.0; 2], "joint {joint}");
        if ![3, 7, 11, 12, 13].contains(&joint) {
            assert_eq!(vector[1], 1.0, "joint {joint}");
        }
    }
    assert!(pack.fighter_translate_scales(0).is_none());

    let anim = pack.shield_pose(4, 0).unwrap();
    let script = pack.anim_script(&anim).unwrap();
    let last = anim.joint_count - 1;
    let (neutral, _) = shield_lookup(script, &anim, last).unwrap();
    let mut skeleton = Skeleton::new();
    skeleton.start(
        &pack,
        &pack.fighter_anim(4, SLOT_GUARD_ON).unwrap(),
        0.0,
        1.0,
    );
    let mut yrotn = JointPose::default();
    apply_shield_pose(
        &pack,
        &anim,
        0.0,
        0.0,
        ShieldJoints::ShieldJoint,
        &mut skeleton,
        &mut yrotn,
    );
    assert!((yrotn.translate[1] - neutral[1] * translation_scale(scales, 3)[1]).abs() < 0.001);

    let yrotn_joint = pack.anim_joint(anim.first_joint + last).unwrap();
    let mut unscaled = JointPose::default();
    StageJoint::start_changed(yrotn_joint.script, 0.0)
        .tick(script, 1.0, &mut unscaled)
        .unwrap();
    apply_shield_pose(
        &pack,
        &anim,
        0.0,
        1.0,
        ShieldJoints::ShieldJoint,
        &mut skeleton,
        &mut yrotn,
    );
    assert!(
        (yrotn.translate[1] - unscaled.translate[1] * translation_scale(scales, 3)[1]).abs()
            < 0.001
    );
}

#[test]
fn luigi_clip_translation_uses_the_matching_model_joint_vector() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let scales = pack.fighter_translate_scales(4).unwrap();
    let object_index = ssb_rom::scene_deps::fighter_object(&pack, 4).unwrap();
    let first_node = pack.object(object_index).unwrap().first_node;
    let mut observed = false;
    for slot in 0..ssb_rom::anim::SLOT_COUNT as u32 {
        let Some(anim) = pack.fighter_anim(4, slot) else {
            continue;
        };
        let script = pack.anim_script(&anim).unwrap();
        let mut plain = Skeleton::new();
        let mut scaled = Skeleton::new();
        plain.start(&pack, &anim, 0.0, 1.0);
        scaled.start(&pack, &anim, 0.0, 1.0);
        for _ in 0..5 {
            plain.tick(script).unwrap();
            scaled
                .tick_scaled(script, Some(scales), first_node)
                .unwrap();
            for j in 0..plain.joint_count() {
                let Some(node) = plain.joint_node(j) else {
                    continue;
                };
                let scale = translation_scale(scales, (node - first_node) as usize + 4);
                let a = plain.pose(j).unwrap();
                let b = scaled.pose(j).unwrap();
                assert_eq!(a.rotate, b.rotate);
                assert_eq!(a.scale, b.scale);
                for (k, factor) in scale.iter().enumerate() {
                    if (a.translate[k] - b.translate[k]).abs() > 0.001 {
                        assert!(
                            (b.translate[k] - a.translate[k] * factor).abs() < 0.001,
                            "slot {slot} joint {j} axis {k}"
                        );
                        observed = true;
                    }
                }
            }
        }
        if observed {
            break;
        }
    }
    assert!(observed, "no scaled Luigi model translation was exercised");
}
