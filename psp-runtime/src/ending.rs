//! The 1P Game's last scenes on the PSP: the ending movie's room
//! (`mvEndingMakeRoom*`) and operator camera, and the staff roll's letters,
//! highlight frame and name motion (`scstaffroll.c`).
//!
//! The ending's props are posed once: each `mvEndingMakeRoom*` adds its
//! joint animation 300 frames in and plays it once (`gcPlayAnimAll`), and
//! no process plays it again. The background's material animation is
//! likewise played once from frame 0; its materials draw as packed.

use alloc::{boxed::Box, vec::Vec};
use ssb_game::spgame::{frontend::StaffrollAssets, staffroll};
use ssb_rom::ending as asset;
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::StageJoint;
use ssb_rom::pack::{ObjectDesc, Pack, MODEL_SCALE};
use ssb_rom::scene::Mat4;

/// The most nodes a room prop has (the background: 53).
const MAX_NODES: usize = 64;

/// A packed `AnimDesc::EFFECT` slot's bytes.
fn effect_blob<'a>(pack: &Pack<'a>, slot: u32) -> Option<&'a [u8]> {
    let a = pack.effect_anim(slot)?;
    pack.anim_script(&a)
}

fn object(pack: &Pack<'_>, file: u32, offset: u32) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| o.source_file == file && o.source_offset == offset)
}

/// Each node's matrix from its pose, parent first, in pack units under a
/// `MODEL_SCALE` base.
fn compose(pack: &Pack<'_>, object: &ObjectDesc, poses: &[JointPose], out: &mut [Mat4]) -> usize {
    let count = poses.len().min(out.len());
    for i in 0..count {
        let pose = poses[i];
        let t = pose.translate.map(|v| v / MODEL_SCALE);
        let local = Mat4::from_trs(t, pose.rotate, pose.scale);
        let parent = pack
            .node(object.first_node + i as u32)
            .and_then(|n| n.parent.checked_sub(object.first_node))
            .map(|p| p as usize)
            .filter(|&p| p < i);
        out[i] = match parent {
            Some(p) => out[p].mul(&local),
            None => local,
        };
    }
    count
}

fn rest_poses(pack: &Pack<'_>, object: &ObjectDesc) -> Vec<JointPose> {
    let count = (object.node_count as usize).min(MAX_NODES);
    (0..count)
        .map(|i| {
            pack.node(object.first_node + i as u32)
                .map_or(JointPose::default(), |node| JointPose {
                    rotate: node.rest_rotate,
                    translate: node.rest_translate,
                    scale: node.rest_scale,
                })
        })
        .collect()
}

/// One room prop, posed.
struct Prop {
    object: ObjectDesc,
    posed: Box<[Mat4; MAX_NODES]>,
    count: usize,
    /// Ejected at tick 540 (all but the desk).
    ejected_at_540: bool,
}

/// How a prop's joint animation is attached.
#[derive(Clone, Copy)]
enum Anim {
    None,
    /// `gcAddAnimJointAll`: one script per node from a table.
    Table(u32),
    /// `gcAddDObjAnimJoint` on the only node.
    Root(u32),
}

/// The ending's room, as `mvEndingFuncStart` makes it.
pub struct Room {
    props: Vec<Prop>,
}

impl Room {
    pub fn new(pack: &Pack<'_>) -> Option<Room> {
        let data = effect_blob(pack, asset::ROOM_SCRIPTS_SLOT)?;
        // `mvEndingFuncStart`'s order: background, desk, books, lamp,
        // pencils, tissues.
        let plan = [
            (asset::ROOM_BACKGROUND, Anim::None, true),
            (asset::ROOM_DESK, Anim::None, false),
            (asset::ROOM_BOOKS, Anim::Table(asset::ROOM_BOOKS_ANIM_JOINT), true),
            (asset::ROOM_LAMP, Anim::Table(asset::ROOM_LAMP_ANIM_JOINT), true),
            (asset::ROOM_PENCILS, Anim::Table(asset::ROOM_PENCILS_ANIM_JOINT), true),
            (asset::ROOM_TISSUES, Anim::Root(asset::ROOM_TISSUES_ANIM_JOINT), true),
        ];
        let mut props = Vec::new();
        for (graph, anim, ejected_at_540) in plan {
            let object = object(pack, asset::ROOM_FILE, graph)?;
            let mut poses = rest_poses(pack, &object);
            let scripts: Vec<Option<u32>> = match anim {
                Anim::None => Vec::new(),
                Anim::Table(table) => ssb_rom::objanim::joint_scripts(data, table, poses.len()),
                Anim::Root(script) => alloc::vec![Some(script)],
            };
            for (pose, script) in poses.iter_mut().zip(scripts) {
                if let Some(script) = script {
                    let mut joint = StageJoint::start_changed(
                        script,
                        ssb_game::spgame::ending::PROP_ANIM_START,
                    );
                    let _ = joint.tick(data, 1.0, pose);
                }
            }
            let mut posed = Box::new([Mat4::IDENTITY; MAX_NODES]);
            let count = compose(pack, &object, &poses, &mut posed[..]);
            props.push(Prop {
                object,
                posed,
                count,
                ejected_at_540,
            });
        }
        Some(Room { props })
    }

