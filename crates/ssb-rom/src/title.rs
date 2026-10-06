//! `mnTitle`'s animated sprite layouts (`mn/mncommon/mntitle.c`).
//!
//! The title positions its labels and its "Press Start" from `DObj` trees
//! that `AnimJoint` scripts move: `mnTitlePlayAnim` runs
//! `gcPlayAnimAll` on the tree, then sets each `SObj`'s scale from its
//! child `DObj`'s scale and centres it on the child's translation (`x +
//! 160`, `120 - y`). The trees draw nothing themselves.
//!
//! Neither tree depends on anything but its own script, so the build plays
//! each one ahead of time and packs the result: per play, per child of the
//! root, `[translate.x, translate.y, scale.x, scale.y]`. Play 0 is the
//! creation's `gcPlayAnimAll`, after which `mnTitleSetPosition` moves each
//! child to its sprite's place (`pins`); later plays only change the tracks
//! their scripts key.

use alloc::vec::Vec;

use crate::figatree::JointPose;
use crate::objanim::{joint_scripts, AnimError, StageJoint};

/// `llMNTitleFileID`.
pub const FILE: u32 = 0xA7;
/// `llMNTitleFireAnimFileID`.
pub const FIRE_FILE: u32 = 0xA8;

/// `llMNTitleLabelsDObjDesc` and `llMNTitleLabelsAnimJoint`.
pub const LABELS_DOBJDESC: u32 = 0x26130;
pub const LABELS_ANIM_JOINT: u32 = 0x25350;
/// `llMNTitlePressStartDObjDesc` and `llMNTitlePressStartAnimJoint`.
pub const PRESS_START_DOBJDESC: u32 = 0x262C0;
pub const PRESS_START_ANIM_JOINT: u32 = 0x258D0;

/// `llMNTitleLogoDObjDesc` and `llMNTitleLogoAnimJoint`: the opening
/// layout's logo (RE-467), whose root's children place the fire logo's
/// cutout and two strikes (0 to 2) and the red logo (3).
pub const LOGO_DOBJDESC: u32 = 0x26020;
pub const LOGO_ANIM_JOINT: u32 = 0x251D0;
/// `llMNTitleSlashDObjDesc` and `llMNTitleSlashAnimJoint` (its
/// `MatAnimJoint` is in `crate::opening::MAT_ANIM_JOINTS`).
pub const SLASH_DOBJDESC: u32 = 0x28DA8;
pub const SLASH_ANIM_JOINT: u32 = 0x25E70;
/// `llMNTitleFireDObjDesc` and `llMNTitleFireAnimJoint`: the tree whose
/// node `root->child->sib_next->child` carries the logo fire's particle
/// generator.
pub const FIRE_DOBJDESC: u32 = 0x28EB0;
pub const FIRE_ANIM_JOINT: u32 = 0x29010;
/// `dMNTitleLogoAnimSprites`: `LogoAnimCutout`, `...StrikeV`,
/// `...StrikeH`, `...Full`.
pub const LOGO_ANIM_SPRITES: [u32; 4] = [0x8FC8, 0x97E8, 0x9B48, 0xBBB0];

/// The reserved animation slots the baked plays are packed under
/// (`AnimDesc::EFFECT`), in the title's menu pack.
pub const LABELS_SLOT: u32 = 0xF200;
pub const PRESS_START_SLOT: u32 = 0xF201;
pub const LOGO_SLOT: u32 = 0xF202;

/// The logo tree's plays the opening layout reads: the creation's, then
/// ticks 1 to 170 (`mnTitleSetEndLogoPosition` stops it).
pub const LOGO_PLAYS: usize = 171;
/// Children of the logo tree's root.
pub const LOGO_CHILDREN: usize = 4;

/// Floats per child per play.
pub const CHILD_FLOATS: usize = 4;

/// `DOBJ_ARRAY_MAX`: the terminator id.
const TERMINATOR: u32 = 18;
const DESC_SIZE: usize = crate::scene::DOBJ_DESC_SIZE;

