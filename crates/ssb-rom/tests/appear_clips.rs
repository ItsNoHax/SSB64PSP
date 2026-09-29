//! The packed battle-entry clips (RE-401), read back from the built pack.

use ssb_rom::anim::{
    is_anim_joint_slot, SLOT_APPEAR_R, SLOT_APPEAR_R_END, SLOT_APPEAR_R_START, SLOT_APPEAR_WAIT,
};
use ssb_rom::pack::{AnimJoint, Pack};

const PLAYABLE: u32 = 12;
const CAPTAIN: u32 = 7;
const NESS: u32 = 11;
/// `anim::SLOT_GUARD_ON`, whose joints are the model's.
const SLOT_GUARD_ON: u32 = 600;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
fn every_fighter_has_an_entry_clip_over_its_model_joints() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for kind in 0..PLAYABLE {
        let slot = match kind {
            CAPTAIN | NESS => SLOT_APPEAR_R_START,
            _ => SLOT_APPEAR_R,
        };
        assert!(is_anim_joint_slot(slot));
        let anim = pack
            .fighter_anim(kind, slot as u32)
            .unwrap_or_else(|| panic!("{kind}"));
        let joint = |j: u32| pack.anim_joint(anim.first_joint + j).unwrap();
        // One runtime joint leads and binds no model node. The model
        // joints follow: Mario's table skips model joints 3 and 9, as his
        // figatree clips do (checked below through the mapping).
        assert_eq!(joint(0).node, AnimJoint::NO_NODE, "{kind}");
        // Then the model joints, the same ones a figatree clip binds.
        let model = pack.fighter_anim(kind, SLOT_GUARD_ON).unwrap();
        let first_model = (0..model.joint_count)
            .map(|j| pack.anim_joint(model.first_joint + j).unwrap().node)
            .find(|&n| n != AnimJoint::NO_NODE)
            .unwrap();
        assert_eq!(joint(1).node, first_model, "{kind}");
        if kind == 0 {
            for (j, figatree_joint) in [(4u32, 3u32), (10, 9)] {
                assert_eq!(joint(j).script, AnimJoint::NO_SCRIPT, "Mario {j}");
                assert_eq!(joint(j).node, first_model + figatree_joint);
            }
        }
        let expected = match kind {
            CAPTAIN => 90,
            NESS => 40,
            _ => 120,
        };
        assert_eq!(anim.frames, expected, "{kind}");
    }
    // Ness's middle phase and both multi-phase ends.
    assert_eq!(
        pack.fighter_anim(NESS, SLOT_APPEAR_WAIT as u32)
            .unwrap()
            .frames,
        50
    );
    assert_eq!(
        pack.fighter_anim(NESS, SLOT_APPEAR_R_END as u32)
            .unwrap()
            .frames,
        30
    );
    assert_eq!(
        pack.fighter_anim(CAPTAIN, SLOT_APPEAR_R_END as u32)
            .unwrap()
            .frames,
        30
    );
}

#[test]
fn the_skeleton_plays_marios_entry_to_its_end() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let anim = pack.fighter_anim(0, SLOT_APPEAR_R as u32).unwrap();
    let script = pack.anim_script(&anim).unwrap();
    let mut skeleton = ssb_rom::skeleton::Skeleton::new();
    skeleton.start(&pack, &anim, 0.0, 1.0);
    let mut heights = Vec::new();
    let mut ticks = 0;
    while !skeleton.ended() && ticks < 400 {
        skeleton.tick(script).unwrap();
        heights.push(skeleton.pose(0).unwrap().translate[1]);
        ticks += 1;
    }
    // Every joint ends on tick 121, and `anim_frame` is then at or below
    // zero, where `ftCommonAppearProcUpdate` ends the entry.
    assert_eq!(ticks, 121);
    assert!(skeleton.frame() <= 0.0, "{}", skeleton.frame());
    // The lead joint carries the entry's rise out of the pipe.
    let (lo, hi) = heights
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    assert!(hi - lo > 100.0, "{lo}..{hi}");
}
