//! The joint samples gameplay reads from a posed skeleton: moved verbatim
//! from the PSP runtime's `scene.rs`, which re-exports them.

use ssb_rom::pack::{ObjectDesc, Pack};

/// The part tree a query walks to TopN: node `n` is joint `n + 4`, a
/// root node hangs from the lead `XRotN` when the clip has one.
pub fn set_joint_parents(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    skeleton: &ssb_rom::skeleton::Skeleton,
    f: &mut ssb_game::fighter::Fighter,
) {
    use ssb_game::fighter::FIGHTER_JOINTS;
    let root_parent = if skeleton.lead_xrotn_pose().is_some() {
        2
    } else {
        0
    };
    for i in 0..(object.node_count as usize).min(FIGHTER_JOINTS - 4) {
        let parent = pack
            .node_parent(object.first_node + i as u32)
            .filter(|&p| {
                p != ssb_rom::pack::NodeDesc::NO_PARENT
                    && p >= object.first_node
                    && p - object.first_node < i as u32
            })
            .map_or(root_parent, |p| (p - object.first_node) as u8 + 4);
        f.joint_parents[i + 4] = parent;
    }
    // The runtime joints: `TransN` under TopN (RE-510 traces), `YRotN`
    // under `XRotN` (the shield's matrix composes through it), and
    // `XRotN` under TopN. A shield collision marks `YRotN`'s chain
    // (RE-511); the YRotN link awaits an N64 shield trace.
    f.joint_parents[1] = 0;
    f.joint_parents[2] = 0;
    f.joint_parents[3] = 2;
}

/// Joints 4.. from the model nodes' collision matrices (`posed`, model
/// units under TopN), placed by TopN's `axes` (its rotation times its
/// scale) at `f.pos`.
pub fn set_posed_joints(
    posed: &[ssb_rom::scene::Mat4],
    axes: [ssb_engine::math::Vec3; 3],
    f: &mut ssb_game::fighter::Fighter,
) {
    use ssb_engine::math::Vec3;
    use ssb_game::fighter::{JointTransform, FIGHTER_JOINTS};
    let world = |v: Vec3| axes[0] * v.x + axes[1] * v.y + axes[2] * v.z;
    let scale = ssb_rom::pack::MODEL_SCALE;
    let count = posed.len().min(FIGHTER_JOINTS - 4);
    for (i, matrix) in posed[..count].iter().enumerate() {
        let axis = |col: usize| {
            let m = &matrix.0;
            world(Vec3::new(m[col * 4], m[col * 4 + 1], m[col * 4 + 2]))
        };
        let t = matrix.translation();
        f.joint_transforms[i + 4] = Some(JointTransform {
            axes: [axis(0), axis(1), axis(2)],
            origin: f.pos + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
        });
    }
}

/// `gmCollisionGetFighterPartsWorldPosition`'s matrices for a demo fighter
/// ([`ssb_game::demo_fighter`]), from its clip's current pose: TopN at
/// `pos`, turned `yaw` about y (`lbCommonSin` axes) and scaled by its DObj
/// scale, which the selects and the results set to
/// `dSCSubsysFighterScales[fkind]` over `ftManagerMakeFighter`'s
/// `attr->size`. Call it after the clip advances and before the queued
/// effects are made.
pub fn sample_demo_joints(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    skeleton: &ssb_rom::skeleton::Skeleton,
    pos: ssb_engine::math::Vec3,
    yaw: f32,
    scale: f32,
    f: &mut ssb_game::fighter::Fighter,
) {
    use ssb_engine::math::Vec3;
    use ssb_game::fighter::{JointTransform, FIGHTER_JOINTS};
    f.pos = pos;
    f.joint_transforms = [None; FIGHTER_JOINTS];
    let (sin, cos) = ssb_engine::math::lb_sin_cos(yaw);
    let axes = [
        Vec3::new(cos, 0.0, -sin),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(sin, 0.0, cos),
    ]
    .map(|a| a * scale);
    f.joint_transforms[0] = Some(JointTransform { axes, origin: pos });
    f.is_use_animlocks = skeleton.locks.flagged;
    set_joint_parents(pack, object, skeleton, f);
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    let count = skeleton.compose_collision(pack, object, &mut posed);
    set_posed_joints(&posed[..count], axes, f);
}

/// Every part's collision local (`unk_dobjtrans_0x10`) and `translate.x`
/// as `skeleton` poses them now, the lead `XRotN` as joint 2; the composed
/// locals are kept in `kept`.
pub fn part_locals_into(
    skeleton: &ssb_rom::skeleton::Skeleton,
    pack: &Pack<'_>,
    object: &ObjectDesc,
    kept: &mut ssb_rom::skeleton::CollisionLocals,
) -> ssb_game::slope_contour::PartLocals {
    use ssb_game::fighter::FIGHTER_JOINTS;
    let mut parts = ssb_game::slope_contour::PartLocals::default();
    let mut lead = None;
    // Written straight into joints 4.. (no 3 KB staging copy, RE-580).
    let count = skeleton.collision_parts_keep(
        pack,
        object,
        kept,
        &mut parts.locals[4..],
        &mut parts.translate_x[4..],
        &mut lead,
    );
    for i in 0..count.min(FIGHTER_JOINTS - 4) {
        parts.present |= 1 << (i + 4);
    }
    if let Some((local, x)) = lead {
        parts.set(2, local, x);
    }
    parts
}
