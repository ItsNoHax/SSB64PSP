//! The quake effect's camera shake, sampled from its packed animation.

use ssb_rom::pack::Pack;

/// `efManagerQuakeProcUpdate`'s `DObj` translation `(y, z)` after `ticks`
/// plays of the quake's packed animation (RE-416).
#[inline(never)]
pub fn quake_translate(p: &Pack<'_>, magnitude: u8, ticks: u16) -> Option<(f32, f32)> {
    let anim = p.effect_anim(ssb_rom::effect::QUAKE_ANIM_SLOT + u32::from(magnitude))?;
    let joint = p.anim_joint(anim.first_joint)?;
    let data = p.anim_script(&anim)?;
    let mut j = ssb_rom::objanim::StageJoint::start_changed(joint.script, 0.0);
    let mut pose = ssb_rom::figatree::JointPose {
        rotate: [0.0; 3],
        translate: [0.0; 3],
        scale: [1.0; 3],
    };
    for _ in 0..ticks {
        j.tick(data, 1.0, &mut pose).ok()?;
    }
    Some((pose.translate[1], pose.translate[2]))
}
