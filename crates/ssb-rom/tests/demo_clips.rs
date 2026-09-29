//! The packed demo clips (RE-408): `nFTDemoStatusWin1` to
//! `nFTDemoStatusLose`, which the VS results screen and the character selects
//! play, read back from the built pack.

use ssb_rom::anim::{is_anim_joint_slot, SLOT_LOSE, SLOT_WIN1, SLOT_WIN2, SLOT_WIN4};
use ssb_rom::pack::{AnimJoint, Pack};

const PLAYABLE: u32 = 12;
const MARIO: u32 = 0;
const SAMUS: u32 = 3;
const LUIGI: u32 = 4;
const KIRBY: u32 = 8;
const PIKACHU: u32 = 9;
const NESS: u32 = 11;
/// `anim::SLOT_GUARD_ON`, whose joints are the model's.
const SLOT_GUARD_ON: u32 = 600;

/// Each fighter's `Win1` to `Win4` and `Lose` lengths, in `FTKind` order.
/// 0 is a clip that loops (the claps). The ROM and the decompilation's C
/// sources agree on all of them (`romtool anims --verify`).
const FRAMES: [[u32; 5]; PLAYABLE as usize] = [
    [120, 120, 120, 65, 0],    // Mario
    [120, 120, 120, 120, 0],   // Fox
    [120, 120, 120, 120, 0],   // Donkey Kong
    [120, 75, 120, 120, 0],    // Samus
    [120, 120, 120, 120, 0],   // Luigi
    [120, 120, 120, 120, 0],   // Link
    [120, 120, 124, 124, 0],   // Yoshi
    [120, 120, 120, 120, 0],   // Captain Falcon
    [161, 161, 120, 120, 0],   // Kirby
    [120, 120, 120, 120, 0],   // Pikachu
    [120, 120, 120, 120, 120], // Jigglypuff
    [120, 120, 120, 120, 0],   // Ness
];

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Whether the submotion row sets a leading runtime joint (`0x80000000` in
/// its `anim_desc`): Kirby's Win1 and Win2, Pikachu's Win1 and Ness's Win2.
fn leads_with_runtime_joint(kind: u32, slot: u32) -> bool {
    matches!(
        (kind, slot as usize),
        (KIRBY, SLOT_WIN1) | (KIRBY, SLOT_WIN2) | (PIKACHU, SLOT_WIN1) | (NESS, SLOT_WIN2)
    )
}

#[test]
fn every_fighter_has_its_demo_clips_over_its_model_joints() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for kind in 0..PLAYABLE {
        let model = pack.fighter_anim(kind, SLOT_GUARD_ON).unwrap();
        let model_nodes: Vec<u32> = (0..model.joint_count)
            .map(|j| pack.anim_joint(model.first_joint + j).unwrap().node)
            .filter(|&n| n != AnimJoint::NO_NODE)
            .collect();
        for (i, slot) in (SLOT_WIN1..=SLOT_LOSE).enumerate() {
            assert!(!is_anim_joint_slot(slot), "figatree slot {slot}");
            let slot = slot as u32;
            let anim = pack
                .fighter_anim(kind, slot)
                .unwrap_or_else(|| panic!("fighter {kind} slot {slot}"));
            assert_eq!(anim.frames, FRAMES[kind as usize][i], "{kind} {slot}");
            let nodes: Vec<u32> = (0..anim.joint_count)
                .map(|j| pack.anim_joint(anim.first_joint + j).unwrap().node)
                .collect();
            // A flagged row's table is one entry longer and its first entry
            // binds no model node; every other row binds the model joints
            // from its first entry, the way a battle figatree does.
            let lead = usize::from(leads_with_runtime_joint(kind, slot));
            if lead == 1 {
                assert_eq!(nodes[0], AnimJoint::NO_NODE, "{kind} {slot}");
            }
            assert_eq!(
                &nodes[lead..lead + model_nodes.len()],
                &model_nodes[..],
                "{kind} {slot}"
            );
            // Samus's claps carry one unused trailing entry, as 73 of her
            // battle figatrees do; nothing else is longer than its joints.
            let extra = usize::from(kind == SAMUS && slot as usize == SLOT_LOSE);
            assert_eq!(
                nodes.len(),
                lead + model_nodes.len() + extra,
                "{kind} {slot}"
            );
            if extra == 1 {
                assert_eq!(*nodes.last().unwrap(), AnimJoint::NO_NODE);
            }
        }
    }
    // Luigi's Lose is Mario's claps (`llFTMarioAnimClapsFileID`), and Win4
    // repeats Win3 for most of the roster (the same submotion row twice).
    let lose = |kind| pack.fighter_anim(kind, SLOT_LOSE as u32).unwrap();
    assert_eq!(lose(LUIGI).source_file, lose(MARIO).source_file);
    assert_eq!(lose(MARIO).source_file, 361);
    let win4 = pack.fighter_anim(NESS, SLOT_WIN4 as u32).unwrap();
    let win3 = pack.fighter_anim(NESS, SLOT_WIN4 as u32 - 1).unwrap();
    assert_eq!(win4.source_file, win3.source_file);
}
