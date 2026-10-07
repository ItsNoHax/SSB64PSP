//! Final Destination's boss wallpaper effects on the PSP
//! (`sc1PGameBossMakeWallpaperEffect`'s `DObj` trees and their
//! `gcPlayAnimAll`), drawn through `sc1PGameBossMakeCamera`'s two fixed
//! cameras.
//!
//! [`ssb_game::spgame::boss::BossWallpaper`] decides which effects live,
//! their alpha, root transform and how many plays each has had; this module
//! keeps each effect's joint clocks in step with those plays and draws its
//! packed graph. The scripts are file 114's own bytes, which the pack
//! already carries for the stage's layer animations. The effects'
//! material animations are not played: their primitives draw their rest
//! materials at the controller's alpha (RE-457).

use ssb_game::spgame::boss::{BossWallpaper, Display, Effect, EFFECT_ASSETS, EFFECT_FILE, MAX_EFFECTS};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::StageJoint;
use ssb_rom::pack::{ObjectDesc, Pack, MODEL_SCALE};
use ssb_rom::scene::Mat4;

/// The most nodes an effect graph has (`llGRLastMapEffects1DObjDesc`: 9).
const MAX_NODES: usize = 12;

/// One effect's tree and clocks.
struct Visual {
    serial: u32,
    object: ObjectDesc,
    count: usize,
    joints: [StageJoint; MAX_NODES],
    live: [bool; MAX_NODES],
    poses: [JointPose; MAX_NODES],
    plays: u32,
}

/// The boss stage's effect visuals, one per controller slot.
pub struct BossEffects {
    script: Option<(u32, u32)>,
    /// File 114's objects, in table order: a new effect's tree is looked
    /// up here rather than in the whole object table (RE-471).
    objects: alloc::vec::Vec<ssb_rom::pack::ObjectDesc>,
    visuals: [Option<Visual>; MAX_EFFECTS],
}

/// `dGCPerspDefault`: the boss cameras keep the default projection.
pub const CAMERA_FOVY: f32 = 30.0;
pub const CAMERA_NEAR: f32 = 100.0;
pub const CAMERA_FAR: f32 = 12800.0;

impl BossEffects {
    pub fn new(pack: &Pack<'_>) -> Self {
        // Every animation packed from file 114 carries the whole file.
        let script = (0..pack.anim_count())
            .filter_map(|i| pack.anim(i))
            .find(|a| a.source_file == EFFECT_FILE)
            .map(|a| (a.script_offset, a.script_len));
        let objects = (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .filter(|o| o.source_file == EFFECT_FILE)
            .collect();
        BossEffects {
            script,
            objects,
            visuals: core::array::from_fn(|_| None),
        }
    }

    fn data<'a>(&self, pack: &Pack<'a>) -> Option<&'a [u8]> {
        let (at, len) = self.script?;
        pack.blob(at, len as usize)
    }

    /// Brings every visual to its effect's plays: a new effect builds its
    /// tree and starts its joint scripts (`lbCommonAddTreeDObjsAnimAll`),
    /// then each play the controller counted ticks them at the effect's
    /// animation speed.
    pub fn sync(&mut self, pack: &Pack<'_>, wallpaper: &BossWallpaper) {
        let data = self.data(pack);
        for (slot, effect) in self.visuals.iter_mut().zip(wallpaper.effects.iter()) {
            let Some(e) = effect else {
                *slot = None;
                continue;
            };
            if slot.as_ref().is_none_or(|v| v.serial != e.serial) {
                *slot = make_visual(pack, &self.objects, data, e);
            }
            let (Some(v), Some(data)) = (slot.as_mut(), data) else {
                continue;
            };
            while v.plays < e.plays {
                v.plays += 1;
                for i in 0..v.count {
                    if v.live[i] && v.joints[i].tick(data, e.anim_speed, &mut v.poses[i]).is_err() {
                        v.live[i] = false;
                    }
                }
            }
        }
    }

    /// Draws the live effects whose plan names camera `tag`, under the
    /// boss camera's projection and view (`sc1PGameBossMakeCamera`).
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    pub unsafe fn draw(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        wallpaper: &BossWallpaper,
        tag: u8,
    ) {
        let mut any = false;
        for (v, e) in self.visuals.iter().zip(wallpaper.effects.iter()) {
            let (Some(v), Some(e)) = (v, e) else {
                continue;
            };
            if e.camera_tag != tag
                || e.hidden
                || matches!(e.display, Display::FadeAlpha | Display::FadeColor)
            {
                continue;
            }
            if !any {
                any = true;
                gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
                gpu.set_perspective(
                    CAMERA_FOVY,
                    ssb_game::camera::BATTLE_VIEWPORT_WIDTH
                        / ssb_game::camera::BATTLE_VIEWPORT_HEIGHT,
                    CAMERA_NEAR,
                    CAMERA_FAR,
                );
                gpu.set_view(&ssb_engine::math::Mat4::look_at(
                    ssb_game::spgame::boss::CAMERA_EYE,
                    ssb_engine::math::Vec3::ZERO,
                    ssb_engine::math::Vec3::Y,
                ));
            }
            let mut posed = [Mat4::IDENTITY; MAX_NODES];
            let n = compose(pack, v, e, &mut posed);
            gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
            let base = gpu.model_matrix();
            let alpha = e.alpha.clamp(0, 0xFF) as u8;
            let env = (e.display == Display::Comet).then(|| {
                let c = ssb_game::spgame::boss::COMET_ENV_COLORS[usize::from(e.color) % 7];
                [c[0], c[1], c[2], alpha]
            });
            st.color_override = Some(ssb_rom::skeleton::EffectColors {
                prim: Some([0xFF, 0xFF, 0xFF, alpha]),
                env,
                blend: None,
                light1: None,
                light2: None,
            });
            crate::meshdraw::draw_object_posed(pack, &v.object, &base, &posed[..n], None, st, None, None, 0);
            st.color_override = None;
        }
    }
}

