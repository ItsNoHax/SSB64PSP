//! The particle world of a scene with demo fighters (`efParticleInitAll`,
//! `efManagerInitEffects`): the selects', the results' and the Characters
//! scene's. Their fighters' status scripts make effects
//! (`ftParamMakeEffect`) placed on their joints; the fighter camera draws
//! them after the fighters (DL links 10, 15 and 18).

use alloc::boxed::Box;
use core::cell::RefCell;

use ssb_game::demo_fighter::DemoFighter;
use ssb_game::effect::{EffectRuntime, Effects};
use ssb_game::particle::Particles;
use ssb_game::results_scene::Camera;
use ssb_psp_runtime::meshdraw;
use ssb_psp_runtime::particles::PackBanks;
use ssb_rom::pack::Pack;
use ssb_rom::skeleton::Skeleton;

/// The lists a fighter camera's DL links draw, in link order: 4 (link 10,
/// `efDisplayZPerspAAXLUProcDisplay`, depth-tested against the fighters),
/// 1 (15), 0 and 2 (18). No fighter camera draws link 25's list 3.
const FIGHTER_CAMERA_LISTS: [usize; 4] = [4, 1, 0, 2];

/// The particle pools and the effect processes. The pools sit in a
/// `RefCell`: drawing prepares their lists, and the screens draw from
/// shared borrows.
pub struct DemoWorld {
    particles: RefCell<Box<Particles>>,
    effects: Effects,
}

/// Where a demo fighter's TopN stands: position, `rotate.y` in radians
/// and the DObj scale its scene gives it.
#[derive(Clone, Copy)]
pub struct Stand {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub scale: f32,
}

impl DemoWorld {
    #[inline(never)]
    pub fn new() -> Self {
        DemoWorld {
            particles: RefCell::new(Particles::new_boxed()),
            effects: Effects::new(0),
        }
    }

    /// `f` with this world's runtime, if the pack has the banks.
    pub fn with<R>(&mut self, p: &Pack<'_>, f: impl FnOnce(&mut EffectRuntime<'_>) -> R) -> Option<R> {
        let banks = PackBanks::new(p)?;
        let mut particles = self.particles.borrow_mut();
        let mut rt = EffectRuntime {
            particles: &mut particles,
            effects: &mut self.effects,
            banks: &banks,
        };
        Some(f(&mut rt))
    }

    /// Link 0's `func_run`s: the particles', before every process.
    pub fn run(&mut self, p: &Pack<'_>) {
        self.with(p, |rt| rt.run());
    }

    /// The effect processes (priority 3), after the fighters' (5).
    pub fn process(&mut self, p: &Pack<'_>) {
        self.with(p, |rt| rt.effects.process(rt.particles, rt.banks));
    }

    /// `ftParamMakeEffect` for the effects `demo`'s script queued this
    /// frame, each placed on its joint as the clip now poses the fighter.
    pub fn make(
        &mut self,
        p: &Pack<'_>,
        object: u32,
        skeleton: &Skeleton,
        stand: Stand,
        demo: &mut DemoFighter,
    ) {
        if demo.fighter.effects.is_empty() {
            return;
        }
        if let Some(obj) = p.object(object) {
            let [x, y, z] = stand.pos;
            ssb_psp_runtime::scene::sample_demo_joints(
                p,
                &obj,
                skeleton,
                ssb_engine::math::Vec3 { x, y, z },
                stand.yaw,
                stand.scale,
                &mut demo.fighter,
            );
        }
        if self.with(p, |rt| demo.flush_effects(rt)).is_none() {
            demo.fighter.effects.clear();
        }
    }

    /// `ftManagerDestroyFighter`'s `ftParamProcStopEffect`, before the host
    /// drops or remakes `demo`: its attached effects are ejected now. The
    /// hosts flush after every tick, so the stop is all the queue holds;
    /// the effects it made unattached keep playing in this world.
    pub fn destroy(&mut self, p: &Pack<'_>, demo: &mut DemoFighter) {
        demo.destroy();
        if self.with(p, |rt| demo.flush_effects(rt)).is_none() {
            demo.fighter.effects.clear();
        }
    }

    /// The fighter camera's particle lists, after its fighters.
    ///
    /// # Safety
    /// GE calls; the caller holds the display list open.
    pub unsafe fn draw(&self, p: &Pack<'_>, cam: &Camera, draw_state: &mut meshdraw::DrawState) {
        let Some(banks) = PackBanks::new(p) else {
            return;
        };
        let mut particles = self.particles.borrow_mut();
        draw_lists(&banks, &mut particles, cam, draw_state);
    }
}

/// The fighter camera's particle lists.
unsafe fn draw_lists(
    banks: &PackBanks<'_, '_>,
    particles: &mut Particles,
    cam: &Camera,
    draw_state: &mut meshdraw::DrawState,
) {
    let view = ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up);
    let proj = ssb_engine::math::Mat4::sy_persp_fast(cam.fovy, cam.aspect, cam.near, cam.far);
    let camera = ssb_psp_runtime::particles::Camera {
        view: &view,
        proj: &proj,
        screen: false,
        planes: (cam.near, cam.far),
        ge_planes: (cam.near, cam.far),
        rect: ssb_engine::coord::n64_rect_to_psp_xywh(cam.viewport),
    };
    ssb_psp_runtime::particles::draw_lists(
        banks,
        particles,
        &camera,
        &FIGHTER_CAMERA_LISTS,
        ssb_psp_runtime::particles::DEPTH_TESTED,
        draw_state,
    );
}
