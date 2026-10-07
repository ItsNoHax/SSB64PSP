//! The title's opening layout on the PSP (`mnTitleMakeSlash`,
//! `mnTitleMakeLogoFire`, `mnTitleMakeLogoFireParticles`, RE-467): the
//! slash under its orthographic camera and the logo fire's particles under
//! its 3D camera, their trees played from the title's file in its scene
//! pack.

use alloc::{boxed::Box, vec::Vec};

use ssb_engine::math::{Mat4, Vec3};
use ssb_game::particle::{self as lb, NoProcDead, Particles};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw::{self, DrawState};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::{joint_scripts, StageJoint};
use ssb_rom::pack::{ObjectDesc, Pack, MODEL_SCALE};
use ssb_rom::skeleton::EffectMaterialAnimator;
use ssb_rom::title as t;

/// The title's file as the scene pack carries it.
fn script_data<'a>(scene: &Pack<'a>) -> Option<&'a [u8]> {
    let slot = ssb_rom::opening::blob_slot(t::FILE);
    let a = scene.effect_anim(slot)?;
    scene.anim_script(&a)
}

/// A `DObjDesc` tree and its `AnimJoint` table (`gcAddAnimJointAll`).
struct Tree {
    object: ObjectDesc,
    poses: Vec<JointPose>,
    joints: Vec<Option<StageJoint>>,
    mats: Option<Box<EffectMaterialAnimator>>,
}

impl Tree {
    fn new(main: &Pack<'_>, data: &[u8], graph: u32, table: u32, mats: bool) -> Option<Tree> {
        let object = ssb_psp_runtime::scene::object_keyed(main, (t::FILE, graph))?;
        let poses: Vec<JointPose> = (0..object.node_count)
            .map(|i| {
                main.node(object.first_node + i)
                    .map_or(JointPose::default(), |n| JointPose {
                        rotate: n.rest_rotate,
                        translate: n.rest_translate,
                        scale: n.rest_scale,
                    })
            })
            .collect();
        let joints = joint_scripts(data, table, poses.len())
            .into_iter()
            .map(|s| s.map(|s| StageJoint::start_changed(s, 0.0)))
            .collect();
        let mats = mats.then(|| {
            let mut m = Box::new(EffectMaterialAnimator::new());
            let prims = (0..object.node_count)
                .filter_map(|n| main.node(object.first_node + n))
                .filter_map(|node| main.mesh(node.mesh))
                .flat_map(|mesh| (0..mesh.prim_count).map(move |j| mesh.first_prim + j))
                .filter_map(|i| main.prim(i))
                .map(|prim| prim.mat_anim);
            m.start(main, prims);
            m
        });
        Some(Tree {
            object,
            poses,
            joints,
            mats,
        })
    }

    /// `gcPlayAnimAll`.
    fn play(&mut self, main: &Pack<'_>, data: &[u8]) {
        for (j, pose) in self.joints.iter_mut().zip(self.poses.iter_mut()) {
            if let Some(j) = j {
                let _ = j.tick(data, 1.0, pose);
            }
        }
        if let Some(m) = self.mats.as_mut() {
            m.tick(main);
        }
    }

    /// Each node's matrix, parent first; translations divided by `k`.
    fn compose(&self, main: &Pack<'_>, k: f32) -> Vec<ssb_rom::scene::Mat4> {
        let mut out: Vec<ssb_rom::scene::Mat4> = Vec::with_capacity(self.poses.len());
        for (i, pose) in self.poses.iter().enumerate() {
            let local = ssb_rom::scene::Mat4::from_trs(
                pose.translate.map(|v| v / k),
                pose.rotate,
                pose.scale,
            );
            let parent = main
                .node(self.object.first_node + i as u32)
                .and_then(|n| n.parent.checked_sub(self.object.first_node))
                .map(|p| p as usize)
                .filter(|&p| p < i);
            let m = match parent {
                Some(p) => out[p].mul(&local),
                None => local,
            };
            out.push(m);
        }
        out
    }

    /// `DOBJ_FLAG_HIDDEN` (bit 0) on the node or `DOBJ_FLAG_HIDETREE` (bit
    /// 1) on it or an ancestor, as its joint's `SetFlags` left them.
    fn hidden(&self, main: &Pack<'_>, global: u32) -> bool {
        let flags = |i: usize| {
            self.joints
                .get(i)
                .and_then(|j| j.as_ref())
                .map_or(0, |j| j.flags)
        };
        let Some(mut i) = global
            .checked_sub(self.object.first_node)
            .map(|i| i as usize)
        else {
            return false;
        };
        if flags(i) & 1 != 0 {
            return true;
        }
        for _ in 0..self.poses.len() {
            if flags(i) & 2 != 0 {
                return true;
            }
            match main
                .node(self.object.first_node + i as u32)
                .and_then(|n| n.parent.checked_sub(self.object.first_node))
            {
                Some(p) if (p as usize) < i => i = p as usize,
                _ => return false,
            }
        }
        false
    }

    /// The node `root->child->sib_next->child`.
    fn generator_node(&self, main: &Pack<'_>) -> Option<usize> {
        let parent = |i: usize| {
            main.node(self.object.first_node + i as u32)
                .and_then(|n| n.parent.checked_sub(self.object.first_node))
                .map(|p| p as usize)
        };
        let n = self.poses.len();
        let children = |of: usize| (0..n).filter(move |&i| i != of && parent(i) == Some(of));
        let second = children(0).nth(1)?;
        children(second).next()
    }
}

