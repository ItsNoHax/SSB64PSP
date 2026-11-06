//! Final Destination's boss wallpaper effects on the PSP
//! (`sc1PGameBossMakeWallpaperEffect`'s `DObj` trees and their
//! `gcPlayAnimAll`), drawn through `sc1PGameBossMakeCamera`'s two fixed
//! cameras.
//!
//! [`ssb_game::spgame::boss::BossWallpaper`] decides which effects live,
//! their alpha, root transform and how many plays each has had; this module
//! keeps each effect's joint clocks in step with those plays and draws its
//! packed graph. The scripts are file 114's own bytes, which the pack
//! already carries for the stage's layer animations. An effect with
//! materials also plays its `o_matanim_joint` scripts (texture, palette and
//! UV tracks) on its `MObj`s, at the same speed; the colours stay the
//! display callback's PRIM and ENV (RE-457).

use ssb_game::spgame::boss::{BossWallpaper, Display, Effect, EFFECT_ASSETS, EFFECT_FILE, MAX_EFFECTS};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::StageJoint;
use ssb_rom::pack::{ObjectDesc, Pack, TextureDesc, MODEL_SCALE};
use ssb_rom::scene::Mat4;
use ssb_rom::skeleton::EffectMaterialAnimator;

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
    /// Each node's parent below it in the tree, read once at the make: the
    /// pack's node rows never change.
    parents: [Option<u8>; MAX_NODES],
    /// The `MObj`s' material scripts, for an effect with materials.
    mats: Option<alloc::boxed::Box<EffectMaterialAnimator>>,
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

/// What [`BossEffects::write_new`] reads from the pack, found before the
/// effects' slot exists: the object list is its one allocation.
pub struct BossEffectFiles {
    script: Option<(u32, u32)>,
    objects: alloc::vec::Vec<ObjectDesc>,
}

impl BossEffectFiles {
    pub fn find(pack: &Pack<'_>) -> Self {
        // Every animation packed from file 114 carries the whole file.
        let script = (0..pack.anim_count())
            .filter_map(|i| pack.anim(i))
            .find(|a| a.source_file == EFFECT_FILE)
            .map(|a| (a.script_offset, a.script_len));
        let objects = (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .filter(|o| o.source_file == EFFECT_FILE)
            .collect();
        BossEffectFiles { script, objects }
    }
}

impl BossEffects {
    /// The effects with no visual live, written into `dst` slot by slot:
    /// by value the visuals land on the stack and are copied (RE-469).
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned; its old contents are
    /// overwritten without being dropped.
    #[inline(never)]
    pub unsafe fn write_new(dst: *mut BossEffects, files: BossEffectFiles) {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(BossEffects {
            script,
            objects,
            visuals
        });
        // SAFETY: every field of `*dst` is written once, through raw
        // pointers.
        unsafe {
            addr_of_mut!((*dst).script).write(files.script);
            addr_of_mut!((*dst).objects).write(files.objects);
            let visuals = addr_of_mut!((*dst).visuals).cast::<Option<Visual>>();
            for i in 0..MAX_EFFECTS {
                visuals.add(i).write(None);
            }
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
                if let Some(m) = v.mats.as_mut() {
                    m.tick_speed(pack, e.anim_speed);
                }
            }
        }
    }

