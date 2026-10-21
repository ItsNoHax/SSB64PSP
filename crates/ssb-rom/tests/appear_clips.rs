//! The packed battle-entry clips (RE-401), read back from the built pack.

use ssb_rom::anim::{
    is_anim_joint_slot, SLOT_APPEAR_R, SLOT_APPEAR_R_END, SLOT_APPEAR_R_START, SLOT_APPEAR_WAIT,
};
use ssb_rom::pack::{AnimJoint, Pack};

const PLAYABLE: u32 = 12;
const CAPTAIN: u32 = 7;
const NESS: u32 = 11;
const LINK: u32 = 5;
/// `anim::SLOT_GUARD_ON`, whose joints are the model's.
const SLOT_GUARD_ON: u32 = 600;

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
        // The runtime joints lead and bind no model node: one, or Link's
        // two (`TransN` then `XRotN`, `ftdata.c:3354`). The model joints
        // follow: Mario's table skips model joints 3 and 9, as his figatree
        // clips do (checked below through the mapping).
        let leads = if kind == LINK { 2 } else { 1 };
        for j in 0..leads {
            assert_eq!(joint(j).node, AnimJoint::NO_NODE, "{kind}");
        }
        // Then the model joints, the same ones a figatree clip binds.
        let model = pack.fighter_anim(kind, SLOT_GUARD_ON).unwrap();
        let model_nodes: Vec<u32> = (0..model.joint_count)
            .map(|j| pack.anim_joint(model.first_joint + j).unwrap().node)
            .filter(|&n| n != AnimJoint::NO_NODE)
            .collect();
        let first_model = model_nodes[0];
        assert_eq!(joint(leads).node, first_model, "{kind}");
        if kind == LINK {
            // Every model joint is bound, the last one included.
            assert_eq!(anim.joint_count, leads + model_nodes.len() as u32);
            let bound: Vec<u32> = (leads..anim.joint_count).map(|j| joint(j).node).collect();
            assert_eq!(bound, model_nodes);
        }
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn yoshis_entry_hides_his_model_until_he_hatches() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    const YOSHI: u32 = 6;
    let anim = pack.fighter_anim(YOSHI, SLOT_APPEAR_R as u32).unwrap();
    let script = pack.anim_script(&anim).unwrap();
    let root = pack.anim_joint(anim.first_joint + 1).unwrap().node;
    let model = pack.fighter_anim(YOSHI, SLOT_GUARD_ON).unwrap();
    let last = (0..model.joint_count)
        .map(|j| pack.anim_joint(model.first_joint + j).unwrap().node)
        .filter(|&n| n != AnimJoint::NO_NODE)
        .max()
        .unwrap();
    let mut skeleton = ssb_rom::skeleton::Skeleton::new();
    skeleton.start(&pack, &anim, 0.0, 1.0);
    assert!(!skeleton.any_flags());
    // The model root's stream sets `DOBJ_FLAG_HIDDEN` on its first parse
    // and clears it 61 frames later; the flag hides the whole subtree,
    // down to the last model node.
    let mut hidden = Vec::new();
    for _ in 0..80 {
        skeleton.tick(script).unwrap();
        hidden.push(skeleton.hidden(&pack, root) && skeleton.hidden(&pack, last));
    }
    assert!(hidden[..61].iter().all(|&h| h), "{hidden:?}");
    assert!(hidden[61..].iter().all(|&h| !h), "{hidden:?}");
    assert!(!skeleton.any_flags());
}

/// Kirby's and Purin's quick cliff escapes are figatrees with both lead
/// bits (`TransN`, then `XRotN`): their model joints start at entry 2 and
/// run to the model's last (pack 124).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn two_lead_figatrees_bind_every_model_joint() {
    const KIRBY: u32 = 8;
    const PURIN: u32 = 10;
    /// `CliffEscapeQuick1` and `2`.
    const SLOTS: [u32; 2] = [483, 484];
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for kind in [KIRBY, PURIN] {
        let model = pack.fighter_anim(kind, SLOT_GUARD_ON).unwrap();
        let model_nodes: Vec<u32> = (0..model.joint_count)
            .map(|j| pack.anim_joint(model.first_joint + j).unwrap().node)
            .filter(|&n| n != AnimJoint::NO_NODE)
            .collect();
        for slot in SLOTS {
            let anim = pack.fighter_anim(kind, slot).unwrap();
            let nodes: Vec<u32> = (0..anim.joint_count)
                .map(|j| pack.anim_joint(anim.first_joint + j).unwrap().node)
                .collect();
            assert_eq!(nodes[..2], [AnimJoint::NO_NODE; 2], "{kind}.{slot}");
            assert_eq!(nodes[2..], model_nodes[..], "{kind}.{slot}");
        }
    }
    use ssb_game::fighter::FighterKind;
    use ssb_game::status::{AnyStatus, Status};
    for (slot, status) in SLOTS
        .into_iter()
        .zip([Status::CliffEscapeQuick1, Status::CliffEscapeQuick2])
    {
        let kirby = pack.fighter_anim(KIRBY, slot).unwrap();
        let purin = pack.fighter_anim(PURIN, slot).unwrap();
        // Purin's rows name Kirby's files (`llFTKirbyAnimCliffEscapeQuick*`,
        // `ftdata.c:5842`): one figatree and script blob, its joints'
        // scripts in one order (each joint table binds its own model's
        // nodes).
        assert_eq!(
            (
                kirby.source_file,
                kirby.joint_count,
                kirby.script_offset,
                kirby.script_len
            ),
            (
                purin.source_file,
                purin.joint_count,
                purin.script_offset,
                purin.script_len
            ),
            "{slot}"
        );
        let scripts = |a: &ssb_rom::pack::AnimDesc| -> Vec<u32> {
            (0..a.joint_count)
                .map(|j| pack.anim_joint(a.first_joint + j).unwrap().script)
                .collect()
        };
        assert_eq!(scripts(&kirby), scripts(&purin), "{slot}");
        // The figatree's decoded length is the motion table's
        // `anim_length` (8 and 27).
        for (kind, anim) in [(FighterKind::Kirby, kirby), (FighterKind::Purin, purin)] {
            let length = ssb_game::motion::anim_length(kind, AnyStatus::Common(status));
            assert_eq!(length, Some(anim.frames as f32), "{kind:?}.{slot}");
        }
    }
}

/// Fox's Appear clips hide his model until the Arwing's canopy opens: the
/// N64 (ParaLLEl, RE-425) first draws his head in the cockpit on the frame
/// his clip and the Arwing's both reach 60.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn foxs_entry_shows_him_from_frame_60() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for slot in [SLOT_APPEAR_R as u32, SLOT_APPEAR_R as u32 + 1] {
        let anim = pack.fighter_anim(1, slot).unwrap();
        let script = pack.anim_script(&anim).unwrap();
        let mut skeleton = ssb_rom::skeleton::Skeleton::new();
        skeleton.start(&pack, &anim, 0.0, 1.0);
        let nodes: Vec<u32> = (0..anim.joint_count)
            .map(|j| pack.anim_joint(anim.first_joint + j).unwrap().node)
            .filter(|&n| n != AnimJoint::NO_NODE)
            .collect();
        for _ in 0..70 {
            skeleton.tick(script).unwrap();
            let hidden = nodes.iter().filter(|&&n| skeleton.hidden(&pack, n)).count();
            let frame = skeleton.frame();
            let want = if frame < 60.0 { nodes.len() } else { 0 };
            assert_eq!(hidden, want, "slot {slot} frame {frame}");
        }
    }
}