/// The opening layout's slash, logo fire tree and particles.
pub(crate) struct TitleOpening {
    slash: Option<Box<Tree>>,
    fire: Option<Box<Tree>>,
    particles: Box<Particles>,
    generator: u8,
    played: u32,
}

impl TitleOpening {
    /// `mnTitleMakeSlash`, `mnTitleMakeLogoFire` and
    /// `mnTitleMakeLogoFireParticles`: each tree played once at its making.
    pub(crate) fn new(main: &Pack<'_>, scene: &Pack<'_>) -> Option<Box<TitleOpening>> {
        let data = script_data(scene)?;
        let mut slash =
            Tree::new(main, data, t::SLASH_DOBJDESC, t::SLASH_ANIM_JOINT, true).map(Box::new);
        let mut fire =
            Tree::new(main, data, t::FIRE_DOBJDESC, t::FIRE_ANIM_JOINT, false).map(Box::new);
        for tree in [&mut slash, &mut fire].into_iter().flatten() {
            tree.play(main, data);
        }
        let mut particles = Box::new(Particles::new());
        let generator = match ssb_psp_runtime::particles::PackBanks::title(main) {
            Some(banks) => lb::make_generator(&mut particles, &banks, 0, 0),
            None => lb::NIL,
        };
        let mut o = Box::new(TitleOpening {
            slash,
            fire,
            particles,
            generator,
            played: 0,
        });
        o.place_generator(main);
        Some(o)
    }

    /// The generator's `dobj`: its node's world matrix.
    fn place_generator(&mut self, main: &Pack<'_>) {
        let Some(fire) = self.fire.as_ref() else {
            return;
        };
        let Some(node) = fire.generator_node(main) else {
            return;
        };
        let m = fire.compose(main, 1.0)[node];
        if self.generator != lb::NIL {
            self.particles.generator_mut(self.generator).dobj = Some(core::array::from_fn(|r| {
                core::array::from_fn(|c| m.0[r * 4 + c])
            }));
        }
    }

    /// The trees' processes and the particles' run, up to `plays` tics.
    pub(crate) fn catch_up(
        &mut self,
        main: &Pack<'_>,
        scene: &Pack<'_>,
        plays: u32,
        slash_shown: bool,
    ) {
        let Some(data) = script_data(scene) else {
            return;
        };
        while self.played < plays {
            // The particles' `func_run` from the tree as the last tic's
            // process left it, then the trees' processes.
            self.place_generator(main);
            if let Some(banks) = ssb_psp_runtime::particles::PackBanks::title(main) {
                lb::run(&mut self.particles, &banks, &mut NoProcDead);
            }
            if slash_shown {
                if let Some(s) = self.slash.as_mut() {
                    s.play(main, data);
                }
            } else {
                self.slash = None;
            }
            if let Some(f) = self.fire.as_mut() {
                f.play(main, data);
            }
            self.played += 1;
        }
    }

    /// Camera 80's particles (`mnTitleLogoFireProcDisplay`): the default
    /// perspective from (0, 0, 1000).
    ///
    /// # Safety
    ///
    /// Between `begin_frame` and `end_frame`.
    pub(crate) unsafe fn draw_particles(
        &mut self,
        gpu: &mut Gpu,
        main: &Pack<'_>,
        st: &mut DrawState,
    ) {
        let Some(banks) = ssb_psp_runtime::particles::PackBanks::title(main) else {
            return;
        };
        gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
        let view = Mat4::look_at(Vec3::new(0.0, 0.0, 1000.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(30.0f32.to_radians(), 4.0 / 3.0, 100.0, 12800.0);
        let (vx, vy, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        let (kx, ky) = (vw as f32 / 320.0, vh as f32 / 240.0);
        let camera = ssb_psp_runtime::particles::Camera {
            view: &view,
            proj: &proj,
            screen: false,
            planes: (100.0, 12800.0),
            ge_planes: (100.0, 12800.0),
            rect: [
                vx as f32 + 10.0 * kx,
                vy as f32 + 10.0 * ky,
                300.0 * kx,
                220.0 * ky,
            ],
        };
        ssb_psp_runtime::particles::draw_lists(
            &banks,
            &mut self.particles,
            &camera,
            &[0, 1, 2, 3, 4],
            0,
            st,
        );
    }

    /// Camera 40's slash: orthographic (`dGCOrthoDefault`) from
    /// (0, 0, 2000).
    ///
    /// # Safety
    ///
    /// Between `begin_frame` and `end_frame`.
    pub(crate) unsafe fn draw_slash(&self, gpu: &mut Gpu, main: &Pack<'_>, st: &mut DrawState) {
        let Some(slash) = self.slash.as_ref() else {
            return;
        };
        gpu.set_viewport_n64([10.0, 10.0, 310.0, 230.0]);
        gpu.set_ortho([-160.0, 160.0, -120.0, 120.0], 100.0, 12800.0);
        gpu.reset_modelview();
        st.begin_frame();
        gpu.set_view(&Mat4::look_at(
            Vec3::new(0.0, 0.0, 2000.0),
            Vec3::ZERO,
            Vec3::Y,
        ));
        gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
        let base = gpu.model_matrix();
        let posed = slash.compose(main, MODEL_SCALE);
        meshdraw::draw_object_posed_hiding(
            main,
            &slash.object,
            &base,
            &posed,
            st,
            slash.mats.as_deref(),
            &|g| slash.hidden(main, g),
        );
        gpu.set_viewport_pillarboxed();
    }
}