    /// Draws the live effects whose plan names camera `tag`, under the
    /// boss camera's projection and view (`sc1PGameBossMakeCamera`), in
    /// [`BossWallpaper::draw_order`]: head 0 (the black
    /// `TranslucentDirect` layer and the fades, whose rectangle is `fade`)
    /// under head 1.
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    #[inline(never)]
    pub unsafe fn draw(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        wallpaper: &BossWallpaper,
        tag: u8,
        fade: Option<[u8; 4]>,
    ) {
        let mut any = false;
        // The RSP clips the wallpaper like any other list, at the game's
        // `FRUSTRATIO_1`: the viewport edges. The >180 tunnel's cone runs
        // from an apex ahead of the camera to a rim at its eye plane, its
        // ring vertices ~6.6 half-widths off screen. The N64 cuts each
        // triangle at the screen edge, takes the new corners' shade from
        // the 3D edge and shades linearly between; unclipped, the GE spread
        // the ring's shade over the whole screen-space edge and the tunnel
        // drew much darker (RE-519).
        let clip = (st.clip_view_proj.take(), st.clip_at_viewport);
        st.clip_at_viewport = true;
        let mut posed = [Mat4::IDENTITY; MAX_NODES];
        for i in wallpaper.draw_order(tag) {
            let Some(e) = &wallpaper.effects[i] else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                if let Some(color) = fade {
                    crate::meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], color, st);
                }
                continue;
            }
            let Some(v) = &self.visuals[i] else {
                continue;
            };
            if !any {
                any = true;
                gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
                let proj = ssb_engine::math::Mat4::sy_persp_fast(
                    CAMERA_FOVY,
                    ssb_game::camera::BATTLE_VIEWPORT_WIDTH
                        / ssb_game::camera::BATTLE_VIEWPORT_HEIGHT,
                    CAMERA_NEAR,
                    CAMERA_FAR,
                );
                gpu.set_projection(&proj);
                let view = ssb_engine::math::Mat4::look_at(
                    ssb_game::spgame::boss::CAMERA_EYE,
                    ssb_engine::math::Vec3::ZERO,
                    ssb_engine::math::Vec3::Y,
                );
                gpu.set_view(&view);
                st.clip_view_proj = Some((proj * view).cols);
            }
            // `compose` writes every node it returns before it is read.
            let n = compose(v, e, &mut posed);
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
            crate::meshdraw::draw_object_posed(
                pack,
                &v.object,
                &base,
                &posed[..n],
                None,
                st,
                None,
                v.mats.as_deref(),
                0,
            );
            st.color_override = None;
        }
        (st.clip_view_proj, st.clip_at_viewport) = clip;
    }

    /// [`Self::draw`], each pass over every effect of the camera before the
    /// next ([`crate::meshdraw::batch_pose`]): the same commands in the same
    /// order. A frame with an effect the passes do not draw as
    /// [`crate::meshdraw::draw_object_posed`] would, or past their scratch,
    /// takes [`Self::draw_unbatched`].
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    #[inline(never)]
    pub unsafe fn draw_batched(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        wallpaper: &BossWallpaper,
        tag: u8,
        fade: Option<[u8; 4]>,
    ) {
        let mut size = crate::meshdraw::BatchSize::default();
        let mut ready = crate::meshdraw::batch_ready(st);
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                continue;
            }
            if let Some(Some(v)) = self.visuals.get(i) {
                ready = ready && crate::meshdraw::plan_object(pack, &v.object, &mut size);
            }
        }
        if !ready || !size.fits() {
            self.draw_unbatched_v2(gpu, pack, st, wallpaper, tag, fade);
            return;
        }
        let clip = (st.clip_view_proj.take(), st.clip_at_viewport);
        st.clip_at_viewport = true;
        let mut batch = None;
        let mut counts = [0usize; MAX_EFFECTS];
        let mut next = 0;
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                if let Some(color) = fade {
                    crate::meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], color, st);
                }
                continue;
            }
            let Some(Some(v)) = self.visuals.get(i) else {
                continue;
            };
            if batch.is_none() {
                gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
                let proj = ssb_engine::math::Mat4::sy_persp_fast(
                    CAMERA_FOVY,
                    ssb_game::camera::BATTLE_VIEWPORT_WIDTH
                        / ssb_game::camera::BATTLE_VIEWPORT_HEIGHT,
                    CAMERA_NEAR,
                    CAMERA_FAR,
                );
                gpu.set_projection(&proj);
                let view = ssb_engine::math::Mat4::look_at(
                    ssb_game::spgame::boss::CAMERA_EYE,
                    ssb_engine::math::Vec3::ZERO,
                    ssb_engine::math::Vec3::Y,
                );
                gpu.set_view(&view);
                st.clip_view_proj = Some((proj * view).cols);
            }
            gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
            let base = gpu.model_matrix();
            // Every effect's base is the same: the first one's serves the
            // passes that run before any is drawn.
            let b = match &mut batch {
                Some(b) => b,
                None => {
                    let mut b = crate::meshdraw::Batch::new(&size);
                    self.pose_batch(pack, st, wallpaper, tag, &base, &mut b, &mut counts);
                    crate::meshdraw::batch_expand(pack, st, &mut b);
                    batch.insert(b)
                }
            };
            st.color_override = Some(effect_colors(e));
            let count = counts.get(i).copied().unwrap_or(0);
            crate::meshdraw::batch_submit(pack, st, b, next, count, v.mats.as_deref());
            next += count;
            st.color_override = None;
        }
        if let Some(b) = batch {
            b.finish();
        }
        (st.clip_view_proj, st.clip_at_viewport) = clip;
    }

    /// [`Self::draw_batched`] for a frame its passes do not take: with the
    /// Fall's two-tile blends or float UVs, the same passes with
    /// [`crate::meshdraw::batch_expand_lod_tag`] and
    /// [`crate::meshdraw::batch_submit_lod_object`] for theirs; anything else,
    /// or past their scratch, [`Self::draw`] whole. Out of line and apart
    /// from [`Self::draw_batched`], so the comets' passes keep their code
    /// (RE-585).
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    #[inline(never)]
    unsafe fn draw_unbatched(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        wallpaper: &BossWallpaper,
        tag: u8,
        fade: Option<[u8; 4]>,
    ) {
        let mut size = crate::meshdraw::BatchSize::default();
        let mut ready = crate::meshdraw::batch_ready(st);
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                continue;
            }
            if let Some(Some(v)) = self.visuals.get(i) {
                ready = ready && crate::meshdraw::plan_object_lod(pack, &v.object, &mut size);
            }
        }
        if !ready || !size.fits() {
            self.draw(gpu, pack, st, wallpaper, tag, fade);
            return;
        }
        let clip = (st.clip_view_proj.take(), st.clip_at_viewport);
        st.clip_at_viewport = true;
        let mut batch = None;
        let mut counts = [0usize; MAX_EFFECTS];
        let mut next = 0;
        let mut spares = 0;
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                if let Some(color) = fade {
                    crate::meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], color, st);
                }
                continue;
            }
            let Some(Some(v)) = self.visuals.get(i) else {
                continue;
            };
            if batch.is_none() {
                gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
                let proj = ssb_engine::math::Mat4::sy_persp_fast(
                    CAMERA_FOVY,
                    ssb_game::camera::BATTLE_VIEWPORT_WIDTH
                        / ssb_game::camera::BATTLE_VIEWPORT_HEIGHT,
                    CAMERA_NEAR,
                    CAMERA_FAR,
                );
                gpu.set_projection(&proj);
                let view = ssb_engine::math::Mat4::look_at(
                    ssb_game::spgame::boss::CAMERA_EYE,
                    ssb_engine::math::Vec3::ZERO,
                    ssb_engine::math::Vec3::Y,
                );
                gpu.set_view(&view);
                st.clip_view_proj = Some((proj * view).cols);
            }
            gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
            let base = gpu.model_matrix();
            // Every effect's base is the same: the first one's serves the
            // passes that run before any is drawn.
            let b = match &mut batch {
                Some(b) => b,
                None => {
                    let mut b = crate::meshdraw::Batch::new(&size);
                    self.pose_batch(pack, st, wallpaper, tag, &base, &mut b, &mut counts);
                    // `pose_batch`'s objects, in its order.
                    let mut objects = [(0usize, None); MAX_EFFECTS];
                    let mut n = 0;
                    for j in wallpaper.draw_order(tag) {
                        let Some(Some(e)) = wallpaper.effects.get(j) else {
                            continue;
                        };
                        if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                            continue;
                        }
                        let Some(Some(v)) = self.visuals.get(j) else {
                            continue;
                        };
                        if let Some(o) = objects.get_mut(n) {
                            *o = (counts.get(j).copied().unwrap_or(0), v.mats.as_deref());
                            n += 1;
                        }
                    }
                    let objects = objects.get(..n).unwrap_or(&[]);
                    crate::meshdraw::batch_expand_lod_tag(pack, st, &mut b, objects);
                    batch.insert(b)
                }
            };
            st.color_override = Some(effect_colors(e));
            let count = counts.get(i).copied().unwrap_or(0);
            crate::meshdraw::batch_submit_lod_object(
                pack,
                st,
                b,
                next,
                count,
                v.mats.as_deref(),
                &mut spares,
            );
            next += count;
            st.color_override = None;
        }
        if let Some(b) = batch {
            b.finish();
        }
        (st.clip_view_proj, st.clip_at_viewport) = clip;
    }

    /// [`Self::draw_batched`]'s first pass: every drawn effect's node
    /// matrices and skins, in draw order; `counts[i]` is effect `i`'s
    /// meshes.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    unsafe fn pose_batch(
        &self,
        pack: &Pack<'_>,
        st: &crate::meshdraw::DrawState,
        wallpaper: &BossWallpaper,
        tag: u8,
        base: &psp::sys::ScePspFMatrix4,
        batch: &mut crate::meshdraw::Batch,
        counts: &mut [usize; MAX_EFFECTS],
    ) {
        let mut posed = [Mat4::IDENTITY; MAX_NODES];
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                continue;
            }
            let Some(Some(v)) = self.visuals.get(i) else {
                continue;
            };
            let n = compose_batched(v, e, &mut posed);
            let nodes = posed.get(..n).unwrap_or(&[]);
            let count = crate::meshdraw::batch_pose(
                pack,
                &v.object,
                base,
                nodes,
                st,
                effect_colors(e),
                batch,
            );
            if let Some(c) = counts.get_mut(i) {
                *c = count;
            }
        }
    }
}