fn word(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// One `DObjDesc`'s depth and rest pose.
fn descs(data: &[u8], offset: u32) -> Option<Vec<(u32, JointPose)>> {
    let mut out = Vec::new();
    let mut at = offset as usize;
    loop {
        let id = word(data, at)?;
        if id == TERMINATOR {
            return Some(out);
        }
        let f = |i: usize| word(data, at + 8 + i * 4).map(f32::from_bits);
        let v = |i: usize| -> Option<[f32; 3]> { Some([f(i)?, f(i + 1)?, f(i + 2)?]) };
        out.push((
            id & 0xFFF,
            JointPose {
                translate: v(0)?,
                rotate: v(3)?,
                scale: v(6)?,
            },
        ));
        at += DESC_SIZE;
        if out.len() > TERMINATOR as usize {
            return None;
        }
    }
}

/// Plays the tree at `dobjdesc` with the scripts at `joints` `plays` times
/// from frame 0, the first play then pinning child `i`'s translation to
/// `pins[i]`. Returns `plays * children` entries of
/// `[translate.x, translate.y, scale.x, scale.y]`, children being the
/// root's direct children in sibling order.
pub fn bake(
    data: &[u8],
    dobjdesc: u32,
    joints: u32,
    pins: &[[f32; 2]],
    plays: usize,
) -> Result<Vec<[f32; CHILD_FLOATS]>, AnimError> {
    let descs = descs(data, dobjdesc).ok_or(AnimError::Truncated {
        at: dobjdesc as usize,
    })?;
    let scripts = joint_scripts(data, joints, descs.len());
    let children: Vec<usize> = (0..descs.len()).filter(|&i| descs[i].0 == 1).collect();
    let mut poses: Vec<JointPose> = descs.iter().map(|d| d.1).collect();
    let mut anims: Vec<Option<StageJoint>> = scripts
        .iter()
        .map(|s| s.map(|s| StageJoint::start_changed(s, 0.0)))
        .collect();
    let mut out = Vec::with_capacity(plays * children.len());
    for play in 0..plays {
        for (anim, pose) in anims.iter_mut().zip(poses.iter_mut()) {
            if let Some(a) = anim {
                a.tick(data, 1.0, pose)?;
            }
        }
        if play == 0 {
            for (&c, pin) in children.iter().zip(pins) {
                poses[c].translate[0] = pin[0];
                poses[c].translate[1] = pin[1];
            }
        }
        for &c in &children {
            let p = &poses[c];
            out.push([p.translate[0], p.translate[1], p.scale[0], p.scale[1]]);
        }
    }
    Ok(out)
}

/// `mnTitleSetPosition`'s `DObj` translation for a sprite centred at
/// `(x, y)` on the 320 x 240 screen.
pub fn pin(x: f32, y: f32) -> [f32; 2] {
    [x - 160.0, -(y - 120.0)]
}

/// `dMNTitleCommonSpriteDescs`' centres (US) for the labels' seven
/// children (`DropShadow`, `Smash`, `Super`, `Bros`, `TM`, `Footer`,
/// `Header`) and "Press Start".
pub const LABEL_CENTRES: [[f32; 2]; 7] = [
    [157.0, 94.0],
    [161.0, 88.0],
    [55.0, 96.0],
    [268.0, 96.0],
    [270.0, 132.0],
    [160.0, 208.0],
    [160.0, 15.0],
];
pub const PRESS_START_CENTRE: [f32; 2] = [162.0, 177.0];

/// Plays the labels' tree takes: the creation's, then ticks 170 to 219.
pub const LABEL_PLAYS: usize = 51;
/// Children of the labels' root.
pub const LABEL_CHILDREN: usize = 7;

/// "Press Start"'s script returns to its second play every 39 plays: 20
/// shown, 19 at no size. Play `n > 0` is play `1 + (n - 1) % 39`.
pub const PRESS_START_PERIOD: usize = 39;

/// A packed bake's frames (the reserved `slot` of `AnimDesc::EFFECT`).
pub fn packed_frames<'a>(pack: &crate::pack::Pack<'a>, slot: u32) -> Option<&'a [u8]> {
    let a = (0..pack.anim_count())
        .filter_map(|i| pack.anim(i))
        .find(|a| a.fighter == crate::pack::AnimDesc::EFFECT && a.slot == slot)?;
    pack.anim_script(&a)
}

/// The baked frames as packed: little-endian `f32`s.
pub fn to_bytes(frames: &[[f32; CHILD_FLOATS]]) -> Vec<u8> {
    frames
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

/// One child's entry of a packed bake.
pub fn packed(bytes: &[u8], children: usize, play: usize, child: usize) -> Option<[f32; 4]> {
    let at = ((play * children) + child) * CHILD_FLOATS * 4;
    let b = bytes.get(at..at + CHILD_FLOATS * 4)?;
    Some(core::array::from_fn(|i| {
        f32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]])
    }))
}
