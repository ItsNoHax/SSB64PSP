//! The ground effects' first animation cycle (`ssb_game::ground_effect::
//! cycle`): the `gcPlayAnimAll` call on which the root's child (the first
//! `DObjDesc` under `efManagerMakeEffect`'s root) reaches `AOBJ_ANIM_NULL`,
//! and the child's X/Y translation then, which `efGroundCommonProcUpdate`
//! adds to the root. `ssb-tablegen`'s own script player generates the
//! table (`ground_effects.rs`); this `ssb_rom::objanim` replay is the
//! independent second reading (D-034).

use ssb_game::ground_effect::{cycle, desc_file, DESC_COUNT, O_ANIM_JOINT, O_DOBJSETUP};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::{joint_scripts, StageJoint};
use ssb_rom::Archive;

/// `DOBJ_ARRAY_MAX` terminator id of a `DObjDesc` array.
const DESC_END: u32 = 18;
/// `sizeof(DObjDesc)`.
const DOBJ_DESC_SIZE: usize = 0x2C;

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn first_cycle_matches_the_objanim_replay() {
    let Some(path) = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")) else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    for k in 0..DESC_COUNT {
        let file = desc_file(k).unwrap_or_else(|| panic!("desc {k} has no stage"));
        let f = archive.load(file).unwrap();
        let d = &f.data;
        let w = |at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap());
        let fl = |at: usize| f32::from_bits(w(at));
        let mut poses = Vec::new();
        let mut at = O_DOBJSETUP[k] as usize;
        while w(at) != DESC_END {
            poses.push(JointPose {
                translate: [fl(at + 8), fl(at + 12), fl(at + 16)],
                rotate: [fl(at + 20), fl(at + 24), fl(at + 28)],
                scale: [1.0; 3],
            });
            at += DOBJ_DESC_SIZE;
        }
        let scripts = joint_scripts(d, O_ANIM_JOINT[k], poses.len());
        let mut child = StageJoint::start_changed(scripts[0].expect("child script"), 0.0);
        let mut pose = poses[0];
        let mut life = None;
        for play in 1..=4000u32 {
            child.tick(d, 1.0, &mut pose).unwrap();
            if child.ended() {
                life = Some(play);
                break;
            }
        }
        assert_eq!(life, Some(u32::from(cycle::LIFE[k])), "desc {k} life");
        // Both readings divide a linear rate as `gcParseDObjAnimJoint` does
        // (objanim.c:395-397), so the ends agree bit for bit (the Zebes
        // ship's X, desc 8, is -1800.0 only by division).
        let end = cycle::END[k][0];
        for axis in 0..2 {
            assert_eq!(
                pose.translate[axis].to_bits(),
                end[axis].to_bits(),
                "desc {k} end {:?} vs {end:?}",
                &pose.translate[..2]
            );
        }
    }
}
