//! `lb/lbtransition.c`: the wipe a scene draws over the previous scene's
//! last frame (RE-146, RE-147, RE-464). The photo itself is
//! [`Gpu::capture_transition_photo_now`] (or the end-of-frame
//! [`Gpu::request_transition_capture`]); this plays one of
//! `dLBTransitionDescs`' animated trees, whose display lists sample it.

use ssb_rom::pack::{AnimDesc, ObjectDesc, Pack};
use ssb_rom::scene::Mat4;
use ssb_rom::skeleton::{StageAnimator, MAX_NODES};

use crate::gu::Gpu;
use crate::meshdraw::{self, DrawState};

/// `lbTransitionMakeCamera`'s viewport: (10, 10) to (310, 230).
pub const VIEWPORT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];
/// Its projection: 45 degrees at 15:11, `dGCPerspDefault`'s planes.
pub const FOVY: f32 = 45.0;
pub const ASPECT: f32 = 15.0 / 11.0;
pub const NEAR: f32 = 100.0;
pub const FAR: f32 = 12800.0;
/// `cobj->vec.eye.z = 1100 / tan(22.5 degrees)`, looking at the origin
/// from `dGCCObjVecDefault`'s other values.
pub const EYE_Z: f32 = 2655.6348;

/// One `lbTransitionMakeTransition` wipe.
pub struct Transition {
    anim: AnimDesc,
    object: ObjectDesc,
    player: StageAnimator,
}

impl Transition {
    /// `lbTransitionMakeTransition(id, ...)`: the tree with its animation
    /// added at frame 0 (`gcAddAnimJointAll`) and played once
    /// (`gcPlayAnimAll`). `None` if the pack lacks the wipe.
    pub fn make(pack: &Pack<'_>, id: u32) -> Option<Self> {
        let mut t = core::mem::MaybeUninit::<Transition>::uninit();
        // SAFETY: a local slot; `write_make` initialises it whole when it
        // returns `true`.
        unsafe { Self::write_make(t.as_mut_ptr(), pack, id).then(|| t.assume_init()) }
    }

    /// [`Self::make`] built in `dst`, field by field, so the 23 KB
    /// animator is never a stack temporary (RE-469). `true` if `dst` now
    /// holds the wipe; on `false` it holds nothing to drop.
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned. Its old contents are
    /// overwritten without being dropped.
    #[inline(never)]
    pub unsafe fn write_make(dst: *mut Transition, pack: &Pack<'_>, id: u32) -> bool {
        use core::ptr::addr_of_mut;
        // A failed make leaves fields written: they own nothing.
        const { assert!(!core::mem::needs_drop::<Transition>()) };
        ssb_engine::all_fields!(Transition {
            anim,
            object,
            player
        });
        let Some(anim) = pack.transition_anim(id) else {
            return false;
        };
        let Some(object) = pack.transition_object(&anim) else {
            return false;
        };
        // SAFETY: `dst` is valid and aligned (the caller's contract); each
        // field is written before it is read.
        unsafe {
            let player = addr_of_mut!((*dst).player);
            StageAnimator::write_new(player);
            (*player).start_changed(pack, &anim);
            let Some(script) = pack.anim_script(&anim) else {
                return false;
            };
            if (*player).tick(script).is_err() {
                return false;
            }
            addr_of_mut!((*dst).anim).write(anim);
            addr_of_mut!((*dst).object).write(object);
        }
        true
    }

    /// `lbTransitionProcUpdate`: plays one frame; `false` once its
    /// `anim_frame` is at or below 0, when the wipe is ejected before it
    /// draws again.
    pub fn update(&mut self, pack: &Pack<'_>) -> bool {
        let Some(script) = pack.anim_script(&self.anim) else {
            return false;
        };
        self.player.tick(script).is_ok() && self.player.gobj_frame() > 0.0
    }

    /// `lbTransitionMakeCamera`'s pass: its viewport and depth cleared
    /// (`COBJ_FLAG_ZBUFFER`), then `lbTransitionProcDisplay`'s tree.
    /// Returns the primitives drawn.
    ///
    /// # Safety
    ///
    /// Between `begin_frame` and `end_frame`; the pack must outlive the
    /// frame.
    pub unsafe fn draw(&self, pack: &Pack<'_>, gpu: &mut Gpu, state: &mut DrawState) -> u32 {
        gpu.set_viewport_n64(VIEWPORT);
        gpu.clear_depth();
        gpu.set_perspective(FOVY, ASPECT, NEAR, FAR);
        gpu.reset_modelview();
        state.begin_frame();
        gpu.model_transform([0.0, 0.0, -EYE_Z], [0.0; 3], meshdraw::MODEL_SCALE);
        let base = gpu.model_matrix();
        let mut posed = [Mat4::IDENTITY; MAX_NODES];
        let n = self.player.compose(pack, &self.object, &mut posed);
        let drawn = meshdraw::draw_object_posed(
            pack,
            &self.object,
            &base,
            &posed[..n],
            None,
            state,
            None,
            None,
            0,
        );
        gpu.set_viewport_pillarboxed();
        drawn
    }
}