/// The display callback's PRIM (white at the effect's alpha) and, for a
/// comet, its ENV: [`BossEffects::draw`]'s `color_override`.
#[inline(always)]
fn effect_colors(e: &Effect) -> ssb_rom::skeleton::EffectColors {
    let alpha = e.alpha.clamp(0, 0xFF) as u8;
    let env = (e.display == Display::Comet).then(|| {
        let c = ssb_game::spgame::boss::COMET_ENV_COLORS[usize::from(e.color) % 7];
        [c[0], c[1], c[2], alpha]
    });
    ssb_rom::skeleton::EffectColors {
        prim: Some([0xFF, 0xFF, 0xFF, alpha]),
        env,
        blend: None,
        light1: None,
        light2: None,
    }
}

/// A new effect's visual. `objects` are file 114's objects and `data` its
/// animation script ([`BossEffectFiles::find`]).
fn make_visual(pack: &Pack<'_>, objects: &[ssb_rom::pack::ObjectDesc], data: Option<&[u8]>, e: &Effect) -> Option<Visual> {
    let asset = EFFECT_ASSETS
        .get()
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
    let mut parents = [None; MAX_NODES];
    for (i, parent) in parents.iter_mut().enumerate().take(count) {
        *parent = pack
            .node(object.first_node + i as u32)
            .and_then(|n| n.parent.checked_sub(object.first_node))
            .map(|p| p as usize)
            .filter(|&p| p < i)
            .map(|p| p as u8);
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
    let mats = asset
        .matanim_joint
        .zip(asset.mobjsub)
        .and_then(|(table, subs)| start_materials(pack, &object, count, table, subs));
    Some(Visual {
        serial: e.serial,
        object,
        count,
        joints,
        live,
        poses: rest,
        parents,
        mats,
        // The make's `gcPlayAnimAll` is the first of `Effect::plays`.
        plays: 0,
    })
}

/// `lbCommonAddTreeDObjsAnimAll`'s materials: every `MObj` of node `n`
/// plays `o_matanim_joint[n][m]` from frame 0, `m` its place in
/// `o_mobjsub[n]`. The pack binds one script per `MObj`; a tree two
/// effects share (`llGRLastMapEffects2_1`) restarts it on this effect's.
fn start_materials(
    pack: &Pack<'_>,
    object: &ObjectDesc,
    count: usize,
    table: u32,
    subs: u32,
) -> Option<alloc::boxed::Box<EffectMaterialAnimator>> {
    let mut mats = alloc::boxed::Box::new(EffectMaterialAnimator::new());
    for n in 0..count {
        let Some(mesh) = pack
            .node(object.first_node + n as u32)
            .and_then(|node| pack.mesh(node.mesh))
        else {
            continue;
        };
        for j in 0..mesh.prim_count {
            let Some(prim) = pack.prim(mesh.first_prim + j) else {
                continue;
            };
            if prim.mat_anim == TextureDesc::NO_ANIM {
                continue;
            }
            let Some(desc) = pack.mat_anim(prim.mat_anim) else {
                continue;
            };
            let script = pack
                .mat_anim_file(&desc)
                .and_then(|file| ssb_rom::matanim::mobj_script(file, table, subs, n, desc.source_offset));
            if let Some(script) = script {
                mats.restart(prim.mat_anim, script);
            }
        }
    }
    (!mats.is_empty()).then_some(mats)
}

/// Every node's matrix: the root from the controller's transform, the
/// rest from their poses, parent first.
fn compose(v: &Visual, e: &Effect, out: &mut [Mat4]) -> usize {
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
        let local = Mat4::from_trs_display(t, pose.rotate, pose.scale);
        out[i] = match v.parents[i] {
            Some(p) => out[usize::from(p)].mul(&local),
            None => local,
        };
    }
    count
}