    /// Draws the room under the current camera: the desk only once the
    /// props are ejected.
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    pub unsafe fn draw(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        props_shown: bool,
    ) {
        for prop in &self.props {
            if prop.ejected_at_540 && !props_shown {
                continue;
            }
            gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
            let base = gpu.model_matrix();
            crate::meshdraw::draw_object_posed(
                pack,
                &prop.object,
                &base,
                &prop.posed[..prop.count],
                None,
                st,
                None,
                None,
                0,
            );
        }
    }
}

/// `llMVEndingOperatorCamAnimJoint`'s plays, baked
/// (`ssb_rom::campaign::camera_frames`).
pub fn ending_camera(pack: &Pack<'_>) -> Option<crate::scene::CameraAnim> {
    crate::scene::CameraAnim::load(pack, asset::ENDING_CAMERA_SLOT)
}

/// The staff roll's name path and tilt, from a copy of file 195.
pub struct StaffrollMotion {
    data: Vec<u8>,
    spline: ssb_rom::interp::Spline,
}

impl staffroll::NameMotion for StaffrollMotion {
    fn path(&self, t: f32) -> [f32; 3] {
        self.spline.cubic(&self.data, t).unwrap_or([0.0; 3])
    }

    fn rotate_z(&self, frame: f32) -> f32 {
        let mut pose = JointPose::default();
        let mut joint = StageJoint::start_changed(asset::STAFFROLL_ANIM_JOINT, frame);
        let _ = joint.tick(&self.data, 1.0, &mut pose);
        pose.rotate[2]
    }
}

/// The staff roll's ROM data from the pack: the credits tables and file
/// 195's path and tilt.
pub fn staffroll_assets(pack: &Pack<'_>) -> Option<StaffrollAssets> {
    let credits = staffroll::Credits::parse(effect_blob(pack, asset::CREDITS_SLOT)?)?;
    let data = effect_blob(pack, asset::STAFFROLL_SCRIPTS_SLOT)?.to_vec();
    let spline = ssb_rom::interp::Spline::read(&data, asset::STAFFROLL_INTERPOLATION)?;
    Some(StaffrollAssets {
        credits,
        motion: Box::new(StaffrollMotion { data, spline }),
    })
}

/// The staff roll's packed textures: each name/job letter and each text
/// box glyph, by font index.
pub struct StaffrollTextures {
    letters: [Option<ssb_rom::pack::SpriteDesc>; 56],
    pub frame: Option<ObjectDesc>,
}

impl StaffrollTextures {
    pub fn new(pack: &Pack<'_>) -> Self {
        StaffrollTextures {
            letters: core::array::from_fn(|i| pack.sprite(asset::STAFFROLL_FILE, asset::NAME_IMAGES[i])),
            frame: object(pack, asset::STAFFROLL_FILE, asset::STAFFROLL_FRAME_GRAPH),
        }
    }

    /// Draws one rolling text's letters: the root `DObj`'s translation and
    /// Z rotation, then each letter's quad (`scStaffrollInitNameAndJobDisplayLists`:
    /// twice the glyph's width and height, centred on the letter's
    /// translation).
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_quad_3d`].
    pub unsafe fn draw_text(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        text: &staffroll::Text,
        color: [u8; 3],
    ) {
        for letter in &text.letters {
            let Some(sprite) = self.letters.get(usize::from(letter.glyph)).copied().flatten() else {
                continue;
            };
            let [w, h] = staffroll::NAME_GLYPHS[usize::from(letter.glyph)].map(f32::from);
            gpu.model_transform(text.translate, [0.0, 0.0, text.rotate_z], 1.0);
            let (x, y) = (letter.x, letter.y);
            crate::meshdraw::draw_quad_3d(
                pack,
                sprite.texture,
                [x - w, y + h, x + w, y - h],
                [color[0], color[1], color[2], 0xFF],
                st,
            );
        }
    }

    /// Draws the highlighted name's frame (`llSCStaffrollDObjDesc`): the
    /// root on the name, the child at the name's right edge.
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    pub unsafe fn draw_frame(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        frame: &staffroll::Frame,
    ) {
        let Some(object) = self.frame else {
            return;
        };
        let mut poses = rest_poses(pack, &object);
        if let Some(root) = poses.get_mut(0) {
            root.translate = frame.translate;
            root.rotate = [0.0, 0.0, frame.rotate_z];
        }
        if let Some(child) = poses.get_mut(1) {
            child.translate[0] = frame.child_x;
        }
        let mut posed = [Mat4::IDENTITY; 4];
        let n = compose(pack, &object, &poses, &mut posed);
        gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
        let base = gpu.model_matrix();
        crate::meshdraw::draw_object_posed(pack, &object, &base, &posed[..n], None, st, None, None, 0);
    }
}