/// A new effect's visual. `objects` are file 114's objects and `data` its
/// animation script ([`BossEffects::new`]).
fn make_visual(pack: &Pack<'_>, objects: &[ssb_rom::pack::ObjectDesc], data: Option<&[u8]>, e: &Effect) -> Option<Visual> {
    let asset = EFFECT_ASSETS
        .get(usize::from(e.wallpaper))?
        .get(usize::from(e.effect))
        .copied()
        .flatten()?;
    let object = *objects.iter().find(|o| o.source_offset == asset.graph)?;
    let count = (object.node_count as usize).min(MAX_NODES);
    let mut rest = [JointPose::default(); MAX_NODES];
    for (i, pose) in rest.iter_mut().enumerate().take(count) {
        if let Some(node) = pack.node(object.first_node + i as u32) {
            *pose = JointPose {
                rotate: node.rest_rotate,
                translate: node.rest_translate,
                scale: node.rest_scale,
            };
        }
    }
    let mut joints = [StageJoint::start(0, 0.0); MAX_NODES];
    let mut live = [false; MAX_NODES];
    if let (Some(table), Some(data)) = (asset.anim_joint, data) {
        for (i, script) in ssb_rom::objanim::joint_scripts(data, table, count)
            .into_iter()
            .enumerate()
        {
            if let Some(s) = script {
                joints[i] = StageJoint::start_changed(s, 0.0);
                live[i] = true;
            }
        }
    }
    Some(Visual {
        serial: e.serial,
        object,
        count,
        joints,
        live,
        poses: rest,
        // The make's `gcPlayAnimAll` is the first of `Effect::plays`.
        plays: 0,
    })
}

/// Every node's matrix: the root from the controller's transform, the
/// rest from their poses, parent first.
fn compose(pack: &Pack<'_>, v: &Visual, e: &Effect, out: &mut [Mat4]) -> usize {
    let count = v.count.min(out.len());
    for i in 0..count {
        let mut pose = v.poses[i];
        if i == 0 {
            pose.translate = [e.translate.x, e.translate.y, e.translate.z];
            if e.sets_rotate_z() {
                pose.rotate[2] = e.rotate_z;
            }
            if e.sets_scale() {
                pose.scale = [e.scale.x, e.scale.y, e.scale.z];
            }
        }
        let t = [
            pose.translate[0] / MODEL_SCALE,
            pose.translate[1] / MODEL_SCALE,
            pose.translate[2] / MODEL_SCALE,
        ];
        let local = Mat4::from_trs(t, pose.rotate, pose.scale);
        let parent = pack
            .node(v.object.first_node + i as u32)
            .and_then(|n| n.parent.checked_sub(v.object.first_node))
            .map(|p| p as usize)
            .filter(|&p| p < i);
        out[i] = match parent {
            Some(p) => out[p].mul(&local),
            None => local,
        };
    }
    count
}