/// [`compose`] for [`BossEffects::draw_batched`]: its text, kept apart so
/// [`BossEffects::draw`] still inlines its only caller's copy (RE-494).
fn compose_batched(v: &Visual, e: &Effect, out: &mut [Mat4]) -> usize {
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
        let local = Mat4::from_trs_display(t, pose.rotate, pose.scale);
        // A parent lies before its child (`make_visual`), so it is always
        // there; `get` only spares the bounds check's panic data.
        let parent = v.parents[i].and_then(|p| out.get(usize::from(p)).copied());
        out[i] = match parent {
            Some(parent) => parent.mul(&local),
            None => local,
        };
    }
    count
}

impl BossEffects {
    /// [`Self::draw_unbatched`] with the effects' shared model matrix built
    /// once and loaded for the rest (RE-585). When the effects cannot be
    /// batched it hands the draw to [`Self::draw_unbatched`].
    ///
    /// # Safety
    ///
    /// Same as [`crate::meshdraw::draw_mesh`].
    #[inline(never)]
    unsafe fn draw_unbatched_v2(
        &self,
        gpu: &mut crate::gu::Gpu,
        pack: &Pack<'_>,
        st: &mut crate::meshdraw::DrawState,
        wallpaper: &BossWallpaper,
        tag: u8,
        fade: Option<[u8; 4]>,
    ) {
        let mut size = crate::meshdraw::BatchSize::default();
        let mut ready = crate::meshdraw::batch_ready(st);
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                continue;
            }
            if let Some(Some(v)) = self.visuals.get(i) {
                ready = ready && crate::meshdraw::plan_object_lod(pack, &v.object, &mut size);
            }
        }
        if !ready || !size.fits() {
            self.draw_unbatched(gpu, pack, st, wallpaper, tag, fade);
            return;
        }
        let clip = (st.clip_view_proj.take(), st.clip_at_viewport);
        st.clip_at_viewport = true;
        let mut batch = None;
        let mut counts = [0usize; MAX_EFFECTS];
        let mut next = 0;
        let mut spares = 0;
        let mut model = None;
        for i in wallpaper.draw_order(tag) {
            let Some(Some(e)) = wallpaper.effects.get(i) else {
                continue;
            };
            if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                if let Some(color) = fade {
                    crate::meshdraw::fill_rect_n64([10.0, 10.0, 310.0, 230.0], color, st);
                }
                continue;
            }
            let Some(Some(v)) = self.visuals.get(i) else {
                continue;
            };
            if batch.is_none() {
                gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
                let proj = ssb_engine::math::Mat4::sy_persp_fast(
                    CAMERA_FOVY,
                    ssb_game::camera::BATTLE_VIEWPORT_WIDTH
                        / ssb_game::camera::BATTLE_VIEWPORT_HEIGHT,
                    CAMERA_NEAR,
                    CAMERA_FAR,
                );
                gpu.set_projection(&proj);
                let view = ssb_engine::math::Mat4::look_at(
                    ssb_game::spgame::boss::CAMERA_EYE,
                    ssb_engine::math::Vec3::ZERO,
                    ssb_engine::math::Vec3::Y,
                );
                gpu.set_view(&view);
                st.clip_view_proj = Some((proj * view).cols);
            }
            // Every effect's model matrix is the same: built once, then
            // loaded (the same matrix and update flag).
            let base = match model {
                Some(m) => {
                    gpu.load_model(&m);
                    m
                }
                None => {
                    gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
                    let m = gpu.model_matrix();
                    model = Some(m);
                    m
                }
            };
            // Every effect's base is the same: the first one's serves the
            // passes that run before any is drawn.
            let b = match &mut batch {
                Some(b) => b,
                None => {
                    let mut b = crate::meshdraw::Batch::new(&size);
                    self.pose_batch(pack, st, wallpaper, tag, &base, &mut b, &mut counts);
                    // `pose_batch`'s objects, in its order.
                    let mut objects = [(0usize, None); MAX_EFFECTS];
                    let mut n = 0;
                    for j in wallpaper.draw_order(tag) {
                        let Some(Some(e)) = wallpaper.effects.get(j) else {
                            continue;
                        };
                        if matches!(e.display, Display::FadeAlpha | Display::FadeColor) {
                            continue;
                        }
                        let Some(Some(v)) = self.visuals.get(j) else {
                            continue;
                        };
                        if let Some(o) = objects.get_mut(n) {
                            *o = (counts.get(j).copied().unwrap_or(0), v.mats.as_deref());
                            n += 1;
                        }
                    }
                    let objects = objects.get(..n).unwrap_or(&[]);
                    crate::meshdraw::batch_expand_lod_tag(pack, st, &mut b, objects);
                    batch.insert(b)
                }
            };
            st.color_override = Some(effect_colors(e));
            let count = counts.get(i).copied().unwrap_or(0);
            crate::meshdraw::batch_submit_lod_object(
                pack,
                st,
                b,
                next,
                count,
                v.mats.as_deref(),
                &mut spares,
            );
            next += count;
            st.color_override = None;
        }
        if let Some(b) = batch {
            b.finish();
        }
        (st.clip_view_proj, st.clip_at_viewport) = clip;
    }
}
